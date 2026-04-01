use std::fmt::Write;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;
use std::collections::HashMap;

pub fn glob_match_simple(pattern: &str, text: &str) -> bool {
    glob_match(pattern, text, false)
}

fn glob_match(pattern: &str, text: &str, case_insensitive: bool) -> bool {
    let pattern: Vec<char> = if case_insensitive {
        pattern.to_lowercase().chars().collect()
    } else {
        pattern.chars().collect()
    };
    let text: Vec<char> = if case_insensitive {
        text.to_lowercase().chars().collect()
    } else {
        text.chars().collect()
    };
    glob_match_inner(&pattern, &text)
}

fn glob_match_inner(pattern: &[char], text: &[char]) -> bool {
    let mut pi = 0;
    let mut ti = 0;
    let mut saved_pat = None;
    let mut saved_text = 0;
    while ti < text.len() {
        if pi < pattern.len() && pattern[pi] == '?' {
            pi += 1;
            ti += 1;
        } else if pi < pattern.len() && pattern[pi] == '*' {
            saved_pat = Some(pi);
            saved_text = ti;
            pi += 1;
        } else if pi < pattern.len() && pattern[pi] == '[' {
            if let Some((matched, end)) = match_bracket(&pattern[pi..], text[ti]) {
                if matched {
                    pi += end;
                    ti += 1;
                } else if let Some(sp) = saved_pat {
                    pi = sp + 1;
                    saved_text += 1;
                    ti = saved_text;
                } else {
                    return false;
                }
            } else if let Some(sp) = saved_pat {
                pi = sp + 1;
                saved_text += 1;
                ti = saved_text;
            } else {
                return false;
            }
        } else if pi < pattern.len() && pattern[pi] == text[ti] {
            pi += 1;
            ti += 1;
        } else if let Some(sp) = saved_pat {
            pi = sp + 1;
            saved_text += 1;
            ti = saved_text;
        } else {
            return false;
        }
    }
    while pi < pattern.len() && pattern[pi] == '*' {
        pi += 1;
    }
    pi == pattern.len()
}

fn match_bracket(pattern: &[char], ch: char) -> Option<(bool, usize)> {
    if pattern.is_empty() || pattern[0] != '[' {
        return None;
    }
    let mut i = 1;
    let negate = i < pattern.len() && (pattern[i] == '!' || pattern[i] == '^');
    if negate {
        i += 1;
    }
    let mut matched = false;
    while i < pattern.len() && pattern[i] != ']' {
        if i + 2 < pattern.len() && pattern[i + 1] == '-' && pattern[i + 2] != ']' {
            if ch >= pattern[i] && ch <= pattern[i + 2] {
                matched = true;
            }
            i += 3;
        } else {
            if pattern[i] == ch {
                matched = true;
            }
            i += 1;
        }
    }
    if i < pattern.len() && pattern[i] == ']' {
        let result = if negate { !matched } else { matched };
        Some((result, i + 1))
    } else {
        None
    }
}

struct FindOpts<'a> {
    name_pattern: Option<&'a str>,
    iname_pattern: Option<&'a str>,
    path_pattern: Option<&'a str>,
    type_filter: Option<char>,
    empty: bool,
    maxdepth: Option<usize>,
    mindepth: Option<usize>,
    print0: bool,
    delete: bool,
    size_spec: Option<String>,
    mtime_spec: Option<String>,
    newer_file: Option<String>,
    exec_cmd: Vec<String>,
}

pub fn find_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut paths = Vec::new();
    let mut opts = FindOpts {
        name_pattern: None,
        iname_pattern: None,
        path_pattern: None,
        type_filter: None,
        empty: false,
        maxdepth: None,
        mindepth: None,
        print0: false,
        delete: false,
        size_spec: None,
        mtime_spec: None,
        newer_file: None,
        exec_cmd: Vec::new(),
    };

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-name" if i + 1 < args.len() => {
                opts.name_pattern = Some(args[i + 1]);
                i += 2;
            }
            "-iname" if i + 1 < args.len() => {
                opts.iname_pattern = Some(args[i + 1]);
                i += 2;
            }
            "-path" if i + 1 < args.len() => {
                opts.path_pattern = Some(args[i + 1]);
                i += 2;
            }
            "-type" if i + 1 < args.len() => {
                opts.type_filter = args[i + 1].chars().next();
                i += 2;
            }
            "-empty" => {
                opts.empty = true;
                i += 1;
            }
            "-maxdepth" if i + 1 < args.len() => {
                opts.maxdepth = args[i + 1].parse().ok();
                i += 2;
            }
            "-mindepth" if i + 1 < args.len() => {
                opts.mindepth = args[i + 1].parse().ok();
                i += 2;
            }
            "-size" if i + 1 < args.len() => {
                opts.size_spec = Some(args[i + 1].to_string());
                i += 2;
            }
            "-mtime" if i + 1 < args.len() => {
                opts.mtime_spec = Some(args[i + 1].to_string());
                i += 2;
            }
            "-newer" if i + 1 < args.len() => {
                let resolved = crate::fs::path::resolve(ctx.cwd, args[i + 1]);
                opts.newer_file = Some(resolved);
                i += 2;
            }
            "-exec" => {
                i += 1;
                let mut cmd_parts = Vec::new();
                while i < args.len() && args[i] != ";" {
                    cmd_parts.push(args[i].to_string());
                    i += 1;
                }
                if i < args.len() {
                    i += 1; // skip the ";"
                }
                opts.exec_cmd = cmd_parts;
            }
            "-print" => {
                i += 1;
            }
            "-print0" => {
                opts.print0 = true;
                i += 1;
            }
            "-delete" => {
                opts.delete = true;
                i += 1;
            }
            arg if !arg.starts_with('-') => {
                paths.push(arg);
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    if paths.is_empty() {
        paths.push(".");
    }

    let newer_mtime = opts.newer_file.as_ref().and_then(|p| {
        ctx.fs.stat(p).ok().map(|m| m.mtime)
    });

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code = 0;
    let mut to_delete = Vec::new();

    for start_path in &paths {
        let resolved = crate::fs::path::resolve(ctx.cwd, start_path);
        find_walk(
            ctx,
            &resolved,
            &opts,
            0,
            newer_mtime.as_ref(),
            &mut stdout,
            &mut stderr,
            &mut exit_code,
            &mut to_delete,
        );
    }

    if opts.delete {
        for path in to_delete.iter().rev() {
            if let Err(e) = ctx.fs.rm(path, false, false) {
                let _ = writeln!(stderr, "find: cannot delete '{path}': {e}");
                exit_code = 1;
            }
        }
    }

    Ok(ExecResult { stdout, stderr, exit_code, env: HashMap::new() })
}

#[allow(clippy::too_many_arguments)]
fn find_walk(
    ctx: &CommandContext<'_>,
    path: &str,
    opts: &FindOpts<'_>,
    depth: usize,
    newer_mtime: Option<&std::time::SystemTime>,
    stdout: &mut String,
    stderr: &mut String,
    exit_code: &mut i32,
    to_delete: &mut Vec<String>,
) {
    if let Some(max) = opts.maxdepth {
        if depth > max {
            return;
        }
    }

    let meta = match ctx.fs.lstat(path) {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(stderr, "find: '{path}': {e}");
            *exit_code = 1;
            return;
        }
    };

    let name = crate::fs::path::basename(path);
    let matches = find_matches(ctx, path, name, &meta, opts, newer_mtime);

    let above_mindepth = opts.mindepth.is_none_or(|min| depth >= min);

    if matches && above_mindepth {
        if !opts.exec_cmd.is_empty() {
            let cmd_str: String = opts.exec_cmd.iter().map(|part| {
                if part == "{}" { path.to_string() } else { part.clone() }
            }).collect::<Vec<_>>().join(" ");
            if let Some(exec_fn) = ctx.exec_fn {
                match exec_fn(&cmd_str) {
                    Ok(result) => {
                        stdout.push_str(&result.stdout);
                        stderr.push_str(&result.stderr);
                        if result.exit_code != 0 {
                            *exit_code = result.exit_code;
                        }
                    }
                    Err(_) => {
                        *exit_code = 1;
                    }
                }
            }
        } else if opts.delete {
            to_delete.push(path.to_string());
        } else if opts.print0 {
            stdout.push_str(path);
            stdout.push('\0');
        } else {
            let _ = writeln!(stdout, "{path}");
        }
    }

    if meta.is_dir() {
        if let Ok(entries) = ctx.fs.readdir(path) {
            let mut entries = entries;
            entries.sort_by(|a, b| a.name.cmp(&b.name));
            for entry in entries {
                let child = if path == "/" {
                    format!("/{}", entry.name)
                } else {
                    format!("{path}/{}", entry.name)
                };
                find_walk(ctx, &child, opts, depth + 1, newer_mtime, stdout, stderr, exit_code, to_delete);
            }
        }
    }
}

fn find_matches(
    ctx: &CommandContext<'_>,
    path: &str,
    name: &str,
    meta: &crate::fs::Metadata,
    opts: &FindOpts<'_>,
    newer_mtime: Option<&std::time::SystemTime>,
) -> bool {
    if let Some(pat) = opts.name_pattern {
        if !glob_match(pat, name, false) {
            return false;
        }
    }
    if let Some(pat) = opts.iname_pattern {
        if !glob_match(pat, name, true) {
            return false;
        }
    }
    if let Some(pat) = opts.path_pattern {
        if !glob_match(pat, path, false) {
            return false;
        }
    }
    if let Some(t) = opts.type_filter {
        let matches_type = match t {
            'f' => meta.is_file(),
            'd' => meta.is_dir(),
            'l' => meta.is_symlink(),
            _ => true,
        };
        if !matches_type {
            return false;
        }
    }
    if opts.empty {
        if meta.is_file() && meta.size != 0 {
            return false;
        }
        if meta.is_dir() {
            if let Ok(entries) = ctx.fs.readdir(path) {
                if !entries.is_empty() {
                    return false;
                }
            }
        }
    }
    if let Some(ref spec) = opts.size_spec {
        if !match_size(meta.size, spec) {
            return false;
        }
    }
    if let Some(ref spec) = opts.mtime_spec {
        if !match_mtime(meta.mtime, spec) {
            return false;
        }
    }
    if let Some(ref_time) = newer_mtime {
        if meta.mtime <= *ref_time {
            return false;
        }
    }
    true
}

fn match_size(actual: u64, spec: &str) -> bool {
    let bytes = spec.as_bytes();
    if bytes.is_empty() {
        return true;
    }
    let (cmp_mode, rest) = if bytes[0] == b'+' {
        (1i8, &spec[1..])
    } else if bytes[0] == b'-' {
        (-1i8, &spec[1..])
    } else {
        (0i8, spec)
    };

    let (num_str, unit) = if let Some(s) = rest.strip_suffix('c') {
        (s, 1u64)
    } else if let Some(s) = rest.strip_suffix('k') {
        (s, 1024u64)
    } else if let Some(s) = rest.strip_suffix('M') {
        (s, 1024 * 1024)
    } else if let Some(s) = rest.strip_suffix('G') {
        (s, 1024 * 1024 * 1024)
    } else {
        (rest, 512u64)
    };

    let target: u64 = match num_str.parse() {
        Ok(n) => n,
        Err(_) => return true,
    };
    let target_bytes = target.saturating_mul(unit);

    match cmp_mode {
        1 => actual > target_bytes,
        -1 => actual < target_bytes,
        _ => {
            let block = if unit == 1 { 1 } else { unit };
            let actual_blocks = actual.div_ceil(block);
            actual_blocks == target
        }
    }
}

fn match_mtime(mtime: std::time::SystemTime, spec: &str) -> bool {
    let bytes = spec.as_bytes();
    if bytes.is_empty() {
        return true;
    }
    let (cmp_mode, num_str) = if bytes[0] == b'+' {
        (1i8, &spec[1..])
    } else if bytes[0] == b'-' {
        (-1i8, &spec[1..])
    } else {
        (0i8, spec)
    };

    let days: f64 = match num_str.parse() {
        Ok(n) => n,
        Err(_) => return true,
    };

    let now = std::time::SystemTime::now();
    let age_secs = now.duration_since(mtime).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    let age_days = age_secs / 86400.0;

    match cmp_mode {
        1 => age_days > days + 1.0,
        -1 => age_days < days,
        _ => age_days >= days && age_days < days + 1.0,
    }
}

pub fn which_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let builtins = [
        "echo", "printf", "true", "false", "pwd", "env", "printenv",
        "test", "[", "cat", "wc", "head", "tail", "sort", "grep",
        "seq", "basename", "dirname", "mkdir", "rm", "touch", "cp", "mv",
        "find", "which", "egrep", "fgrep",
        "date", "sleep", "yes", "expr", "bc", "realpath", "mktemp",
        "whoami", "hostname", "uname", "timeout", "nohup", "xargs",
        "md5sum", "sha1sum", "sha256sum", "sha512sum",
    ];

    if args.is_empty() {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: 1,
            env: HashMap::new(),
});
    }

    let mut stdout = String::new();
    let mut exit_code = 0;

    let path_var = ctx.env.get("PATH").cloned().unwrap_or_default();
    let path_dirs: Vec<&str> = path_var.split(':').collect();

    for name in args {
        if builtins.contains(name) {
            let mut found = false;
            for dir in &path_dirs {
                let full = format!("{dir}/{name}");
                if ctx.fs.exists(&full) {
                    let _ = writeln!(stdout, "{full}");
                    found = true;
                    break;
                }
            }
            if !found {
                let _ = writeln!(stdout, "/usr/bin/{name}");
            }
        } else {
            let mut found = false;
            for dir in &path_dirs {
                let full = format!("{dir}/{name}");
                if ctx.fs.exists(&full) {
                    let _ = writeln!(stdout, "{full}");
                    found = true;
                    break;
                }
            }
            if !found {
                exit_code = 1;
            }
        }
    }

    Ok(ExecResult { stdout, stderr: String::new(), exit_code, env: HashMap::new() })
}

pub fn egrep_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    super::text::grep_cmd(args, ctx)
}

pub fn fgrep_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut ignore_case = false;
    let mut invert = false;
    let mut count_only = false;
    let mut line_numbers = false;
    let mut pattern_str = None;
    let mut file_args = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-v" | "--invert-match" => invert = true,
            "-c" | "--count" => count_only = true,
            "-i" | "--ignore-case" => ignore_case = true,
            "-n" | "--line-number" => line_numbers = true,
            "-e" if i + 1 < args.len() => {
                pattern_str = Some(args[i + 1]);
                i += 1;
            }
            arg if !arg.starts_with('-') => {
                if pattern_str.is_none() {
                    pattern_str = Some(arg);
                } else {
                    file_args.push(arg);
                }
            }
            _ => {}
        }
        i += 1;
    }

    let Some(pattern) = pattern_str else {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "fgrep: no pattern\n".to_string(),
            exit_code: 2,
            env: HashMap::new(),
});
    };

    let content = if file_args.is_empty() {
        ctx.stdin.to_string()
    } else {
        let path = crate::fs::path::resolve(ctx.cwd, file_args[0]);
        ctx.fs.read_file_string(&path).unwrap_or_default()
    };

    let pattern_cmp = if ignore_case { pattern.to_lowercase() } else { pattern.to_string() };
    let mut stdout = String::new();
    let mut match_count = 0u64;

    for (line_idx, line) in content.lines().enumerate() {
        let line_cmp = if ignore_case { line.to_lowercase() } else { line.to_string() };
        let matches = line_cmp.contains(&pattern_cmp);
        let show = if invert { !matches } else { matches };
        if show {
            match_count += 1;
            if !count_only {
                if line_numbers {
                    let _ = writeln!(stdout, "{}:{line}", line_idx + 1);
                } else {
                    let _ = writeln!(stdout, "{line}");
                }
            }
        }
    }

    if count_only {
        stdout = format!("{match_count}\n");
    }

    let exit_code = i32::from(match_count == 0);
    Ok(ExecResult { stdout, stderr: String::new(), exit_code, env: HashMap::new() })
}
