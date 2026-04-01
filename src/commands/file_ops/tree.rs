use std::fmt::Write;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;
use crate::fs::FileType;
use std::collections::HashMap;

pub fn tree_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut max_depth: Option<usize> = None;
    let mut dirs_only = false;
    let mut show_all = false;
    let mut paths = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-L" if i + 1 < args.len() => {
                max_depth = args[i + 1].parse().ok();
                i += 2;
            }
            "-d" => { dirs_only = true; i += 1; }
            "-a" => { show_all = true; i += 1; }
            arg if !arg.starts_with('-') => { paths.push(arg); i += 1; }
            _ => { i += 1; }
        }
    }

    if paths.is_empty() {
        paths.push(".");
    }

    let mut stdout = String::new();
    let mut dir_count = 0u64;
    let mut file_count = 0u64;

    for path_arg in &paths {
        let resolved = crate::fs::path::resolve(ctx.cwd, path_arg);
        let _ = writeln!(stdout, "{path_arg}");
        tree_walk(
            ctx, &resolved, "", max_depth, 0, dirs_only, show_all,
            &mut stdout, &mut dir_count, &mut file_count,
        );
    }

    if dirs_only {
        let _ = writeln!(stdout, "\n{dir_count} directories");
    } else {
        let _ = writeln!(stdout, "\n{dir_count} directories, {file_count} files");
    }

    Ok(ExecResult { stdout, stderr: String::new(), exit_code: 0, env: HashMap::new() })
}

#[allow(clippy::too_many_arguments)]
fn tree_walk(
    ctx: &mut CommandContext<'_>,
    path: &str,
    prefix: &str,
    max_depth: Option<usize>,
    depth: usize,
    dirs_only: bool,
    show_all: bool,
    out: &mut String,
    dir_count: &mut u64,
    file_count: &mut u64,
) {
    if let Some(max) = max_depth {
        if depth >= max {
            return;
        }
    }

    let Ok(entries) = ctx.fs.readdir(path) else { return };

    let mut filtered: Vec<&crate::fs::DirEntry> = entries.iter()
        .filter(|e| show_all || !e.name.starts_with('.'))
        .filter(|e| !dirs_only || e.file_type == FileType::Directory)
        .collect();

    filtered.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    let count = filtered.len();
    for (idx, entry) in filtered.iter().enumerate() {
        let is_last = idx + 1 == count;
        let connector = if is_last { "\u{2514}\u{2500}\u{2500}" } else { "\u{251c}\u{2500}\u{2500}" };
        let child_prefix = if is_last {
            format!("{prefix}    ")
        } else {
            format!("{prefix}\u{2502}   ")
        };

        let _ = writeln!(out, "{prefix}{connector} {}", entry.name);

        if entry.file_type == FileType::Directory {
            *dir_count += 1;
            let child_path = crate::fs::path::join(path, &entry.name);
            tree_walk(
                ctx, &child_path, &child_prefix, max_depth, depth + 1,
                dirs_only, show_all, out, dir_count, file_count,
            );
        } else {
            *file_count += 1;
        }
    }
}

pub fn file_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let file_args: Vec<&str> = args.iter().filter(|a| !a.starts_with('-')).copied().collect();

    if file_args.is_empty() {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "file: missing operand\n".to_string(),
            exit_code: 1,
            env: HashMap::new(),
});
    }

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code = 0;

    for path_arg in &file_args {
        let resolved = crate::fs::path::resolve(ctx.cwd, path_arg);

        let lmeta = match ctx.fs.lstat(&resolved) {
            Ok(m) => m,
            Err(e) => {
                let _ = writeln!(stderr, "file: {e}");
                exit_code = 1;
                continue;
            }
        };

        if lmeta.is_symlink() {
            let target = ctx.fs.readlink(&resolved).unwrap_or_default();
            let _ = writeln!(stdout, "{path_arg}: symbolic link to {target}");
            continue;
        }

        if lmeta.is_dir() {
            let _ = writeln!(stdout, "{path_arg}: directory");
            continue;
        }

        let data = match ctx.fs.read_file(&resolved) {
            Ok(d) => d,
            Err(e) => {
                let _ = writeln!(stderr, "file: {e}");
                exit_code = 1;
                continue;
            }
        };

        let description = detect_file_type(&data, path_arg);
        let _ = writeln!(stdout, "{path_arg}: {description}");
    }

    Ok(ExecResult { stdout, stderr, exit_code, env: HashMap::new() })
}

fn detect_file_type(data: &[u8], name: &str) -> &'static str {
    if data.is_empty() {
        return "empty";
    }

    if data.starts_with(b"#!") {
        return "shell script";
    }

    if let Ok(text) = std::str::from_utf8(data) {
        let trimmed = text.trim();
        if (trimmed.starts_with('{') && trimmed.ends_with('}'))
            || (trimmed.starts_with('[') && trimmed.ends_with(']'))
        {
            return "JSON text";
        }
        if text.is_ascii() {
            return "ASCII text";
        }
        return "UTF-8 Unicode text";
    }

    let ext = name.rsplit('.').next().unwrap_or("");
    match ext {
        "png" => "PNG image data",
        "jpg" | "jpeg" => "JPEG image data",
        "gif" => "GIF image data",
        "pdf" => "PDF document",
        "gz" | "tgz" => "gzip compressed data",
        "zip" => "Zip archive data",
        _ => "binary data",
    }
}

pub fn split_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut line_count: Option<usize> = None;
    let mut byte_count: Option<usize> = None;
    let mut positional = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-l" if i + 1 < args.len() => {
                line_count = args[i + 1].parse().ok();
                i += 2;
            }
            "-b" if i + 1 < args.len() => {
                byte_count = parse_size_spec(args[i + 1]);
                i += 2;
            }
            arg if !arg.starts_with('-') => {
                positional.push(arg);
                i += 1;
            }
            _ => { i += 1; }
        }
    }

    if positional.is_empty() {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "split: missing operand\n".to_string(),
            exit_code: 1,
            env: HashMap::new(),
});
    }

    let input_path = crate::fs::path::resolve(ctx.cwd, positional[0]);
    let prefix = if positional.len() > 1 { positional[1] } else { "x" };

    let data = ctx.fs.read_file(&input_path)?;

    let mut stderr = String::new();
    let mut exit_code = 0;

    if let Some(bytes) = byte_count {
        if bytes == 0 {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: "split: invalid number of bytes: '0'\n".to_string(),
                exit_code: 1,
                env: HashMap::new(),
});
        }
        let mut part = 0usize;
        let mut offset = 0;
        while offset < data.len() {
            let end = (offset + bytes).min(data.len());
            let chunk = &data[offset..end];
            let suffix = split_suffix(part);
            let out_path = crate::fs::path::resolve(ctx.cwd, &format!("{prefix}{suffix}"));
            if let Err(e) = ctx.fs.write_file(&out_path, chunk) {
                let _ = writeln!(stderr, "split: {e}");
                exit_code = 1;
                break;
            }
            part += 1;
            offset = end;
        }
    } else {
        let lines = line_count.unwrap_or(1000);
        if lines == 0 {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: "split: invalid number of lines: '0'\n".to_string(),
                exit_code: 1,
                env: HashMap::new(),
});
        }
        let content = String::from_utf8_lossy(&data);
        let all_lines: Vec<&str> = content.lines().collect();
        let mut part = 0usize;
        let mut offset = 0;
        while offset < all_lines.len() {
            let end = (offset + lines).min(all_lines.len());
            let mut chunk = String::new();
            for line in &all_lines[offset..end] {
                let _ = writeln!(chunk, "{line}");
            }
            let suffix = split_suffix(part);
            let out_path = crate::fs::path::resolve(ctx.cwd, &format!("{prefix}{suffix}"));
            if let Err(e) = ctx.fs.write_file(&out_path, chunk.as_bytes()) {
                let _ = writeln!(stderr, "split: {e}");
                exit_code = 1;
                break;
            }
            part += 1;
            offset = end;
        }
    }

    Ok(ExecResult { stdout: String::new(), stderr, exit_code, env: HashMap::new() })
}

#[allow(clippy::cast_possible_truncation)]
fn split_suffix(n: usize) -> String {
    let first = b'a' + (n / 26) as u8;
    let second = b'a' + (n % 26) as u8;
    let mut s = String::with_capacity(2);
    s.push(first as char);
    s.push(second as char);
    s
}

fn parse_size_spec(spec: &str) -> Option<usize> {
    let spec = spec.trim();
    if spec.is_empty() {
        return None;
    }
    let (num_part, multiplier) = if spec.ends_with('k') || spec.ends_with('K') {
        (&spec[..spec.len() - 1], 1024usize)
    } else if spec.ends_with('m') || spec.ends_with('M') {
        (&spec[..spec.len() - 1], 1024 * 1024)
    } else if spec.ends_with('g') || spec.ends_with('G') {
        (&spec[..spec.len() - 1], 1024 * 1024 * 1024)
    } else {
        (spec, 1)
    };
    num_part.parse::<usize>().ok().map(|n| n * multiplier)
}
