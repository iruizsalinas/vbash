use std::fmt::Write;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;
use std::collections::HashMap;

pub fn expr_cmd(args: &[&str], _ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    if args.is_empty() {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "expr: missing operand\n".to_string(),
            exit_code: 2,
            env: HashMap::new(),
});
    }

    match eval_expr(args) {
        Ok(val) => {
            let exit_code = i32::from(val == "0" || val.is_empty());
            Ok(ExecResult {
                stdout: format!("{val}\n"),
                stderr: String::new(),
                exit_code,
                env: HashMap::new(),
})
        }
        Err(msg) => Ok(ExecResult {
            stdout: String::new(),
            stderr: format!("expr: {msg}\n"),
            exit_code: 2,
            env: HashMap::new(),
}),
    }
}

fn eval_expr(args: &[&str]) -> Result<String, String> {
    let (val, rest) = parse_or(args)?;
    if rest.is_empty() {
        Ok(val)
    } else {
        Err("syntax error".to_string())
    }
}

fn parse_or<'a>(args: &'a [&str]) -> Result<(String, &'a [&'a str]), String> {
    let (mut left, mut rest) = parse_and(args)?;
    while !rest.is_empty() && rest[0] == "|" {
        let (right, r) = parse_and(&rest[1..])?;
        left = if is_null_or_zero(&left) {
            if is_null_or_zero(&right) { "0".to_string() } else { right }
        } else {
            left
        };
        rest = r;
    }
    Ok((left, rest))
}

fn parse_and<'a>(args: &'a [&str]) -> Result<(String, &'a [&'a str]), String> {
    let (mut left, mut rest) = parse_compare(args)?;
    while !rest.is_empty() && rest[0] == "&" {
        let (right, r) = parse_compare(&rest[1..])?;
        left = if is_null_or_zero(&left) || is_null_or_zero(&right) {
            "0".to_string()
        } else {
            left
        };
        rest = r;
    }
    Ok((left, rest))
}

fn parse_compare<'a>(args: &'a [&str]) -> Result<(String, &'a [&'a str]), String> {
    let (left, rest) = parse_add(args)?;
    if rest.is_empty() {
        return Ok((left, rest));
    }
    match rest[0] {
        "=" | "==" | "!=" | "<" | ">" | "<=" | ">=" => {
            let op = rest[0];
            let (right, rest2) = parse_add(&rest[1..])?;
            let li = left.parse::<i64>();
            let ri = right.parse::<i64>();
            let result = if let (Ok(l), Ok(r)) = (li, ri) {
                match op {
                    "=" | "==" => l == r,
                    "!=" => l != r,
                    "<" => l < r,
                    ">" => l > r,
                    "<=" => l <= r,
                    ">=" => l >= r,
                    _ => false,
                }
            } else {
                match op {
                    "=" | "==" => left == right,
                    "!=" => left != right,
                    "<" => left < right,
                    ">" => left > right,
                    "<=" => left <= right,
                    ">=" => left >= right,
                    _ => false,
                }
            };
            Ok((if result { "1".to_string() } else { "0".to_string() }, rest2))
        }
        _ => Ok((left, rest)),
    }
}

fn parse_add<'a>(args: &'a [&str]) -> Result<(String, &'a [&'a str]), String> {
    let (mut left, mut rest) = parse_mul(args)?;
    while !rest.is_empty() && (rest[0] == "+" || rest[0] == "-") {
        let op = rest[0];
        let (right, r) = parse_mul(&rest[1..])?;
        let l: i64 = left.parse().map_err(|_| "non-integer argument".to_string())?;
        let rr: i64 = right.parse().map_err(|_| "non-integer argument".to_string())?;
        left = match op {
            "+" => (l + rr).to_string(),
            "-" => (l - rr).to_string(),
            _ => left,
        };
        rest = r;
    }
    Ok((left, rest))
}

fn parse_mul<'a>(args: &'a [&str]) -> Result<(String, &'a [&'a str]), String> {
    let (mut left, mut rest) = parse_string_ops(args)?;
    while !rest.is_empty() && (rest[0] == "*" || rest[0] == "/" || rest[0] == "%") {
        let op = rest[0];
        let (right, r) = parse_string_ops(&rest[1..])?;
        let l: i64 = left.parse().map_err(|_| "non-integer argument".to_string())?;
        let rr: i64 = right.parse().map_err(|_| "non-integer argument".to_string())?;
        if (op == "/" || op == "%") && rr == 0 {
            return Err("division by zero".to_string());
        }
        left = match op {
            "*" => (l * rr).to_string(),
            "/" => (l / rr).to_string(),
            "%" => (l % rr).to_string(),
            _ => left,
        };
        rest = r;
    }
    Ok((left, rest))
}

fn parse_string_ops<'a>(args: &'a [&str]) -> Result<(String, &'a [&'a str]), String> {
    if args.is_empty() {
        return Err("syntax error".to_string());
    }
    match args[0] {
        "match" if args.len() >= 3 => {
            let s = args[1];
            let pat = args[2];
            let re = regex::Regex::new(pat).map_err(|e| format!("invalid regex: {e}"))?;
            let result = if let Some(m) = re.captures(s) {
                if let Some(g) = m.get(1) {
                    g.as_str().to_string()
                } else {
                    m.get(0).map_or(0, |m| m.as_str().len()).to_string()
                }
            } else {
                "0".to_string()
            };
            Ok((result, &args[3..]))
        }
        "substr" if args.len() >= 4 => {
            let s = args[1];
            let pos: usize = args[2].parse().unwrap_or(0);
            let len: usize = args[3].parse().unwrap_or(0);
            let start = pos.saturating_sub(1);
            let chars: Vec<char> = s.chars().collect();
            let end = (start + len).min(chars.len());
            let result: String = chars.get(start..end).map_or(String::new(), |c| c.iter().collect());
            Ok((result, &args[4..]))
        }
        "index" if args.len() >= 3 => {
            let s = args[1];
            let chars = args[2];
            let mut pos = 0usize;
            for (i, c) in s.chars().enumerate() {
                if chars.contains(c) {
                    pos = i + 1;
                    break;
                }
            }
            Ok((pos.to_string(), &args[3..]))
        }
        "length" if args.len() >= 2 => {
            let len = args[1].len();
            Ok((len.to_string(), &args[2..]))
        }
        _ => parse_atom(args),
    }
}

fn parse_atom<'a>(args: &'a [&str]) -> Result<(String, &'a [&'a str]), String> {
    if args.is_empty() {
        return Err("syntax error".to_string());
    }
    if args[0] == "(" {
        let close = args.iter().position(|a| *a == ")").ok_or("syntax error")?;
        let (val, inner_rest) = parse_or(&args[1..close])?;
        if !inner_rest.is_empty() {
            return Err("syntax error".to_string());
        }
        Ok((val, &args[close + 1..]))
    } else {
        Ok((args[0].to_string(), &args[1..]))
    }
}

fn is_null_or_zero(s: &str) -> bool {
    s.is_empty() || s == "0"
}

pub fn bc_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let _ = args;
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code = 0;

    for line in ctx.stdin.lines() {
        let line = line.trim();
        if line.is_empty() || line == "quit" {
            continue;
        }
        match eval_bc_line(line) {
            Ok(val) => {
                let _ = writeln!(stdout, "{val}");
            }
            Err(e) => {
                let _ = writeln!(stderr, "(standard_in) 1: {e}");
                exit_code = 1;
            }
        }
    }

    Ok(ExecResult { stdout, stderr, exit_code, env: HashMap::new() })
}

fn eval_bc_line(line: &str) -> Result<String, String> {
    let tokens = tokenize_bc(line)?;
    let (val, rest) = bc_parse_add(&tokens)?;
    if rest.is_empty() {
        Ok(format_bc_number(val))
    } else {
        Err("parse error".to_string())
    }
}

#[derive(Debug, Clone)]
enum BcToken {
    Num(f64),
    Op(char),
    LParen,
    RParen,
}

fn tokenize_bc(s: &str) -> Result<Vec<BcToken>, String> {
    let mut tokens = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if bytes[i].is_ascii_digit() || bytes[i] == b'.' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            let s = std::str::from_utf8(&bytes[start..i]).map_err(|_| "invalid number".to_string())?;
            let n: f64 = s.parse().map_err(|_| "invalid number".to_string())?;
            tokens.push(BcToken::Num(n));
        } else if bytes[i] == b'-' && (tokens.is_empty() || matches!(tokens.last(), Some(BcToken::Op(_) | BcToken::LParen))) {
            i += 1;
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            if start == i {
                return Err("parse error".to_string());
            }
            let s = std::str::from_utf8(&bytes[start..i]).map_err(|_| "invalid number".to_string())?;
            let n: f64 = s.parse().map_err(|_| "invalid number".to_string())?;
            tokens.push(BcToken::Num(-n));
        } else {
            match bytes[i] {
                b'+' | b'-' | b'*' | b'/' | b'%' => {
                    tokens.push(BcToken::Op(bytes[i] as char));
                }
                b'(' => tokens.push(BcToken::LParen),
                b')' => tokens.push(BcToken::RParen),
                _ => return Err("parse error".to_string()),
            }
            i += 1;
        }
    }
    Ok(tokens)
}

fn bc_parse_add(tokens: &[BcToken]) -> Result<(f64, &[BcToken]), String> {
    let (mut left, mut rest) = bc_parse_mul(tokens)?;
    while let Some(BcToken::Op(op @ ('+' | '-'))) = rest.first() {
        let op = *op;
        let (right, r) = bc_parse_mul(&rest[1..])?;
        left = if op == '+' { left + right } else { left - right };
        rest = r;
    }
    Ok((left, rest))
}

fn bc_parse_mul(tokens: &[BcToken]) -> Result<(f64, &[BcToken]), String> {
    let (mut left, mut rest) = bc_parse_atom(tokens)?;
    while let Some(BcToken::Op(op @ ('*' | '/' | '%'))) = rest.first() {
        let op = *op;
        let (right, r) = bc_parse_atom(&rest[1..])?;
        if (op == '/' || op == '%') && right == 0.0 {
            return Err("divide by zero".to_string());
        }
        left = match op {
            '*' => left * right,
            '/' => left / right,
            '%' => left % right,
            _ => left,
        };
        rest = r;
    }
    Ok((left, rest))
}

fn bc_parse_atom(tokens: &[BcToken]) -> Result<(f64, &[BcToken]), String> {
    match tokens.first() {
        Some(BcToken::Num(n)) => Ok((*n, &tokens[1..])),
        Some(BcToken::LParen) => {
            let (val, rest) = bc_parse_add(&tokens[1..])?;
            match rest.first() {
                Some(BcToken::RParen) => Ok((val, &rest[1..])),
                _ => Err("parse error".to_string()),
            }
        }
        _ => Err("parse error".to_string()),
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn format_bc_number(val: f64) -> String {
    if (val - val.trunc()).abs() < f64::EPSILON && val.abs() < i64::MAX as f64 {
        format!("{}", val as i64)
    } else {
        let s = format!("{val:.10}");
        let s = s.trim_end_matches('0');
        let s = s.strip_suffix('.').unwrap_or(s);
        s.to_string()
    }
}
