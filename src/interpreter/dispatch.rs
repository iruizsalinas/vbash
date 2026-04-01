use std::collections::HashMap;

use crate::ast::FunctionDef;
use crate::commands::{CommandContext, CommandFn};
use crate::error::{ControlFlow, Error, ShellSignal};

use super::{ExecOutput, InterpResult, Interpreter};

impl Interpreter<'_> {
    pub(super) fn dispatch_command(&mut self, name: &str, args: &[String], stdin: &str) -> InterpResult {
        let args_refs: Vec<&str> = args.iter().map(String::as_str).collect();

        match name {
            "cd" => return Ok(self.builtin_cd(&args_refs)),
            "export" => return Ok(self.builtin_export(&args_refs)),
            "unset" => return Ok(self.builtin_unset(&args_refs)),
            "set" => return Ok(self.builtin_set(&args_refs)),
            "local" => return Ok(self.builtin_local(&args_refs)),
            "readonly" => return Ok(self.builtin_readonly(&args_refs)),
            "declare" => return Ok(self.builtin_declare(&args_refs)),
            "source" | "." => return self.builtin_source(&args_refs),
            "eval" => return self.builtin_eval(&args_refs),
            "exit" => return self.builtin_exit(&args_refs),
            "return" => return self.builtin_return(&args_refs),
            "break" => return Self::builtin_break(&args_refs),
            "continue" => return Self::builtin_continue(&args_refs),
            "shift" => return Ok(self.builtin_shift(&args_refs)),
            "alias" => return Ok(self.builtin_alias(&args_refs)),
            "unalias" => return Ok(self.builtin_unalias(&args_refs)),
            "type" => return Ok(self.builtin_type(&args_refs)),
            "command" => return self.builtin_command(&args_refs, stdin),
            "builtin" => return self.builtin_builtin(&args_refs, stdin),
            "hash" => return Ok(Self::builtin_hash(&args_refs)),
            "pushd" => return Ok(self.builtin_pushd(&args_refs)),
            "popd" => return Ok(self.builtin_popd(&args_refs)),
            "dirs" => return Ok(self.builtin_dirs(&args_refs)),
            "read" => return Ok(self.builtin_read(&args_refs, stdin)),
            "getopts" => return Ok(self.builtin_getopts(&args_refs)),
            "trap" => return Ok(self.builtin_trap(&args_refs)),
            "shopt" => return Ok(self.builtin_shopt(&args_refs)),
            "mapfile" | "readarray" => return Ok(self.builtin_mapfile(&args_refs, stdin)),
            "bash" | "sh" => return self.builtin_bash(&args_refs, stdin),
            ":" => return Ok(ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 0 }),
            "let" => {
                let expr_str = args.join(" ");
                let val = self.evaluate_arith_string(&expr_str)?;
                return Ok(ExecOutput {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: i32::from(val == 0),
                });
            }
            "clear" => return Ok(ExecOutput {
                stdout: "\x1b[2J\x1b[H".to_string(),
                stderr: String::new(),
                exit_code: 0,
            }),
            "help" => return Ok(Self::builtin_help()),
            _ => {}
        }

        if self.state.shopt.expand_aliases {
            if let Some(alias_val) = self.state.aliases.get(name).cloned() {
                self.state.call_depth += 1;
                if self.state.call_depth > self.limits.max_call_depth {
                    self.state.call_depth -= 1;
                    return Err(crate::error::LimitKind::CallDepth.into());
                }
                let full_cmd = if args.is_empty() {
                    alias_val
                } else {
                    format!(
                        "{} {}",
                        alias_val,
                        args.iter().map(String::as_str).collect::<Vec<_>>().join(" ")
                    )
                };
                let script = crate::parser::parse(&full_cmd)
                    .map_err(|e| ShellSignal::Error(Error::Parse(e)))?;
                let result = self.execute_script(&script);
                self.state.call_depth -= 1;
                return result;
            }
        }

        if let Some(func) = self.state.functions.get(name).cloned() {
            return self.call_function(&func, args, stdin);
        }

        if let Some(cmd_fn) = self.registry.lookup(name) {
            return self.run_external_command(cmd_fn, args, stdin);
        }

        let stderr = format!("{name}: command not found\n");
        Ok(ExecOutput {
            stdout: String::new(),
            stderr,
            exit_code: 127,
        })
    }

    pub(super) fn run_external_command(&mut self, cmd_fn: CommandFn, args: &[String], stdin: &str) -> InterpResult {
        let mut stdout = String::new();
        let mut stderr = String::new();
        let args_refs: Vec<&str> = args.iter().map(String::as_str).collect();

        let mut ctx = CommandContext {
            fs: self.fs,
            cwd: &self.state.cwd,
            env: &self.state.env,
            stdin,
            stdout: &mut stdout,
            stderr: &mut stderr,
            exec_fn: None,
            limits: self.limits,
            #[cfg(feature = "network")]
            network_policy: self.network_policy,
        };

        match cmd_fn(&args_refs, &mut ctx) {
            Ok(result) => Ok(ExecOutput {
                stdout: result.stdout,
                stderr: result.stderr,
                exit_code: result.exit_code,
            }),
            Err(e) => Err(ShellSignal::Error(e)),
        }
    }

    pub(super) fn call_function(&mut self, func: &FunctionDef, args: &[String], _stdin: &str) -> InterpResult {
        self.state.call_depth += 1;
        if self.state.call_depth > self.limits.max_call_depth {
            self.state.call_depth -= 1;
            return Err(crate::error::LimitKind::CallDepth.into());
        }

        self.state.func_name_stack.push(func.name.clone());

        let saved_params = std::mem::replace(
            &mut self.state.positional_params,
            args.to_vec(),
        );

        self.state.local_scopes.push(HashMap::new());

        let result = self.execute_compound_command(&func.body, "");

        if let Some(scope) = self.state.local_scopes.pop() {
            for (name, old_value) in scope {
                if let Some(v) = old_value {
                    let _ = self.state.set_var(&name, v);
                } else {
                    let _ = self.state.unset_var(&name);
                }
            }
        }

        self.state.positional_params = saved_params;
        self.state.func_name_stack.pop();
        self.state.call_depth -= 1;

        match result {
            Err(ShellSignal::Flow(ControlFlow::Return { code, stdout, stderr })) => {
                Ok(ExecOutput {
                    stdout,
                    stderr,
                    exit_code: code,
                })
            }
            other => other,
        }
    }
}
