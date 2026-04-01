use std::fmt::Write;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;

use super::read_input;
use std::collections::HashMap;

pub fn uniq(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut count = false;
    let mut repeated_only = false;
    let mut unique_only = false;
    let mut ignore_case = false;
    let mut file_args = Vec::new();

    for arg in args {
        match *arg {
            "-c" | "--count" => count = true,
            "-d" | "--repeated" => repeated_only = true,
            "-u" | "--unique" => unique_only = true,
            "-i" | "--ignore-case" => ignore_case = true,
            _ => file_args.push(*arg),
        }
    }

    let content = match read_input(&file_args, ctx) {
        Ok(c) => c,
        Err(e) => {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: format!("uniq: {e}\n"),
                exit_code: 1,
                env: HashMap::new(),
})
        }
    };

    let lines: Vec<&str> = content.lines().collect();
    let mut stdout = String::new();
    let mut i = 0;

    while i < lines.len() {
        let current = lines[i];
        let mut run_count = 1usize;
        while i + run_count < lines.len() {
            let next = lines[i + run_count];
            let eq = if ignore_case {
                current.eq_ignore_ascii_case(next)
            } else {
                current == next
            };
            if eq {
                run_count += 1;
            } else {
                break;
            }
        }

        let show = if repeated_only {
            run_count > 1
        } else if unique_only {
            run_count == 1
        } else {
            true
        };

        if show {
            if count {
                let _ = writeln!(stdout, "{run_count:>7} {current}");
            } else {
                let _ = writeln!(stdout, "{current}");
            }
        }

        i += run_count;
    }

    Ok(ExecResult {
        stdout,
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}

pub fn rev(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut file_args: Vec<&str> = args.iter().filter(|a| !a.starts_with('-')).copied().collect();
    if file_args.is_empty() {
        file_args.clear();
    }

    let content = match read_input(&file_args, ctx) {
        Ok(c) => c,
        Err(e) => {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: format!("rev: {e}\n"),
                exit_code: 1,
                env: HashMap::new(),
})
        }
    };

    let mut stdout = String::new();
    for line in content.lines() {
        let reversed: String = line.chars().rev().collect();
        let _ = writeln!(stdout, "{reversed}");
    }

    Ok(ExecResult {
        stdout,
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}

pub fn tac(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let file_args: Vec<&str> = args.iter().filter(|a| !a.starts_with('-')).copied().collect();

    let content = match read_input(&file_args, ctx) {
        Ok(c) => c,
        Err(e) => {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: format!("tac: {e}\n"),
                exit_code: 1,
                env: HashMap::new(),
})
        }
    };

    let lines: Vec<&str> = content.lines().collect();
    let mut stdout = String::new();
    for line in lines.iter().rev() {
        let _ = writeln!(stdout, "{line}");
    }

    Ok(ExecResult {
        stdout,
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}

pub fn fold(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut width = 80usize;
    let mut break_spaces = false;
    let mut file_args = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-w" if i + 1 < args.len() => {
                width = args[i + 1].parse().unwrap_or(80);
                i += 2;
            }
            "-s" => {
                break_spaces = true;
                i += 1;
            }
            arg if arg.starts_with("-w") => {
                width = arg[2..].parse().unwrap_or(80);
                i += 1;
            }
            _ => {
                file_args.push(args[i]);
                i += 1;
            }
        }
    }

    if width == 0 {
        width = 80;
    }

    let content = match read_input(&file_args, ctx) {
        Ok(c) => c,
        Err(e) => {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: format!("fold: {e}\n"),
                exit_code: 1,
                env: HashMap::new(),
})
        }
    };

    let mut stdout = String::new();
    for line in content.lines() {
        if line.len() <= width {
            let _ = writeln!(stdout, "{line}");
            continue;
        }

        let chars: Vec<char> = line.chars().collect();
        let mut pos = 0;
        while pos < chars.len() {
            let remaining = chars.len() - pos;
            if remaining <= width {
                let seg: String = chars[pos..].iter().collect();
                let _ = writeln!(stdout, "{seg}");
                break;
            }

            let mut end = pos + width;
            if break_spaces {
                let mut space_pos = None;
                for j in (pos..end).rev() {
                    if chars[j] == ' ' {
                        space_pos = Some(j);
                        break;
                    }
                }
                if let Some(sp) = space_pos {
                    let seg: String = chars[pos..=sp].iter().collect();
                    let _ = writeln!(stdout, "{seg}");
                    pos = sp + 1;
                    continue;
                }
            }

            if end > chars.len() {
                end = chars.len();
            }
            let seg: String = chars[pos..end].iter().collect();
            let _ = writeln!(stdout, "{seg}");
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

pub fn expand_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut tab_width = 8usize;
    let mut file_args = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-t" if i + 1 < args.len() => {
                tab_width = args[i + 1].parse().unwrap_or(8);
                i += 2;
            }
            arg if arg.starts_with("-t") => {
                tab_width = arg[2..].parse().unwrap_or(8);
                i += 1;
            }
            _ => {
                file_args.push(args[i]);
                i += 1;
            }
        }
    }

    if tab_width == 0 {
        tab_width = 8;
    }

    let content = match read_input(&file_args, ctx) {
        Ok(c) => c,
        Err(e) => {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: format!("expand: {e}\n"),
                exit_code: 1,
                env: HashMap::new(),
})
        }
    };

    let mut stdout = String::new();
    for line in content.lines() {
        let mut col = 0usize;
        for ch in line.chars() {
            if ch == '\t' {
                let spaces = tab_width - (col % tab_width);
                for _ in 0..spaces {
                    stdout.push(' ');
                }
                col += spaces;
            } else {
                stdout.push(ch);
                col += 1;
            }
        }
        stdout.push('\n');
    }

    Ok(ExecResult {
        stdout,
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}

pub fn unexpand_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut tab_width = 8usize;
    let mut first_only = false;
    let mut file_args = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-t" if i + 1 < args.len() => { tab_width = args[i + 1].parse().unwrap_or(8); i += 1; }
            "--first-only" => first_only = true,
            "-a" | "--all" => first_only = false,
            arg if !arg.starts_with('-') => file_args.push(arg),
            _ => {}
        }
        i += 1;
    }

    let content = match read_input(&file_args, ctx) {
        Ok(c) => c,
        Err(e) => {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: format!("unexpand: {e}\n"),
                exit_code: 1,
                env: HashMap::new(),
            })
        }
    };
    let mut stdout = String::new();

    for line in content.lines() {
        let converted = unexpand_line(line, tab_width, first_only);
        stdout.push_str(&converted);
        stdout.push('\n');
    }

    Ok(ExecResult { stdout, stderr: String::new(), exit_code: 0, env: HashMap::new() })
}

fn unexpand_line(line: &str, tab_width: usize, first_only: bool) -> String {
    if tab_width == 0 { return line.to_string(); }

    let mut result = String::new();
    let mut col = 0;
    let mut space_count = 0;
    let mut past_leading = false;

    for ch in line.chars() {
        if ch == ' ' && !(first_only && past_leading) {
            space_count += 1;
            col += 1;
            if col % tab_width == 0 && space_count > 0 {
                result.push('\t');
                space_count = 0;
            }
        } else {
            for _ in 0..space_count { result.push(' '); }
            space_count = 0;
            result.push(ch);
            col += 1;
            if ch != ' ' { past_leading = true; }
        }
    }
    for _ in 0..space_count { result.push(' '); }
    result
}

pub fn column(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut table_mode = false;
    let mut separator = None;
    let mut file_args = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-t" => {
                table_mode = true;
                i += 1;
            }
            "-s" if i + 1 < args.len() => {
                separator = Some(args[i + 1].to_string());
                i += 2;
            }
            arg if arg.starts_with("-s") => {
                separator = Some(arg[2..].to_string());
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
                stderr: format!("column: {e}\n"),
                exit_code: 1,
                env: HashMap::new(),
})
        }
    };

    if !table_mode {
        return Ok(ExecResult {
            stdout: if content.ends_with('\n') {
                content
            } else {
                format!("{content}\n")
            },
            stderr: String::new(),
            exit_code: 0,
            env: HashMap::new(),
});
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut max_cols = 0usize;

    for line in &lines {
        let fields: Vec<String> = if let Some(ref sep) = separator {
            line.split(sep.as_str())
                .map(str::to_string)
                .collect()
        } else {
            line.split_whitespace()
                .map(str::to_string)
                .collect()
        };
        if fields.len() > max_cols {
            max_cols = fields.len();
        }
        rows.push(fields);
    }

    let mut col_widths = vec![0usize; max_cols];
    for row in &rows {
        for (ci, field) in row.iter().enumerate() {
            if field.len() > col_widths[ci] {
                col_widths[ci] = field.len();
            }
        }
    }

    let mut stdout = String::new();
    for row in &rows {
        for (ci, field) in row.iter().enumerate() {
            if ci > 0 {
                stdout.push_str("  ");
            }
            if ci + 1 < row.len() {
                let _ = write!(stdout, "{:<width$}", field, width = col_widths[ci]);
            } else {
                stdout.push_str(field);
            }
        }
        stdout.push('\n');
    }

    Ok(ExecResult {
        stdout,
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}
