use std::fmt::Write;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;
use crate::fs::VirtualFs;
use std::collections::HashMap;

pub fn yes_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let text = if args.is_empty() { "y" } else { args[0] };
    let line_len = text.len() + 1;
    let max = ctx.limits.max_output_size;
    let mut stdout = String::new();

    while stdout.len() + line_len <= max {
        let _ = writeln!(stdout, "{text}");
    }

    Ok(ExecResult {
        stdout,
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}

pub fn realpath_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    if args.is_empty() {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "realpath: missing operand\n".to_string(),
            exit_code: 1,
            env: HashMap::new(),
});
    }

    let mut stdout = String::new();

    for path in args {
        let resolved = crate::fs::path::resolve(ctx.cwd, path);
        match ctx.fs.realpath(&resolved) {
            Ok(rp) => {
                let _ = writeln!(stdout, "{rp}");
            }
            Err(_) => {
                let _ = writeln!(stdout, "{resolved}");
            }
        }
    }

    Ok(ExecResult { stdout, stderr: String::new(), exit_code: 0, env: HashMap::new() })
}

pub fn mktemp_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut make_dir = false;
    let mut parent_dir: Option<&str> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-d" | "--directory" => {
                make_dir = true;
                i += 1;
            }
            "-p" | "--tmpdir" if i + 1 < args.len() => {
                parent_dir = Some(args[i + 1]);
                i += 2;
            }
            _ => {
                i += 1;
            }
        }
    }

    let base = parent_dir.unwrap_or("/tmp");
    let resolved_base = crate::fs::path::resolve(ctx.cwd, base);

    let mut counter = 0u64;
    let name = loop {
        let candidate = format!("{resolved_base}/tmp.{counter:010}");
        if !ctx.fs.exists(&candidate) {
            break candidate;
        }
        counter += 1;
        if counter > 999_999 {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: "mktemp: failed to create temp file\n".to_string(),
                exit_code: 1,
                env: HashMap::new(),
});
        }
    };

    if make_dir {
        if let Err(e) = ctx.fs.mkdir(&name, true) {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: format!("mktemp: {e}\n"),
                exit_code: 1,
                env: HashMap::new(),
});
        }
    } else if let Err(e) = ctx.fs.write_file(&name, b"") {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: format!("mktemp: {e}\n"),
            exit_code: 1,
            env: HashMap::new(),
});
    }

    Ok(ExecResult {
        stdout: format!("{name}\n"),
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}

pub fn whoami_cmd(_args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let user = ctx.env.get("USER").cloned().unwrap_or_default();
    Ok(ExecResult {
        stdout: format!("{user}\n"),
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}

pub fn hostname_cmd(_args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let host = ctx.env.get("HOSTNAME").cloned().unwrap_or_default();
    Ok(ExecResult {
        stdout: format!("{host}\n"),
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}

pub fn uname_cmd(args: &[&str], _ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let sysname = "Linux";
    let nodename = "vbash";
    let release = "5.15.0";
    let machine = "x86_64";

    if args.is_empty() {
        return Ok(ExecResult {
            stdout: format!("{sysname}\n"),
            stderr: String::new(),
            exit_code: 0,
            env: HashMap::new(),
});
    }

    let mut parts = Vec::new();
    let mut all = false;

    for arg in args {
        match *arg {
            "-a" | "--all" => all = true,
            "-s" | "--kernel-name" => parts.push(sysname),
            "-n" | "--nodename" => parts.push(nodename),
            "-r" | "--kernel-release" => parts.push(release),
            "-m" | "--machine" => parts.push(machine),
            _ => {}
        }
    }

    if all {
        parts.clear();
        parts.extend_from_slice(&[sysname, nodename, release, machine]);
    }

    if parts.is_empty() {
        parts.push(sysname);
    }

    Ok(ExecResult {
        stdout: format!("{}\n", parts.join(" ")),
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}

pub fn du_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut human = false;
    let mut summary = false;
    let mut max_depth = None;
    let mut paths = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-h" | "--human-readable" => human = true,
            "-s" | "--summarize" => summary = true,
            "-d" | "--max-depth" if i + 1 < args.len() => {
                max_depth = Some(args[i + 1].parse::<usize>().unwrap_or(0));
                i += 1;
            }
            arg if arg.starts_with("--max-depth=") => {
                max_depth = Some(arg.split_once('=').map_or(0, |(_, v)| v.parse().unwrap_or(0)));
            }
            arg if !arg.starts_with('-') => paths.push(arg),
            _ => {}
        }
        i += 1;
    }

    if paths.is_empty() { paths.push("."); }

    let mut stdout = String::new();
    for path in &paths {
        let resolved = crate::fs::path::resolve(ctx.cwd, path);
        let size = du_walk(ctx.fs, &resolved, 0, max_depth.unwrap_or(usize::MAX), summary, human, &mut stdout);
        if summary {
            format_du_line(&mut stdout, size, path, human);
        }
    }

    Ok(ExecResult { stdout, stderr: String::new(), exit_code: 0, env: HashMap::new() })
}

fn du_walk(fs: &dyn VirtualFs, path: &str, depth: usize, max_depth: usize, summary: bool, human: bool, stdout: &mut String) -> u64 {
    let Ok(meta) = fs.stat(path) else { return 0 };

    if meta.is_file() {
        if !summary && depth <= max_depth {
            format_du_line(stdout, meta.size, path, human);
        }
        return meta.size;
    }

    let mut total = 0u64;
    if let Ok(entries) = fs.readdir(path) {
        for entry in &entries {
            let child = if path == "/" { format!("/{}", entry.name) } else { format!("{}/{}", path, entry.name) };
            total += du_walk(fs, &child, depth + 1, max_depth, summary, human, stdout);
        }
    }

    if !summary && depth <= max_depth {
        format_du_line(stdout, total, path, human);
    }
    total
}

fn format_du_line(stdout: &mut String, size: u64, path: &str, human: bool) {
    let display_size = if human { format_human_size(size) } else { (size / 1024).to_string() };
    let _ = writeln!(stdout, "{display_size}\t{path}");
}

fn format_human_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 { format!("{:.1}G", bytes as f64 / 1_073_741_824.0) }
    else if bytes >= 1_048_576 { format!("{:.1}M", bytes as f64 / 1_048_576.0) }
    else if bytes >= 1024 { format!("{:.1}K", bytes as f64 / 1024.0) }
    else { bytes.to_string() }
}

pub fn cmd_true(_args: &[&str], _ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    Ok(ExecResult { stdout: String::new(), stderr: String::new(), exit_code: 0, env: HashMap::new() })
}

pub fn cmd_false(_args: &[&str], _ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    Ok(ExecResult { stdout: String::new(), stderr: String::new(), exit_code: 1, env: HashMap::new() })
}

pub fn pwd(_args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    Ok(ExecResult {
        stdout: format!("{}\n", ctx.cwd),
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}

pub fn env(_args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut stdout = String::new();
    let mut vars: Vec<_> = ctx.env.iter().collect();
    vars.sort_by_key(|(k, _)| k.as_str());
    for (k, v) in vars {
        let _ = writeln!(stdout, "{k}={v}");
    }
    Ok(ExecResult { stdout, stderr: String::new(), exit_code: 0, env: HashMap::new() })
}

pub fn printenv(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    if args.is_empty() {
        return env(args, ctx);
    }
    let mut stdout = String::new();
    let mut exit_code = 0;
    for name in args {
        if let Some(val) = ctx.env.get(*name) {
            let _ = writeln!(stdout, "{val}");
        } else {
            exit_code = 1;
        }
    }
    Ok(ExecResult { stdout, stderr: String::new(), exit_code, env: HashMap::new() })
}

pub fn seq_cmd(args: &[&str], _ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let (start, end, step) = match args.len() {
        1 => (1i64, args[0].parse::<i64>().unwrap_or(1), 1i64),
        2 => (args[0].parse().unwrap_or(1), args[1].parse().unwrap_or(1), 1),
        3 => (args[0].parse().unwrap_or(1), args[2].parse().unwrap_or(1), args[1].parse().unwrap_or(1)),
        _ => return Ok(ExecResult { stdout: String::new(), stderr: "seq: missing operand\n".to_string(), exit_code: 1, env: HashMap::new() }),
    };

    let mut stdout = String::new();
    let mut current = start;
    if step > 0 {
        while current <= end {
            let _ = writeln!(stdout, "{current}");
            current += step;
        }
    } else if step < 0 {
        while current >= end {
            let _ = writeln!(stdout, "{current}");
            current += step;
        }
    }

    Ok(ExecResult { stdout, stderr: String::new(), exit_code: 0, env: HashMap::new() })
}
