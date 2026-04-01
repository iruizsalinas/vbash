use std::fmt::Write;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;
use std::collections::HashMap;

pub fn comm(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut suppress1 = false;
    let mut suppress2 = false;
    let mut suppress3 = false;
    let mut file_args = Vec::new();

    for arg in args {
        match *arg {
            "-1" => suppress1 = true,
            "-2" => suppress2 = true,
            "-3" => suppress3 = true,
            "-12" | "-21" => {
                suppress1 = true;
                suppress2 = true;
            }
            "-13" | "-31" => {
                suppress1 = true;
                suppress3 = true;
            }
            "-23" | "-32" => {
                suppress2 = true;
                suppress3 = true;
            }
            "-123" | "-132" | "-213" | "-231" | "-312" | "-321" => {
                suppress1 = true;
                suppress2 = true;
                suppress3 = true;
            }
            _ => file_args.push(*arg),
        }
    }

    if file_args.len() < 2 {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "comm: missing operand\n".to_string(),
            exit_code: 1,
            env: HashMap::new(),
});
    }

    let content1 = if file_args[0] == "-" {
        ctx.stdin.to_string()
    } else {
        let path = crate::fs::path::resolve(ctx.cwd, file_args[0]);
        match ctx.fs.read_file_string(&path) {
            Ok(c) => c,
            Err(e) => {
                return Ok(ExecResult {
                    stdout: String::new(),
                    stderr: format!("comm: {e}\n"),
                    exit_code: 1,
                    env: HashMap::new(),
})
            }
        }
    };

    let content2 = if file_args[1] == "-" {
        ctx.stdin.to_string()
    } else {
        let path = crate::fs::path::resolve(ctx.cwd, file_args[1]);
        match ctx.fs.read_file_string(&path) {
            Ok(c) => c,
            Err(e) => {
                return Ok(ExecResult {
                    stdout: String::new(),
                    stderr: format!("comm: {e}\n"),
                    exit_code: 1,
                    env: HashMap::new(),
})
            }
        }
    };

    let lines1: Vec<&str> = content1.lines().collect();
    let lines2: Vec<&str> = content2.lines().collect();

    let col1_prefix = "";
    let col2_prefix = if suppress1 { "" } else { "\t" };
    let col3_prefix = match (suppress1, suppress2) {
        (true, true) => "",
        (true, false) | (false, true) => "\t",
        (false, false) => "\t\t",
    };

    let mut stdout = String::new();
    let mut i = 0;
    let mut j = 0;

    while i < lines1.len() && j < lines2.len() {
        match lines1[i].cmp(lines2[j]) {
            std::cmp::Ordering::Less => {
                if !suppress1 {
                    let _ = writeln!(stdout, "{col1_prefix}{}", lines1[i]);
                }
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                if !suppress2 {
                    let _ = writeln!(stdout, "{col2_prefix}{}", lines2[j]);
                }
                j += 1;
            }
            std::cmp::Ordering::Equal => {
                if !suppress3 {
                    let _ = writeln!(stdout, "{col3_prefix}{}", lines1[i]);
                }
                i += 1;
                j += 1;
            }
        }
    }

    while i < lines1.len() {
        if !suppress1 {
            let _ = writeln!(stdout, "{col1_prefix}{}", lines1[i]);
        }
        i += 1;
    }

    while j < lines2.len() {
        if !suppress2 {
            let _ = writeln!(stdout, "{col2_prefix}{}", lines2[j]);
        }
        j += 1;
    }

    Ok(ExecResult {
        stdout,
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}

pub fn join(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut join_field_1 = 1usize;
    let mut join_field_2 = 1usize;
    let mut separator = " ".to_string();
    let mut file_args = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-1" if i + 1 < args.len() => {
                join_field_1 = args[i + 1].parse().unwrap_or(1);
                i += 2;
            }
            "-2" if i + 1 < args.len() => {
                join_field_2 = args[i + 1].parse().unwrap_or(1);
                i += 2;
            }
            "-t" if i + 1 < args.len() => {
                separator = args[i + 1].to_string();
                i += 2;
            }
            _ => {
                file_args.push(args[i]);
                i += 1;
            }
        }
    }

    if file_args.len() < 2 {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "join: missing operand\n".to_string(),
            exit_code: 1,
            env: HashMap::new(),
});
    }

    let content1 = if file_args[0] == "-" {
        ctx.stdin.to_string()
    } else {
        let path = crate::fs::path::resolve(ctx.cwd, file_args[0]);
        match ctx.fs.read_file_string(&path) {
            Ok(c) => c,
            Err(e) => {
                return Ok(ExecResult {
                    stdout: String::new(),
                    stderr: format!("join: {e}\n"),
                    exit_code: 1,
                    env: HashMap::new(),
})
            }
        }
    };

    let content2 = if file_args[1] == "-" {
        ctx.stdin.to_string()
    } else {
        let path = crate::fs::path::resolve(ctx.cwd, file_args[1]);
        match ctx.fs.read_file_string(&path) {
            Ok(c) => c,
            Err(e) => {
                return Ok(ExecResult {
                    stdout: String::new(),
                    stderr: format!("join: {e}\n"),
                    exit_code: 1,
                    env: HashMap::new(),
})
            }
        }
    };

    let split_line = |line: &str, sep: &str| -> Vec<String> {
        if sep == " " {
            line.split_whitespace().map(str::to_string).collect()
        } else {
            line.split(sep.chars().next().unwrap_or(' '))
                .map(str::to_string)
                .collect()
        }
    };

    let lines1: Vec<&str> = content1.lines().collect();
    let lines2: Vec<&str> = content2.lines().collect();

    let mut stdout = String::new();

    for line1 in &lines1 {
        let cols_a = split_line(line1, &separator);
        let key1 = cols_a.get(join_field_1.saturating_sub(1)).cloned().unwrap_or_default();

        for line2 in &lines2 {
            let cols_b = split_line(line2, &separator);
            let key2 = cols_b.get(join_field_2.saturating_sub(1)).cloned().unwrap_or_default();

            if key1 == key2 {
                let mut parts: Vec<&str> = Vec::new();
                parts.push(&key1);
                for (fi, f) in cols_a.iter().enumerate() {
                    if fi != join_field_1.saturating_sub(1) {
                        parts.push(f);
                    }
                }
                for (fi, f) in cols_b.iter().enumerate() {
                    if fi != join_field_2.saturating_sub(1) {
                        parts.push(f);
                    }
                }
                let _ = writeln!(stdout, "{}", parts.join(&separator));
            }
        }
    }

    Ok(ExecResult {
        stdout,
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}
