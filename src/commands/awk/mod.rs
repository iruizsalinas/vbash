mod ast;
mod builtins;
mod eval;
mod expr;
mod lexer;
mod parser;
mod value;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::{ExecError, Error};

use eval::Interpreter;
use lexer::lex;
use parser::parse;
use std::collections::HashMap;

pub fn awk_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut fs = " ".to_string();
    let mut pre_vars: Vec<(String, String)> = Vec::new();
    let mut program_src: Option<&str> = None;
    let mut file_args: Vec<&str> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-F" if i + 1 < args.len() => {
                fs = args[i + 1].to_string();
                i += 2;
            }
            a if a.starts_with("-F") => {
                fs = a[2..].to_string();
                i += 1;
            }
            "-v" if i + 1 < args.len() => {
                if let Some(eq) = args[i + 1].find('=') {
                    let name = args[i + 1][..eq].to_string();
                    let val = args[i + 1][eq + 1..].to_string();
                    pre_vars.push((name, val));
                }
                i += 2;
            }
            a if a.starts_with("-v") && a.contains('=') => {
                let rest = &a[2..];
                if let Some(eq) = rest.find('=') {
                    let name = rest[..eq].to_string();
                    let val = rest[eq + 1..].to_string();
                    pre_vars.push((name, val));
                }
                i += 1;
            }
            _ => {
                if program_src.is_none() {
                    program_src = Some(args[i]);
                } else {
                    file_args.push(args[i]);
                }
                i += 1;
            }
        }
    }

    let Some(src) = program_src else {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "awk: no program given\n".to_string(),
            exit_code: 1,
            env: HashMap::new(),
});
    };

    let tokens = lex(src).map_err(|e| awk_err(&e))?;
    let program = parse(&tokens).map_err(|e| awk_err(&e))?;

    let input = if file_args.is_empty() {
        ctx.stdin.to_string()
    } else {
        let mut combined = String::new();
        for path in &file_args {
            let resolved = crate::fs::path::resolve(ctx.cwd, path);
            match ctx.fs.read_file_string(&resolved) {
                Ok(content) => combined.push_str(&content),
                Err(e) => {
                    return Ok(ExecResult {
                        stdout: String::new(),
                        stderr: format!("awk: {e}\n"),
                        exit_code: 2,
                        env: HashMap::new(),
});
                }
            }
        }
        combined
    };

    let mut interp = Interpreter::new(fs, pre_vars, &program);

    interp.run(&input, &program).map_err(|e| awk_err(&e))?;

    Ok(ExecResult {
        stdout: interp.output,
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}

fn awk_err(msg: &str) -> Error {
    Error::Exec(ExecError::Other(format!("awk: {msg}")))
}
