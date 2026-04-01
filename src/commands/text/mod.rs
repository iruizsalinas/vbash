mod field_ops;
mod transform;
mod compare;
mod format;
mod diff;
mod io;
mod grep;
mod sort;

use std::fmt::Write;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;
use std::collections::HashMap;

pub use field_ops::{cut, tr, paste};
pub use transform::{uniq, rev, tac, fold, column, unexpand_cmd};
pub use transform::expand_cmd as expand;
pub use unexpand_cmd as unexpand;
pub use compare::{comm, join};
pub use format::{nl, od, base64, strings};
pub use diff::diff;
pub use io::{echo, printf, cat, wc, head, tail};
pub use grep::grep_cmd;
pub use sort::sort_cmd;

fn read_input(args: &[&str], ctx: &CommandContext<'_>) -> Result<String, String> {
    if args.is_empty() || (args.len() == 1 && args[0] == "-") {
        Ok(ctx.stdin.to_string())
    } else {
        let path = crate::fs::path::resolve(ctx.cwd, args[0]);
        ctx.fs
            .read_file_string(&path)
            .map_err(|e| format!("{e}"))
    }
}

fn read_input_bytes(args: &[&str], ctx: &CommandContext<'_>) -> Result<Vec<u8>, String> {
    if args.is_empty() || (args.len() == 1 && args[0] == "-") {
        Ok(ctx.stdin.as_bytes().to_vec())
    } else {
        let path = crate::fs::path::resolve(ctx.cwd, args[0]);
        ctx.fs.read_file(&path).map_err(|e| format!("{e}"))
    }
}

enum FieldSpec {
    Single(usize),
    Range(Option<usize>, Option<usize>),
}

fn parse_field_specs(spec: &str) -> Vec<FieldSpec> {
    let mut specs = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if let Some(idx) = part.find('-') {
            let left = part[..idx].trim();
            let right = part[idx + 1..].trim();
            let start = if left.is_empty() {
                None
            } else {
                left.parse::<usize>().ok()
            };
            let end = if right.is_empty() {
                None
            } else {
                right.parse::<usize>().ok()
            };
            specs.push(FieldSpec::Range(start, end));
        } else if let Ok(n) = part.parse::<usize>() {
            specs.push(FieldSpec::Single(n));
        }
    }
    specs
}

fn field_matches(specs: &[FieldSpec], idx: usize) -> bool {
    for spec in specs {
        match spec {
            FieldSpec::Single(n) => {
                if idx == *n {
                    return true;
                }
            }
            FieldSpec::Range(start, end) => {
                let s = start.unwrap_or(1);
                let matches = match end {
                    Some(e) => idx >= s && idx <= *e,
                    None => idx >= s,
                };
                if matches {
                    return true;
                }
            }
        }
    }
    false
}

pub fn tee(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut append = false;
    let mut file_args = Vec::new();

    for arg in args {
        match *arg {
            "-a" | "--append" => append = true,
            _ => file_args.push(*arg),
        }
    }

    let input = ctx.stdin.to_string();

    let mut stderr = String::new();
    let mut exit_code = 0;

    for path in &file_args {
        let resolved = crate::fs::path::resolve(ctx.cwd, path);
        let result = if append {
            ctx.fs.append_file(&resolved, input.as_bytes())
        } else {
            ctx.fs.write_file(&resolved, input.as_bytes())
        };
        if let Err(e) = result {
            let _ = writeln!(stderr, "tee: {e}");
            exit_code = 1;
        }
    }

    Ok(ExecResult {
        stdout: input,
        stderr,
        exit_code,
        env: HashMap::new(),
})
}
