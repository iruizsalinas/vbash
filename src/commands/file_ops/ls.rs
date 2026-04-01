//! File operation commands: ls and helpers.

use std::fmt::Write;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;
use crate::fs::FileType;
use std::collections::HashMap;

struct EntryInfo {
    name: String,
    file_type: FileType,
    size: u64,
    mode: u32,
    mtime: std::time::SystemTime,
}

pub fn ls(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut show_all = false;
    let mut long = false;
    let mut one_per_line = false;
    let mut recursive = false;
    let mut reverse = false;
    let mut sort_size = false;
    let mut sort_time = false;
    let mut human_readable = false;
    let mut dir_only = false;
    let mut classify = false;
    let mut paths = Vec::new();

    for arg in args {
        match *arg {
            "-a" | "--all" => show_all = true,
            "-l" => long = true,
            "-1" => one_per_line = true,
            "-R" | "--recursive" => recursive = true,
            "-r" | "--reverse" => reverse = true,
            "-S" => sort_size = true,
            "-t" => sort_time = true,
            "-h" | "--human-readable" => human_readable = true,
            "-d" | "--directory" => dir_only = true,
            "-F" | "--classify" => classify = true,
            "-la" | "-al" => { long = true; show_all = true; }
            "-lh" | "-hl" => { long = true; human_readable = true; }
            "-lah" | "-alh" | "-hal" | "-hla" | "-ahl" | "-lha" => {
                long = true; show_all = true; human_readable = true;
            }
            other if !other.starts_with('-') => paths.push(*arg),
            _ => {}
        }
    }

    if paths.is_empty() {
        paths.push(".");
    }

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code = 0;
    let multi = paths.len() > 1 || recursive;

    for (idx, path_arg) in paths.iter().enumerate() {
        let resolved = crate::fs::path::resolve(ctx.cwd, path_arg);

        if dir_only {
            let meta = match ctx.fs.stat(&resolved) {
                Ok(m) => m,
                Err(e) => {
                    let _ = writeln!(stderr, "ls: {e}");
                    exit_code = 1;
                    continue;
                }
            };
            let name = crate::fs::path::basename(&resolved);
            if long {
                format_long_entry(&mut stdout, name, &meta, human_readable, classify);
            } else {
                let suffix = classify_suffix(classify, meta.file_type);
                let _ = writeln!(stdout, "{name}{suffix}");
            }
            continue;
        }

        if let Err(e) = ls_path(
            ctx, &resolved, path_arg, multi, show_all, long, one_per_line,
            recursive, reverse, sort_size, sort_time, human_readable,
            classify, idx > 0, &mut stdout, &mut stderr,
        ) {
            let _ = writeln!(stderr, "ls: {e}");
            exit_code = 1;
        }
    }

    Ok(ExecResult { stdout, stderr, exit_code, env: HashMap::new() })
}

#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools, clippy::only_used_in_recursion)]
fn ls_path(
    ctx: &mut CommandContext<'_>,
    resolved: &str,
    display_path: &str,
    multi: bool,
    show_all: bool,
    long: bool,
    one_per_line: bool,
    recursive: bool,
    reverse: bool,
    sort_size: bool,
    sort_time: bool,
    human_readable: bool,
    classify: bool,
    needs_blank: bool,
    stdout: &mut String,
    stderr: &mut String,
) -> Result<(), Error> {
    let meta = ctx.fs.stat(resolved)?;

    if meta.is_file() || meta.is_symlink() {
        let name = crate::fs::path::basename(resolved);
        if long {
            format_long_entry(stdout, name, &meta, human_readable, classify);
        } else {
            let suffix = classify_suffix(classify, meta.file_type);
            let _ = writeln!(stdout, "{name}{suffix}");
        }
        return Ok(());
    }

    let entries = ctx.fs.readdir(resolved)?;

    let mut items: Vec<EntryInfo> = Vec::new();
    for entry in &entries {
        if !show_all && entry.name.starts_with('.') {
            continue;
        }
        let child_path = crate::fs::path::join(resolved, &entry.name);
        let child_meta = ctx.fs.lstat(&child_path).unwrap_or(crate::fs::Metadata {
            file_type: entry.file_type,
            size: 0,
            mode: 0o644,
            mtime: std::time::SystemTime::UNIX_EPOCH,
        });
        items.push(EntryInfo {
            name: entry.name.clone(),
            file_type: child_meta.file_type,
            size: child_meta.size,
            mode: child_meta.mode,
            mtime: child_meta.mtime,
        });
    }

    if sort_size {
        items.sort_by(|a, b| b.size.cmp(&a.size));
    } else if sort_time {
        items.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    } else {
        items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    }

    if reverse {
        items.reverse();
    }

    if needs_blank {
        stdout.push('\n');
    }

    if multi {
        let _ = writeln!(stdout, "{display_path}:");
    }

    if long {
        for item in &items {
            let child_path = crate::fs::path::join(resolved, &item.name);
            let child_meta = ctx.fs.lstat(&child_path).unwrap_or(crate::fs::Metadata {
                file_type: item.file_type,
                size: item.size,
                mode: item.mode,
                mtime: item.mtime,
            });
            format_long_entry(stdout, &item.name, &child_meta, human_readable, classify);
        }
    } else {
        let names: Vec<String> = items.iter().map(|i| {
            let suffix = classify_suffix(classify, i.file_type);
            format!("{}{suffix}", i.name)
        }).collect();

        if one_per_line || names.len() > 4 {
            for name in &names {
                let _ = writeln!(stdout, "{name}");
            }
        } else {
            let _ = writeln!(stdout, "{}", names.join("  "));
        }
    }

    if recursive {
        let subdirs: Vec<String> = items.iter()
            .filter(|i| i.file_type == FileType::Directory)
            .map(|i| i.name.clone())
            .collect();

        for subdir in subdirs {
            let sub_resolved = crate::fs::path::join(resolved, &subdir);
            let sub_display = if display_path == "." {
                format!("./{subdir}")
            } else {
                format!("{display_path}/{subdir}")
            };
            let _ = ls_path(
                ctx, &sub_resolved, &sub_display, true,
                show_all, long, one_per_line, recursive,
                reverse, sort_size, sort_time, human_readable,
                classify, true, stdout, stderr,
            );
        }
    }

    Ok(())
}

fn format_long_entry(
    out: &mut String,
    name: &str,
    meta: &crate::fs::Metadata,
    human_readable: bool,
    classify: bool,
) {
    let mode = format_mode_symbolic(meta.mode, meta.file_type);
    let nlinks = 1u64;
    let owner = "user";
    let group = "group";
    let size_str = if human_readable {
        format_human_size(meta.size)
    } else {
        meta.size.to_string()
    };
    let date = format_mtime(meta.mtime);
    let suffix = classify_suffix(classify, meta.file_type);
    let _ = writeln!(out, "{mode} {nlinks:>3} {owner} {group} {size_str:>8} {date} {name}{suffix}");
}

fn format_mode_symbolic(mode: u32, file_type: FileType) -> String {
    let type_char = match file_type {
        FileType::Directory => 'd',
        FileType::Symlink => 'l',
        FileType::File => '-',
    };

    let mut perms = String::with_capacity(10);
    perms.push(type_char);

    for shift in [6, 3, 0] {
        let bits = (mode >> shift) & 0o7;
        perms.push(if bits & 4 != 0 { 'r' } else { '-' });
        perms.push(if bits & 2 != 0 { 'w' } else { '-' });
        perms.push(if bits & 1 != 0 { 'x' } else { '-' });
    }

    perms
}

#[allow(clippy::cast_precision_loss)]
fn format_human_size(size: u64) -> String {
    if size < 1024 {
        return format!("{size}");
    }
    let units = ["K", "M", "G", "T"];
    let mut val = size as f64 / 1024.0;
    for unit in &units {
        if val < 1024.0 {
            return if val < 10.0 {
                format!("{val:.1}{unit}")
            } else {
                format!("{val:.0}{unit}")
            };
        }
        val /= 1024.0;
    }
    format!("{val:.0}P")
}

pub(crate) fn format_mtime(time: std::time::SystemTime) -> String {
    let duration = time
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let (year, month, day, hour, min) = epoch_to_datetime(secs);
    format!("{year}-{month:02}-{day:02} {hour:02}:{min:02}")
}

fn epoch_to_datetime(secs: u64) -> (u64, u64, u64, u64, u64) {
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hour = time_of_day / 3600;
    let min = (time_of_day % 3600) / 60;

    let mut y = 1970u64;
    let mut remaining_days = days;

    loop {
        let days_in_year = if is_leap_year(y) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        y += 1;
    }

    let month_days: [u64; 12] = if is_leap_year(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut m = 0u64;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining_days < md {
            m = i as u64 + 1;
            break;
        }
        remaining_days -= md;
    }
    if m == 0 {
        m = 12;
    }

    let d = remaining_days + 1;
    (y, m, d, hour, min)
}

fn is_leap_year(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn classify_suffix(classify: bool, ft: FileType) -> &'static str {
    if !classify {
        return "";
    }
    match ft {
        FileType::Directory => "/",
        FileType::Symlink => "@",
        FileType::File => "",
    }
}
