use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;
use std::collections::HashMap;

pub fn xargs_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let Some(exec_fn) = ctx.exec_fn else {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "xargs: cannot execute subcommand\n".to_string(),
            exit_code: 126,
            env: HashMap::new(),
});
    };

    let mut replace_str: Option<&str> = None;
    let mut max_args: Option<usize> = None;
    let mut delimiter: Option<&str> = None;
    let mut cmd_args = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-I" if i + 1 < args.len() => {
                replace_str = Some(args[i + 1]);
                i += 2;
            }
            "-n" if i + 1 < args.len() => {
                max_args = args[i + 1].parse().ok();
                i += 2;
            }
            "-d" if i + 1 < args.len() => {
                delimiter = Some(args[i + 1]);
                i += 2;
            }
            _ => {
                cmd_args.push(args[i]);
                i += 1;
            }
        }
    }

    let base_cmd = if cmd_args.is_empty() {
        "echo".to_string()
    } else {
        cmd_args.join(" ")
    };

    let input_items: Vec<&str> = if let Some(d) = delimiter {
        if let Some(ch) = parse_delimiter(d) {
            ctx.stdin.split(ch).filter(|s| !s.is_empty()).collect()
        } else {
            ctx.stdin.split_whitespace().collect()
        }
    } else {
        ctx.stdin.split_whitespace().collect()
    };

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code = 0;

    if let Some(repl) = replace_str {
        for item in &input_items {
            let cmd = base_cmd.replace(repl, item);
            match exec_fn(&cmd) {
                Ok(r) => {
                    stdout.push_str(&r.stdout);
                    stderr.push_str(&r.stderr);
                    if r.exit_code != 0 {
                        exit_code = r.exit_code;
                    }
                }
                Err(e) => return Err(e),
            }
        }
    } else if let Some(n) = max_args {
        for chunk in input_items.chunks(n.max(1)) {
            let cmd = format!("{base_cmd} {}", chunk.join(" "));
            match exec_fn(&cmd) {
                Ok(r) => {
                    stdout.push_str(&r.stdout);
                    stderr.push_str(&r.stderr);
                    if r.exit_code != 0 {
                        exit_code = r.exit_code;
                    }
                }
                Err(e) => return Err(e),
            }
        }
    } else if !input_items.is_empty() {
        let cmd = format!("{base_cmd} {}", input_items.join(" "));
        match exec_fn(&cmd) {
            Ok(r) => {
                stdout = r.stdout;
                stderr = r.stderr;
                exit_code = r.exit_code;
            }
            Err(e) => return Err(e),
        }
    }

    Ok(ExecResult { stdout, stderr, exit_code, env: HashMap::new() })
}

fn parse_delimiter(s: &str) -> Option<char> {
    if s.len() == 1 {
        s.chars().next()
    } else if s == "\\n" {
        Some('\n')
    } else if s == "\\t" {
        Some('\t')
    } else if s == "\\0" {
        Some('\0')
    } else {
        s.chars().next()
    }
}
