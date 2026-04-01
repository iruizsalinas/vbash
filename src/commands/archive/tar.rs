use std::fmt::Write;
use std::io::{Read as _, Write as _};

use crate::ExecResult;
use crate::commands::CommandContext;

const MAX_DECOMPRESS: usize = 100 * 1024 * 1024;
use crate::error::Error;
use crate::fs::FileType;
use std::collections::HashMap;

const BLOCK_SIZE: usize = 512;

fn octal_encode(val: u64, width: usize) -> Vec<u8> {
    let s = format!("{val:0>width$o}", width = width - 1);
    let bytes = s.as_bytes();
    let start = if bytes.len() >= width { bytes.len() - (width - 1) } else { 0 };
    let mut out = Vec::with_capacity(width);
    out.extend_from_slice(&bytes[start..]);
    out.push(0);
    while out.len() < width {
        out.push(0);
    }
    out.truncate(width);
    out
}

fn octal_decode(data: &[u8]) -> u64 {
    let s: String = data.iter()
        .take_while(|&&b| b != 0 && b != b' ')
        .filter(|&&b| (b'0'..=b'7').contains(&b))
        .map(|&b| b as char)
        .collect();
    u64::from_str_radix(&s, 8).unwrap_or(0)
}

fn make_header(name: &str, size: u64, mode: u32, is_dir: bool) -> [u8; BLOCK_SIZE] {
    let mut header = [0u8; BLOCK_SIZE];

    let name_bytes = if is_dir && !name.ends_with('/') {
        format!("{name}/")
    } else {
        name.to_string()
    };

    let name_b = name_bytes.as_bytes();
    if name_b.len() <= 100 {
        header[..name_b.len()].copy_from_slice(name_b);
    } else {
        let split = name_b.len().min(155);
        let prefix_end = name_b[..split].iter().rposition(|&b| b == b'/').unwrap_or(0);
        let prefix = &name_b[..prefix_end];
        let suffix = &name_b[prefix_end + 1..];
        let slen = suffix.len().min(100);
        header[..slen].copy_from_slice(&suffix[..slen]);
        let plen = prefix.len().min(155);
        header[345..345 + plen].copy_from_slice(&prefix[..plen]);
    }

    let mode_oct = octal_encode(u64::from(mode), 8);
    header[100..108].copy_from_slice(&mode_oct);

    let uid_oct = octal_encode(0, 8);
    header[108..116].copy_from_slice(&uid_oct);
    header[116..124].copy_from_slice(&uid_oct);

    let size_oct = octal_encode(size, 12);
    header[124..136].copy_from_slice(&size_oct);

    let mtime_oct = octal_encode(0, 12);
    header[136..148].copy_from_slice(&mtime_oct);

    header[148..156].copy_from_slice(b"        ");

    header[156] = if is_dir { b'5' } else { b'0' };

    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");

    let uname = b"user";
    header[265..265 + uname.len()].copy_from_slice(uname);
    header[297..297 + uname.len()].copy_from_slice(uname);

    let cksum: u32 = header.iter().map(|&b| u32::from(b)).sum();
    let cksum_oct = format!("{cksum:06o}\0 ");
    header[148..156].copy_from_slice(&cksum_oct.as_bytes()[..8]);

    header
}

fn collect_entries(
    fs: &dyn crate::fs::VirtualFs,
    base: &str,
    prefix: &str,
    entries: &mut Vec<(String, bool, u32)>,
) {
    let Ok(dir_entries) = fs.readdir(base) else { return };
    let mut sorted: Vec<_> = dir_entries.into_iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));

    for entry in &sorted {
        let rel = if prefix.is_empty() {
            entry.name.clone()
        } else {
            format!("{prefix}/{}", entry.name)
        };
        let full = crate::fs::path::join(base, &entry.name);

        if entry.file_type == FileType::Directory {
            let mode = fs.stat(&full).map(|m| m.mode).unwrap_or(0o755);
            entries.push((rel.clone(), true, mode));
            collect_entries(fs, &full, &rel, entries);
        } else {
            let mode = fs.stat(&full).map(|m| m.mode).unwrap_or(0o644);
            entries.push((rel, false, mode));
        }
    }
}

fn tar_create(
    ctx: &mut CommandContext<'_>,
    archive_path: &str,
    sources: &[&str],
    gzip: bool,
    verbose: bool,
) -> ExecResult {
    let mut stdout = String::new();
    let mut tar_data: Vec<u8> = Vec::new();

    for source in sources {
        let resolved = crate::fs::path::resolve(ctx.cwd, source);
        let meta = match ctx.fs.stat(&resolved) {
            Ok(m) => m,
            Err(e) => {
                return ExecResult {
                    stdout: String::new(),
                    stderr: format!("tar: {e}\n"),
                    exit_code: 2,
                    env: HashMap::new(),
};
            }
        };

        if meta.is_dir() {
            let header = make_header(source, 0, meta.mode, true);
            tar_data.extend_from_slice(&header);
            if verbose {
                let _ = writeln!(stdout, "{source}/");
            }

            let mut file_entries = Vec::new();
            collect_entries(ctx.fs, &resolved, source, &mut file_entries);

            for (rel, is_dir, mode) in &file_entries {
                let full = crate::fs::path::resolve(ctx.cwd, rel);
                if *is_dir {
                    let header = make_header(rel, 0, *mode, true);
                    tar_data.extend_from_slice(&header);
                    if verbose {
                        let _ = writeln!(stdout, "{rel}/");
                    }
                } else {
                    let content = ctx.fs.read_file(&full).unwrap_or_default();
                    let header = make_header(rel, content.len() as u64, *mode, false);
                    tar_data.extend_from_slice(&header);
                    tar_data.extend_from_slice(&content);
                    let padding = (BLOCK_SIZE - (content.len() % BLOCK_SIZE)) % BLOCK_SIZE;
                    tar_data.extend(std::iter::repeat_n(0u8, padding));
                    if verbose {
                        let _ = writeln!(stdout, "{rel}");
                    }
                }
            }
        } else {
            let content = ctx.fs.read_file(&resolved).unwrap_or_default();
            let header = make_header(source, content.len() as u64, meta.mode, false);
            tar_data.extend_from_slice(&header);
            tar_data.extend_from_slice(&content);
            let padding = (BLOCK_SIZE - (content.len() % BLOCK_SIZE)) % BLOCK_SIZE;
            tar_data.extend(std::iter::repeat_n(0u8, padding));
            if verbose {
                let _ = writeln!(stdout, "{source}");
            }
        }
    }

    tar_data.extend_from_slice(&[0u8; BLOCK_SIZE]);
    tar_data.extend_from_slice(&[0u8; BLOCK_SIZE]);

    let output_data = if gzip {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        if encoder.write_all(&tar_data).is_err() {
            return ExecResult {
                stdout: String::new(),
                stderr: "tar: gzip compression failed\n".to_string(),
                exit_code: 2,
                env: HashMap::new(),
};
        }
        match encoder.finish() {
            Ok(d) => d,
            Err(_) => {
                return ExecResult {
                    stdout: String::new(),
                    stderr: "tar: gzip compression failed\n".to_string(),
                    exit_code: 2,
                    env: HashMap::new(),
};
            }
        }
    } else {
        tar_data
    };

    let archive_resolved = crate::fs::path::resolve(ctx.cwd, archive_path);
    let parent = crate::fs::path::parent(&archive_resolved);
    if parent != "/" {
        let _ = ctx.fs.mkdir(parent, true);
    }
    if let Err(e) = ctx.fs.write_file(&archive_resolved, &output_data) {
        return ExecResult {
            stdout: String::new(),
            stderr: format!("tar: {e}\n"),
            exit_code: 2,
            env: HashMap::new(),
};
    }

    ExecResult { stdout, stderr: String::new(), exit_code: 0, env: HashMap::new() }
}

struct TarEntry {
    name: String,
    size: u64,
    mode: u32,
    is_dir: bool,
    data: Vec<u8>,
}

fn parse_tar_entries(raw: &[u8]) -> Vec<TarEntry> {
    let mut entries = Vec::new();
    let mut offset = 0;

    while offset + BLOCK_SIZE <= raw.len() {
        let header = &raw[offset..offset + BLOCK_SIZE];

        if header.iter().all(|&b| b == 0) {
            break;
        }

        let prefix: String = header[345..500]
            .iter()
            .take_while(|&&b| b != 0)
            .map(|&b| b as char)
            .collect();
        let name_raw: String = header[..100]
            .iter()
            .take_while(|&&b| b != 0)
            .map(|&b| b as char)
            .collect();
        let name = if prefix.is_empty() {
            name_raw
        } else {
            format!("{prefix}/{name_raw}")
        };

        let size = octal_decode(&header[124..136]);
        let mode = u32::try_from(octal_decode(&header[100..108])).unwrap_or(0);
        let typeflag = header[156];
        let is_dir = typeflag == b'5' || name.ends_with('/');

        offset += BLOCK_SIZE;

        let data_size = if is_dir { 0 } else { usize::try_from(size).unwrap_or(0) };
        let data = if data_size > 0 && offset + data_size <= raw.len() {
            raw[offset..offset + data_size].to_vec()
        } else {
            Vec::new()
        };

        if data_size > 0 {
            let blocks = data_size.div_ceil(BLOCK_SIZE);
            offset += blocks * BLOCK_SIZE;
        }

        entries.push(TarEntry { name, size, mode, is_dir, data });
    }

    entries
}

fn strip_components(name: &str, n: usize) -> Option<String> {
    let parts: Vec<&str> = name.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() <= n {
        return None;
    }
    Some(parts[n..].join("/"))
}

fn tar_extract(
    ctx: &mut CommandContext<'_>,
    archive_path: &str,
    dest_dir: &str,
    gzip: bool,
    verbose: bool,
    strip: usize,
) -> ExecResult {
    let archive_resolved = crate::fs::path::resolve(ctx.cwd, archive_path);
    let raw = match ctx.fs.read_file(&archive_resolved) {
        Ok(d) => d,
        Err(e) => {
            return ExecResult {
                stdout: String::new(),
                stderr: format!("tar: {e}\n"),
                exit_code: 2,
                env: HashMap::new(),
};
        }
    };

    let tar_data = if gzip {
        let mut decoder = flate2::read::GzDecoder::new(raw.as_slice());
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match decoder.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    if buf.len() + n > MAX_DECOMPRESS {
                        return ExecResult {
                            stdout: String::new(),
                            stderr: "tar: decompressed data exceeds size limit\n".to_string(),
                            exit_code: 2,
                            env: HashMap::new(),
                        };
                    }
                    buf.extend_from_slice(&chunk[..n]);
                }
                Err(_) => {
                    return ExecResult {
                        stdout: String::new(),
                        stderr: "tar: gzip decompression failed\n".to_string(),
                        exit_code: 2,
                        env: HashMap::new(),
                    };
                }
            }
        }
        buf
    } else {
        raw
    };

    let entries = parse_tar_entries(&tar_data);
    let dest_resolved = crate::fs::path::resolve(ctx.cwd, dest_dir);

    let mut stdout = String::new();
    for entry in &entries {
        let name = if strip > 0 {
            match strip_components(&entry.name, strip) {
                Some(n) => n,
                None => continue,
            }
        } else {
            entry.name.trim_end_matches('/').to_string()
        };

        if name.is_empty() {
            continue;
        }

        let full = crate::fs::path::join(&dest_resolved, &name);
        if entry.is_dir {
            let _ = ctx.fs.mkdir(&full, true);
            if verbose {
                let _ = writeln!(stdout, "{name}/");
            }
        } else {
            let parent = crate::fs::path::parent(&full);
            if parent != "/" {
                let _ = ctx.fs.mkdir(parent, true);
            }
            if let Err(e) = ctx.fs.write_file(&full, &entry.data) {
                return ExecResult {
                    stdout: String::new(),
                    stderr: format!("tar: {e}\n"),
                    exit_code: 2,
                    env: HashMap::new(),
};
            }
            let _ = ctx.fs.chmod(&full, entry.mode);
            if verbose {
                let _ = writeln!(stdout, "{name}");
            }
        }
    }

    ExecResult { stdout, stderr: String::new(), exit_code: 0, env: HashMap::new() }
}

fn tar_list(
    ctx: &mut CommandContext<'_>,
    archive_path: &str,
    gzip: bool,
    verbose: bool,
) -> ExecResult {
    let archive_resolved = crate::fs::path::resolve(ctx.cwd, archive_path);
    let raw = match ctx.fs.read_file(&archive_resolved) {
        Ok(d) => d,
        Err(e) => {
            return ExecResult {
                stdout: String::new(),
                stderr: format!("tar: {e}\n"),
                exit_code: 2,
                env: HashMap::new(),
};
        }
    };

    let tar_data = if gzip {
        let mut decoder = flate2::read::GzDecoder::new(raw.as_slice());
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match decoder.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    if buf.len() + n > MAX_DECOMPRESS {
                        return ExecResult {
                            stdout: String::new(),
                            stderr: "tar: decompressed data exceeds size limit\n".to_string(),
                            exit_code: 2,
                            env: HashMap::new(),
                        };
                    }
                    buf.extend_from_slice(&chunk[..n]);
                }
                Err(_) => {
                    return ExecResult {
                        stdout: String::new(),
                        stderr: "tar: gzip decompression failed\n".to_string(),
                        exit_code: 2,
                        env: HashMap::new(),
                    };
                }
            }
        }
        buf
    } else {
        raw
    };

    let entries = parse_tar_entries(&tar_data);
    let mut stdout = String::new();

    for entry in &entries {
        if verbose {
            let type_ch = if entry.is_dir { 'd' } else { '-' };
            let _ = writeln!(
                stdout,
                "{type_ch}{} {:>8} {}",
                format_mode(entry.mode),
                entry.size,
                entry.name,
            );
        } else {
            let _ = writeln!(stdout, "{}", entry.name);
        }
    }

    ExecResult { stdout, stderr: String::new(), exit_code: 0, env: HashMap::new() }
}

fn format_mode(mode: u32) -> String {
    let mut s = String::with_capacity(9);
    let flags = [
        (0o400, 'r'), (0o200, 'w'), (0o100, 'x'),
        (0o040, 'r'), (0o020, 'w'), (0o010, 'x'),
        (0o004, 'r'), (0o002, 'w'), (0o001, 'x'),
    ];
    for (bit, ch) in &flags {
        s.push(if mode & bit != 0 { *ch } else { '-' });
    }
    s
}

pub fn tar(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut create = false;
    let mut extract = false;
    let mut list = false;
    let mut gzip = false;
    let mut verbose = false;
    let mut archive_file: Option<&str> = None;
    let mut change_dir: Option<&str> = None;
    let mut strip: usize = 0;
    let mut file_args: Vec<&str> = Vec::new();

    let mut i = 0;

    if !args.is_empty() && !args[0].starts_with('-') && args[0] != "." && args[0] != ".." {
        let first = args[0];
        let mut consumed_f = false;
        for ch in first.chars() {
            match ch {
                'c' => create = true,
                'x' => extract = true,
                't' => list = true,
                'z' => gzip = true,
                'v' => verbose = true,
                'f' => consumed_f = true,
                _ => {}
            }
        }
        i = 1;
        if consumed_f && i < args.len() {
            archive_file = Some(args[i]);
            i += 1;
        }
    }

    while i < args.len() {
        match args[i] {
            "-c" | "--create" => create = true,
            "-x" | "--extract" => extract = true,
            "-t" | "--list" => list = true,
            "-z" | "--gzip" => gzip = true,
            "-v" | "--verbose" => verbose = true,
            "-f" if i + 1 < args.len() => {
                i += 1;
                archive_file = Some(args[i]);
            }
            "-C" if i + 1 < args.len() => {
                i += 1;
                change_dir = Some(args[i]);
            }
            arg if arg.starts_with("--strip-components=") => {
                if let Some(val) = arg.strip_prefix("--strip-components=") {
                    strip = val.parse().unwrap_or(0);
                }
            }
            arg if arg.starts_with("-f") => {
                archive_file = Some(&arg[2..]);
            }
            arg if arg.starts_with("-C") => {
                change_dir = Some(&arg[2..]);
            }
            _ => file_args.push(args[i]),
        }
        i += 1;
    }

    let Some(archive) = archive_file else {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "tar: no archive file specified (-f)\n".to_string(),
            exit_code: 2,
            env: HashMap::new(),
});
    };

    if create {
        if file_args.is_empty() {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: "tar: cowardly refusing to create an empty archive\n".to_string(),
                exit_code: 2,
                env: HashMap::new(),
});
        }

        let saved_cwd = ctx.cwd.to_string();
        if let Some(dir) = change_dir {
            let new_cwd = crate::fs::path::resolve(&saved_cwd, dir);
            ctx.cwd = Box::leak(new_cwd.into_boxed_str());
        }
        let result = tar_create(ctx, archive, &file_args, gzip, verbose);
        if change_dir.is_some() {
            ctx.cwd = Box::leak(saved_cwd.into_boxed_str());
        }
        Ok(result)
    } else if extract {
        let dest = change_dir.unwrap_or(".");
        Ok(tar_extract(ctx, archive, dest, gzip, verbose, strip))
    } else if list {
        Ok(tar_list(ctx, archive, gzip, verbose))
    } else {
        Ok(ExecResult {
            stdout: String::new(),
            stderr: "tar: must specify one of -c, -x, -t\n".to_string(),
            exit_code: 2,
            env: HashMap::new(),
})
    }
}
