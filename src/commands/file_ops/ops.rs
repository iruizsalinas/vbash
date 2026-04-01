use std::fmt::Write;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;
use std::collections::HashMap;

pub fn basename_cmd(args: &[&str], _ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    if args.is_empty() {
        return Ok(ExecResult { stdout: String::new(), stderr: "basename: missing operand\n".to_string(), exit_code: 1, env: HashMap::new() });
    }
    let mut name = crate::fs::path::basename(args[0]).to_string();
    if args.len() > 1 {
        if let Some(stripped) = name.strip_suffix(args[1]) {
            name = stripped.to_string();
        }
    }
    Ok(ExecResult { stdout: format!("{name}\n"), stderr: String::new(), exit_code: 0, env: HashMap::new() })
}

pub fn dirname_cmd(args: &[&str], _ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    if args.is_empty() {
        return Ok(ExecResult { stdout: String::new(), stderr: "dirname: missing operand\n".to_string(), exit_code: 1, env: HashMap::new() });
    }
    let dir = crate::fs::path::parent(args[0]);
    Ok(ExecResult { stdout: format!("{dir}\n"), stderr: String::new(), exit_code: 0, env: HashMap::new() })
}

pub fn mkdir_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut parents = false;
    let mut dir_args = Vec::new();
    for arg in args {
        match *arg {
            "-p" | "--parents" => parents = true,
            _ => dir_args.push(*arg),
        }
    }

    let mut stderr = String::new();
    let mut exit_code = 0;
    for dir in &dir_args {
        let path = crate::fs::path::resolve(ctx.cwd, dir);
        if let Err(e) = ctx.fs.mkdir(&path, parents) {
            let _ = writeln!(stderr, "mkdir: {e}");
            exit_code = 1;
        }
    }
    Ok(ExecResult { stdout: String::new(), stderr, exit_code, env: HashMap::new() })
}

pub fn rm_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut recursive = false;
    let mut force = false;
    let mut file_args = Vec::new();
    for arg in args {
        match *arg {
            "-r" | "-R" | "--recursive" | "-rf" | "-fr" => { recursive = true; force = true; }
            "-f" | "--force" => force = true,
            _ => file_args.push(*arg),
        }
    }

    let mut stderr = String::new();
    let mut exit_code = 0;
    for path in &file_args {
        let resolved = crate::fs::path::resolve(ctx.cwd, path);
        if let Err(e) = ctx.fs.rm(&resolved, recursive, force) {
            let _ = writeln!(stderr, "rm: {e}");
            exit_code = 1;
        }
    }
    Ok(ExecResult { stdout: String::new(), stderr, exit_code, env: HashMap::new() })
}

pub fn touch_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut file_args = Vec::new();
    for arg in args {
        if !arg.starts_with('-') {
            file_args.push(*arg);
        }
    }

    let mut stderr = String::new();
    let mut exit_code = 0;
    for path in &file_args {
        let resolved = crate::fs::path::resolve(ctx.cwd, path);
        if let Err(e) = ctx.fs.touch(&resolved) {
            let _ = writeln!(stderr, "touch: {e}");
            exit_code = 1;
        }
    }
    Ok(ExecResult { stdout: String::new(), stderr, exit_code, env: HashMap::new() })
}

pub fn cp_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut recursive = false;
    let mut file_args = Vec::new();
    for arg in args {
        match *arg {
            "-r" | "-R" | "--recursive" => recursive = true,
            _ => file_args.push(*arg),
        }
    }

    if file_args.len() < 2 {
        return Ok(ExecResult { stdout: String::new(), stderr: "cp: missing operand\n".to_string(), exit_code: 1, env: HashMap::new() });
    }

    let Some(last) = file_args.last() else {
        return Ok(ExecResult { stdout: String::new(), stderr: "cp: missing operand\n".to_string(), exit_code: 1, env: HashMap::new() });
    };
    let dest = crate::fs::path::resolve(ctx.cwd, last);
    for src in &file_args[..file_args.len() - 1] {
        let src_resolved = crate::fs::path::resolve(ctx.cwd, src);
        if let Err(e) = ctx.fs.cp(&src_resolved, &dest, recursive) {
            return Ok(ExecResult { stdout: String::new(), stderr: format!("cp: {e}\n"), exit_code: 1, env: HashMap::new() });
        }
    }
    Ok(ExecResult { stdout: String::new(), stderr: String::new(), exit_code: 0, env: HashMap::new() })
}

pub fn mv_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut file_args: Vec<&str> = args.iter().filter(|a| !a.starts_with('-')).copied().collect();
    if file_args.len() < 2 {
        return Ok(ExecResult { stdout: String::new(), stderr: "mv: missing operand\n".to_string(), exit_code: 1, env: HashMap::new() });
    }
    let Some(last) = file_args.pop() else {
        return Ok(ExecResult { stdout: String::new(), stderr: "mv: missing operand\n".to_string(), exit_code: 1, env: HashMap::new() });
    };
    let dest = crate::fs::path::resolve(ctx.cwd, last);
    for src in &file_args {
        let src_resolved = crate::fs::path::resolve(ctx.cwd, src);
        if let Err(e) = ctx.fs.mv(&src_resolved, &dest) {
            return Ok(ExecResult { stdout: String::new(), stderr: format!("mv: {e}\n"), exit_code: 1, env: HashMap::new() });
        }
    }
    Ok(ExecResult { stdout: String::new(), stderr: String::new(), exit_code: 0, env: HashMap::new() })
}
