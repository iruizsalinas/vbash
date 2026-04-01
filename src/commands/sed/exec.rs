use std::fmt::Write;

use crate::error::Error;
use crate::fs::VirtualFs;

const MAX_SED_ITERATIONS: u32 = 100_000;

use super::types::{Address, SedAction, SedCommand, SedProgram, SubFlags};
use super::parser::build_regex;

pub(super) fn apply_replacement(re: &regex::Regex, text: &str, replacement: &str, flags: &SubFlags) -> String {
    let expand = |caps: &regex::Captures, repl: &str| -> String {
        let mut out = String::new();
        let bytes = repl.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                match bytes[i + 1] {
                    b'0'..=b'9' => {
                        let group = (bytes[i + 1] - b'0') as usize;
                        if let Some(m) = caps.get(group) {
                            out.push_str(m.as_str());
                        }
                        i += 2;
                    }
                    b'n' => { out.push('\n'); i += 2; }
                    b't' => { out.push('\t'); i += 2; }
                    b'\\' => { out.push('\\'); i += 2; }
                    _ => {
                        out.push(bytes[i + 1] as char);
                        i += 2;
                    }
                }
            } else if bytes[i] == b'&' {
                out.push_str(caps.get(0).map_or("", |m| m.as_str()));
                i += 1;
            } else {
                out.push(bytes[i] as char);
                i += 1;
            }
        }
        out
    };

    if flags.global {
        let mut result = String::new();
        let mut last_end = 0;
        for caps in re.captures_iter(text) {
            let whole = caps.get(0).map_or((0, 0), |m| (m.start(), m.end()));
            result.push_str(&text[last_end..whole.0]);
            result.push_str(&expand(&caps, replacement));
            last_end = whole.1;
            if whole.0 == whole.1 {
                if last_end < text.len() {
                    result.push(text.as_bytes()[last_end] as char);
                    last_end += 1;
                } else {
                    break;
                }
            }
        }
        result.push_str(&text[last_end..]);
        result
    } else if let Some(nth) = flags.nth {
        let mut result = String::new();
        let mut last_end = 0;
        let mut count = 0;
        for caps in re.captures_iter(text) {
            count += 1;
            let whole = caps.get(0).map_or((0, 0), |m| (m.start(), m.end()));
            if count == nth {
                result.push_str(&text[last_end..whole.0]);
                result.push_str(&expand(&caps, replacement));
                last_end = whole.1;
                break;
            }
        }
        result.push_str(&text[last_end..]);
        result
    } else if let Some(caps) = re.captures(text) {
        let whole = caps.get(0).map_or((0, 0), |m| (m.start(), m.end()));
        let mut result = String::with_capacity(text.len());
        result.push_str(&text[..whole.0]);
        result.push_str(&expand(&caps, replacement));
        result.push_str(&text[whole.1..]);
        result
    } else {
        text.to_string()
    }
}

pub(super) fn address_matches(
    addr: &Address,
    line_num: usize,
    line: &str,
    is_last: bool,
    use_ere: bool,
) -> Result<bool, Error> {
    match addr {
        Address::Line(n) => Ok(line_num == *n),
        Address::Last => Ok(is_last),
        Address::Regex(pat) => {
            let re = build_regex(pat, use_ere, false)?;
            Ok(re.is_match(line))
        }
        Address::Step(first, step) => {
            if *step == 0 {
                Ok(line_num == *first)
            } else {
                Ok(line_num >= *first && (line_num - *first) % *step == 0)
            }
        }
    }
}

pub(super) struct ExecState {
    pub pattern_space: String,
    pub hold_space: String,
    pub line_num: usize,
    pub is_last_line: bool,
    pub sub_success: bool,
    pub output: String,
    pub use_ere: bool,
    pub append_text: Vec<String>,
    pub range_active: Vec<bool>,
}

pub(super) fn find_label(commands: &[SedCommand], label: &str) -> Option<usize> {
    commands.iter().position(|cmd| {
        matches!(&cmd.action, SedAction::Label(l) if l == label)
    })
}

pub(super) fn execute_line(
    program: &SedProgram,
    st: &mut ExecState,
    lines: &[&str],
    line_idx: &mut usize,
    fs: &dyn VirtualFs,
    cwd: &str,
) -> Result<Option<i32>, Error> {
    let commands = &program.commands;
    let mut range_active: Vec<bool> = vec![false; commands.len()];
    std::mem::swap(&mut range_active, &mut st.range_active);
    while range_active.len() < commands.len() {
        range_active.push(false);
    }

    let mut cmd_idx = 0;
    let mut branch_count = 0u32;

    while cmd_idx < commands.len() {
        let cmd = &commands[cmd_idx];

        let in_range = match (&cmd.addr1, &cmd.addr2) {
            (None, None) => true,
            (Some(a1), None) => {
                address_matches(a1, st.line_num, &st.pattern_space, st.is_last_line, st.use_ere)?
            }
            (Some(a1), Some(a2)) => {
                if range_active[cmd_idx] {
                    if address_matches(a2, st.line_num, &st.pattern_space, st.is_last_line, st.use_ere)? {
                        range_active[cmd_idx] = false;
                    }
                    true
                } else if address_matches(a1, st.line_num, &st.pattern_space, st.is_last_line, st.use_ere)? {
                    range_active[cmd_idx] = true;
                    // Immediately check if addr2 also matches (single-line range)
                    if address_matches(a2, st.line_num, &st.pattern_space, st.is_last_line, st.use_ere)? {
                        range_active[cmd_idx] = false;
                    }
                    true
                } else {
                    false
                }
            }
            (None, Some(a2)) => {
                address_matches(a2, st.line_num, &st.pattern_space, st.is_last_line, st.use_ere)?
            }
        };

        let matched = if cmd.negated { !in_range } else { in_range };
        if !matched {
            cmd_idx += 1;
            continue;
        }

        match &cmd.action {
            SedAction::Substitute { pattern, replacement, flags } => {
                let re = build_regex(pattern, st.use_ere, flags.ignore_case)?;
                let old = st.pattern_space.clone();
                st.pattern_space = apply_replacement(&re, &st.pattern_space, replacement, flags);
                let changed = st.pattern_space != old;
                if changed {
                    st.sub_success = true;
                    if flags.print {
                        let _ = writeln!(st.output, "{}", st.pattern_space);
                    }
                    if let Some(ref wf) = flags.write_file {
                        let path = crate::fs::path::resolve(cwd, wf);
                        let data = format!("{}\n", st.pattern_space);
                        let _ = fs.append_file(&path, data.as_bytes());
                    }
                }
            }
            SedAction::Transliterate { from, to } => {
                let mut result = String::with_capacity(st.pattern_space.len());
                for ch in st.pattern_space.chars() {
                    if let Some(pos) = from.iter().position(|&c| c == ch) {
                        result.push(to[pos]);
                    } else {
                        result.push(ch);
                    }
                }
                st.pattern_space = result;
            }
            SedAction::Delete => {
                std::mem::swap(&mut range_active, &mut st.range_active);
                return Ok(None);
            }
            SedAction::DeleteFirst => {
                if let Some(pos) = st.pattern_space.find('\n') {
                    st.pattern_space = st.pattern_space[pos + 1..].to_string();
                    cmd_idx = 0;
                    st.sub_success = false;
                    continue;
                }
                std::mem::swap(&mut range_active, &mut st.range_active);
                return Ok(None);
            }
            SedAction::Print => {
                let _ = writeln!(st.output, "{}", st.pattern_space);
            }
            SedAction::PrintFirst => {
                let first = st.pattern_space.split('\n').next().unwrap_or("");
                let _ = writeln!(st.output, "{first}");
            }
            SedAction::Quit(code) => {
                if !program.suppress_print {
                    let _ = writeln!(st.output, "{}", st.pattern_space);
                }
                for text in st.append_text.drain(..) {
                    let _ = writeln!(st.output, "{text}");
                }
                std::mem::swap(&mut range_active, &mut st.range_active);
                return Ok(Some(*code));
            }
            SedAction::QuitSilent(code) => {
                std::mem::swap(&mut range_active, &mut st.range_active);
                return Ok(Some(*code));
            }
            SedAction::Append(text) => {
                st.append_text.push(text.clone());
            }
            SedAction::Insert(text) => {
                let _ = writeln!(st.output, "{text}");
            }
            SedAction::Change(text) => {
                let _ = writeln!(st.output, "{text}");
                std::mem::swap(&mut range_active, &mut st.range_active);
                return Ok(None);
            }
            SedAction::PrintLineNumber => {
                let _ = writeln!(st.output, "{}", st.line_num);
            }
            SedAction::Next => {
                if !program.suppress_print {
                    let _ = writeln!(st.output, "{}", st.pattern_space);
                }
                for text in st.append_text.drain(..) {
                    let _ = writeln!(st.output, "{text}");
                }
                *line_idx += 1;
                if *line_idx < lines.len() {
                    st.line_num += 1;
                    st.is_last_line = *line_idx == lines.len() - 1;
                    st.pattern_space = lines[*line_idx].to_string();
                    st.sub_success = false;
                } else {
                    std::mem::swap(&mut range_active, &mut st.range_active);
                    return Ok(None);
                }
            }
            SedAction::NextAppend => {
                *line_idx += 1;
                if *line_idx < lines.len() {
                    st.line_num += 1;
                    st.is_last_line = *line_idx == lines.len() - 1;
                    st.pattern_space.push('\n');
                    st.pattern_space.push_str(lines[*line_idx]);
                } else {
                    if !program.suppress_print {
                        let _ = writeln!(st.output, "{}", st.pattern_space);
                    }
                    for text in st.append_text.drain(..) {
                        let _ = writeln!(st.output, "{text}");
                    }
                    std::mem::swap(&mut range_active, &mut st.range_active);
                    return Ok(Some(0));
                }
            }
            SedAction::HoldCopy => { st.hold_space = st.pattern_space.clone(); }
            SedAction::HoldAppend => {
                st.hold_space.push('\n');
                st.hold_space.push_str(&st.pattern_space);
            }
            SedAction::GetCopy => { st.pattern_space = st.hold_space.clone(); }
            SedAction::GetAppend => {
                st.pattern_space.push('\n');
                st.pattern_space.push_str(&st.hold_space);
            }
            SedAction::Exchange => {
                std::mem::swap(&mut st.pattern_space, &mut st.hold_space);
            }
            SedAction::Branch(label) => {
                if let Some(l) = label {
                    branch_count += 1;
                    if branch_count > MAX_SED_ITERATIONS {
                        return Err(Error::Exec(crate::error::ExecError::Other("sed: branch loop limit exceeded".to_string())));
                    }
                    if let Some(target) = find_label(commands, l) {
                        cmd_idx = target + 1;
                        continue;
                    }
                } else {
                    break;
                }
            }
            SedAction::BranchIfSub(label) => {
                if st.sub_success {
                    st.sub_success = false;
                    if let Some(l) = label {
                        branch_count += 1;
                        if branch_count > MAX_SED_ITERATIONS {
                            return Err(Error::Exec(crate::error::ExecError::Other("sed: branch loop limit exceeded".to_string())));
                        }
                        if let Some(target) = find_label(commands, l) {
                            cmd_idx = target + 1;
                            continue;
                        }
                    } else {
                        break;
                    }
                }
            }
            SedAction::BranchIfNotSub(label) => {
                if !st.sub_success {
                    if let Some(l) = label {
                        branch_count += 1;
                        if branch_count > MAX_SED_ITERATIONS {
                            return Err(Error::Exec(crate::error::ExecError::Other("sed: branch loop limit exceeded".to_string())));
                        }
                        if let Some(target) = find_label(commands, l) {
                            cmd_idx = target + 1;
                            continue;
                        }
                    } else {
                        break;
                    }
                }
                st.sub_success = false;
            }
            SedAction::Group(inner_commands) => {
                let inner_program = SedProgram {
                    commands: inner_commands.clone(),
                    suppress_print: true, // group doesn't auto-print
                    use_ere: st.use_ere,
                };
                let saved_output = std::mem::take(&mut st.output);
                let saved_append = std::mem::take(&mut st.append_text);
                let saved_range = std::mem::take(&mut st.range_active);
                st.range_active = vec![false; inner_commands.len()];

                let result = execute_line(&inner_program, st, lines, line_idx, fs, cwd)?;

                let group_output = std::mem::replace(&mut st.output, saved_output);
                st.output.push_str(&group_output);
                let group_append = std::mem::replace(&mut st.append_text, saved_append);
                st.append_text.extend(group_append);
                st.range_active = saved_range;

                if let Some(code) = result {
                    std::mem::swap(&mut range_active, &mut st.range_active);
                    return Ok(Some(code));
                }
            }
            SedAction::Label(_) => {}
            SedAction::ReadFile(file) => {
                let path = crate::fs::path::resolve(cwd, file);
                if let Ok(content) = fs.read_file_string(&path) {
                    st.append_text.push(content.trim_end_matches('\n').to_string());
                }
            }
            SedAction::WriteFile(file) => {
                let path = crate::fs::path::resolve(cwd, file);
                let data = format!("{}\n", st.pattern_space);
                let _ = fs.append_file(&path, data.as_bytes());
            }
        }

        cmd_idx += 1;
    }

    if !program.suppress_print {
        let _ = writeln!(st.output, "{}", st.pattern_space);
    }
    for text in st.append_text.drain(..) {
        let _ = writeln!(st.output, "{text}");
    }

    std::mem::swap(&mut range_active, &mut st.range_active);
    Ok(None)
}
