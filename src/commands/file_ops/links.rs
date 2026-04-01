use std::fmt::Write;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;
use std::collections::HashMap;

pub fn ln_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut symbolic = false;
    let mut force = false;
    let mut positional = Vec::new();

    for arg in args {
        match *arg {
            "-s" | "--symbolic" => symbolic = true,
            "-f" | "--force" => force = true,
            "-sf" | "-fs" => { symbolic = true; force = true; }
            other if !other.starts_with('-') => positional.push(*arg),
            _ => {}
        }
    }

    if positional.len() < 2 {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "ln: missing operand\n".to_string(),
            exit_code: 1,
            env: HashMap::new(),
});
    }

    let target = positional[0];
    let link_path = crate::fs::path::resolve(ctx.cwd, positional[1]);

    if force && ctx.fs.exists(&link_path) {
        ctx.fs.rm(&link_path, false, true)?;
    }

    if symbolic {
        ctx.fs.symlink(target, &link_path)?;
    } else {
        let target_resolved = crate::fs::path::resolve(ctx.cwd, target);
        ctx.fs.hard_link(&target_resolved, &link_path)?;
    }

    Ok(ExecResult { stdout: String::new(), stderr: String::new(), exit_code: 0, env: HashMap::new() })
}

pub fn readlink_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut canonicalize = false;
    let mut file_args = Vec::new();

    for arg in args {
        match *arg {
            "-f" => canonicalize = true,
            other if !other.starts_with('-') => file_args.push(other),
            _ => {}
        }
    }

    if file_args.is_empty() {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "readlink: missing operand\n".to_string(),
            exit_code: 1,
            env: HashMap::new(),
});
    }

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code = 0;

    for path_arg in &file_args {
        let resolved = crate::fs::path::resolve(ctx.cwd, path_arg);
        if canonicalize {
            match ctx.fs.realpath(&resolved) {
                Ok(real) => { let _ = writeln!(stdout, "{real}"); }
                Err(e) => {
                    let _ = writeln!(stderr, "readlink: {e}");
                    exit_code = 1;
                }
            }
        } else {
            match ctx.fs.readlink(&resolved) {
                Ok(target) => { let _ = writeln!(stdout, "{target}"); }
                Err(e) => {
                    let _ = writeln!(stderr, "readlink: {e}");
                    exit_code = 1;
                }
            }
        }
    }

    Ok(ExecResult { stdout, stderr, exit_code, env: HashMap::new() })
}

pub fn rmdir_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut parents = false;
    let mut dir_args = Vec::new();

    for arg in args {
        match *arg {
            "-p" | "--parents" => parents = true,
            other if !other.starts_with('-') => dir_args.push(other),
            _ => {}
        }
    }

    if dir_args.is_empty() {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "rmdir: missing operand\n".to_string(),
            exit_code: 1,
            env: HashMap::new(),
});
    }

    let mut stderr = String::new();
    let mut exit_code = 0;

    for path_arg in &dir_args {
        let resolved = crate::fs::path::resolve(ctx.cwd, path_arg);
        if let Err(e) = rmdir_one(ctx, &resolved, parents) {
            let _ = writeln!(stderr, "rmdir: {e}");
            exit_code = 1;
        }
    }

    Ok(ExecResult { stdout: String::new(), stderr, exit_code, env: HashMap::new() })
}

fn rmdir_one(
    ctx: &mut CommandContext<'_>,
    path: &str,
    parents: bool,
) -> Result<(), Error> {
    ctx.fs.rm(path, false, false)?;

    if parents {
        let mut current = crate::fs::path::parent(path).to_string();
        while current != "/" {
            if ctx.fs.rm(&current, false, false).is_err() {
                break;
            }
            let parent = crate::fs::path::parent(&current).to_string();
            current = parent;
        }
    }

    Ok(())
}
