//! Executes parsed AST nodes against a virtual shell environment.

pub mod state;
mod arithmetic;
mod builtins;
mod builtins_io;
mod compound;
mod dispatch;
mod expand;
mod pattern;

use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::ast::{
    Command, ListOp, Pipeline, RedirectOp,
    RedirectTarget, Script, SimpleCommand, Statement,
    Word, WordPart,
};
use crate::error::{ControlFlow, ExecError, Error, ShellSignal};
use crate::fs::VirtualFs;
use crate::{ExecResult, ExecutionLimits};
use crate::commands::CommandRegistry;
use state::ShellState;

use pattern::glob_match;

/// Internal execution output before conversion to the public `ExecResult`.
struct ExecOutput {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

type InterpResult = Result<ExecOutput, ShellSignal>;

/// Execute a parsed script against the given filesystem and configuration.
#[allow(clippy::too_many_arguments)]
pub(crate) fn execute(
    script: &Script,
    fs: &dyn VirtualFs,
    env: &HashMap<String, String>,
    cwd: &str,
    limits: &ExecutionLimits,
    registry: &CommandRegistry,
    stdin: &str,
    cancel: Option<Arc<AtomicBool>>,
    #[cfg(feature = "network")] network_policy: Option<&crate::NetworkPolicy>,
) -> Result<ExecResult, Error> {
    let mut state = ShellState::new(cwd.to_string());
    for (k, v) in env {
        let _ = state.set_var(k, v.clone());
    }

    let mut interp = Interpreter {
        fs,
        state,
        limits,
        registry,
        stdin: stdin.to_string(),
        cancel,
        #[cfg(feature = "network")]
        network_policy,
    };

    let result = match interp.execute_script(script) {
        Ok(out) => Ok(ExecResult {
            stdout: out.stdout,
            stderr: out.stderr,
            exit_code: out.exit_code,
            env: interp.state.env.clone(),
        }),
        Err(ShellSignal::Flow(
            ControlFlow::Exit { code, stdout, stderr }
            | ControlFlow::Return { code, stdout, stderr },
        )) => {
            Ok(ExecResult {
                stdout,
                stderr,
                exit_code: code,
                env: interp.state.env.clone(),
            })
        }
        Err(ShellSignal::Flow(cf)) => {
            Err(Error::Exec(ExecError::Other(format!("unexpected control flow: {cf:?}"))))
        }
        Err(ShellSignal::Error(e)) => Err(e),
    };

    if let Some(trap_cmd) = interp.state.traps.get("EXIT").cloned() {
        if let Ok(trap_script) = crate::parser::parse(&trap_cmd) {
            if let Ok(mut r) = result {
                if let Ok(trap_out) = interp.execute_script(&trap_script) {
                    r.stdout.push_str(&trap_out.stdout);
                    r.stderr.push_str(&trap_out.stderr);
                }
                return Ok(r);
            }
            return result;
        }
    }

    result
}

pub(super) struct Interpreter<'a> {
    pub(super) fs: &'a dyn VirtualFs,
    pub(super) state: ShellState,
    pub(super) limits: &'a ExecutionLimits,
    pub(super) registry: &'a CommandRegistry,
    pub(super) stdin: String,
    pub(super) cancel: Option<Arc<AtomicBool>>,
    #[cfg(feature = "network")]
    pub(super) network_policy: Option<&'a crate::NetworkPolicy>,
}

impl Interpreter<'_> {
    fn check_output_size(&self, stdout: &str, stderr: &str) -> Result<(), ShellSignal> {
        if stdout.len() + stderr.len() > self.limits.max_output_size {
            return Err(crate::error::LimitKind::OutputSize.into());
        }
        Ok(())
    }

    fn execute_script(&mut self, script: &Script) -> InterpResult {
        let mut stdout = String::new();
        let mut stderr = String::new();
        let mut exit_code = 0;

        for stmt in &script.statements {
            let out = self.execute_statement(stmt)?;
            stdout.push_str(&out.stdout);
            stderr.push_str(&out.stderr);
            self.check_output_size(&stdout, &stderr)?;
            exit_code = out.exit_code;
            self.state.last_exit_code = exit_code;
        }

        Ok(ExecOutput {
            stdout,
            stderr,
            exit_code,
        })
    }

    fn execute_statement(&mut self, stmt: &Statement) -> InterpResult {
        let mut stdout = String::new();
        let mut stderr = String::new();
        let mut exit_code = 0;

        for (i, pipeline) in stmt.pipelines.iter().enumerate() {
            self.check_command_limit()?;

            if i > 0 {
                let op = &stmt.operators[i - 1];
                match op {
                    ListOp::And if exit_code != 0 => continue,
                    ListOp::Or if exit_code == 0 => continue,
                    _ => {}
                }
            }

            let out = self.execute_pipeline(pipeline)?;
            stdout.push_str(&out.stdout);
            stderr.push_str(&out.stderr);
            self.check_output_size(&stdout, &stderr)?;
            exit_code = out.exit_code;
            self.state.last_exit_code = exit_code;

            if self.state.options.errexit && exit_code != 0 && !self.state.in_condition {
                let in_chain = !stmt.operators.is_empty();
                if !in_chain {
                    return Err(ShellSignal::Flow(ControlFlow::Exit {
                        code: exit_code,
                        stdout,
                        stderr,
                    }));
                }
            }
        }

        if stmt.background {
            self.state.pid += 1;
            let bg_pid = self.state.pid;
            let _ = self.state.set_var("!", bg_pid.to_string());
            writeln!(stderr, "[1] {bg_pid}").ok();
        }

        Ok(ExecOutput {
            stdout,
            stderr,
            exit_code,
        })
    }

    fn execute_pipeline(&mut self, pipeline: &Pipeline) -> InterpResult {
        let start = if pipeline.timed { Some(std::time::Instant::now()) } else { None };

        let mut result = self.execute_pipeline_inner(pipeline)?;

        if let Some(start) = start {
            let elapsed = start.elapsed();
            let secs = elapsed.as_secs();
            let millis = elapsed.subsec_millis();
            write!(result.stderr, "\nreal\t0m{secs}.{millis:03}s\n").ok();
        }

        Ok(result)
    }

    fn execute_pipeline_inner(&mut self, pipeline: &Pipeline) -> InterpResult {
        if pipeline.commands.len() == 1 {
            let out = self.execute_command(&pipeline.commands[0], "")?;
            let exit_code = if pipeline.negated {
                i32::from(out.exit_code == 0)
            } else {
                out.exit_code
            };
            return Ok(ExecOutput {
                stdout: out.stdout,
                stderr: out.stderr,
                exit_code,
            });
        }

        let mut prev_stdout = String::new();
        let mut all_stderr = String::new();
        let mut last_exit_code = 0;
        let mut pipe_exit_codes = Vec::new();

        for (i, cmd) in pipeline.commands.iter().enumerate() {
            let stdin = if i == 0 { "" } else { &prev_stdout };
            let out = self.execute_command(cmd, stdin)?;

            if i < pipeline.commands.len() - 1 {
                if i < pipeline.pipe_stderr.len() && pipeline.pipe_stderr[i] {
                    prev_stdout = format!("{}{}", out.stdout, out.stderr);
                } else {
                    all_stderr.push_str(&out.stderr);
                    prev_stdout = out.stdout;
                }
            } else {
                all_stderr.push_str(&out.stderr);
                prev_stdout = out.stdout;
            }

            pipe_exit_codes.push(out.exit_code);
            last_exit_code = out.exit_code;
            self.check_output_size(&prev_stdout, &all_stderr)?;
        }

        let exit_code = if self.state.options.pipefail {
            pipe_exit_codes
                .iter()
                .rev()
                .find(|&&c| c != 0)
                .copied()
                .unwrap_or(0)
        } else {
            last_exit_code
        };

        let exit_code = if pipeline.negated {
            i32::from(exit_code == 0)
        } else {
            exit_code
        };

        Ok(ExecOutput {
            stdout: prev_stdout,
            stderr: all_stderr,
            exit_code,
        })
    }

    fn execute_command(&mut self, cmd: &Command, stdin: &str) -> InterpResult {
        match cmd {
            Command::Simple(sc) => self.execute_simple_command(sc, stdin),
            Command::Compound(cc) => self.execute_compound_command(cc, stdin),
            Command::FunctionDef(fd) => {
                self.state.functions.insert(fd.name.clone(), fd.clone());
                Ok(ExecOutput {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: 0,
                })
            }
        }
    }

    fn execute_simple_command(&mut self, sc: &SimpleCommand, stdin: &str) -> InterpResult {
        if sc.name.is_none() {
            for assign in &sc.assignments {
                if let Some(ref elements) = assign.array {
                    let mut values = Vec::new();
                    for w in elements {
                        values.push(self.expand_word(w)?);
                    }
                    if values.len() > self.limits.max_array_elements {
                        return Err(crate::error::LimitKind::ArrayElements.into());
                    }
                    if assign.append {
                        if let Some(existing) = self.state.arrays.get_mut(&assign.name) {
                            existing.extend(values);
                            if existing.len() > self.limits.max_array_elements {
                                return Err(crate::error::LimitKind::ArrayElements.into());
                            }
                        } else {
                            self.state.set_array(&assign.name, values);
                        }
                    } else {
                        self.state.set_array(&assign.name, values);
                    }
                    continue;
                }

                let value = if let Some(ref word) = assign.value {
                    self.expand_word(word)?
                } else {
                    String::new()
                };

                if let Some((arr_name, idx_str)) = parse_array_index(&assign.name) {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        self.state.set_array_element(&arr_name, idx, value, self.limits.max_array_elements)
                            .map_err(|e| ShellSignal::Error(Error::Exec(ExecError::Other(e))))?;
                    }
                } else if assign.append {
                    if self.state.arrays.contains_key(&assign.name) {
                        if self.state.array_len(&assign.name) >= self.limits.max_array_elements {
                            return Err(crate::error::LimitKind::ArrayElements.into());
                        }
                        self.state.array_push(&assign.name, value);
                    } else {
                        let old = self.state.get_var(&assign.name).unwrap_or("").to_string();
                        self.state.set_var(&assign.name, format!("{old}{value}"))
                            .map_err(|e| ShellSignal::Error(Error::Exec(ExecError::Other(e))))?;
                    }
                } else {
                    self.state.set_var(&assign.name, value)
                        .map_err(|e| ShellSignal::Error(Error::Exec(ExecError::Other(e))))?;
                }
            }
            return Ok(ExecOutput {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 0,
            });
        }

        let Some(name_word) = sc.name.as_ref() else {
            return Ok(ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 0 });
        };
        let name = self.expand_word(name_word)?;
        let mut args = Vec::new();
        for arg in &sc.args {
            let expanded = self.expand_word_splitting(arg)?;
            args.extend(expanded);
        }

        let saved_env: Vec<(String, Option<String>)> = sc
            .assignments
            .iter()
            .map(|a| {
                let old = self.state.get_var(&a.name).map(String::from);
                let value = a.value.as_ref().map_or_else(
                    || Ok(String::new()),
                    |w| self.expand_word(w),
                );
                if let Ok(v) = value {
                    let _ = self.state.set_var(&a.name, v);
                }
                (a.name.clone(), old)
            })
            .collect();

        let effective_stdin = if stdin.is_empty() && !self.stdin.is_empty() {
            self.stdin.clone()
        } else {
            stdin.to_string()
        };
        let mut redir_stdin = effective_stdin;

        let mut stdout_redirects: Vec<(Option<u32>, String, bool)> = Vec::new();
        let mut dup_redirects: Vec<(u32, String)> = Vec::new();
        let mut output_all: Option<(String, bool)> = None;

        for redir in &sc.redirections {
            match &redir.target {
                RedirectTarget::Word(w) => {
                    let target = self.expand_word(w)?;
                    match redir.operator {
                        RedirectOp::Input => {
                            redir_stdin = self.fs.read_file_string(&self.resolve_path(&target))
                                .map_err(|e| ShellSignal::Error(Error::Fs(e)))?;
                        }
                        RedirectOp::Output | RedirectOp::Clobber => {
                            stdout_redirects.push((redir.fd, target, false));
                        }
                        RedirectOp::Append => {
                            stdout_redirects.push((redir.fd, target, true));
                        }
                        RedirectOp::OutputAll => {
                            output_all = Some((target, false));
                        }
                        RedirectOp::AppendAll => {
                            output_all = Some((target, true));
                        }
                        RedirectOp::HereString => {
                            redir_stdin = format!("{target}\n");
                        }
                        RedirectOp::DupOutput => {
                            let fd = redir.fd.unwrap_or(1);
                            dup_redirects.push((fd, target));
                        }
                        _ => {}
                    }
                }
                RedirectTarget::HereDoc(heredoc) => {
                    let content = if heredoc.quoted {
                        word_to_literal(&heredoc.content)
                    } else {
                        self.expand_word(&heredoc.content)?
                    };
                    redir_stdin = content;
                }
            }
        }

        let mut xtrace_line = String::new();
        if self.state.options.xtrace {
            if args.is_empty() {
                xtrace_line = format!("+ {name}\n");
            } else {
                xtrace_line = format!("+ {name} {}\n", args.join(" "));
            }
        }

        let result = self.dispatch_command(&name, &args, &redir_stdin);

        if !sc.assignments.is_empty() && sc.name.is_some() {
            for (name, old) in saved_env {
                if let Some(v) = old {
                    let _ = self.state.set_var(&name, v);
                } else {
                    let _ = self.state.unset_var(&name);
                }
            }
        }

        let mut out = result?;

        if !xtrace_line.is_empty() {
            out.stderr = format!("{xtrace_line}{}", out.stderr);
        }

        for (fd, target) in &dup_redirects {
            match (*fd, target.as_str()) {
                (2, "1") => {
                    out.stdout.push_str(&out.stderr);
                    out.stderr = String::new();
                }
                (1, "2") => {
                    out.stderr.push_str(&out.stdout);
                    out.stdout = String::new();
                }
                _ => {}
            }
        }

        if let Some((path, append)) = output_all {
            let resolved = self.resolve_path(&path);
            let combined = format!("{}{}", out.stdout, out.stderr);
            if append {
                self.fs.append_file(&resolved, combined.as_bytes())
                    .map_err(|e| ShellSignal::Error(Error::Fs(e)))?;
            } else {
                let parent = crate::fs::path::parent(&resolved);
                if parent != "/" {
                    let _ = self.fs.mkdir(parent, true);
                }
                self.fs.write_file(&resolved, combined.as_bytes())
                    .map_err(|e| ShellSignal::Error(Error::Fs(e)))?;
            }
            return Ok(ExecOutput {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: out.exit_code,
            });
        }

        let mut has_stdout_redir = false;
        let mut has_stderr_redir = false;
        for (fd, path, append) in &stdout_redirects {
            let effective_fd = fd.unwrap_or(1);
            let resolved = self.resolve_path(path);
            let content = if effective_fd == 2 {
                has_stderr_redir = true;
                &out.stderr
            } else {
                has_stdout_redir = true;
                &out.stdout
            };
            if *append {
                self.fs.append_file(&resolved, content.as_bytes())
                    .map_err(|e| ShellSignal::Error(Error::Fs(e)))?;
            } else {
                let parent = crate::fs::path::parent(&resolved);
                if parent != "/" {
                    let _ = self.fs.mkdir(parent, true);
                }
                self.fs.write_file(&resolved, content.as_bytes())
                    .map_err(|e| ShellSignal::Error(Error::Fs(e)))?;
            }
        }

        if has_stdout_redir || has_stderr_redir {
            return Ok(ExecOutput {
                stdout: if has_stdout_redir { String::new() } else { out.stdout },
                stderr: if has_stderr_redir { String::new() } else { out.stderr },
                exit_code: out.exit_code,
            });
        }

        Ok(out)
    }
}

fn is_shell_builtin(name: &str) -> bool {
    matches!(
        name,
        "cd" | "echo" | "printf" | "read" | "export" | "declare" | "local"
            | "set" | "unset" | "eval" | "source" | "." | "exit" | "return"
            | "shift" | "test" | "[" | "true" | "false" | "type" | "command"
            | "builtin" | "alias" | "unalias" | "getopts" | "hash"
            | "pushd" | "popd" | "dirs" | "pwd" | "env" | "printenv"
            | "break" | "continue" | "trap" | "shopt"
            | "mapfile" | "readarray"
            | "bash" | "sh" | "clear" | "help"
            | ":" | "let"
    )
}

fn contains_glob_chars(s: &str) -> bool {
    let bytes = s.as_bytes();
    for i in 0..bytes.len() {
        match bytes[i] {
            b'*' | b'?' | b'[' => return true,
            b'@' | b'+' | b'!' if i + 1 < bytes.len() && bytes[i + 1] == b'(' => return true,
            _ => {}
        }
    }
    false
}

fn parse_array_index(name: &str) -> Option<(String, String)> {
    let open = name.find('[')?;
    let close = name.find(']')?;
    if close <= open {
        return None;
    }
    let arr_name = name[..open].to_string();
    let idx_str = name[open + 1..close].to_string();
    Some((arr_name, idx_str))
}

fn word_to_literal(word: &Word) -> String {
    let mut result = String::new();
    for part in &word.parts {
        word_part_to_literal(part, &mut result);
    }
    result
}

fn word_part_to_literal(part: &WordPart, result: &mut String) {
    match part {
        WordPart::Literal(s) | WordPart::SingleQuoted(s) => result.push_str(s),
        WordPart::DoubleQuoted(parts) => {
            for p in parts {
                word_part_to_literal(p, result);
            }
        }
        WordPart::Escaped(c) => result.push(*c),
        WordPart::Parameter(p) => {
            result.push('$');
            result.push_str(&p.name);
        }
        WordPart::CommandSubstitution(cmd) => {
            result.push_str("$(");
            result.push_str(cmd);
            result.push(')');
        }
        WordPart::ArithmeticExpansion(_) => {
            result.push_str("$((...))");
        }
        _ => {}
    }
}
