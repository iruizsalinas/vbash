use std::fmt::Write;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;

use super::{read_input, read_input_bytes};
use std::collections::HashMap;

pub fn nl(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut body_numbering = 't';
    let mut number_width = 6usize;
    let mut file_args = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-b" if i + 1 < args.len() => {
                let val = args[i + 1];
                if val == "a" || val == "t" || val == "n" {
                    body_numbering = val.chars().next().unwrap_or('t');
                }
                i += 2;
            }
            "-w" if i + 1 < args.len() => {
                number_width = args[i + 1].parse().unwrap_or(6);
                i += 2;
            }
            arg if arg.starts_with("-b") => {
                let val = &arg[2..];
                if val == "a" || val == "t" || val == "n" {
                    body_numbering = val.chars().next().unwrap_or('t');
                }
                i += 1;
            }
            arg if arg.starts_with("-w") => {
                number_width = arg[2..].parse().unwrap_or(6);
                i += 1;
            }
            _ => {
                file_args.push(args[i]);
                i += 1;
            }
        }
    }

    let content = match read_input(&file_args, ctx) {
        Ok(c) => c,
        Err(e) => {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: format!("nl: {e}\n"),
                exit_code: 1,
                env: HashMap::new(),
})
        }
    };

    let mut stdout = String::new();
    let mut line_num = 1u64;

    for line in content.lines() {
        let should_number = match body_numbering {
            'a' => true,
            't' => !line.is_empty(),
            _ => false,
        };
        if should_number {
            let _ = writeln!(stdout, "{line_num:>number_width$}\t{line}");
            line_num += 1;
        } else {
            let _ = writeln!(stdout, "{:>number_width$}\t{line}", "");
        }
    }

    Ok(ExecResult {
        stdout,
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}

pub fn od(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut address_radix = 'o';
    let mut output_type = 'o';
    let mut file_args = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-A" if i + 1 < args.len() => {
                let val = args[i + 1];
                if matches!(val, "o" | "d" | "x" | "n") {
                    address_radix = val.chars().next().unwrap_or('o');
                }
                i += 2;
            }
            "-t" if i + 1 < args.len() => {
                let val = args[i + 1];
                if let Some(c) = val.chars().next() {
                    if matches!(c, 'o' | 'x' | 'c' | 'd') {
                        output_type = c;
                    }
                }
                i += 2;
            }
            arg if arg.starts_with("-A") => {
                let val = &arg[2..];
                if matches!(val, "o" | "d" | "x" | "n") {
                    address_radix = val.chars().next().unwrap_or('o');
                }
                i += 1;
            }
            arg if arg.starts_with("-t") && arg.len() > 2 => {
                let c = arg.as_bytes()[2] as char;
                if matches!(c, 'o' | 'x' | 'c' | 'd') {
                    output_type = c;
                }
                i += 1;
            }
            _ => {
                file_args.push(args[i]);
                i += 1;
            }
        }
    }

    let data = match read_input_bytes(&file_args, ctx) {
        Ok(d) => d,
        Err(e) => {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: format!("od: {e}\n"),
                exit_code: 1,
                env: HashMap::new(),
})
        }
    };

    let format_addr = |off: usize| -> String {
        match address_radix {
            'o' => format!("{off:07o}"),
            'd' => format!("{off:07}"),
            'x' => format!("{off:07x}"),
            _ => String::new(),
        }
    };

    let mut stdout = String::new();
    let bytes_per_line = 16;

    let mut offset = 0;
    while offset < data.len() {
        let end = std::cmp::min(offset + bytes_per_line, data.len());
        let chunk = &data[offset..end];

        if address_radix != 'n' {
            stdout.push_str(&format_addr(offset));
        }

        match output_type {
            'o' => {
                let mut wi = 0;
                while wi + 1 < chunk.len() {
                    let word = u16::from_le_bytes([chunk[wi], chunk[wi + 1]]);
                    let _ = write!(stdout, " {word:06o}");
                    wi += 2;
                }
                if wi < chunk.len() {
                    let _ = write!(stdout, " {:06o}", u16::from(chunk[wi]));
                }
            }
            'x' => {
                for byte in chunk {
                    let _ = write!(stdout, " {byte:02x}");
                }
            }
            'c' => {
                for byte in chunk {
                    match *byte {
                        b'\0' => { let _ = write!(stdout, "  \\0"); }
                        b'\t' => { let _ = write!(stdout, "  \\t"); }
                        b'\n' => { let _ = write!(stdout, "  \\n"); }
                        b'\r' => { let _ = write!(stdout, "  \\r"); }
                        b' '..=b'~' => { let _ = write!(stdout, "   {}", *byte as char); }
                        _ => { let _ = write!(stdout, " {byte:03o}"); }
                    }
                }
            }
            'd' => {
                let mut wi = 0;
                while wi + 1 < chunk.len() {
                    let word = u16::from_le_bytes([chunk[wi], chunk[wi + 1]]);
                    let _ = write!(stdout, " {word:6}");
                    wi += 2;
                }
                if wi < chunk.len() {
                    let _ = write!(stdout, " {:6}", u16::from(chunk[wi]));
                }
            }
            _ => {}
        }

        stdout.push('\n');
        offset = end;
    }

    if address_radix != 'n' {
        let _ = writeln!(stdout, "{}", format_addr(data.len()));
    }

    Ok(ExecResult {
        stdout,
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}

const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let mut result = String::new();
    let mut i = 0;
    while i < data.len() {
        let b0 = u32::from(data[i]);
        let b1 = if i + 1 < data.len() { u32::from(data[i + 1]) } else { 0 };
        let b2 = if i + 2 < data.len() { u32::from(data[i + 2]) } else { 0 };

        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(BASE64_CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(BASE64_CHARS[((triple >> 12) & 0x3F) as usize] as char);

        if i + 1 < data.len() {
            result.push(BASE64_CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if i + 2 < data.len() {
            result.push(BASE64_CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        i += 3;
    }
    result
}

fn base64_decode_char(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    let filtered: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let mut result = Vec::new();
    let mut i = 0;

    while i < filtered.len() {
        if filtered.len() - i < 4 {
            return Err("base64: invalid input".to_string());
        }
        let c0 = base64_decode_char(filtered[i]).ok_or("base64: invalid input")?;
        let c1 = base64_decode_char(filtered[i + 1]).ok_or("base64: invalid input")?;

        result.push((c0 << 2) | (c1 >> 4));

        if filtered[i + 2] != b'=' {
            let c2 = base64_decode_char(filtered[i + 2]).ok_or("base64: invalid input")?;
            result.push((c1 << 4) | (c2 >> 2));

            if filtered[i + 3] != b'=' {
                let c3 = base64_decode_char(filtered[i + 3]).ok_or("base64: invalid input")?;
                result.push((c2 << 6) | c3);
            }
        }

        i += 4;
    }

    Ok(result)
}

pub fn base64(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut decode = false;
    let mut wrap_cols = 76usize;
    let mut file_args = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-d" | "--decode" => {
                decode = true;
                i += 1;
            }
            "-w" if i + 1 < args.len() => {
                wrap_cols = args[i + 1].parse().unwrap_or(76);
                i += 2;
            }
            arg if arg.starts_with("-w") => {
                wrap_cols = arg[2..].parse().unwrap_or(76);
                i += 1;
            }
            _ => {
                file_args.push(args[i]);
                i += 1;
            }
        }
    }

    if decode {
        let content = match read_input(&file_args, ctx) {
            Ok(c) => c,
            Err(e) => {
                return Ok(ExecResult {
                    stdout: String::new(),
                    stderr: format!("base64: {e}\n"),
                    exit_code: 1,
                    env: HashMap::new(),
})
            }
        };
        match base64_decode(&content) {
            Ok(bytes) => {
                let stdout = String::from_utf8(bytes).unwrap_or_default();
                Ok(ExecResult {
                    stdout,
                    stderr: String::new(),
                    exit_code: 0,
                    env: HashMap::new(),
})
            }
            Err(e) => Ok(ExecResult {
                stdout: String::new(),
                stderr: format!("{e}\n"),
                exit_code: 1,
                env: HashMap::new(),
}),
        }
    } else {
        let data = match read_input_bytes(&file_args, ctx) {
            Ok(d) => d,
            Err(e) => {
                return Ok(ExecResult {
                    stdout: String::new(),
                    stderr: format!("base64: {e}\n"),
                    exit_code: 1,
                    env: HashMap::new(),
})
            }
        };
        let encoded = base64_encode(&data);
        let mut stdout = String::new();
        if wrap_cols == 0 {
            stdout.push_str(&encoded);
            stdout.push('\n');
        } else {
            let chars: Vec<char> = encoded.chars().collect();
            let mut pos = 0;
            while pos < chars.len() {
                let end = std::cmp::min(pos + wrap_cols, chars.len());
                let line: String = chars[pos..end].iter().collect();
                let _ = writeln!(stdout, "{line}");
                pos = end;
            }
        }
        Ok(ExecResult {
            stdout,
            stderr: String::new(),
            exit_code: 0,
            env: HashMap::new(),
})
    }
}

pub fn strings(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut min_len = 4usize;
    let mut file_args = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-n" if i + 1 < args.len() => {
                min_len = args[i + 1].parse().unwrap_or(4);
                i += 2;
            }
            arg if arg.starts_with("-n") => {
                min_len = arg[2..].parse().unwrap_or(4);
                i += 1;
            }
            _ => {
                file_args.push(args[i]);
                i += 1;
            }
        }
    }

    if min_len == 0 {
        min_len = 1;
    }

    let data = match read_input_bytes(&file_args, ctx) {
        Ok(d) => d,
        Err(e) => {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: format!("strings: {e}\n"),
                exit_code: 1,
                env: HashMap::new(),
})
        }
    };

    let mut stdout = String::new();
    let mut current = String::new();

    for byte in &data {
        if byte.is_ascii_graphic() || *byte == b' ' {
            current.push(*byte as char);
        } else {
            if current.len() >= min_len {
                let _ = writeln!(stdout, "{current}");
            }
            current.clear();
        }
    }
    if current.len() >= min_len {
        let _ = writeln!(stdout, "{current}");
    }

    Ok(ExecResult {
        stdout,
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}
