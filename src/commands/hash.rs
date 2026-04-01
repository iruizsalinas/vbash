use std::fmt::Write;

use digest::Digest;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;
use std::collections::HashMap;

fn compute_hash<D: Digest>(data: &[u8]) -> String {
    let mut hasher = D::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hex = String::with_capacity(result.len() * 2);
    for byte in &result {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

fn hash_command<D: Digest>(
    args: &[&str],
    ctx: &mut CommandContext<'_>,
    cmd_name: &str,
) -> ExecResult {
    let mut check_mode = false;
    let mut file_args = Vec::new();

    for arg in args {
        match *arg {
            "-c" | "--check" => check_mode = true,
            _ if !arg.starts_with('-') => file_args.push(*arg),
            _ => {}
        }
    }

    if check_mode {
        return check_hashes::<D>(&file_args, ctx, cmd_name);
    }

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code = 0;

    if file_args.is_empty() {
        let hash = compute_hash::<D>(ctx.stdin.as_bytes());
        let _ = writeln!(stdout, "{hash}  -");
    } else {
        for path in &file_args {
            let resolved = crate::fs::path::resolve(ctx.cwd, path);
            match ctx.fs.read_file(&resolved) {
                Ok(data) => {
                    let hash = compute_hash::<D>(&data);
                    let _ = writeln!(stdout, "{hash}  {path}");
                }
                Err(e) => {
                    let _ = writeln!(stderr, "{cmd_name}: {path}: {e}");
                    exit_code = 1;
                }
            }
        }
    }

    ExecResult { stdout, stderr, exit_code, env: HashMap::new() }
}

fn check_hashes<D: Digest>(
    file_args: &[&str],
    ctx: &mut CommandContext<'_>,
    cmd_name: &str,
) -> ExecResult {
    let content = if file_args.is_empty() {
        ctx.stdin.to_string()
    } else {
        let path = crate::fs::path::resolve(ctx.cwd, file_args[0]);
        ctx.fs.read_file_string(&path).unwrap_or_default()
    };

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut failed = 0u64;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (expected_hash, filename) = if let Some(pos) = line.find("  ") {
            (&line[..pos], &line[pos + 2..])
        } else if let Some(pos) = line.find(' ') {
            (&line[..pos], line[pos + 1..].trim_start())
        } else {
            let _ = writeln!(stderr, "{cmd_name}: invalid line: {line}");
            failed += 1;
            continue;
        };

        let resolved = crate::fs::path::resolve(ctx.cwd, filename);
        match ctx.fs.read_file(&resolved) {
            Ok(data) => {
                let actual = compute_hash::<D>(&data);
                if actual == expected_hash {
                    let _ = writeln!(stdout, "{filename}: OK");
                } else {
                    let _ = writeln!(stdout, "{filename}: FAILED");
                    failed += 1;
                }
            }
            Err(e) => {
                let _ = writeln!(stderr, "{cmd_name}: {filename}: {e}");
                failed += 1;
            }
        }
    }

    if failed > 0 {
        let _ = writeln!(stderr, "{cmd_name}: WARNING: {failed} computed checksum(s) did NOT match");
    }

    let exit_code = i32::from(failed > 0);
    ExecResult { stdout, stderr, exit_code, env: HashMap::new() }
}

pub fn md5sum_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    Ok(hash_command::<md5::Md5>(args, ctx, "md5sum"))
}

pub fn sha1sum_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    Ok(hash_command::<sha1::Sha1>(args, ctx, "sha1sum"))
}

pub fn sha256sum_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    Ok(hash_command::<sha2::Sha256>(args, ctx, "sha256sum"))
}

pub fn sha512sum_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    Ok(hash_command::<sha2::Sha512>(args, ctx, "sha512sum"))
}
