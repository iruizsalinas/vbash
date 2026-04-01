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

enum FindExpr {
    Name(String, bool),
    Path(String),
    Type(char),
    Empty,
    Size(String),
    Mtime(String),
    Newer(String),
    Exec(Vec<String>),
    Print,
    Print0,
    Delete,
    Not(Box<FindExpr>),
    And(Box<FindExpr>, Box<FindExpr>),
    Or(Box<FindExpr>, Box<FindExpr>),
    True,
}

struct FindGlobals {
    maxdepth: Option<usize>,
    mindepth: Option<usize>,
    has_print_action: bool,
    newer_mtime: Option<std::time::SystemTime>,
}

fn parse_find_expr(args: &[&str], pos: &mut usize, cwd: &str) -> FindExpr {
    parse_or(args, pos, cwd)
}

fn parse_or(args: &[&str], pos: &mut usize, cwd: &str) -> FindExpr {
    let mut left = parse_and(args, pos, cwd);
    while *pos < args.len() && (args[*pos] == "-o" || args[*pos] == "-or") {
        *pos += 1;
        let right = parse_and(args, pos, cwd);
        left = FindExpr::Or(Box::new(left), Box::new(right));
    }
    left
}

fn parse_and(args: &[&str], pos: &mut usize, cwd: &str) -> FindExpr {
    let mut left = parse_not(args, pos, cwd);
    loop {
        if *pos >= args.len() { break; }
        if args[*pos] == "-o" || args[*pos] == "-or" || args[*pos] == ")" { break; }
        if args[*pos] == "-a" || args[*pos] == "-and" {
            *pos += 1;
        }
        if *pos >= args.len() || args[*pos] == "-o" || args[*pos] == "-or" || args[*pos] == ")" { break; }
        let right = parse_not(args, pos, cwd);
        left = FindExpr::And(Box::new(left), Box::new(right));
    }
    left
}

fn parse_not(args: &[&str], pos: &mut usize, cwd: &str) -> FindExpr {
    if *pos < args.len() && (args[*pos] == "!" || args[*pos] == "-not") {
        *pos += 1;
        let inner = parse_not(args, pos, cwd);
        return FindExpr::Not(Box::new(inner));
    }
    parse_primary(args, pos, cwd)
}

fn parse_primary(args: &[&str], pos: &mut usize, cwd: &str) -> FindExpr {
    if *pos >= args.len() {
        return FindExpr::True;
    }
    match args[*pos] {
        "(" => {
            *pos += 1;
            let expr = parse_or(args, pos, cwd);
            if *pos < args.len() && args[*pos] == ")" {
                *pos += 1;
            }
            expr
        }
        "-name" if *pos + 1 < args.len() => {
            *pos += 1;
            let pat = args[*pos].to_string();
            *pos += 1;
            FindExpr::Name(pat, false)
        }
        "-iname" if *pos + 1 < args.len() => {
            *pos += 1;
            let pat = args[*pos].to_string();
            *pos += 1;
            FindExpr::Name(pat, true)
        }
        "-path" if *pos + 1 < args.len() => {
            *pos += 1;
            let pat = args[*pos].to_string();
            *pos += 1;
            FindExpr::Path(pat)
        }
        "-type" if *pos + 1 < args.len() => {
            *pos += 1;
            let t = args[*pos].chars().next().unwrap_or('f');
            *pos += 1;
            FindExpr::Type(t)
        }
        "-empty" => { *pos += 1; FindExpr::Empty }
        "-size" if *pos + 1 < args.len() => {
            *pos += 1;
            let s = args[*pos].to_string();
            *pos += 1;
            FindExpr::Size(s)
        }
        "-mtime" if *pos + 1 < args.len() => {
            *pos += 1;
            let s = args[*pos].to_string();
            *pos += 1;
            FindExpr::Mtime(s)
        }
        "-newer" if *pos + 1 < args.len() => {
            *pos += 1;
            let resolved = crate::fs::path::resolve(cwd, args[*pos]);
            *pos += 1;
            FindExpr::Newer(resolved)
        }
        "-exec" => {
            *pos += 1;
            let mut parts = Vec::new();
            while *pos < args.len() && args[*pos] != ";" {
                parts.push(args[*pos].to_string());
                *pos += 1;
            }
            if *pos < args.len() { *pos += 1; }
            FindExpr::Exec(parts)
        }
        "-print" => { *pos += 1; FindExpr::Print }
        "-print0" => { *pos += 1; FindExpr::Print0 }
        "-delete" => { *pos += 1; FindExpr::Delete }
        _ => { *pos += 1; FindExpr::True }
    }
}

fn expr_has_action(e: &FindExpr) -> bool {
    match e {
        FindExpr::Print | FindExpr::Print0 | FindExpr::Delete | FindExpr::Exec(_) => true,
        FindExpr::Not(inner) => expr_has_action(inner),
        FindExpr::And(l, r) | FindExpr::Or(l, r) => expr_has_action(l) || expr_has_action(r),
        _ => false,
    }
}

fn expr_find_newer(e: &FindExpr) -> Option<&str> {
    match e {
        FindExpr::Newer(p) => Some(p),
        FindExpr::Not(inner) => expr_find_newer(inner),
        FindExpr::And(l, r) | FindExpr::Or(l, r) => expr_find_newer(l).or_else(|| expr_find_newer(r)),
        _ => None,
    }
}

pub fn find_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut paths = Vec::new();
    let mut globals = FindGlobals {
        maxdepth: None,
        mindepth: None,
        has_print_action: false,
        newer_mtime: None,
    };

    let mut pred_start = 0;
    for (i, arg) in args.iter().enumerate() {
        match *arg {
            "-maxdepth" if i + 1 < args.len() => {
                globals.maxdepth = args[i + 1].parse().ok();
                pred_start = i + 2;
            }
            "-mindepth" if i + 1 < args.len() => {
                globals.mindepth = args[i + 1].parse().ok();
                pred_start = i + 2;
            }
            a if !a.starts_with('-') && !a.starts_with('(') && !a.starts_with('!') && pred_start == i => {
                paths.push(*arg);
                pred_start = i + 1;
            }
            _ => { break; }
        }
    }

    let pred_args: Vec<&str> = args[pred_start..].to_vec();
    let expr = if pred_args.is_empty() {
        FindExpr::True
    } else {
        let filtered: Vec<&str> = {
            let mut out = Vec::new();
            let mut j = 0;
            while j < pred_args.len() {
                match pred_args[j] {
                    "-maxdepth" | "-mindepth" if j + 1 < pred_args.len() => {
                        if pred_args[j] == "-maxdepth" { globals.maxdepth = pred_args[j + 1].parse().ok(); }
                        else { globals.mindepth = pred_args[j + 1].parse().ok(); }
                        j += 2;
                    }
                    _ => { out.push(pred_args[j]); j += 1; }
                }
            }
            out
        };
        let mut fpos = 0;
        parse_find_expr(&filtered, &mut fpos, ctx.cwd)
    };

    globals.has_print_action = expr_has_action(&expr);

    if let Some(p) = expr_find_newer(&expr) {
        globals.newer_mtime = ctx.fs.stat(p).ok().map(|m| m.mtime);
    }

    if paths.is_empty() { paths.push("."); }

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code = 0;
    let mut to_delete = Vec::new();

    for start_path in &paths {
        let resolved = crate::fs::path::resolve(ctx.cwd, start_path);
        find_walk(ctx, &resolved, &expr, &globals, 0, &mut stdout, &mut stderr, &mut exit_code, &mut to_delete);
    }

    for path in to_delete.iter().rev() {
        if let Err(e) = ctx.fs.rm(path, false, false) {
            let _ = writeln!(stderr, "find: cannot delete '{path}': {e}");
            exit_code = 1;
        }
    }

    Ok(ExecResult { stdout, stderr, exit_code, env: HashMap::new() })
}

#[allow(clippy::too_many_arguments)]
fn find_walk(
    ctx: &CommandContext<'_>,
    path: &str,
    expr: &FindExpr,
    globals: &FindGlobals,
    depth: usize,
    stdout: &mut String,
    stderr: &mut String,
    exit_code: &mut i32,
    to_delete: &mut Vec<String>,
) {
    if globals.maxdepth.is_some_and(|max| depth > max) {
        return;
    }

    let meta = match ctx.fs.lstat(path) {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(stderr, "find: '{path}': {e}");
            *exit_code = 1;
            return;
        }
    };

    let above_mindepth = globals.mindepth.is_none_or(|min| depth >= min);

    if above_mindepth {
        let name = crate::fs::path::basename(path);
        eval_expr(ctx, path, name, &meta, expr, globals, stdout, stderr, exit_code, to_delete);

        if !globals.has_print_action && eval_filter(ctx, path, name, &meta, expr, globals) {
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
                find_walk(ctx, &child, expr, globals, depth + 1, stdout, stderr, exit_code, to_delete);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn eval_expr(
    ctx: &CommandContext<'_>,
    path: &str,
    name: &str,
    meta: &crate::fs::Metadata,
    expr: &FindExpr,
    globals: &FindGlobals,
    stdout: &mut String,
    stderr: &mut String,
    exit_code: &mut i32,
    to_delete: &mut Vec<String>,
) -> bool {
    match expr {
        FindExpr::True => true,
        FindExpr::Name(pat, ic) => glob_match(pat, name, *ic),
        FindExpr::Path(pat) => glob_match(pat, path, false),
        FindExpr::Type(t) => match *t {
            'f' => meta.is_file(),
            'd' => meta.is_dir(),
            'l' => meta.is_symlink(),
            _ => true,
        },
        FindExpr::Empty => {
            if meta.is_file() { meta.size == 0 }
            else if meta.is_dir() { ctx.fs.readdir(path).map_or(true, |e| e.is_empty()) }
            else { false }
        }
        FindExpr::Size(spec) => match_size(meta.size, spec),
        FindExpr::Mtime(spec) => match_mtime(meta.mtime, spec),
        FindExpr::Newer(_) => globals.newer_mtime.is_none_or(|ref_time| meta.mtime > ref_time),
        FindExpr::Print => {
            let _ = writeln!(stdout, "{path}");
            true
        }
        FindExpr::Print0 => {
            stdout.push_str(path);
            stdout.push('\0');
            true
        }
        FindExpr::Delete => {
            to_delete.push(path.to_string());
            true
        }
        FindExpr::Exec(parts) => {
            let cmd_str: String = parts.iter().map(|p| {
                if p == "{}" { path.to_string() } else { p.clone() }
            }).collect::<Vec<_>>().join(" ");
            if let Some(exec_fn) = ctx.exec_fn {
                if let Ok(result) = exec_fn(&cmd_str) {
                    stdout.push_str(&result.stdout);
                    stderr.push_str(&result.stderr);
                    if result.exit_code != 0 { *exit_code = result.exit_code; }
                    result.exit_code == 0
                } else {
                    *exit_code = 1;
                    false
                }
            } else { false }
        }
        FindExpr::Not(inner) => !eval_expr(ctx, path, name, meta, inner, globals, stdout, stderr, exit_code, to_delete),
        FindExpr::And(l, r) => {
            eval_expr(ctx, path, name, meta, l, globals, stdout, stderr, exit_code, to_delete)
                && eval_expr(ctx, path, name, meta, r, globals, stdout, stderr, exit_code, to_delete)
        }
        FindExpr::Or(l, r) => {
            eval_expr(ctx, path, name, meta, l, globals, stdout, stderr, exit_code, to_delete)
                || eval_expr(ctx, path, name, meta, r, globals, stdout, stderr, exit_code, to_delete)
        }
    }
}

fn eval_filter(
    ctx: &CommandContext<'_>,
    path: &str,
    name: &str,
    meta: &crate::fs::Metadata,
    expr: &FindExpr,
    globals: &FindGlobals,
) -> bool {
    match expr {
        FindExpr::True | FindExpr::Print | FindExpr::Print0 | FindExpr::Delete | FindExpr::Exec(_) => true,
        FindExpr::Name(pat, ic) => glob_match(pat, name, *ic),
        FindExpr::Path(pat) => glob_match(pat, path, false),
        FindExpr::Type(t) => match *t {
            'f' => meta.is_file(),
            'd' => meta.is_dir(),
            'l' => meta.is_symlink(),
            _ => true,
        },
        FindExpr::Empty => {
            if meta.is_file() { meta.size == 0 }
            else if meta.is_dir() { ctx.fs.readdir(path).map_or(true, |e| e.is_empty()) }
            else { false }
        }
        FindExpr::Size(spec) => match_size(meta.size, spec),
        FindExpr::Mtime(spec) => match_mtime(meta.mtime, spec),
        FindExpr::Newer(_) => globals.newer_mtime.is_none_or(|ref_time| meta.mtime > ref_time),
        FindExpr::Not(inner) => !eval_filter(ctx, path, name, meta, inner, globals),
        FindExpr::And(l, r) => eval_filter(ctx, path, name, meta, l, globals) && eval_filter(ctx, path, name, meta, r, globals),
        FindExpr::Or(l, r) => eval_filter(ctx, path, name, meta, l, globals) || eval_filter(ctx, path, name, meta, r, globals),
    }
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
