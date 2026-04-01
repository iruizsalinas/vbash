use std::collections::HashSet;
use std::fmt::Write;

use crate::error::{ControlFlow, Error, ShellSignal};

use super::{is_shell_builtin, ExecOutput, InterpResult, Interpreter};

impl Interpreter<'_> {
    pub(super) fn builtin_cd(&mut self, args: &[&str]) -> ExecOutput {
        let target = if args.is_empty() {
            self.state.get_var("HOME").unwrap_or("/").to_string()
        } else if args[0] == "-" {
            self.state.previous_dir.clone()
        } else {
            self.resolve_path(args[0])
        };

        if self.fs.stat(&target).is_ok_and(|m| m.is_dir()) {
            let old = self.state.cwd.clone();
            self.state.cwd.clone_from(&target);
            self.state.previous_dir = old;
            let _ = self.state.set_var("PWD", target);
            ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 0 }
        } else {
            ExecOutput {
                stdout: String::new(),
                stderr: format!("cd: {target}: No such file or directory\n"),
                exit_code: 1,
            }
        }
    }

    pub(super) fn builtin_export(&mut self, args: &[&str]) -> ExecOutput {
        for arg in args {
            if let Some((name, value)) = arg.split_once('=') {
                let _ = self.state.set_var(name, value.to_string());
                self.state.exported.insert(name.to_string());
            } else {
                self.state.exported.insert((*arg).to_string());
            }
        }
        ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 0 }
    }

    pub(super) fn builtin_unset(&mut self, args: &[&str]) -> ExecOutput {
        let mut stderr = String::new();
        let mut exit_code = 0;
        for arg in args {
            if *arg == "-f" || *arg == "-v" {
                continue;
            }
            if let Err(e) = self.state.unset_var(arg) {
                let _ = writeln!(stderr, "unset: {e}");
                exit_code = 1;
            }
        }
        ExecOutput { stdout: String::new(), stderr, exit_code }
    }

    pub(super) fn builtin_set(&mut self, args: &[&str]) -> ExecOutput {
        if args.is_empty() {
            let mut stdout = String::new();
            let mut vars: Vec<_> = self.state.env.iter().collect();
            vars.sort_unstable_by_key(|(k, _)| *k);
            for (k, v) in vars {
                let _ = writeln!(stdout, "{k}={v}");
            }
            return ExecOutput { stdout, stderr: String::new(), exit_code: 0 };
        }

        let mut i = 0;
        while i < args.len() {
            let arg = args[i];
            if arg == "--" {
                self.state.positional_params = args[i + 1..].iter().map(std::string::ToString::to_string).collect();
                break;
            }
            let (enable, flag) = if let Some(f) = arg.strip_prefix('-') {
                (true, f)
            } else if let Some(f) = arg.strip_prefix('+') {
                (false, f)
            } else {
                break;
            };
            match flag {
                "e" => self.state.options.errexit = enable,
                "u" => self.state.options.nounset = enable,
                "x" => self.state.options.xtrace = enable,
                "v" => self.state.options.verbose = enable,
                "f" => self.state.options.noglob = enable,
                "C" => self.state.options.noclobber = enable,
                "a" => self.state.options.allexport = enable,
                "o" if i + 1 < args.len() => {
                    i += 1;
                    match args[i] {
                        "errexit" => self.state.options.errexit = enable,
                        "nounset" => self.state.options.nounset = enable,
                        "pipefail" => self.state.options.pipefail = enable,
                        "xtrace" => self.state.options.xtrace = enable,
                        "verbose" => self.state.options.verbose = enable,
                        "noclobber" => self.state.options.noclobber = enable,
                        "noglob" => self.state.options.noglob = enable,
                        "allexport" => self.state.options.allexport = enable,
                        "noexec" => self.state.options.noexec = enable,
                        _ => {}
                    }
                }
                _ => {}
            }
            i += 1;
        }
        ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 0 }
    }

    pub(super) fn builtin_local(&mut self, args: &[&str]) -> ExecOutput {
        if self.state.local_scopes.is_empty() {
            return ExecOutput {
                stdout: String::new(),
                stderr: "local: can only be used in a function\n".to_string(),
                exit_code: 1,
            };
        }
        for arg in args {
            if let Some((name, value)) = arg.split_once('=') {
                let old = self.state.get_var(name).map(String::from);
                if let Some(scope) = self.state.local_scopes.last_mut() {
                    scope.entry(name.to_string()).or_insert(old);
                }
                let _ = self.state.set_var(name, value.to_string());
            } else {
                let old = self.state.get_var(arg).map(String::from);
                if let Some(scope) = self.state.local_scopes.last_mut() {
                    scope.entry((*arg).to_string()).or_insert(old);
                }
            }
        }
        ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 0 }
    }

    pub(super) fn builtin_readonly(&mut self, args: &[&str]) -> ExecOutput {
        for arg in args {
            if let Some((name, value)) = arg.split_once('=') {
                let _ = self.state.set_var(name, value.to_string());
                self.state.readonly.insert(name.to_string());
            } else {
                self.state.readonly.insert((*arg).to_string());
            }
        }
        ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 0 }
    }

    pub(super) fn builtin_declare(&mut self, args: &[&str]) -> ExecOutput {
        let mut flags = HashSet::new();
        let mut var_args = Vec::new();
        let mut i = 0;
        while i < args.len() {
            let arg = args[i];
            if arg.starts_with('-') || arg.starts_with('+') {
                for c in arg[1..].chars() {
                    flags.insert(c);
                }
            } else {
                var_args.push(arg);
            }
            i += 1;
        }

        if flags.contains(&'p') {
            let mut stdout = String::new();
            let mut vars: Vec<_> = self.state.env.iter().collect();
            vars.sort_unstable_by_key(|(k, _)| *k);
            for (k, v) in vars {
                let _ = writeln!(stdout, "declare -- {k}=\"{v}\"");
            }
            return ExecOutput { stdout, stderr: String::new(), exit_code: 0 };
        }

        for var_arg in &var_args {
            if let Some((name, value)) = var_arg.split_once('=') {
                if flags.contains(&'a') {
                    let elements: Vec<String> = if value.starts_with('(') && value.ends_with(')') {
                        value[1..value.len() - 1]
                            .split_whitespace()
                            .map(std::string::ToString::to_string)
                            .collect()
                    } else {
                        vec![value.to_string()]
                    };
                    self.state.arrays.insert(name.to_string(), elements);
                } else if flags.contains(&'A') {
                    self.state.assoc_arrays.entry(name.to_string()).or_default();
                } else {
                    let _ = self.state.set_var(name, value.to_string());
                }
                if flags.contains(&'r') {
                    self.state.readonly.insert(name.to_string());
                }
                if flags.contains(&'x') {
                    self.state.exported.insert(name.to_string());
                }
            } else {
                if flags.contains(&'a') {
                    self.state.arrays.entry((*var_arg).to_string()).or_default();
                } else if flags.contains(&'A') {
                    self.state.assoc_arrays.entry((*var_arg).to_string()).or_default();
                }
                if flags.contains(&'x') {
                    self.state.exported.insert((*var_arg).to_string());
                }
            }
        }
        ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 0 }
    }

    pub(super) fn builtin_source(&mut self, args: &[&str]) -> InterpResult {
        if args.is_empty() {
            return Ok(ExecOutput {
                stdout: String::new(),
                stderr: "source: filename argument required\n".to_string(),
                exit_code: 2,
            });
        }

        self.state.source_depth += 1;
        if self.state.source_depth > self.limits.max_source_depth {
            self.state.source_depth -= 1;
            return Err(crate::error::LimitKind::SourceDepth.into());
        }

        let path = self.resolve_path(args[0]);
        let content = self.fs.read_file_string(&path)
            .map_err(|e| ShellSignal::Error(Error::Fs(e)))?;

        let saved_params = std::mem::replace(
            &mut self.state.positional_params,
            args[1..].iter().map(std::string::ToString::to_string).collect(),
        );

        let script = crate::parser::parse(&content)
            .map_err(|e| ShellSignal::Error(Error::Parse(e)))?;
        let result = self.execute_script(&script);

        self.state.positional_params = saved_params;
        self.state.source_depth -= 1;

        result
    }

    pub(super) fn builtin_eval(&mut self, args: &[&str]) -> InterpResult {
        self.state.call_depth += 1;
        if self.state.call_depth > self.limits.max_call_depth {
            self.state.call_depth -= 1;
            return Err(crate::error::LimitKind::CallDepth.into());
        }
        let cmd = args.join(" ");
        let script = crate::parser::parse(&cmd)
            .map_err(|e| ShellSignal::Error(Error::Parse(e)))?;
        let result = self.execute_script(&script);
        self.state.call_depth -= 1;
        result
    }

    pub(super) fn builtin_exit(&mut self, args: &[&str]) -> InterpResult {
        let code = args.first().and_then(|s| s.parse::<i32>().ok()).unwrap_or(self.state.last_exit_code);
        Err(ShellSignal::Flow(ControlFlow::Exit {
            code,
            stdout: String::new(),
            stderr: String::new(),
        }))
    }

    pub(super) fn builtin_return(&mut self, args: &[&str]) -> InterpResult {
        let code = args.first().and_then(|s| s.parse::<i32>().ok()).unwrap_or(self.state.last_exit_code);
        Err(ShellSignal::Flow(ControlFlow::Return {
            code,
            stdout: String::new(),
            stderr: String::new(),
        }))
    }

    pub(super) fn builtin_break(args: &[&str]) -> InterpResult {
        let n = args.first().and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
        Err(ShellSignal::Flow(ControlFlow::Break { n, stdout: String::new(), stderr: String::new() }))
    }

    pub(super) fn builtin_continue(args: &[&str]) -> InterpResult {
        let n = args.first().and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
        Err(ShellSignal::Flow(ControlFlow::Continue { n, stdout: String::new(), stderr: String::new() }))
    }

    pub(super) fn builtin_shift(&mut self, args: &[&str]) -> ExecOutput {
        let n = args.first().and_then(|s| s.parse::<usize>().ok()).unwrap_or(1);
        for _ in 0..n.min(self.state.positional_params.len()) {
            self.state.positional_params.remove(0);
        }
        ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 0 }
    }

    pub(super) fn builtin_alias(&mut self, args: &[&str]) -> ExecOutput {
        if args.is_empty() {
            let mut stdout = String::new();
            let mut aliases: Vec<_> = self.state.aliases.iter().collect();
            aliases.sort_unstable_by_key(|(k, _)| *k);
            for (k, v) in aliases {
                let _ = writeln!(stdout, "alias {k}='{v}'");
            }
            return ExecOutput { stdout, stderr: String::new(), exit_code: 0 };
        }
        for arg in args {
            if let Some((name, value)) = arg.split_once('=') {
                self.state.aliases.insert(name.to_string(), value.to_string());
            }
        }
        ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 0 }
    }

    pub(super) fn builtin_unalias(&mut self, args: &[&str]) -> ExecOutput {
        for arg in args {
            if *arg == "-a" {
                self.state.aliases.clear();
            } else {
                self.state.aliases.remove(*arg);
            }
        }
        ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 0 }
    }

    pub(super) fn builtin_type(&mut self, args: &[&str]) -> ExecOutput {
        let mut stdout = String::new();
        let mut exit_code = 0;
        for arg in args {
            if is_shell_builtin(arg) {
                let _ = writeln!(stdout, "{arg} is a shell builtin");
            } else if self.state.functions.contains_key(*arg) {
                let _ = writeln!(stdout, "{arg} is a function");
            } else if self.state.aliases.contains_key(*arg) {
                let _ = writeln!(stdout, "{arg} is aliased to `{}`", self.state.aliases[*arg]);
            } else if self.registry.lookup(arg).is_some() {
                let _ = writeln!(stdout, "{arg} is /usr/bin/{arg}");
            } else {
                let _ = writeln!(stdout, "type: {arg}: not found");
                exit_code = 1;
            }
        }
        ExecOutput { stdout, stderr: String::new(), exit_code }
    }

    pub(super) fn builtin_command(&mut self, args: &[&str], stdin: &str) -> InterpResult {
        if args.is_empty() {
            return Ok(ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 0 });
        }
        let name = args[0];
        let cmd_args: Vec<String> = args[1..].iter().map(std::string::ToString::to_string).collect();
        if let Some(cmd_fn) = self.registry.lookup(name) {
            self.run_external_command(cmd_fn, &cmd_args, stdin)
        } else {
            self.dispatch_command(name, &cmd_args, stdin)
        }
    }

    pub(super) fn builtin_builtin(&mut self, args: &[&str], stdin: &str) -> InterpResult {
        self.builtin_command(args, stdin)
    }

    pub(super) fn builtin_hash(_args: &[&str]) -> ExecOutput {
        ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 0 }
    }

    pub(super) fn builtin_pushd(&mut self, args: &[&str]) -> ExecOutput {
        let dir = args.first().copied().unwrap_or("~");
        let target = self.resolve_path(dir);
        self.state.dir_stack.push(self.state.cwd.clone());
        let cd_result = self.builtin_cd(&[&target]);
        if cd_result.exit_code != 0 {
            self.state.dir_stack.pop();
        }
        cd_result
    }

    pub(super) fn builtin_popd(&mut self, _args: &[&str]) -> ExecOutput {
        if let Some(dir) = self.state.dir_stack.pop() {
            self.builtin_cd(&[&dir])
        } else {
            ExecOutput {
                stdout: String::new(),
                stderr: "popd: directory stack empty\n".to_string(),
                exit_code: 1,
            }
        }
    }

    pub(super) fn builtin_dirs(&mut self, _args: &[&str]) -> ExecOutput {
        let mut stdout = self.state.cwd.clone();
        for dir in self.state.dir_stack.iter().rev() {
            stdout.push(' ');
            stdout.push_str(dir);
        }
        stdout.push('\n');
        ExecOutput { stdout, stderr: String::new(), exit_code: 0 }
    }

    pub(super) fn builtin_trap(&mut self, args: &[&str]) -> ExecOutput {
        if args.len() >= 2 {
            self.state.traps.insert(args[1].to_string(), args[0].to_string());
        }
        ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 0 }
    }

    pub(super) fn builtin_shopt(&mut self, args: &[&str]) -> ExecOutput {
        if args.len() >= 2 {
            let enable = args[0] == "-s";
            for opt in &args[1..] {
                match *opt {
                    "extglob" => self.state.shopt.extglob = enable,
                    "globstar" => self.state.shopt.globstar = enable,
                    "nullglob" => self.state.shopt.nullglob = enable,
                    "failglob" => self.state.shopt.failglob = enable,
                    "dotglob" => self.state.shopt.dotglob = enable,
                    "nocaseglob" => self.state.shopt.nocaseglob = enable,
                    "nocasematch" => self.state.shopt.nocasematch = enable,
                    "expand_aliases" => self.state.shopt.expand_aliases = enable,
                    "xpg_echo" => self.state.shopt.xpg_echo = enable,
                    _ => {}
                }
            }
        }
        ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 0 }
    }

    pub(super) fn builtin_bash(&mut self, args: &[&str], stdin: &str) -> InterpResult {
        self.state.call_depth += 1;
        if self.state.call_depth > self.limits.max_call_depth {
            self.state.call_depth -= 1;
            return Err(crate::error::LimitKind::CallDepth.into());
        }

        let mut command_string = None;
        let mut script_file = None;

        let mut i = 0;
        while i < args.len() {
            match args[i] {
                "-c" if i + 1 < args.len() => {
                    command_string = Some(args[i + 1]);
                    break;
                }
                arg if !arg.starts_with('-') => {
                    script_file = Some(arg);
                    break;
                }
                _ => { i += 1; }
            }
        }

        let result = if let Some(cmd) = command_string {
            let script = crate::parser::parse(cmd)
                .map_err(|e| ShellSignal::Error(Error::Parse(e)))?;
            self.execute_script(&script)
        } else if let Some(file) = script_file {
            let path = self.resolve_path(file);
            let content = self.fs.read_file_string(&path)
                .map_err(|e| ShellSignal::Error(Error::Fs(e)))?;
            let script_text = if content.starts_with("#!") {
                content.split_once('\n').map_or_else(String::new, |(_, rest)| rest.to_string())
            } else {
                content
            };
            let script = crate::parser::parse(&script_text)
                .map_err(|e| ShellSignal::Error(Error::Parse(e)))?;
            self.execute_script(&script)
        } else if !stdin.is_empty() {
            let script = crate::parser::parse(stdin)
                .map_err(|e| ShellSignal::Error(Error::Parse(e)))?;
            self.execute_script(&script)
        } else {
            Ok(ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 0 })
        };

        self.state.call_depth -= 1;
        result
    }

    pub(super) fn builtin_help() -> ExecOutput {
        let mut stdout = String::new();
        for name in [
            "alias", "bg", "cd", "command", "declare", "echo", "eval", "exec",
            "exit", "export", "fg", "getopts", "hash", "help", "history", "jobs",
            "local", "popd", "printf", "pushd", "pwd", "read", "readonly", "return",
            "set", "shift", "shopt", "source", "test", "trap", "type", "unalias", "unset",
        ] {
            let _ = writeln!(stdout, "{name}");
        }
        ExecOutput { stdout, stderr: String::new(), exit_code: 0 }
    }

    pub(super) fn resolve_path(&self, path: &str) -> String {
        crate::fs::path::resolve(&self.state.cwd, path)
    }

    pub(super) fn check_command_limit(&mut self) -> Result<(), ShellSignal> {
        if let Some(ref cancel) = self.cancel {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(crate::error::LimitKind::Cancelled.into());
            }
        }
        self.state.command_count += 1;
        if self.state.command_count > self.limits.max_command_count {
            return Err(crate::error::LimitKind::CommandCount.into());
        }
        Ok(())
    }

}
