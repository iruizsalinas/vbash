use std::fmt::Write;
use std::collections::HashMap;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::{ExecError, Error};

pub fn grep_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut invert = false;
    let mut count_only = false;
    let mut ignore_case = false;
    let mut line_numbers = false;
    let mut recursive = false;
    let mut files_with_matches = false;
    let mut files_without_match = false;
    let mut only_matching = false;
    let mut word_regexp = false;
    let mut line_regexp = false;
    let mut quiet = false;
    let mut max_count: Option<u64> = None;
    let mut no_filename = false;
    let mut with_filename = false;
    let mut after_context: usize = 0;
    let mut before_context: usize = 0;
    let mut fixed_strings = false;
    let mut include_globs: Vec<String> = Vec::new();
    let mut exclude_globs: Vec<String> = Vec::new();
    let mut pattern_str = None;
    let mut file_args = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-v" | "--invert-match" => invert = true,
            "-c" | "--count" => count_only = true,
            "-i" | "--ignore-case" => ignore_case = true,
            "-n" | "--line-number" => line_numbers = true,
            "-r" | "-R" | "--recursive" => recursive = true,
            "-l" | "--files-with-matches" => files_with_matches = true,
            "-L" | "--files-without-match" => files_without_match = true,
            "-o" | "--only-matching" => only_matching = true,
            "-w" | "--word-regexp" => word_regexp = true,
            "-x" | "--line-regexp" => line_regexp = true,
            "-q" | "--quiet" | "--silent" => quiet = true,
            "-h" | "--no-filename" => no_filename = true,
            "-H" | "--with-filename" => with_filename = true,
            "-F" | "--fixed-strings" => fixed_strings = true,
            "-e" if i + 1 < args.len() => {
                pattern_str = Some(args[i + 1]);
                i += 1;
            }
            "-m" | "--max-count" if i + 1 < args.len() => {
                max_count = args[i + 1].parse().ok();
                i += 1;
            }
            "-A" | "--after-context" if i + 1 < args.len() => {
                after_context = args[i + 1].parse().unwrap_or(0);
                i += 1;
            }
            "-B" | "--before-context" if i + 1 < args.len() => {
                before_context = args[i + 1].parse().unwrap_or(0);
                i += 1;
            }
            "-C" | "--context" if i + 1 < args.len() => {
                let n = args[i + 1].parse().unwrap_or(0);
                before_context = n;
                after_context = n;
                i += 1;
            }
            arg if arg.starts_with("--max-count=") => {
                max_count = arg.strip_prefix("--max-count=").and_then(|v| v.parse().ok());
            }
            arg if arg.starts_with("--include=") => {
                if let Some(g) = arg.strip_prefix("--include=") {
                    include_globs.push(g.to_string());
                }
            }
            arg if arg.starts_with("--exclude=") => {
                if let Some(g) = arg.strip_prefix("--exclude=") {
                    exclude_globs.push(g.to_string());
                }
            }
            arg if arg.starts_with("-m") => {
                max_count = arg[2..].parse().ok();
            }
            arg if arg.starts_with("-A") => {
                after_context = arg[2..].parse().unwrap_or(0);
            }
            arg if arg.starts_with("-B") => {
                before_context = arg[2..].parse().unwrap_or(0);
            }
            arg if arg.starts_with("-C") => {
                let n: usize = arg[2..].parse().unwrap_or(0);
                before_context = n;
                after_context = n;
            }
            "-E" | "--extended-regexp" | "-G" | "--basic-regexp" | "-P" | "--perl-regexp" => {
                // Rust regex is always extended; these are effectively no-ops
            }
            arg if arg.starts_with('-') && !arg.starts_with("--") && arg.len() > 2
                && !arg.starts_with("-e") && !arg.starts_with("-m")
                && !arg.starts_with("-A") && !arg.starts_with("-B") && !arg.starts_with("-C") => {
                for c in arg[1..].chars() {
                    match c {
                        'v' => invert = true,
                        'c' => count_only = true,
                        'i' => ignore_case = true,
                        'n' => line_numbers = true,
                        'r' | 'R' => recursive = true,
                        'l' => files_with_matches = true,
                        'L' => files_without_match = true,
                        'o' => only_matching = true,
                        'w' => word_regexp = true,
                        'x' => line_regexp = true,
                        'q' => quiet = true,
                        'h' => no_filename = true,
                        'H' => with_filename = true,
                        'F' => fixed_strings = true,
                        'E' | 'G' | 'P' => {} // regex mode flags (no-op)
                        _ => {}
                    }
                }
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
            stderr: "grep: no pattern\n".to_string(),
            exit_code: 2,
            env: HashMap::new(),
        });
    };

    let matcher = build_grep_matcher(pattern, ignore_case, word_regexp, line_regexp, fixed_strings)?;

    let multi_file = file_args.len() > 1 || recursive;
    let show_filename = !no_filename && (with_filename || multi_file);

    let mut stdout = String::new();
    let mut total_match_count = 0u64;

    if file_args.is_empty() && !recursive {
        let content = ctx.stdin.to_string();
        total_match_count += grep_content(
            &content, &matcher, invert, count_only, line_numbers, only_matching,
            quiet, max_count, before_context, after_context,
            files_with_matches, files_without_match, show_filename, None,
            &mut stdout,
        );
    } else if recursive {
        let search_paths: Vec<&str> = if file_args.is_empty() { vec!["."] } else { file_args };
        for start in search_paths {
            let resolved = crate::fs::path::resolve(ctx.cwd, start);
            grep_walk_recursive(
                ctx, &resolved, &matcher, invert, count_only, line_numbers, only_matching,
                quiet, max_count, before_context, after_context,
                files_with_matches, files_without_match, show_filename,
                &include_globs, &exclude_globs, &mut stdout, &mut total_match_count,
            );
        }
    } else {
        for path_arg in &file_args {
            let resolved = crate::fs::path::resolve(ctx.cwd, path_arg);
            let Ok(content) = ctx.fs.read_file_string(&resolved) else { continue };
            let label = if show_filename { Some(*path_arg) } else { None };
            total_match_count += grep_content(
                &content, &matcher, invert, count_only, line_numbers, only_matching,
                quiet, max_count, before_context, after_context,
                files_with_matches, files_without_match, show_filename, label,
                &mut stdout,
            );
        }
    }

    let exit_code = i32::from(total_match_count == 0);
    Ok(ExecResult { stdout, stderr: String::new(), exit_code, env: HashMap::new() })
}

enum GrepMatcher {
    Regex(regex::Regex),
    Fixed { pattern: String, ignore_case: bool },
}

impl GrepMatcher {
    fn is_match(&self, line: &str) -> bool {
        match self {
            GrepMatcher::Regex(re) => re.is_match(line),
            GrepMatcher::Fixed { pattern, ignore_case } => {
                if *ignore_case {
                    line.to_lowercase().contains(&pattern.to_lowercase())
                } else {
                    line.contains(pattern.as_str())
                }
            }
        }
    }

    fn find_matches<'a>(&self, line: &'a str) -> Vec<&'a str> {
        match self {
            GrepMatcher::Regex(re) => re.find_iter(line).map(|m| m.as_str()).collect(),
            GrepMatcher::Fixed { pattern, ignore_case } => {
                let mut results = Vec::new();
                let haystack = if *ignore_case { line.to_lowercase() } else { line.to_string() };
                let needle = if *ignore_case { pattern.to_lowercase() } else { pattern.clone() };
                let mut start = 0;
                while let Some(pos) = haystack[start..].find(&needle) {
                    let abs = start + pos;
                    results.push(&line[abs..abs + pattern.len()]);
                    start = abs + 1;
                }
                results
            }
        }
    }
}

#[allow(clippy::fn_params_excessive_bools)]
fn build_grep_matcher(
    pattern: &str,
    ignore_case: bool,
    word_regexp: bool,
    line_regexp: bool,
    fixed_strings: bool,
) -> Result<GrepMatcher, Error> {
    if fixed_strings && !word_regexp && !line_regexp {
        return Ok(GrepMatcher::Fixed {
            pattern: pattern.to_string(),
            ignore_case,
        });
    }

    let escaped = if fixed_strings {
        regex::escape(pattern)
    } else {
        pattern.to_string()
    };

    let wrapped = if line_regexp {
        format!("^(?:{escaped})$")
    } else if word_regexp {
        format!("\\b(?:{escaped})\\b")
    } else {
        escaped
    };

    let full = if ignore_case {
        format!("(?i){wrapped}")
    } else {
        wrapped
    };

    let re = regex::Regex::new(&full).map_err(|e| {
        Error::Exec(ExecError::Other(format!("grep: invalid regex: {e}")))
    })?;
    Ok(GrepMatcher::Regex(re))
}

#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn grep_content(
    content: &str,
    matcher: &GrepMatcher,
    invert: bool,
    count_only: bool,
    line_numbers: bool,
    only_matching: bool,
    quiet: bool,
    max_count: Option<u64>,
    before_context: usize,
    after_context: usize,
    files_with_matches: bool,
    files_without_match: bool,
    show_filename: bool,
    filename: Option<&str>,
    stdout: &mut String,
) -> u64 {
    let lines: Vec<&str> = content.lines().collect();
    let mut match_count = 0u64;
    let mut printed: Vec<bool> = vec![false; lines.len()];
    let has_context = before_context > 0 || after_context > 0;

    if files_with_matches || files_without_match {
        let any_match = lines.iter().any(|line| {
            let m = matcher.is_match(line);
            if invert { !m } else { m }
        });
        if files_with_matches && any_match {
            if let Some(name) = filename {
                let _ = writeln!(stdout, "{name}");
            }
            return 1;
        }
        if files_without_match && !any_match {
            if let Some(name) = filename {
                let _ = writeln!(stdout, "{name}");
            }
            return 1;
        }
        return 0;
    }

    if quiet {
        for line in &lines {
            let m = matcher.is_match(line);
            let hit = if invert { !m } else { m };
            if hit {
                match_count += 1;
                if let Some(mc) = max_count {
                    if match_count >= mc {
                        break;
                    }
                }
            }
        }
        return match_count;
    }

    let mut match_indices: Vec<usize> = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        let m = matcher.is_match(line);
        let hit = if invert { !m } else { m };
        if hit {
            match_count += 1;
            match_indices.push(idx);
            if let Some(mc) = max_count {
                if match_count >= mc {
                    break;
                }
            }
        }
    }

    if count_only {
        if let Some(name) = filename.filter(|_| show_filename) {
            let _ = writeln!(stdout, "{name}:{match_count}");
        } else {
            let _ = writeln!(stdout, "{match_count}");
        }
        return match_count;
    }

    if has_context {
        let mut visible = vec![false; lines.len()];
        let mut context_line = vec![false; lines.len()];
        for &idx in &match_indices {
            visible[idx] = true;
            let bstart = idx.saturating_sub(before_context);
            for vi in bstart..idx {
                visible[vi] = true;
                context_line[vi] = true;
            }
            let aend = (idx + after_context + 1).min(lines.len());
            for vi in (idx + 1)..aend {
                visible[vi] = true;
                context_line[vi] = true;
            }
        }

        let mut last_printed: Option<usize> = None;
        for (idx, line) in lines.iter().enumerate() {
            if !visible[idx] {
                continue;
            }
            if let Some(lp) = last_printed {
                if idx > lp + 1 {
                    let _ = writeln!(stdout, "--");
                }
            }
            let sep = if context_line[idx] && !match_indices.contains(&idx) { '-' } else { ':' };
            format_grep_line(stdout, filename, show_filename, line_numbers, only_matching, idx, line, sep, matcher, invert);
            last_printed = Some(idx);
            printed[idx] = true;
        }
    } else {
        for &idx in &match_indices {
            if !printed[idx] {
                format_grep_line(stdout, filename, show_filename, line_numbers, only_matching, idx, lines[idx], ':', matcher, invert);
                printed[idx] = true;
            }
        }
    }

    match_count
}

#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn format_grep_line(
    stdout: &mut String,
    filename: Option<&str>,
    show_filename: bool,
    line_numbers: bool,
    only_matching: bool,
    line_idx: usize,
    line: &str,
    sep: char,
    matcher: &GrepMatcher,
    invert: bool,
) {
    let prefix = match (filename.filter(|_| show_filename), line_numbers) {
        (Some(name), true) => format!("{name}{sep}{}{sep}", line_idx + 1),
        (Some(name), false) => format!("{name}{sep}"),
        (None, true) => format!("{}{sep}", line_idx + 1),
        (None, false) => String::new(),
    };

    if only_matching && !invert {
        for m in matcher.find_matches(line) {
            let _ = writeln!(stdout, "{prefix}{m}");
        }
    } else {
        let _ = writeln!(stdout, "{prefix}{line}");
    }
}

fn grep_filename_matches_glob(name: &str, patterns: &[String]) -> bool {
    for pat in patterns {
        if crate::commands::search::glob_match_simple(pat, name) {
            return true;
        }
    }
    false
}

#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn grep_walk_recursive(
    ctx: &CommandContext<'_>,
    path: &str,
    matcher: &GrepMatcher,
    invert: bool,
    count_only: bool,
    line_numbers: bool,
    only_matching: bool,
    quiet: bool,
    max_count: Option<u64>,
    before_context: usize,
    after_context: usize,
    files_with_matches: bool,
    files_without_match: bool,
    show_filename: bool,
    include_globs: &[String],
    exclude_globs: &[String],
    stdout: &mut String,
    total: &mut u64,
) {
    let Ok(meta) = ctx.fs.lstat(path) else { return };

    if meta.is_file() {
        let name = crate::fs::path::basename(path);
        if !include_globs.is_empty() && !grep_filename_matches_glob(name, include_globs) {
            return;
        }
        if !exclude_globs.is_empty() && grep_filename_matches_glob(name, exclude_globs) {
            return;
        }
        let Ok(content) = ctx.fs.read_file_string(path) else { return };
        let label = if show_filename { Some(path) } else { None };
        *total += grep_content(
            &content, matcher, invert, count_only, line_numbers, only_matching,
            quiet, max_count, before_context, after_context,
            files_with_matches, files_without_match, show_filename, label,
            stdout,
        );
    } else if meta.is_dir() {
        if let Ok(entries) = ctx.fs.readdir(path) {
            let mut entries = entries;
            entries.sort_by(|a, b| a.name.cmp(&b.name));
            for entry in entries {
                let child = if path == "/" {
                    format!("/{}", entry.name)
                } else {
                    format!("{path}/{}", entry.name)
                };
                grep_walk_recursive(
                    ctx, &child, matcher, invert, count_only, line_numbers, only_matching,
                    quiet, max_count, before_context, after_context,
                    files_with_matches, files_without_match, show_filename,
                    include_globs, exclude_globs, stdout, total,
                );
            }
        }
    }
}
