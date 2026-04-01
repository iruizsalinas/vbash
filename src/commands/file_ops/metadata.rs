use std::fmt::Write;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;
use crate::fs::FileType;

use super::ls::format_mtime;
use std::collections::HashMap;

pub fn stat_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut format_str: Option<&str> = None;
    let mut file_args = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-c" if i + 1 < args.len() => {
                format_str = Some(args[i + 1]);
                i += 2;
            }
            arg if arg.starts_with("-c") => {
                format_str = Some(&arg[2..]);
                i += 1;
            }
            other if !other.starts_with('-') => {
                file_args.push(other);
                i += 1;
            }
            _ => { i += 1; }
        }
    }

    if file_args.is_empty() {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "stat: missing operand\n".to_string(),
            exit_code: 1,
            env: HashMap::new(),
});
    }

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code = 0;

    for path_arg in &file_args {
        let resolved = crate::fs::path::resolve(ctx.cwd, path_arg);
        let meta = match ctx.fs.lstat(&resolved) {
            Ok(m) => m,
            Err(e) => {
                let _ = writeln!(stderr, "stat: {e}");
                exit_code = 1;
                continue;
            }
        };

        if let Some(fmt) = format_str {
            let formatted = apply_stat_format(fmt, path_arg, &meta);
            let _ = writeln!(stdout, "{formatted}");
        } else {
            let ftype = match meta.file_type {
                FileType::File => "regular file",
                FileType::Directory => "directory",
                FileType::Symlink => "symbolic link",
            };
            let _ = writeln!(stdout, "  File: {path_arg}");
            let _ = writeln!(stdout, "  Size: {}\tType: {ftype}", meta.size);
            let _ = writeln!(stdout, "Access: ({:04o})", meta.mode & 0o7777);
            let _ = writeln!(stdout, "Modify: {}", format_mtime(meta.mtime));
        }
    }

    Ok(ExecResult { stdout, stderr, exit_code, env: HashMap::new() })
}

fn apply_stat_format(fmt: &str, name: &str, meta: &crate::fs::Metadata) -> String {
    let mut result = String::new();
    let bytes = fmt.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 1 < bytes.len() {
            i += 1;
            match bytes[i] {
                b'n' => result.push_str(name),
                b's' => {
                    let _ = write!(result, "{}", meta.size);
                }
                b'a' => {
                    let _ = write!(result, "{:o}", meta.mode & 0o7777);
                }
                b'F' => {
                    let ft = match meta.file_type {
                        FileType::File => "regular file",
                        FileType::Directory => "directory",
                        FileType::Symlink => "symbolic link",
                    };
                    result.push_str(ft);
                }
                other => {
                    result.push('%');
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

pub fn chmod_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut recursive = false;
    let mut mode_str: Option<&str> = None;
    let mut file_args = Vec::new();

    for arg in args {
        match *arg {
            "-R" | "--recursive" => recursive = true,
            _ if mode_str.is_none() && !arg.starts_with('-') => mode_str = Some(arg),
            _ if mode_str.is_some() => file_args.push(*arg),
            _ => {}
        }
    }

    let Some(mode_arg) = mode_str else {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "chmod: missing operand\n".to_string(),
            exit_code: 1,
            env: HashMap::new(),
});
    };

    if file_args.is_empty() {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "chmod: missing operand\n".to_string(),
            exit_code: 1,
            env: HashMap::new(),
});
    }

    let Ok(mode) = u32::from_str_radix(mode_arg, 8) else {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: format!("chmod: invalid mode: '{mode_arg}'\n"),
            exit_code: 1,
            env: HashMap::new(),
});
    };

    let mut stderr = String::new();
    let mut exit_code = 0;

    for path_arg in &file_args {
        let resolved = crate::fs::path::resolve(ctx.cwd, path_arg);
        if let Err(e) = chmod_apply(ctx, &resolved, mode, recursive) {
            let _ = writeln!(stderr, "chmod: {e}");
            exit_code = 1;
        }
    }

    Ok(ExecResult { stdout: String::new(), stderr, exit_code, env: HashMap::new() })
}

fn chmod_apply(
    ctx: &mut CommandContext<'_>,
    path: &str,
    mode: u32,
    recursive: bool,
) -> Result<(), Error> {
    ctx.fs.chmod(path, mode)?;

    if recursive {
        let meta = ctx.fs.stat(path)?;
        if meta.is_dir() {
            let entries = ctx.fs.readdir(path)?;
            for entry in &entries {
                let child = crate::fs::path::join(path, &entry.name);
                chmod_apply(ctx, &child, mode, true)?;
            }
        }
    }

    Ok(())
}
