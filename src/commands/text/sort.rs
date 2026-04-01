use std::fmt::Write;
use std::collections::HashMap;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;

pub fn sort_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut reverse = false;
    let mut numeric = false;
    let mut unique = false;
    let mut ignore_case = false;
    let mut stable = false;
    let mut check = false;
    let mut human_numeric = false;
    let mut separator: Option<String> = None;
    let mut key_specs: Vec<String> = Vec::new();
    let mut output_file: Option<String> = None;
    let mut file_args = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-r" | "--reverse" => reverse = true,
            "-n" | "--numeric-sort" => numeric = true,
            "-u" | "--unique" => unique = true,
            "-f" | "--ignore-case" => ignore_case = true,
            "-s" | "--stable" => stable = true,
            "-c" | "--check" => check = true,
            "-h" | "--human-numeric-sort" => human_numeric = true,
            "-t" | "--field-separator" if i + 1 < args.len() => {
                separator = Some(args[i + 1].to_string());
                i += 1;
            }
            "-k" | "--key" if i + 1 < args.len() => {
                key_specs.push(args[i + 1].to_string());
                i += 1;
            }
            "-o" | "--output" if i + 1 < args.len() => {
                output_file = Some(args[i + 1].to_string());
                i += 1;
            }
            arg if arg.starts_with("--key=") => {
                if let Some(v) = arg.strip_prefix("--key=") {
                    key_specs.push(v.to_string());
                }
            }
            arg if arg.starts_with("--field-separator=") => {
                if let Some(v) = arg.strip_prefix("--field-separator=") {
                    separator = Some(v.to_string());
                }
            }
            arg if arg.starts_with("--output=") => {
                if let Some(v) = arg.strip_prefix("--output=") {
                    output_file = Some(v.to_string());
                }
            }
            arg if arg.starts_with("-t") && arg.len() > 2 => {
                separator = Some(arg[2..].to_string());
            }
            arg if arg.starts_with("-k") && arg.len() > 2 => {
                key_specs.push(arg[2..].to_string());
            }
            arg if arg.starts_with("-o") && arg.len() > 2 => {
                output_file = Some(arg[2..].to_string());
            }
            arg if arg.starts_with('-') && !arg.starts_with("--") && arg.len() > 2 => {
                for c in arg[1..].chars() {
                    match c {
                        'r' => reverse = true,
                        'n' => numeric = true,
                        'u' => unique = true,
                        'f' => ignore_case = true,
                        's' => stable = true,
                        'c' => check = true,
                        'h' => human_numeric = true,
                        _ => {}
                    }
                }
            }
            _ => file_args.push(args[i]),
        }
        i += 1;
    }

    let content = if file_args.is_empty() {
        ctx.stdin.to_string()
    } else {
        let mut combined = String::new();
        for f in &file_args {
            let path = crate::fs::path::resolve(ctx.cwd, f);
            if let Ok(c) = ctx.fs.read_file_string(&path) {
                combined.push_str(&c);
            }
        }
        combined
    };

    let mut lines: Vec<&str> = content.lines().collect();

    if check {
        let sorted = is_sorted(&lines, numeric, human_numeric, ignore_case, reverse, &separator, &key_specs);
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: i32::from(!sorted),
            env: HashMap::new(),
        });
    }

    let cmp = |a: &&str, b: &&str| -> std::cmp::Ordering {
        if key_specs.is_empty() {
            let ka = a.to_string();
            let kb = b.to_string();
            let ord = if numeric {
                let na = parse_leading_number(&ka);
                let nb = parse_leading_number(&kb);
                let num_ord = na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal);
                // When numeric values are equal, use string comparison
                // as a tiebreaker (matching GNU sort behaviour)
                if num_ord == std::cmp::Ordering::Equal {
                    ka.cmp(&kb)
                } else {
                    num_ord
                }
            } else if human_numeric {
                parse_human_size(&ka).partial_cmp(&parse_human_size(&kb)).unwrap_or(std::cmp::Ordering::Equal)
            } else if ignore_case {
                ka.to_lowercase().cmp(&kb.to_lowercase())
            } else {
                ka.cmp(&kb)
            };
            return if reverse { ord.reverse() } else { ord };
        }

        for spec in &key_specs {
            let ka = sort_extract_key_single(a, &separator, spec);
            let kb = sort_extract_key_single(b, &separator, spec);

            let key_numeric = numeric || spec.contains('n');
            let key_human = human_numeric || spec.contains('h');
            let key_reverse = reverse || spec.contains('r');
            let key_ignore_case = ignore_case || spec.contains('f');

            let ord = if key_numeric {
                let na = parse_leading_number(&ka);
                let nb = parse_leading_number(&kb);
                na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal)
            } else if key_human {
                parse_human_size(&ka).partial_cmp(&parse_human_size(&kb)).unwrap_or(std::cmp::Ordering::Equal)
            } else if key_ignore_case {
                ka.to_lowercase().cmp(&kb.to_lowercase())
            } else {
                ka.cmp(&kb)
            };

            let ord = if key_reverse { ord.reverse() } else { ord };
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        }
        if stable {
            std::cmp::Ordering::Equal
        } else {
            a.cmp(b)
        }
    };

    if stable {
        lines.sort_by(cmp);
    } else {
        lines.sort_unstable_by(cmp);
    }

    if unique {
        lines.dedup();
    }

    let stdout = lines.iter().fold(String::new(), |mut acc, l| {
        let _ = writeln!(acc, "{l}");
        acc
    });

    if let Some(out_path) = output_file {
        let resolved = crate::fs::path::resolve(ctx.cwd, &out_path);
        let parent = crate::fs::path::parent(&resolved);
        if parent != "/" {
            let _ = ctx.fs.mkdir(parent, true);
        }
        if let Err(e) = ctx.fs.write_file(&resolved, stdout.as_bytes()) {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: format!("sort: write error: {e}\n"),
                exit_code: 2,
                env: HashMap::new(),
            });
        }
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: 0,
            env: HashMap::new(),
        });
    }

    Ok(ExecResult { stdout, stderr: String::new(), exit_code: 0, env: HashMap::new() })
}

#[allow(clippy::ref_option)]
fn sort_extract_key_single(line: &str, separator: &Option<String>, spec: &str) -> String {
    let parts: Vec<&str> = spec.split(',').collect();
    let (start_field, _start_char) = parse_key_field(parts[0]);

    let fields: Vec<&str> = if let Some(sep) = separator {
        line.split(sep.as_str()).collect()
    } else {
        line.split_whitespace().collect()
    };

    let end_field = if parts.len() > 1 {
        let (ef, _) = parse_key_field(parts[1]);
        ef
    } else {
        start_field
    };

    let sf = start_field.saturating_sub(1);
    let ef = end_field.min(fields.len());

    if sf >= fields.len() {
        return String::new();
    }

    fields[sf..ef].join(" ")
}

fn parse_key_field(spec: &str) -> (usize, usize) {
    let digits: String = spec.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let parts: Vec<&str> = digits.split('.').collect();
    let field: usize = parts[0].parse().unwrap_or(1);
    let char_pos: usize = if parts.len() > 1 { parts[1].parse().unwrap_or(0) } else { 0 };
    (field, char_pos)
}

/// Parse the leading numeric value from a string, skipping leading
/// whitespace and ignoring trailing non-numeric text.  This matches the
/// behaviour of `sort -n` which compares by the initial numeric portion.
fn parse_leading_number(s: &str) -> f64 {
    let s = s.trim_start();
    if s.is_empty() {
        return 0.0;
    }
    let mut end = 0;
    let bytes = s.as_bytes();
    // Optional leading sign
    if end < bytes.len() && (bytes[end] == b'-' || bytes[end] == b'+') {
        end += 1;
    }
    // Digits before decimal
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    // Optional decimal part
    if end < bytes.len() && bytes[end] == b'.' {
        end += 1;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
    }
    if end == 0 || (end == 1 && (bytes[0] == b'-' || bytes[0] == b'+' || bytes[0] == b'.')) {
        return 0.0;
    }
    s[..end].parse().unwrap_or(0.0)
}

fn parse_human_size(s: &str) -> f64 {
    let s = s.trim();
    if s.is_empty() {
        return 0.0;
    }
    let last = s.as_bytes()[s.len() - 1];
    let multiplier = match last {
        b'K' | b'k' => 1024.0,
        b'M' | b'm' => 1024.0 * 1024.0,
        b'G' | b'g' => 1024.0 * 1024.0 * 1024.0,
        b'T' | b't' => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return s.parse().unwrap_or(0.0),
    };
    let num_part = &s[..s.len() - 1];
    let base: f64 = num_part.parse().unwrap_or(0.0);
    base * multiplier
}

#[allow(clippy::fn_params_excessive_bools, clippy::ref_option)]
fn is_sorted(
    lines: &[&str],
    numeric: bool,
    human_numeric: bool,
    ignore_case: bool,
    reverse: bool,
    separator: &Option<String>,
    key_specs: &[String],
) -> bool {
    for pair in lines.windows(2) {
        let ka = if key_specs.is_empty() {
            pair[0].to_string()
        } else {
            sort_extract_key_single(pair[0], separator, &key_specs[0])
        };
        let kb = if key_specs.is_empty() {
            pair[1].to_string()
        } else {
            sort_extract_key_single(pair[1], separator, &key_specs[0])
        };

        let ord = if numeric {
            let na = parse_leading_number(&ka);
            let nb = parse_leading_number(&kb);
            na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal)
        } else if human_numeric {
            parse_human_size(&ka).partial_cmp(&parse_human_size(&kb)).unwrap_or(std::cmp::Ordering::Equal)
        } else if ignore_case {
            ka.to_lowercase().cmp(&kb.to_lowercase())
        } else {
            ka.cmp(&kb)
        };

        let ord = if reverse { ord.reverse() } else { ord };
        if ord == std::cmp::Ordering::Greater {
            return false;
        }
    }
    true
}
