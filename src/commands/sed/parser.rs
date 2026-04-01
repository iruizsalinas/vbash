use regex::Regex;

use crate::error::Error;

use super::types::{Address, SedAction, SedCommand, SubFlags};
use super::sed_error;

pub(super) fn parse_delimited(s: &str, delim: char) -> Option<(String, usize)> {
    let mut result = String::new();
    let bytes = s.as_bytes();
    let d = delim as u8;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            if bytes[i + 1] == d {
                result.push(delim);
                i += 2;
                continue;
            }
            result.push('\\');
            result.push(bytes[i + 1] as char);
            i += 2;
            continue;
        }
        if bytes[i] == d {
            return Some((result, i));
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    None
}

pub(super) fn parse_address(s: &str) -> Option<(Address, usize)> {
    if s.is_empty() {
        return None;
    }
    let bytes = s.as_bytes();

    if bytes[0] == b'$' {
        return Some((Address::Last, 1));
    }

    if bytes[0] == b'/' {
        let (pat, end) = parse_delimited(&s[1..], '/')?;
        return Some((Address::Regex(pat), end + 2));
    }

    if bytes[0] == b'\\' && s.len() > 1 {
        let delim = bytes[1] as char;
        let (pat, end) = parse_delimited(&s[2..], delim)?;
        return Some((Address::Regex(pat), end + 3));
    }

    if bytes[0].is_ascii_digit() {
        let mut end = 0;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        let num: usize = s[..end].parse().ok()?;

        if end < bytes.len() && bytes[end] == b'~' {
            let rest = &s[end + 1..];
            let rb = rest.as_bytes();
            let mut end2 = 0;
            while end2 < rb.len() && rb[end2].is_ascii_digit() {
                end2 += 1;
            }
            if end2 > 0 {
                let step: usize = rest[..end2].parse().ok()?;
                return Some((Address::Step(num, step), end + 1 + end2));
            }
        }

        return Some((Address::Line(num), end));
    }

    None
}

pub(super) struct ParseState<'a> {
    pub script: &'a str,
    pub pos: usize,
}

impl ParseState<'_> {
    pub fn remaining(&self) -> &str {
        &self.script[self.pos..]
    }

    pub fn skip_whitespace(&mut self) {
        let bytes = self.remaining().as_bytes();
        let mut i = 0;
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        self.pos += i;
    }

    pub fn peek(&self) -> Option<u8> {
        self.remaining().as_bytes().first().copied()
    }

    pub fn advance(&mut self, n: usize) {
        self.pos += n;
    }

    pub fn at_end(&self) -> bool {
        self.pos >= self.script.len()
    }
}

fn parse_text_argument(state: &mut ParseState) -> String {
    let rest = state.remaining();
    if rest.starts_with('\\') {
        state.advance(1);
    }
    let rest = state.remaining();
    let mut end = rest.len();
    for (i, b) in rest.bytes().enumerate() {
        if b == b'\n' || b == b';' {
            end = i;
            break;
        }
    }
    let text = rest[..end].to_string();
    state.advance(end);
    text
}

fn parse_label(state: &mut ParseState) -> Option<String> {
    state.skip_whitespace();
    let rest = state.remaining();
    if rest.is_empty() {
        return None;
    }
    let mut end = 0;
    let bytes = rest.as_bytes();
    while end < bytes.len()
        && bytes[end] != b';' && bytes[end] != b'\n'
        && bytes[end] != b'}' && bytes[end] != b' ' && bytes[end] != b'\t'
    {
        end += 1;
    }
    if end == 0 {
        return None;
    }
    let label = rest[..end].to_string();
    state.advance(end);
    Some(label)
}

pub(super) fn parse_substitute(state: &mut ParseState) -> Result<SedAction, Error> {
    let rest = state.remaining();
    if rest.is_empty() {
        return Err(sed_error("sed: incomplete substitute command"));
    }
    let delim = rest.as_bytes()[0] as char;
    state.advance(1);

    let rest = state.remaining();
    let (pattern, pend) = parse_delimited(rest, delim)
        .ok_or_else(|| sed_error("sed: unterminated `s' command"))?;
    state.advance(pend + 1);

    let rest = state.remaining();
    let (replacement, rend) = parse_delimited(rest, delim)
        .ok_or_else(|| sed_error("sed: unterminated `s' command"))?;
    state.advance(rend + 1);

    let mut flags = SubFlags {
        global: false,
        print: false,
        ignore_case: false,
        nth: None,
        write_file: None,
    };

    loop {
        match state.peek() {
            Some(b'g') => { flags.global = true; state.advance(1); }
            Some(b'p') => { flags.print = true; state.advance(1); }
            Some(b'i' | b'I') => { flags.ignore_case = true; state.advance(1); }
            Some(b'w') => {
                state.advance(1);
                state.skip_whitespace();
                let rest = state.remaining();
                let mut end = rest.len();
                for (i, b) in rest.bytes().enumerate() {
                    if b == b';' || b == b'\n' {
                        end = i;
                        break;
                    }
                }
                flags.write_file = Some(rest[..end].to_string());
                state.advance(end);
                break;
            }
            Some(d) if d.is_ascii_digit() && d != b'0' => {
                let rest = state.remaining();
                let bytes = rest.as_bytes();
                let mut end = 0;
                while end < bytes.len() && bytes[end].is_ascii_digit() {
                    end += 1;
                }
                if let Ok(n) = rest[..end].parse::<usize>() {
                    flags.nth = Some(n);
                }
                state.advance(end);
            }
            _ => break,
        }
    }

    Ok(SedAction::Substitute { pattern, replacement, flags })
}

pub(super) fn parse_transliterate(state: &mut ParseState) -> Result<SedAction, Error> {
    let rest = state.remaining();
    if rest.is_empty() {
        return Err(sed_error("sed: incomplete `y' command"));
    }
    let delim = rest.as_bytes()[0] as char;
    state.advance(1);

    let rest = state.remaining();
    let (from_str, fend) = parse_delimited(rest, delim)
        .ok_or_else(|| sed_error("sed: unterminated `y' command"))?;
    state.advance(fend + 1);

    let rest = state.remaining();
    let (to_str, tend) = parse_delimited(rest, delim)
        .ok_or_else(|| sed_error("sed: unterminated `y' command"))?;
    state.advance(tend + 1);

    let from: Vec<char> = from_str.chars().collect();
    let to: Vec<char> = to_str.chars().collect();

    if from.len() != to.len() {
        return Err(sed_error("sed: `y' command strings are different lengths"));
    }

    Ok(SedAction::Transliterate { from, to })
}

fn parse_quit_code(state: &mut ParseState) -> i32 {
    state.skip_whitespace();
    let rest = state.remaining();
    let bytes = rest.as_bytes();
    let mut end = 0;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end == 0 {
        return 0;
    }
    let code = rest[..end].parse().unwrap_or(0);
    state.advance(end);
    code
}

fn parse_file_arg(state: &mut ParseState) -> String {
    state.skip_whitespace();
    let rest = state.remaining();
    let mut end = rest.len();
    for (i, b) in rest.bytes().enumerate() {
        if b == b';' || b == b'\n' {
            end = i;
            break;
        }
    }
    let file = rest[..end].to_string();
    state.advance(end);
    file
}

pub(super) fn parse_commands(state: &mut ParseState) -> Result<Vec<SedCommand>, Error> {
    let mut commands = Vec::new();

    while !state.at_end() {
        state.skip_whitespace();
        match state.peek() {
            None => break,
            Some(b'\n' | b';') => { state.advance(1); continue; }
            Some(b'#') => {
                while state.peek().is_some_and(|b| b != b'\n') {
                    state.advance(1);
                }
                continue;
            }
            Some(b'}') => {
                state.advance(1);
                break;
            }
            _ => {}
        }

        let mut addr1 = None;
        let mut addr2 = None;

        if let Some((a, consumed)) = parse_address(state.remaining()) {
            addr1 = Some(a);
            state.advance(consumed);
            state.skip_whitespace();

            if state.peek() == Some(b',') {
                state.advance(1);
                state.skip_whitespace();
                if let Some((a2, consumed2)) = parse_address(state.remaining()) {
                    addr2 = Some(a2);
                    state.advance(consumed2);
                }
            }
        }

        state.skip_whitespace();

        let mut negated = false;
        if state.peek() == Some(b'!') {
            negated = true;
            state.advance(1);
            state.skip_whitespace();
        }

        if state.at_end() {
            break;
        }

        let cmd_byte = state.peek()
            .ok_or_else(|| sed_error("sed: unexpected end of script"))?;
        state.advance(1);

        let action = match cmd_byte {
            b's' => parse_substitute(state)?,
            b'y' => parse_transliterate(state)?,
            b'd' => SedAction::Delete,
            b'D' => SedAction::DeleteFirst,
            b'p' => SedAction::Print,
            b'P' => SedAction::PrintFirst,
            b'q' => SedAction::Quit(parse_quit_code(state)),
            b'Q' => SedAction::QuitSilent(parse_quit_code(state)),
            b'a' => SedAction::Append(parse_text_argument(state)),
            b'i' => SedAction::Insert(parse_text_argument(state)),
            b'c' => SedAction::Change(parse_text_argument(state)),
            b'=' => SedAction::PrintLineNumber,
            b'n' => SedAction::Next,
            b'N' => SedAction::NextAppend,
            b'h' => SedAction::HoldCopy,
            b'H' => SedAction::HoldAppend,
            b'g' => SedAction::GetCopy,
            b'G' => SedAction::GetAppend,
            b'x' => SedAction::Exchange,
            b'b' => SedAction::Branch(parse_label(state)),
            b't' => SedAction::BranchIfSub(parse_label(state)),
            b'T' => SedAction::BranchIfNotSub(parse_label(state)),
            b':' => {
                let label = parse_label(state)
                    .ok_or_else(|| sed_error("sed: `:' lacks a label"))?;
                SedAction::Label(label)
            }
            b'r' => SedAction::ReadFile(parse_file_arg(state)),
            b'w' => SedAction::WriteFile(parse_file_arg(state)),
            b'{' => {
                let inner = parse_commands(state)?;
                if addr1.is_some() {
                    // Address + brace group: execute inner commands as a
                    // single group so the address is tested once, not per
                    // inner command.
                    let action = SedAction::Group(inner);
                    commands.push(SedCommand { addr1, addr2, negated, action });
                } else {
                    // No address: flatten inner commands as before.
                    for cmd in inner {
                        commands.push(cmd);
                    }
                }
                continue;
            }
            other => {
                return Err(sed_error(format!("sed: unknown command: `{}'", other as char)));
            }
        };

        commands.push(SedCommand { addr1, addr2, negated, action });
    }

    Ok(commands)
}

pub(super) fn parse_script(script: &str) -> Result<Vec<SedCommand>, Error> {
    let mut state = ParseState { script, pos: 0 };
    parse_commands(&mut state)
}

pub(super) fn build_regex(pattern: &str, use_ere: bool, case_insensitive: bool) -> Result<Regex, Error> {
    let mut regex_str = String::new();
    if case_insensitive {
        regex_str.push_str("(?i)");
    }

    if use_ere {
        regex_str.push_str(pattern);
    } else {
        let bytes = pattern.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                match bytes[i + 1] {
                    b'(' => { regex_str.push('('); i += 2; }
                    b')' => { regex_str.push(')'); i += 2; }
                    b'{' => { regex_str.push('{'); i += 2; }
                    b'}' => { regex_str.push('}'); i += 2; }
                    b'+' => { regex_str.push('+'); i += 2; }
                    b'?' => { regex_str.push('?'); i += 2; }
                    b'|' => { regex_str.push('|'); i += 2; }
                    b'1'..=b'9' => {
                        regex_str.push('\\');
                        regex_str.push(bytes[i + 1] as char);
                        i += 2;
                    }
                    b'n' => { regex_str.push('\n'); i += 2; }
                    b't' => { regex_str.push('\t'); i += 2; }
                    b'w' => { regex_str.push_str("\\w"); i += 2; }
                    b'W' => { regex_str.push_str("\\W"); i += 2; }
                    b'b' => { regex_str.push_str("\\b"); i += 2; }
                    b'B' => { regex_str.push_str("\\B"); i += 2; }
                    b's' => { regex_str.push_str("\\s"); i += 2; }
                    b'S' => { regex_str.push_str("\\S"); i += 2; }
                    b'd' => { regex_str.push_str("\\d"); i += 2; }
                    b'D' => { regex_str.push_str("\\D"); i += 2; }
                    other => {
                        regex_str.push('\\');
                        regex_str.push(other as char);
                        i += 2;
                    }
                }
            } else {
                match bytes[i] {
                    b'(' | b')' | b'{' | b'}' | b'+' | b'?' | b'|' => {
                        regex_str.push('\\');
                        regex_str.push(bytes[i] as char);
                    }
                    _ => regex_str.push(bytes[i] as char),
                }
                i += 1;
            }
        }
    }

    Regex::new(&regex_str).map_err(|e| sed_error(format!("sed: invalid regex: {e}")))
}
