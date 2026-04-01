use std::fmt::Write;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;
use std::collections::HashMap;

pub fn echo(args: &[&str], _ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut newline = true;
    let mut interpret_escapes = false;
    let mut arg_start = 0;

    for (i, arg) in args.iter().enumerate() {
        match *arg {
            "-n" => { newline = false; arg_start = i + 1; }
            "-e" => { interpret_escapes = true; arg_start = i + 1; }
            "-E" => { interpret_escapes = false; arg_start = i + 1; }
            "-ne" | "-en" => { newline = false; interpret_escapes = true; arg_start = i + 1; }
            "-nE" | "-En" => { newline = false; interpret_escapes = false; arg_start = i + 1; }
            _ => break,
        }
    }

    let text = args[arg_start..].join(" ");
    let output = if interpret_escapes {
        expand_echo_escapes(&text)
    } else {
        text
    };

    let mut stdout = output;
    if newline {
        stdout.push('\n');
    }

    Ok(ExecResult {
        stdout,
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}

fn expand_echo_escapes(s: &str) -> String {
    let mut result = String::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 1;
            match bytes[i] {
                b'n' => result.push('\n'),
                b't' => result.push('\t'),
                b'r' => result.push('\r'),
                b'a' => result.push('\x07'),
                b'b' => result.push('\x08'),
                b'f' => result.push('\x0c'),
                b'v' => result.push('\x0b'),
                b'e' | b'E' => result.push('\x1b'),
                b'\\' => result.push('\\'),
                b'c' => return result, // stop output
                b'0' => {
                    let start = i + 1;
                    let mut end = start;
                    while end < bytes.len() && end < start + 3 && bytes[end] >= b'0' && bytes[end] <= b'7' {
                        end += 1;
                    }
                    let oct: String = bytes[start..end].iter().map(|&b| b as char).collect();
                    if let Ok(n) = u8::from_str_radix(&oct, 8) {
                        result.push(n as char);
                    }
                    i = end - 1;
                }
                b'x' => {
                    let start = i + 1;
                    let mut end = start;
                    while end < bytes.len() && end < start + 2 && bytes[end].is_ascii_hexdigit() {
                        end += 1;
                    }
                    let hex: String = bytes[start..end].iter().map(|&b| b as char).collect();
                    if let Ok(n) = u8::from_str_radix(&hex, 16) {
                        result.push(n as char);
                    }
                    i = end - 1;
                }
                b'u' => {
                    let start = i + 1;
                    let mut end = start;
                    while end < bytes.len() && end < start + 4 && bytes[end].is_ascii_hexdigit() {
                        end += 1;
                    }
                    if end > start {
                        let hex: String = bytes[start..end].iter().map(|&b| b as char).collect();
                        if let Ok(n) = u32::from_str_radix(&hex, 16) {
                            if let Some(c) = char::from_u32(n) {
                                result.push(c);
                            }
                        }
                        i = end - 1;
                    } else {
                        result.push('\\');
                        result.push('u');
                    }
                }
                b'U' => {
                    let start = i + 1;
                    let mut end = start;
                    while end < bytes.len() && end < start + 8 && bytes[end].is_ascii_hexdigit() {
                        end += 1;
                    }
                    if end > start {
                        let hex: String = bytes[start..end].iter().map(|&b| b as char).collect();
                        if let Ok(n) = u32::from_str_radix(&hex, 16) {
                            if let Some(c) = char::from_u32(n) {
                                result.push(c);
                            }
                        }
                        i = end - 1;
                    } else {
                        result.push('\\');
                        result.push('U');
                    }
                }
                other => {
                    result.push('\\');
                    result.push(other as char);
                }
            }
        } else {
            result.push(bytes[i] as char);
        }
        i += 1;
    }
    result
}

pub fn printf(args: &[&str], _ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    if args.is_empty() {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "printf: usage: printf format [arguments]\n".to_string(),
            exit_code: 1,
            env: HashMap::new(),
});
    }

    let (var_name, fmt_args) = if args.len() >= 3 && args[0] == "-v" {
        (Some(args[1]), &args[2..])
    } else {
        (None, args)
    };

    if fmt_args.is_empty() {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "printf: usage: printf [-v var] format [arguments]\n".to_string(),
            exit_code: 1,
            env: HashMap::new(),
        });
    }

    let format = fmt_args[0];
    let format_args = &fmt_args[1..];
    let output = simple_printf(format, format_args);

    if let Some(var) = var_name {
        let mut env = HashMap::new();
        env.insert(var.to_string(), output);
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: 0,
            env,
        });
    }

    Ok(ExecResult {
        stdout: output,
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}

fn simple_printf(format: &str, args: &[&str]) -> String {
    let mut result = String::new();
    let mut arg_idx = 0;

    loop {
        let start_arg = arg_idx;
        printf_pass(format, args, &mut arg_idx, &mut result);
        if arg_idx <= start_arg || arg_idx >= args.len() {
            break;
        }
    }

    result
}

fn printf_pass(format: &str, args: &[&str], arg_idx: &mut usize, result: &mut String) {
    let bytes = format.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%' && i + 1 < bytes.len() {
            i += 1;

            if bytes[i] == b'%' {
                result.push('%');
                i += 1;
                continue;
            }

            let mut flags = String::new();
            while i < bytes.len() && matches!(bytes[i], b'-' | b'+' | b' ' | b'0' | b'#') {
                flags.push(bytes[i] as char);
                i += 1;
            }

            let mut width_str = String::new();
            if i < bytes.len() && bytes[i] == b'*' {
                let w: i64 = args.get(*arg_idx).and_then(|s| s.parse().ok()).unwrap_or(0);
                width_str = w.to_string();
                *arg_idx += 1;
                i += 1;
            } else {
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    width_str.push(bytes[i] as char);
                    i += 1;
                }
            }

            let mut prec_str = String::new();
            let mut has_precision = false;
            if i < bytes.len() && bytes[i] == b'.' {
                has_precision = true;
                i += 1;
                if i < bytes.len() && bytes[i] == b'*' {
                    let p: i64 = args.get(*arg_idx).and_then(|s| s.parse().ok()).unwrap_or(0);
                    prec_str = p.to_string();
                    *arg_idx += 1;
                    i += 1;
                } else {
                    while i < bytes.len() && bytes[i].is_ascii_digit() {
                        prec_str.push(bytes[i] as char);
                        i += 1;
                    }
                }
            }

            if i >= bytes.len() {
                result.push('%');
                result.push_str(&flags);
                result.push_str(&width_str);
                if has_precision {
                    result.push('.');
                    result.push_str(&prec_str);
                }
                break;
            }

            let width: usize = width_str.parse().unwrap_or(0);
            let precision: usize = prec_str.parse().unwrap_or(6);
            let left_align = flags.contains('-');
            let zero_pad = flags.contains('0') && !left_align;

            let spec = bytes[i];
            i += 1;

            let formatted = match spec {
                b's' => {
                    let s = args.get(*arg_idx).copied().unwrap_or("");
                    *arg_idx += 1;
                    let val = if has_precision {
                        let limit = prec_str.parse::<usize>().unwrap_or(s.len());
                        &s[..s.len().min(limit)]
                    } else {
                        s
                    };
                    val.to_string()
                }
                b'd' | b'i' => {
                    let val: i64 = args.get(*arg_idx).and_then(|s| s.parse().ok()).unwrap_or(0);
                    *arg_idx += 1;
                    val.to_string()
                }
                b'u' => {
                    let val: u64 = args.get(*arg_idx).and_then(|s| s.parse().ok()).unwrap_or(0);
                    *arg_idx += 1;
                    val.to_string()
                }
                b'o' => {
                    let val: i64 = args.get(*arg_idx).and_then(|s| s.parse().ok()).unwrap_or(0);
                    *arg_idx += 1;
                    format!("{val:o}")
                }
                b'x' => {
                    let val: i64 = args.get(*arg_idx).and_then(|s| s.parse().ok()).unwrap_or(0);
                    *arg_idx += 1;
                    format!("{val:x}")
                }
                b'X' => {
                    let val: i64 = args.get(*arg_idx).and_then(|s| s.parse().ok()).unwrap_or(0);
                    *arg_idx += 1;
                    format!("{val:X}")
                }
                b'c' => {
                    let c = args.get(*arg_idx).and_then(|s| s.chars().next()).unwrap_or('\0');
                    *arg_idx += 1;
                    if c == '\0' { String::new() } else { c.to_string() }
                }
                b'f' => {
                    let val: f64 = args.get(*arg_idx).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    *arg_idx += 1;
                    let p = if has_precision { precision } else { 6 };
                    format!("{val:.p$}")
                }
                b'e' => {
                    let val: f64 = args.get(*arg_idx).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    *arg_idx += 1;
                    let p = if has_precision { precision } else { 6 };
                    format_scientific(val, p, false)
                }
                b'E' => {
                    let val: f64 = args.get(*arg_idx).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    *arg_idx += 1;
                    let p = if has_precision { precision } else { 6 };
                    format_scientific(val, p, true)
                }
                b'g' | b'G' => {
                    let val: f64 = args.get(*arg_idx).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    *arg_idx += 1;
                    format!("{val}")
                }
                b'b' => {
                    let s = args.get(*arg_idx).copied().unwrap_or("");
                    *arg_idx += 1;
                    expand_echo_escapes(s)
                }
                _ => {
                    let mut s = String::from('%');
                    s.push_str(&flags);
                    s.push_str(&width_str);
                    if has_precision {
                        s.push('.');
                        s.push_str(&prec_str);
                    }
                    s.push(spec as char);
                    s
                }
            };

            if width > formatted.len() {
                let pad_char = if zero_pad && !matches!(spec, b's' | b'c' | b'b') {
                    '0'
                } else {
                    ' '
                };
                let padding: String = std::iter::repeat_n(pad_char, width - formatted.len())
                    .collect();
                if left_align {
                    result.push_str(&formatted);
                    result.push_str(&padding);
                } else {
                    result.push_str(&padding);
                    result.push_str(&formatted);
                }
            } else {
                result.push_str(&formatted);
            }
        } else if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 1;
            match bytes[i] {
                b'n' => result.push('\n'),
                b't' => result.push('\t'),
                b'r' => result.push('\r'),
                b'a' => result.push('\x07'),
                b'b' => result.push('\x08'),
                b'f' => result.push('\x0c'),
                b'v' => result.push('\x0b'),
                b'\\' => result.push('\\'),
                b'0' => {
                    let start = i + 1;
                    let mut end = start;
                    while end < bytes.len() && end < start + 3 && bytes[end] >= b'0' && bytes[end] <= b'7' {
                        end += 1;
                    }
                    let oct: String = bytes[start..end].iter().map(|&b| b as char).collect();
                    if let Ok(n) = u8::from_str_radix(&oct, 8) {
                        result.push(n as char);
                    }
                    i = end - 1;
                }
                other => {
                    result.push('\\');
                    result.push(other as char);
                }
            }
            i += 1;
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }
}

fn format_scientific(val: f64, precision: usize, upper: bool) -> String {
    if val == 0.0 {
        let zeros: String = "0".repeat(precision);
        return if upper {
            format!("0.{zeros}E+00")
        } else {
            format!("0.{zeros}e+00")
        };
    }
    let exp = val.abs().log10().floor() as i32;
    let mantissa = val / 10f64.powi(exp);
    let letter = if upper { 'E' } else { 'e' };
    let sign = if exp >= 0 { '+' } else { '-' };
    format!("{mantissa:.precision$}{letter}{sign}{:02}", exp.unsigned_abs())
}

pub fn cat(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code = 0;

    if args.is_empty() || (args.len() == 1 && args[0] == "-") {
        stdout.push_str(ctx.stdin);
        return Ok(ExecResult { stdout, stderr, exit_code, env: HashMap::new() });
    }

    let mut number_lines = false;
    let mut file_args = Vec::new();
    for arg in args {
        match *arg {
            "-n" | "--number" => number_lines = true,
            _ => file_args.push(*arg),
        }
    }

    let mut line_num = 1u64;
    for path in &file_args {
        if *path == "-" {
            if number_lines {
                for line in ctx.stdin.lines() {
                    let _ = writeln!(stdout, "     {line_num}\t{line}");
                    line_num += 1;
                }
            } else {
                stdout.push_str(ctx.stdin);
            }
            continue;
        }

        let resolved = crate::fs::path::resolve(ctx.cwd, path);
        match ctx.fs.read_file_string(&resolved) {
            Ok(content) => {
                if number_lines {
                    for line in content.lines() {
                        let _ = writeln!(stdout, "     {line_num}\t{line}");
                        line_num += 1;
                    }
                } else {
                    stdout.push_str(&content);
                }
            }
            Err(e) => {
                let _ = writeln!(stderr, "cat: {e}");
                exit_code = 1;
            }
        }
    }

    Ok(ExecResult { stdout, stderr, exit_code, env: HashMap::new() })
}

pub fn wc(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut count_lines = false;
    let mut count_words = false;
    let mut count_bytes = false;
    let mut file_args = Vec::new();

    for arg in args {
        match *arg {
            "-l" | "--lines" => count_lines = true,
            "-w" | "--words" => count_words = true,
            "-c" | "--bytes" | "-m" => count_bytes = true,
            _ => file_args.push(*arg),
        }
    }

    if !count_lines && !count_words && !count_bytes {
        count_lines = true;
        count_words = true;
        count_bytes = true;
    }

    let mut stdout = String::new();
    let mut total_lines = 0u64;
    let mut total_words = 0u64;
    let mut total_bytes = 0u64;

    let inputs: Vec<(&str, String)> = if file_args.is_empty() {
        vec![("-", ctx.stdin.to_string())]
    } else {
        let mut v = Vec::new();
        for path in &file_args {
            if *path == "-" {
                v.push(("-", ctx.stdin.to_string()));
            } else {
                let resolved = crate::fs::path::resolve(ctx.cwd, path);
                if let Ok(content) = ctx.fs.read_file_string(&resolved) {
                    v.push((path, content));
                }
            }
        }
        v
    };

    for (name, content) in &inputs {
        let lines = content.lines().count() as u64;
        let words = content.split_whitespace().count() as u64;
        let bytes = content.len() as u64;

        total_lines += lines;
        total_words += words;
        total_bytes += bytes;

        let mut parts = Vec::new();
        if count_lines { parts.push(format!("{lines:>7}")); }
        if count_words { parts.push(format!("{words:>7}")); }
        if count_bytes { parts.push(format!("{bytes:>7}")); }

        if *name != "-" && !inputs.is_empty() {
            let _ = writeln!(stdout, "{} {name}", parts.join(""));
        } else {
            let _ = writeln!(stdout, "{}", parts.join(""));
        }
    }

    if inputs.len() > 1 {
        let mut parts = Vec::new();
        if count_lines { parts.push(format!("{total_lines:>7}")); }
        if count_words { parts.push(format!("{total_words:>7}")); }
        if count_bytes { parts.push(format!("{total_bytes:>7}")); }
        let _ = writeln!(stdout, "{} total", parts.join(""));
    }

    Ok(ExecResult { stdout, stderr: String::new(), exit_code: 0, env: HashMap::new() })
}

pub fn head(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut n = 10usize;
    let mut file_args = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-n" | "--lines" if i + 1 < args.len() => {
                n = args[i + 1].parse().unwrap_or(10);
                i += 2;
            }
            arg if arg.starts_with("-n") => {
                n = arg[2..].parse().unwrap_or(10);
                i += 1;
            }
            arg if arg.len() > 1
                && arg.starts_with('-')
                && arg.as_bytes()[1].is_ascii_digit() =>
            {
                n = arg[1..].parse().unwrap_or(10);
                i += 1;
            }
            _ => {
                file_args.push(args[i]);
                i += 1;
            }
        }
    }

    let content = if file_args.is_empty() {
        ctx.stdin.to_string()
    } else {
        let path = crate::fs::path::resolve(ctx.cwd, file_args[0]);
        ctx.fs.read_file_string(&path).unwrap_or_default()
    };

    let stdout = content.lines().take(n).fold(String::new(), |mut acc, l| {
        let _ = writeln!(acc, "{l}");
        acc
    });
    Ok(ExecResult { stdout, stderr: String::new(), exit_code: 0, env: HashMap::new() })
}

pub fn tail(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut n = 10usize;
    let mut from_start = false;
    let mut file_args = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-n" | "--lines" if i + 1 < args.len() => {
                let val = args[i + 1];
                if let Some(v) = val.strip_prefix('+') {
                    from_start = true;
                    n = v.parse().unwrap_or(1);
                } else {
                    n = val.parse().unwrap_or(10);
                }
                i += 2;
            }
            arg if arg.starts_with("-n") => {
                let val = &arg[2..];
                if let Some(v) = val.strip_prefix('+') {
                    from_start = true;
                    n = v.parse().unwrap_or(1);
                } else {
                    n = val.parse().unwrap_or(10);
                }
                i += 1;
            }
            arg if arg.len() > 1
                && arg.starts_with('-')
                && arg.as_bytes()[1].is_ascii_digit() =>
            {
                n = arg[1..].parse().unwrap_or(10);
                i += 1;
            }
            _ => {
                file_args.push(args[i]);
                i += 1;
            }
        }
    }

    let content = if file_args.is_empty() {
        ctx.stdin.to_string()
    } else {
        let path = crate::fs::path::resolve(ctx.cwd, file_args[0]);
        ctx.fs.read_file_string(&path).unwrap_or_default()
    };

    let lines: Vec<&str> = content.lines().collect();
    let selected: Box<dyn Iterator<Item = &&str>> = if from_start {
        Box::new(lines.iter().skip(n.saturating_sub(1)))
    } else {
        let start = lines.len().saturating_sub(n);
        Box::new(lines[start..].iter())
    };
    let stdout = selected.fold(String::new(), |mut acc, l| {
        let _ = writeln!(acc, "{l}");
        acc
    });

    Ok(ExecResult { stdout, stderr: String::new(), exit_code: 0, env: HashMap::new() })
}
