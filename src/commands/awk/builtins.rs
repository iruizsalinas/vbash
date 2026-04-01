use std::fmt::Write;

use regex::Regex;

use super::value::AwkValue;

pub(super) fn looks_numeric(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return false;
    }
    trimmed.parse::<f64>().is_ok()
}

pub(super) fn awk_regex_replace_first(re: &Regex, target: &str, repl: &str) -> String {
    if let Some(m) = re.find(target) {
        let mut result = String::new();
        result.push_str(&target[..m.start()]);
        result.push_str(&awk_replacement(repl, m.as_str()));
        result.push_str(&target[m.end()..]);
        result
    } else {
        target.to_string()
    }
}

pub(super) fn awk_regex_replace_all(re: &Regex, target: &str, repl: &str) -> (String, usize) {
    let mut result = String::new();
    let mut count = 0usize;
    let mut last_end = 0;
    for m in re.find_iter(target) {
        result.push_str(&target[last_end..m.start()]);
        result.push_str(&awk_replacement(repl, m.as_str()));
        last_end = m.end();
        count += 1;
        if m.start() == m.end() {
            if last_end < target.len() {
                let ch = &target[last_end..last_end + target[last_end..].chars().next().map_or(0, char::len_utf8)];
                result.push_str(ch);
                last_end += ch.len();
            } else {
                break;
            }
        }
    }
    result.push_str(&target[last_end..]);
    (result, count)
}

fn awk_replacement(repl: &str, matched: &str) -> String {
    let mut result = String::new();
    let bytes = repl.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'&' {
            result.push_str(matched);
        } else if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 1;
            if bytes[i] == b'&' {
                result.push('&');
            } else if bytes[i] == b'\\' {
                result.push('\\');
            } else {
                result.push('\\');
                result.push(bytes[i] as char);
            }
        } else {
            result.push(bytes[i] as char);
        }
        i += 1;
    }
    result
}

pub(super) fn awk_sprintf(fmt: &str, args: &[AwkValue]) -> String {
    let mut result = String::new();
    let bytes = fmt.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut arg_idx = 0;

    while i < len {
        if bytes[i] == b'\\' && i + 1 < len {
            i += 1;
            match bytes[i] {
                b'n' => result.push('\n'),
                b't' => result.push('\t'),
                b'r' => result.push('\r'),
                b'\\' => result.push('\\'),
                b'"' => result.push('"'),
                b'a' => result.push('\x07'),
                b'b' => result.push('\x08'),
                b'f' => result.push('\x0c'),
                b'v' => result.push('\x0b'),
                other => {
                    result.push('\\');
                    result.push(other as char);
                }
            }
            i += 1;
            continue;
        }

        if bytes[i] != b'%' {
            result.push(bytes[i] as char);
            i += 1;
            continue;
        }

        i += 1;
        if i >= len {
            result.push('%');
            break;
        }

        if bytes[i] == b'%' {
            result.push('%');
            i += 1;
            continue;
        }

        let mut flags = String::new();
        while i < len && matches!(bytes[i], b'-' | b'+' | b' ' | b'0' | b'#') {
            flags.push(bytes[i] as char);
            i += 1;
        }

        let mut width = String::new();
        if i < len && bytes[i] == b'*' {
            let w = args.get(arg_idx).map_or(0.0, AwkValue::to_num) as i64;
            width = w.to_string();
            arg_idx += 1;
            i += 1;
        } else {
            while i < len && bytes[i].is_ascii_digit() {
                width.push(bytes[i] as char);
                i += 1;
            }
        }

        let mut precision = String::new();
        let has_precision;
        if i < len && bytes[i] == b'.' {
            has_precision = true;
            i += 1;
            if i < len && bytes[i] == b'*' {
                let p = args.get(arg_idx).map_or(0.0, AwkValue::to_num) as i64;
                precision = p.to_string();
                arg_idx += 1;
                i += 1;
            } else {
                while i < len && bytes[i].is_ascii_digit() {
                    precision.push(bytes[i] as char);
                    i += 1;
                }
                if precision.is_empty() {
                    precision = "0".to_string();
                }
            }
        } else {
            has_precision = false;
        }

        if i >= len {
            break;
        }

        let conv = bytes[i] as char;
        i += 1;

        let arg = args.get(arg_idx).cloned().unwrap_or(AwkValue::Uninit);
        arg_idx += 1;

        let w: usize = width.parse().unwrap_or(0);
        let p: usize = precision.parse().unwrap_or(6);
        let left_align = flags.contains('-');
        let zero_pad = flags.contains('0') && !left_align;
        let plus_sign = flags.contains('+');
        let space_sign = flags.contains(' ');

        let formatted = match conv {
            'd' | 'i' => {
                let n = arg.to_num() as i64;
                let mut s = n.to_string();
                if n >= 0 {
                    if plus_sign {
                        s = format!("+{s}");
                    } else if space_sign {
                        s = format!(" {s}");
                    }
                }
                s
            }
            'o' => {
                let n = arg.to_num() as i64 as u64;
                format!("{n:o}")
            }
            'x' => {
                let n = arg.to_num() as i64 as u64;
                format!("{n:x}")
            }
            'X' => {
                let n = arg.to_num() as i64 as u64;
                format!("{n:X}")
            }
            'f' => {
                let n = arg.to_num();
                let prec = if has_precision { p } else { 6 };
                let mut s = format!("{n:.prec$}");
                if n >= 0.0 {
                    if plus_sign {
                        s = format!("+{s}");
                    } else if space_sign {
                        s = format!(" {s}");
                    }
                }
                s
            }
            'e' => {
                let n = arg.to_num();
                let prec = if has_precision { p } else { 6 };
                format_scientific(n, prec, false)
            }
            'E' => {
                let n = arg.to_num();
                let prec = if has_precision { p } else { 6 };
                format_scientific(n, prec, true)
            }
            'g' => {
                let n = arg.to_num();
                let prec = if has_precision { p.max(1) } else { 6 };
                format_g(n, prec, false)
            }
            'G' => {
                let n = arg.to_num();
                let prec = if has_precision { p.max(1) } else { 6 };
                format_g(n, prec, true)
            }
            's' => {
                let s = arg.to_str();
                if has_precision {
                    s.chars().take(p).collect()
                } else {
                    s
                }
            }
            'c' => {
                match &arg {
                    AwkValue::Num(n) => {
                        let ch = (*n as u32).min(0x10FFFF);
                        char::from_u32(ch).map_or_else(String::new, |c| c.to_string())
                    }
                    AwkValue::Str(s) => s.chars().next().map_or_else(String::new, |c| c.to_string()),
                    AwkValue::Uninit => "\0".to_string(),
                }
            }
            _ => {
                let _ = write!(result, "%{conv}");
                continue;
            }
        };

        if w > formatted.len() {
            let pad = w - formatted.len();
            if left_align {
                result.push_str(&formatted);
                for _ in 0..pad {
                    result.push(' ');
                }
            } else if zero_pad && matches!(conv, 'd' | 'i' | 'f' | 'e' | 'E' | 'g' | 'G') {
                let (sign, digits) = if formatted.starts_with('-') || formatted.starts_with('+') || formatted.starts_with(' ') {
                    (&formatted[..1], &formatted[1..])
                } else {
                    ("", formatted.as_str())
                };
                result.push_str(sign);
                for _ in 0..pad {
                    result.push('0');
                }
                result.push_str(digits);
            } else {
                for _ in 0..pad {
                    result.push(' ');
                }
                result.push_str(&formatted);
            }
        } else {
            result.push_str(&formatted);
        }
    }

    result
}

pub(super) fn format_scientific(n: f64, prec: usize, upper: bool) -> String {
    if n == 0.0 {
        let e_char = if upper { 'E' } else { 'e' };
        return format!("{:.prec$}{e_char}+00", 0.0);
    }
    let abs_n = n.abs();
    let exp = abs_n.log10().floor() as i32;
    let mantissa = n / 10f64.powi(exp);
    let e_char = if upper { 'E' } else { 'e' };
    let sign = if exp >= 0 { '+' } else { '-' };
    format!("{mantissa:.prec$}{e_char}{sign}{:02}", exp.unsigned_abs())
}

pub(super) fn format_g(n: f64, prec: usize, upper: bool) -> String {
    if n == 0.0 {
        return "0".to_string();
    }
    let abs_n = n.abs();
    let exp = if abs_n == 0.0 { 0 } else { abs_n.log10().floor() as i32 };
    if exp >= -(1i32) && exp < prec as i32 {
        let decimal_places = if prec as i32 - 1 - exp > 0 {
            (prec as i32 - 1 - exp) as usize
        } else {
            0
        };
        let s = format!("{n:.decimal_places$}");
        trim_trailing_zeros(&s)
    } else {
        format_scientific(n, prec.saturating_sub(1), upper)
    }
}

fn trim_trailing_zeros(s: &str) -> String {
    if !s.contains('.') {
        return s.to_string();
    }
    let trimmed = s.trim_end_matches('0');
    let trimmed = trimmed.trim_end_matches('.');
    trimmed.to_string()
}
