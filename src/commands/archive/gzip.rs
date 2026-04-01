use std::fmt::Write;
use std::io::Write as _;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;
use std::collections::HashMap;

const MAX_DECOMPRESS_SIZE: usize = 100 * 1024 * 1024; // 100MB cap on decompressed output

fn read_limited(reader: &mut impl std::io::Read, limit: usize) -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                if buf.len() + n > limit {
                    return Err(std::io::Error::other("decompressed data exceeds size limit"));
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            Err(e) => return Err(e),
        }
    }
    Ok(buf)
}

pub fn gzip_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut decompress = false;
    let mut to_stdout = false;
    let mut keep = false;
    let mut level = flate2::Compression::default();
    let mut file_args: Vec<&str> = Vec::new();

    for arg in args {
        match *arg {
            "-d" | "--decompress" => decompress = true,
            "-c" | "--stdout" => to_stdout = true,
            "-k" | "--keep" => keep = true,
            "-f" | "--force" => {}
            "-1" | "--fast" => level = flate2::Compression::new(1),
            "-2" => level = flate2::Compression::new(2),
            "-3" => level = flate2::Compression::new(3),
            "-4" => level = flate2::Compression::new(4),
            "-5" => level = flate2::Compression::new(5),
            "-6" => level = flate2::Compression::new(6),
            "-7" => level = flate2::Compression::new(7),
            "-8" => level = flate2::Compression::new(8),
            "-9" | "--best" => level = flate2::Compression::new(9),
            "-dc" | "-cd" => { decompress = true; to_stdout = true; }
            other if !other.starts_with('-') => file_args.push(other),
            _ => {}
        }
    }

    if file_args.is_empty() {
        let input = ctx.stdin.as_bytes();
        if decompress {
            let mut decoder = flate2::read::GzDecoder::new(input);
            let Ok(buf) = read_limited(&mut decoder, MAX_DECOMPRESS_SIZE) else {
                return Ok(ExecResult {
                    stdout: String::new(),
                    stderr: "gzip: decompression failed or size limit exceeded\n".to_string(),
                    exit_code: 1,
                    env: HashMap::new(),
                });
            };
            let stdout = String::from_utf8_lossy(&buf).into_owned();
            return Ok(ExecResult { stdout, stderr: String::new(), exit_code: 0, env: HashMap::new() });
        }
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), level);
        if encoder.write_all(input).is_err() {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: "gzip: compression failed\n".to_string(),
                exit_code: 1,
                env: HashMap::new(),
});
        }
        let Ok(compressed) = encoder.finish() else {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: "gzip: compression failed\n".to_string(),
                exit_code: 1,
                env: HashMap::new(),
});
        };
        let stdout = String::from_utf8_lossy(&compressed).into_owned();
        return Ok(ExecResult { stdout, stderr: String::new(), exit_code: 0, env: HashMap::new() });
    }

    let mut stderr = String::new();
    let mut stdout = String::new();
    let mut exit_code = 0;

    for file in &file_args {
        let resolved = crate::fs::path::resolve(ctx.cwd, file);
        if decompress {
            let input = match ctx.fs.read_file(&resolved) {
                Ok(d) => d,
                Err(e) => {
                    let _ = writeln!(stderr, "gzip: {e}");
                    exit_code = 1;
                    continue;
                }
            };
            let mut decoder = flate2::read::GzDecoder::new(input.as_slice());
            let Ok(buf) = read_limited(&mut decoder, MAX_DECOMPRESS_SIZE) else {
                let _ = writeln!(stderr, "gzip: {file}: decompression failed or size limit exceeded");
                exit_code = 1;
                continue;
            };
            if to_stdout {
                stdout.push_str(&String::from_utf8_lossy(&buf));
            } else {
                let out_path = if let Some(stripped) = resolved.strip_suffix(".gz") {
                    stripped.to_string()
                } else {
                    format!("{resolved}.out")
                };
                let parent = crate::fs::path::parent(&out_path);
                if parent != "/" {
                    let _ = ctx.fs.mkdir(parent, true);
                }
                if let Err(e) = ctx.fs.write_file(&out_path, &buf) {
                    let _ = writeln!(stderr, "gzip: {e}");
                    exit_code = 1;
                    continue;
                }
                if !keep {
                    let _ = ctx.fs.rm(&resolved, false, true);
                }
            }
        } else {
            let input = match ctx.fs.read_file(&resolved) {
                Ok(d) => d,
                Err(e) => {
                    let _ = writeln!(stderr, "gzip: {e}");
                    exit_code = 1;
                    continue;
                }
            };
            let mut encoder = flate2::write::GzEncoder::new(Vec::new(), level);
            if encoder.write_all(&input).is_err() {
                let _ = writeln!(stderr, "gzip: {file}: compression failed");
                exit_code = 1;
                continue;
            }
            let Ok(compressed) = encoder.finish() else {
                let _ = writeln!(stderr, "gzip: {file}: compression failed");
                exit_code = 1;
                continue;
            };
            if to_stdout {
                stdout.push_str(&String::from_utf8_lossy(&compressed));
            } else {
                let out_path = format!("{resolved}.gz");
                let parent = crate::fs::path::parent(&out_path);
                if parent != "/" {
                    let _ = ctx.fs.mkdir(parent, true);
                }
                if let Err(e) = ctx.fs.write_file(&out_path, &compressed) {
                    let _ = writeln!(stderr, "gzip: {e}");
                    exit_code = 1;
                    continue;
                }
                if !keep {
                    let _ = ctx.fs.rm(&resolved, false, true);
                }
            }
        }
    }

    Ok(ExecResult { stdout, stderr, exit_code, env: HashMap::new() })
}

pub fn gunzip(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut new_args: Vec<&str> = vec!["-d"];
    new_args.extend_from_slice(args);
    gzip_cmd(&new_args, ctx)
}

pub fn zcat(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut new_args: Vec<&str> = vec!["-dc"];
    new_args.extend_from_slice(args);
    gzip_cmd(&new_args, ctx)
}
