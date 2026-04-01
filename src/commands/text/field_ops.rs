use std::fmt::Write;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;

use super::{read_input, parse_field_specs, field_matches};
use std::collections::HashMap;

pub fn cut(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut delim = '\t';
    let mut fields_spec = None;
    let mut chars_spec = None;
    let mut only_delimited = false;
    let mut file_args = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-d" if i + 1 < args.len() => {
                let d = args[i + 1];
                delim = d.chars().next().unwrap_or('\t');
                i += 2;
            }
            "-f" if i + 1 < args.len() => {
                fields_spec = Some(args[i + 1].to_string());
                i += 2;
            }
            "-c" if i + 1 < args.len() => {
                chars_spec = Some(args[i + 1].to_string());
                i += 2;
            }
            "-s" | "--only-delimited" => {
                only_delimited = true;
                i += 1;
            }
            arg if arg.starts_with("-d") => {
                delim = arg[2..].chars().next().unwrap_or('\t');
                i += 1;
            }
            arg if arg.starts_with("-f") => {
                fields_spec = Some(arg[2..].to_string());
                i += 1;
            }
            arg if arg.starts_with("-c") => {
                chars_spec = Some(arg[2..].to_string());
                i += 1;
            }
            _ => {
                file_args.push(args[i]);
                i += 1;
            }
        }
    }

    let content = match read_input(&file_args, ctx) {
        Ok(c) => c,
        Err(e) => {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: format!("cut: {e}\n"),
                exit_code: 1,
                env: HashMap::new(),
})
        }
    };

    let mut stdout = String::new();

    if let Some(ref spec) = chars_spec {
        let specs = parse_field_specs(spec);
        for line in content.lines() {
            let chars: Vec<char> = line.chars().collect();
            let mut selected = String::new();
            for (ci, ch) in chars.iter().enumerate() {
                if field_matches(&specs, ci + 1) {
                    selected.push(*ch);
                }
            }
            let _ = writeln!(stdout, "{selected}");
        }
    } else if let Some(ref spec) = fields_spec {
        let specs = parse_field_specs(spec);
        let delim_str = String::from(delim);
        for line in content.lines() {
            if !line.contains(delim) {
                if !only_delimited {
                    let _ = writeln!(stdout, "{line}");
                }
                continue;
            }
            let fields: Vec<&str> = line.split(delim).collect();
            let mut selected: Vec<&str> = Vec::new();
            for (fi, field) in fields.iter().enumerate() {
                if field_matches(&specs, fi + 1) {
                    selected.push(field);
                }
            }
            let _ = writeln!(stdout, "{}", selected.join(&delim_str));
        }
    } else {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "cut: you must specify a list of bytes, characters, or fields\n".to_string(),
            exit_code: 1,
            env: HashMap::new(),
});
    }

    Ok(ExecResult {
        stdout,
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}

fn expand_char_class(name: &str) -> Vec<char> {
    match name {
        "alnum" => {
            let mut v: Vec<char> = ('0'..='9').collect();
            v.extend('A'..='Z');
            v.extend('a'..='z');
            v
        }
        "alpha" => {
            let mut v: Vec<char> = ('A'..='Z').collect();
            v.extend('a'..='z');
            v
        }
        "digit" => ('0'..='9').collect(),
        "lower" => ('a'..='z').collect(),
        "upper" => ('A'..='Z').collect(),
        "space" => vec![' ', '\t', '\n', '\r', '\x0b', '\x0c'],
        "blank" => vec![' ', '\t'],
        _ => Vec::new(),
    }
}

fn parse_tr_set(set: &str) -> Vec<char> {
    let mut chars = Vec::new();
    let bytes = set.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' && i + 2 < bytes.len() && bytes[i + 1] == b':' {
            if let Some(end) = set[i + 2..].find(":]") {
                let class_name = &set[i + 2..i + 2 + end];
                chars.extend(expand_char_class(class_name));
                i = i + 2 + end + 2;
                continue;
            }
        }
        if i + 2 < bytes.len() && bytes[i + 1] == b'-' {
            let start = bytes[i];
            let end = bytes[i + 2];
            if start <= end {
                for c in start..=end {
                    chars.push(c as char);
                }
            }
            i += 3;
            continue;
        }
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 1;
            match bytes[i] {
                b'n' => chars.push('\n'),
                b't' => chars.push('\t'),
                b'r' => chars.push('\r'),
                b'\\' => chars.push('\\'),
                other => chars.push(other as char),
            }
        } else {
            chars.push(bytes[i] as char);
        }
        i += 1;
    }
    chars
}

pub fn tr(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut delete = false;
    let mut squeeze = false;
    let mut complement = false;
    let mut set_args = Vec::new();

    for arg in args {
        match *arg {
            "-d" => delete = true,
            "-s" => squeeze = true,
            "-c" => complement = true,
            "-cd" | "-dc" => {
                complement = true;
                delete = true;
            }
            "-cs" | "-sc" => {
                complement = true;
                squeeze = true;
            }
            "-ds" | "-sd" => {
                delete = true;
                squeeze = true;
            }
            _ => set_args.push(*arg),
        }
    }

    let input = ctx.stdin;

    if delete {
        let set1 = if set_args.is_empty() {
            Vec::new()
        } else {
            parse_tr_set(set_args[0])
        };
        let mut stdout = String::new();
        let mut last_char: Option<char> = None;
        let squeeze_set = if squeeze && set_args.len() > 1 {
            parse_tr_set(set_args[1])
        } else {
            Vec::new()
        };
        for ch in input.chars() {
            let in_set = set1.contains(&ch);
            let should_delete = if complement { !in_set } else { in_set };
            if should_delete {
                continue;
            }
            if squeeze && !squeeze_set.is_empty() && squeeze_set.contains(&ch)
                && last_char == Some(ch)
            {
                continue;
            }
            stdout.push(ch);
            last_char = Some(ch);
        }
        return Ok(ExecResult {
            stdout,
            stderr: String::new(),
            exit_code: 0,
            env: HashMap::new(),
});
    }

    if set_args.len() < 2 && !squeeze {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "tr: missing operand\n".to_string(),
            exit_code: 1,
            env: HashMap::new(),
});
    }

    let set1 = parse_tr_set(set_args[0]);

    if squeeze && set_args.len() == 1 {
        let mut stdout = String::new();
        let mut last_char: Option<char> = None;
        for ch in input.chars() {
            let in_set = set1.contains(&ch);
            let should_squeeze = if complement { !in_set } else { in_set };
            if should_squeeze && last_char == Some(ch) {
                continue;
            }
            stdout.push(ch);
            last_char = Some(ch);
        }
        return Ok(ExecResult {
            stdout,
            stderr: String::new(),
            exit_code: 0,
            env: HashMap::new(),
});
    }

    let set2 = if set_args.len() > 1 {
        parse_tr_set(set_args[1])
    } else {
        Vec::new()
    };

    let mut stdout = String::new();
    let mut last_char: Option<char> = None;
    for ch in input.chars() {
        let in_set = set1.contains(&ch);
        let matched = if complement { !in_set } else { in_set };
        let replacement = if matched {
            if complement {
                set2.last().copied().unwrap_or(ch)
            } else if let Some(pos) = set1.iter().position(|c| *c == ch) {
                if pos < set2.len() {
                    set2[pos]
                } else {
                    set2.last().copied().unwrap_or(ch)
                }
            } else {
                ch
            }
        } else {
            ch
        };
        if squeeze && last_char == Some(replacement) {
            let in_squeeze = set2.contains(&replacement);
            if in_squeeze {
                continue;
            }
        }
        stdout.push(replacement);
        last_char = Some(replacement);
    }

    Ok(ExecResult {
        stdout,
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}

pub fn paste(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut delims = "\t";
    let mut serial = false;
    let mut file_args = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-d" if i + 1 < args.len() => {
                delims = args[i + 1];
                i += 2;
            }
            "-s" | "--serial" => {
                serial = true;
                i += 1;
            }
            arg if arg.starts_with("-d") => {
                delims = &arg[2..];
                i += 1;
            }
            _ => {
                file_args.push(args[i]);
                i += 1;
            }
        }
    }

    let delim_chars: Vec<char> = delims.chars().collect();
    if delim_chars.is_empty() {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "paste: empty delimiter list\n".to_string(),
            exit_code: 1,
            env: HashMap::new(),
});
    }

    let mut file_contents: Vec<String> = Vec::new();
    for path in &file_args {
        if *path == "-" {
            file_contents.push(ctx.stdin.to_string());
        } else {
            let resolved = crate::fs::path::resolve(ctx.cwd, path);
            match ctx.fs.read_file_string(&resolved) {
                Ok(c) => file_contents.push(c),
                Err(e) => {
                    return Ok(ExecResult {
                        stdout: String::new(),
                        stderr: format!("paste: {e}\n"),
                        exit_code: 1,
                        env: HashMap::new(),
})
                }
            }
        }
    }

    if file_contents.is_empty() {
        file_contents.push(ctx.stdin.to_string());
    }

    let mut stdout = String::new();

    if serial {
        for (fi, content) in file_contents.iter().enumerate() {
            let lines: Vec<&str> = content.lines().collect();
            for (li, line) in lines.iter().enumerate() {
                if li > 0 {
                    stdout.push(delim_chars[li.saturating_sub(1) % delim_chars.len()]);
                }
                stdout.push_str(line);
            }
            if fi < file_contents.len() {
                stdout.push('\n');
            }
        }
    } else {
        let line_sets: Vec<Vec<&str>> = file_contents
            .iter()
            .map(|c| c.lines().collect::<Vec<_>>())
            .collect();
        let max_lines = line_sets.iter().map(Vec::len).max().unwrap_or(0);
        for li in 0..max_lines {
            for (fi, ls) in line_sets.iter().enumerate() {
                if fi > 0 {
                    stdout.push(delim_chars[(fi - 1) % delim_chars.len()]);
                }
                if let Some(line) = ls.get(li) {
                    stdout.push_str(line);
                }
            }
            stdout.push('\n');
        }
    }

    Ok(ExecResult {
        stdout,
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}
