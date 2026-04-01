use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;
use std::collections::HashMap;

pub fn test_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let result = evaluate_test(args, ctx);
    Ok(ExecResult {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: i32::from(!result),
        env: HashMap::new(),
})
}

fn evaluate_test(args: &[&str], ctx: &CommandContext<'_>) -> bool {
    let args = if !args.is_empty() && args[args.len() - 1] == "]" {
        &args[..args.len() - 1]
    } else {
        args
    };
    if args.is_empty() {
        return false;
    }
    let mut pos = 0;
    eval_test_or(args, &mut pos, ctx)
}

fn eval_test_or(args: &[&str], pos: &mut usize, ctx: &CommandContext<'_>) -> bool {
    let mut result = eval_test_and(args, pos, ctx);
    while *pos < args.len() && args[*pos] == "-o" {
        *pos += 1;
        let right = eval_test_and(args, pos, ctx);
        result = result || right;
    }
    result
}

fn eval_test_and(args: &[&str], pos: &mut usize, ctx: &CommandContext<'_>) -> bool {
    let mut result = eval_test_not(args, pos, ctx);
    while *pos < args.len() && args[*pos] == "-a" {
        *pos += 1;
        let right = eval_test_not(args, pos, ctx);
        result = result && right;
    }
    result
}

fn eval_test_not(args: &[&str], pos: &mut usize, ctx: &CommandContext<'_>) -> bool {
    if *pos < args.len() && args[*pos] == "!" {
        *pos += 1;
        return !eval_test_primary(args, pos, ctx);
    }
    eval_test_primary(args, pos, ctx)
}

fn is_unary_test_op(op: &str) -> bool {
    matches!(
        op,
        "-z" | "-n" | "-e" | "-f" | "-d" | "-s" | "-r" | "-w" | "-x" | "-L" | "-h" | "-a"
    )
}

fn is_binary_test_op(op: &str) -> bool {
    matches!(
        op,
        "=" | "==" | "!=" | "-eq" | "-ne" | "-lt" | "-le" | "-gt" | "-ge"
    )
}

fn eval_unary_test(op: &str, operand: &str, ctx: &CommandContext<'_>) -> bool {
    match op {
        "-z" => operand.is_empty(),
        "-n" => !operand.is_empty(),
        "-e" | "-a" => ctx.fs.exists(&crate::fs::path::resolve(ctx.cwd, operand)),
        "-f" => {
            let p = crate::fs::path::resolve(ctx.cwd, operand);
            ctx.fs.stat(&p).is_ok_and(|m| m.is_file())
        }
        "-d" => {
            let p = crate::fs::path::resolve(ctx.cwd, operand);
            ctx.fs.stat(&p).is_ok_and(|m| m.is_dir())
        }
        "-s" => {
            let p = crate::fs::path::resolve(ctx.cwd, operand);
            ctx.fs.stat(&p).is_ok_and(|m| m.size > 0)
        }
        "-r" | "-w" | "-x" => {
            let p = crate::fs::path::resolve(ctx.cwd, operand);
            ctx.fs.exists(&p)
        }
        "-L" | "-h" => {
            let p = crate::fs::path::resolve(ctx.cwd, operand);
            ctx.fs.lstat(&p).is_ok_and(|m| m.is_symlink())
        }
        _ => false,
    }
}

fn eval_binary_test(left: &str, op: &str, right: &str) -> bool {
    match op {
        "=" | "==" => left == right,
        "!=" => left != right,
        "-eq" => left.parse::<i64>().unwrap_or(0) == right.parse::<i64>().unwrap_or(0),
        "-ne" => left.parse::<i64>().unwrap_or(0) != right.parse::<i64>().unwrap_or(0),
        "-lt" => left.parse::<i64>().unwrap_or(0) < right.parse::<i64>().unwrap_or(0),
        "-le" => left.parse::<i64>().unwrap_or(0) <= right.parse::<i64>().unwrap_or(0),
        "-gt" => left.parse::<i64>().unwrap_or(0) > right.parse::<i64>().unwrap_or(0),
        "-ge" => left.parse::<i64>().unwrap_or(0) >= right.parse::<i64>().unwrap_or(0),
        _ => false,
    }
}

fn eval_test_primary(args: &[&str], pos: &mut usize, ctx: &CommandContext<'_>) -> bool {
    if *pos >= args.len() {
        return false;
    }

    if args[*pos] == "(" {
        *pos += 1;
        let result = eval_test_or(args, pos, ctx);
        if *pos < args.len() && args[*pos] == ")" {
            *pos += 1;
        }
        return result;
    }

    if *pos + 1 < args.len() && is_unary_test_op(args[*pos]) {
        let has_binary_ahead = *pos + 2 < args.len() && is_binary_test_op(args[*pos + 1]);
        if !has_binary_ahead {
            let op = args[*pos];
            *pos += 1;
            let operand = args[*pos];
            *pos += 1;
            return eval_unary_test(op, operand, ctx);
        }
    }

    if *pos + 2 < args.len() && is_binary_test_op(args[*pos + 1]) {
        let left = args[*pos];
        *pos += 1;
        let op = args[*pos];
        *pos += 1;
        let right = args[*pos];
        *pos += 1;
        return eval_binary_test(left, op, right);
    }

    let s = args[*pos];
    *pos += 1;
    !s.is_empty()
}
