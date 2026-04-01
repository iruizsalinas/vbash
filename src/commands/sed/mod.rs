mod types;
mod parser;
mod exec;

use std::fmt::Write;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::{ExecError, Error};

use types::SedProgram;
use parser::parse_script;
use exec::{ExecState, execute_line};
use std::collections::HashMap;

fn sed_error(msg: impl Into<String>) -> Error {
    Error::Exec(ExecError::Other(msg.into()))
}

fn run_sed(
    program: &SedProgram,
    input: &str,
    fs: &dyn crate::fs::VirtualFs,
    cwd: &str,
) -> Result<(String, i32), Error> {
    let lines: Vec<&str> = input.lines().collect();
    let total = lines.len();

    let mut st = ExecState {
        pattern_space: String::new(),
        hold_space: String::new(),
        line_num: 0,
        is_last_line: false,
        sub_success: false,
        output: String::new(),
        use_ere: program.use_ere,
        append_text: Vec::new(),
        range_active: vec![false; program.commands.len()],
    };

    let mut line_idx = 0;
    let mut exit_code = 0;

    while line_idx < total {
        st.line_num = line_idx + 1;
        st.is_last_line = line_idx == total - 1;
        st.pattern_space = lines[line_idx].to_string();
        st.sub_success = false;
        st.append_text.clear();

        if let Some(code) = execute_line(program, &mut st, &lines, &mut line_idx, fs, cwd)? {
            exit_code = code;
            break;
        }

        line_idx += 1;
    }

    Ok((st.output, exit_code))
}

pub fn sed_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut suppress_print = false;
    let mut use_ere = false;
    let mut in_place = false;
    let mut scripts: Vec<String> = Vec::new();
    let mut script_files: Vec<String> = Vec::new();
    let mut file_args: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-n" | "--quiet" | "--silent" => suppress_print = true,
            "-E" | "-r" | "--regexp-extended" => use_ere = true,
            "-i" | "--in-place" => in_place = true,
            "-e" => {
                if i + 1 < args.len() {
                    scripts.push(args[i + 1].to_string());
                    i += 1;
                } else {
                    return Err(sed_error("sed: option requires an argument -- 'e'"));
                }
            }
            "-f" => {
                if i + 1 < args.len() {
                    script_files.push(args[i + 1].to_string());
                    i += 1;
                } else {
                    return Err(sed_error("sed: option requires an argument -- 'f'"));
                }
            }
            arg if arg.starts_with("-ne") || arg.starts_with("-nE") => {
                suppress_print = true;
                if arg.contains('E') { use_ere = true; }
                if arg.len() > 3 {
                    scripts.push(arg[3..].to_string());
                } else if i + 1 < args.len() {
                    scripts.push(args[i + 1].to_string());
                    i += 1;
                }
            }
            arg if arg.starts_with('-') && arg.len() > 1 => {
                let flags_str = &arg[1..];
                for (fi, ch) in flags_str.chars().enumerate() {
                    match ch {
                        'n' => suppress_print = true,
                        'E' | 'r' => use_ere = true,
                        'i' => in_place = true,
                        'e' => {
                            let rest = &flags_str[fi + 1..];
                            if !rest.is_empty() {
                                scripts.push(rest.to_string());
                            } else if i + 1 < args.len() {
                                scripts.push(args[i + 1].to_string());
                                i += 1;
                            }
                            break;
                        }
                        'f' => {
                            let rest = &flags_str[fi + 1..];
                            if !rest.is_empty() {
                                script_files.push(rest.to_string());
                            } else if i + 1 < args.len() {
                                script_files.push(args[i + 1].to_string());
                                i += 1;
                            }
                            break;
                        }
                        _ => {
                            file_args.push(arg.to_string());
                            break;
                        }
                    }
                }
            }
            _ => {
                if scripts.is_empty() && script_files.is_empty() {
                    scripts.push(args[i].to_string());
                } else {
                    file_args.push(args[i].to_string());
                }
            }
        }
        i += 1;
    }

    for sf in &script_files {
        let path = crate::fs::path::resolve(ctx.cwd, sf);
        let content = ctx.fs.read_file_string(&path).map_err(|e| {
            sed_error(format!("sed: couldn't open file {sf}: {e}"))
        })?;
        scripts.push(content);
    }

    if scripts.is_empty() {
        return Err(sed_error("sed: no script command"));
    }

    let combined_script = scripts.join("\n");
    let commands = parse_script(&combined_script)?;

    let program = SedProgram {
        commands,
        suppress_print,
        use_ere,
    };

    if in_place {
        if file_args.is_empty() {
            return Err(sed_error("sed: no input files for in-place editing"));
        }

        let mut stderr = String::new();
        let mut exit_code = 0;

        for file in &file_args {
            let path = crate::fs::path::resolve(ctx.cwd, file);
            let content = match ctx.fs.read_file_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    let _ = writeln!(stderr, "sed: {e}");
                    exit_code = 1;
                    continue;
                }
            };

            let (output, code) = run_sed(&program, &content, ctx.fs, ctx.cwd)?;
            if code != 0 {
                exit_code = code;
            }

            let trimmed = output.strip_suffix('\n').unwrap_or(&output);
            if let Err(e) = ctx.fs.write_file(&path, trimmed.as_bytes()) {
                let _ = writeln!(stderr, "sed: {e}");
                exit_code = 1;
            }
        }

        return Ok(ExecResult { stdout: String::new(), stderr, exit_code, env: HashMap::new() });
    }

    let input = if file_args.is_empty() {
        ctx.stdin.to_string()
    } else {
        let mut combined = String::new();
        for file in &file_args {
            let path = crate::fs::path::resolve(ctx.cwd, file);
            let content = ctx.fs.read_file_string(&path).map_err(|e| {
                sed_error(format!("sed: {e}"))
            })?;
            combined.push_str(&content);
        }
        combined
    };

    let (output, exit_code) = run_sed(&program, &input, ctx.fs, ctx.cwd)?;

    Ok(ExecResult {
        stdout: output,
        stderr: String::new(),
        exit_code,
        env: HashMap::new(),
})
}
