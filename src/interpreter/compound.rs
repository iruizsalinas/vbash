use crate::ast::{
    ArithmeticCmd, CStyleForCmd, CaseCmd, CaseTerminator, CompoundCommand, ConditionalCmd,
    ForCmd, GroupCmd, IfCmd, Redirection, RedirectOp, RedirectTarget,
    Statement, SubshellCmd, UntilCmd, WhileCmd,
};
use crate::error::{ControlFlow, Error, ShellSignal};

use super::{glob_match, ExecOutput, InterpResult, Interpreter};

impl Interpreter<'_> {
    pub(super) fn execute_compound_command(&mut self, cc: &CompoundCommand, stdin: &str) -> InterpResult {
        match cc {
            CompoundCommand::If(cmd) => self.execute_if(cmd),
            CompoundCommand::For(cmd) => self.execute_for(cmd),
            CompoundCommand::CStyleFor(cmd) => self.execute_c_style_for(cmd),
            CompoundCommand::While(cmd) => self.execute_while(cmd),
            CompoundCommand::Until(cmd) => self.execute_until(cmd),
            CompoundCommand::Case(cmd) => self.execute_case(cmd),
            CompoundCommand::Subshell(cmd) => self.execute_subshell(cmd),
            CompoundCommand::Group(cmd) => self.execute_group(cmd, stdin),
            CompoundCommand::Arithmetic(cmd) => self.execute_arithmetic_cmd(cmd),
            CompoundCommand::Conditional(cmd) => self.execute_conditional_cmd(cmd),
        }
    }

    fn apply_compound_redirections(
        &mut self,
        result: InterpResult,
        redirections: &[Redirection],
    ) -> InterpResult {
        if redirections.is_empty() {
            return result;
        }
        let mut out = result?;
        for redir in redirections {
            if let RedirectTarget::Word(w) = &redir.target {
                let target = self.expand_word(w)?;
                let resolved = self.resolve_path(&target);
                match redir.operator {
                    RedirectOp::Output | RedirectOp::Clobber => {
                        let effective_fd = redir.fd.unwrap_or(1);
                        let content = if effective_fd == 2 {
                            std::mem::take(&mut out.stderr)
                        } else {
                            std::mem::take(&mut out.stdout)
                        };
                        let parent = crate::fs::path::parent(&resolved);
                        if parent != "/" {
                            let _ = self.fs.mkdir(parent, true);
                        }
                        self.fs
                            .write_file(&resolved, content.as_bytes())
                            .map_err(|e| ShellSignal::Error(Error::Fs(e)))?;
                    }
                    RedirectOp::Append => {
                        let effective_fd = redir.fd.unwrap_or(1);
                        let content = if effective_fd == 2 {
                            std::mem::take(&mut out.stderr)
                        } else {
                            std::mem::take(&mut out.stdout)
                        };
                        let parent = crate::fs::path::parent(&resolved);
                        if parent != "/" {
                            let _ = self.fs.mkdir(parent, true);
                        }
                        self.fs
                            .append_file(&resolved, content.as_bytes())
                            .map_err(|e| ShellSignal::Error(Error::Fs(e)))?;
                    }
                    _ => {}
                }
            }
        }
        Ok(out)
    }

    fn execute_if(&mut self, cmd: &IfCmd) -> InterpResult {
        self.state.in_condition = true;
        for clause in &cmd.clauses {
            let cond = self.execute_statements(&clause.condition)?;
            self.state.in_condition = false;
            if cond.exit_code == 0 {
                let result = self.execute_statements(&clause.body);
                return self.apply_compound_redirections(result, &cmd.redirections);
            }
            self.state.in_condition = true;
        }
        self.state.in_condition = false;

        if let Some(ref else_body) = cmd.else_body {
            let result = self.execute_statements(else_body);
            return self.apply_compound_redirections(result, &cmd.redirections);
        }

        self.apply_compound_redirections(
            Ok(ExecOutput {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 0,
            }),
            &cmd.redirections,
        )
    }

    fn execute_for(&mut self, cmd: &ForCmd) -> InterpResult {
        let words = if let Some(ref word_list) = cmd.words {
            let mut expanded = Vec::new();
            for w in word_list {
                expanded.extend(self.expand_word_splitting(w)?);
            }
            expanded
        } else {
            self.state.positional_params.clone()
        };

        let mut stdout = String::new();
        let mut stderr = String::new();
        let mut exit_code = 0;

        self.state.loop_depth += 1;
        let mut iteration = 0u32;

        for word in &words {
            iteration += 1;
            if iteration > self.limits.max_loop_iterations {
                self.state.loop_depth -= 1;
                return Err(crate::error::LimitKind::LoopIterations.into());
            }

            let _ = self.state.set_var(&cmd.variable, word.clone());
            match self.execute_statements(&cmd.body) {
                Ok(out) => {
                    stdout.push_str(&out.stdout);
                    stderr.push_str(&out.stderr);
                    exit_code = out.exit_code;
                }
                Err(ShellSignal::Flow(ControlFlow::Break { n, stdout: bs, stderr: be })) => {
                    stdout.push_str(&bs);
                    stderr.push_str(&be);
                    if n > 1 {
                        self.state.loop_depth -= 1;
                        return Err(ShellSignal::Flow(ControlFlow::Break { n: n - 1, stdout, stderr }));
                    }
                    break;
                }
                Err(ShellSignal::Flow(ControlFlow::Continue { n, stdout: cs, stderr: ce })) => {
                    stdout.push_str(&cs);
                    stderr.push_str(&ce);
                    if n > 1 {
                        self.state.loop_depth -= 1;
                        return Err(ShellSignal::Flow(ControlFlow::Continue { n: n - 1, stdout, stderr }));
                    }
                }
                Err(e) => {
                    self.state.loop_depth -= 1;
                    return Err(e);
                }
            }
        }

        self.state.loop_depth -= 1;
        self.apply_compound_redirections(
            Ok(ExecOutput { stdout, stderr, exit_code }),
            &cmd.redirections,
        )
    }

    fn execute_c_style_for(&mut self, cmd: &CStyleForCmd) -> InterpResult {
        if let Some(ref init) = cmd.init {
            self.evaluate_arith(init)?;
        }

        let mut stdout = String::new();
        let mut stderr = String::new();
        let mut exit_code = 0;
        let mut iteration = 0u32;

        self.state.loop_depth += 1;
        loop {
            iteration += 1;
            if iteration > self.limits.max_loop_iterations {
                self.state.loop_depth -= 1;
                return Err(crate::error::LimitKind::LoopIterations.into());
            }

            if let Some(ref cond) = cmd.condition {
                let val = self.evaluate_arith(cond)?;
                if val == 0 {
                    break;
                }
            }

            match self.execute_statements(&cmd.body) {
                Ok(out) => {
                    stdout.push_str(&out.stdout);
                    stderr.push_str(&out.stderr);
                    exit_code = out.exit_code;
                }
                Err(ShellSignal::Flow(ControlFlow::Break { n, stdout: bs, stderr: be })) => {
                    stdout.push_str(&bs);
                    stderr.push_str(&be);
                    if n > 1 {
                        self.state.loop_depth -= 1;
                        return Err(ShellSignal::Flow(ControlFlow::Break { n: n - 1, stdout, stderr }));
                    }
                    break;
                }
                Err(ShellSignal::Flow(ControlFlow::Continue { n, stdout: cs, stderr: ce })) => {
                    stdout.push_str(&cs);
                    stderr.push_str(&ce);
                    if n > 1 {
                        self.state.loop_depth -= 1;
                        return Err(ShellSignal::Flow(ControlFlow::Continue { n: n - 1, stdout, stderr }));
                    }
                }
                Err(e) => {
                    self.state.loop_depth -= 1;
                    return Err(e);
                }
            }

            if let Some(ref update) = cmd.update {
                self.evaluate_arith(update)?;
            }
        }

        self.state.loop_depth -= 1;
        self.apply_compound_redirections(
            Ok(ExecOutput { stdout, stderr, exit_code }),
            &cmd.redirections,
        )
    }

    fn execute_while(&mut self, cmd: &WhileCmd) -> InterpResult {
        let mut stdout = String::new();
        let mut stderr = String::new();
        let mut exit_code = 0;
        let mut iteration = 0u32;

        // Handle input redirection on the while loop (e.g., while read line; do ... done < file)
        let input_lines = self.extract_input_redirect(&cmd.redirections);
        let mut input_cursor = 0usize;
        let saved_stdin = if input_lines.is_some() {
            Some(std::mem::take(&mut self.stdin))
        } else {
            None
        };

        self.state.loop_depth += 1;
        loop {
            iteration += 1;
            if iteration > self.limits.max_loop_iterations {
                self.state.loop_depth -= 1;
                if let Some(saved) = saved_stdin { self.stdin = saved; }
                return Err(crate::error::LimitKind::LoopIterations.into());
            }

            // Feed the next line from input redirection to stdin for `read`
            if let Some(ref lines) = input_lines {
                if input_cursor >= lines.len() {
                    break; // EOF
                }
                self.stdin = format!("{}\n", lines[input_cursor]);
                input_cursor += 1;
            }

            self.state.in_condition = true;
            let cond = self.execute_statements(&cmd.condition)?;
            self.state.in_condition = false;
            if cond.exit_code != 0 {
                break;
            }

            match self.execute_statements(&cmd.body) {
                Ok(out) => {
                    stdout.push_str(&out.stdout);
                    stderr.push_str(&out.stderr);
                    exit_code = out.exit_code;
                }
                Err(ShellSignal::Flow(ControlFlow::Break { n, stdout: bs, stderr: be })) => {
                    stdout.push_str(&bs);
                    stderr.push_str(&be);
                    if n > 1 {
                        self.state.loop_depth -= 1;
        
                        return Err(ShellSignal::Flow(ControlFlow::Break { n: n - 1, stdout, stderr }));
                    }
                    break;
                }
                Err(ShellSignal::Flow(ControlFlow::Continue { n, stdout: cs, stderr: ce })) => {
                    stdout.push_str(&cs);
                    stderr.push_str(&ce);
                    if n > 1 {
                        self.state.loop_depth -= 1;
        
                        return Err(ShellSignal::Flow(ControlFlow::Continue { n: n - 1, stdout, stderr }));
                    }
                }
                Err(e) => {
                    self.state.loop_depth -= 1;
                    return Err(e);
                }
            }
        }

        self.state.loop_depth -= 1;
        if let Some(saved) = saved_stdin {
            self.stdin = saved;
        }
        self.apply_compound_redirections(
            Ok(ExecOutput { stdout, stderr, exit_code }),
            &cmd.redirections,
        )
    }

    fn extract_input_redirect(&mut self, redirections: &[Redirection]) -> Option<Vec<String>> {
        for redir in redirections {
            if redir.operator == RedirectOp::Input {
                if let RedirectTarget::Word(w) = &redir.target {
                    if let Ok(target) = self.expand_word(w) {
                        let resolved = self.resolve_path(&target);
                        if let Ok(content) = self.fs.read_file_string(&resolved) {
                            return Some(content.lines().map(String::from).collect());
                        }
                    }
                }
            }
        }
        None
    }

    fn execute_until(&mut self, cmd: &UntilCmd) -> InterpResult {
        let mut stdout = String::new();
        let mut stderr = String::new();
        let mut exit_code = 0;
        let mut iteration = 0u32;

        self.state.loop_depth += 1;
        loop {
            iteration += 1;
            if iteration > self.limits.max_loop_iterations {
                self.state.loop_depth -= 1;
                return Err(crate::error::LimitKind::LoopIterations.into());
            }

            self.state.in_condition = true;
            let cond = self.execute_statements(&cmd.condition)?;
            self.state.in_condition = false;
            if cond.exit_code == 0 {
                break;
            }

            match self.execute_statements(&cmd.body) {
                Ok(out) => {
                    stdout.push_str(&out.stdout);
                    stderr.push_str(&out.stderr);
                    exit_code = out.exit_code;
                }
                Err(ShellSignal::Flow(ControlFlow::Break { n, stdout: bs, stderr: be })) => {
                    stdout.push_str(&bs);
                    stderr.push_str(&be);
                    if n > 1 {
                        self.state.loop_depth -= 1;
                        return Err(ShellSignal::Flow(ControlFlow::Break { n: n - 1, stdout, stderr }));
                    }
                    break;
                }
                Err(ShellSignal::Flow(ControlFlow::Continue { n, stdout: cs, stderr: ce })) => {
                    stdout.push_str(&cs);
                    stderr.push_str(&ce);
                    if n > 1 {
                        self.state.loop_depth -= 1;
                        return Err(ShellSignal::Flow(ControlFlow::Continue { n: n - 1, stdout, stderr }));
                    }
                }
                Err(e) => {
                    self.state.loop_depth -= 1;
                    return Err(e);
                }
            }
        }

        self.state.loop_depth -= 1;
        self.apply_compound_redirections(
            Ok(ExecOutput { stdout, stderr, exit_code }),
            &cmd.redirections,
        )
    }

    fn execute_case(&mut self, cmd: &CaseCmd) -> InterpResult {
        let word_val = self.expand_word(&cmd.word)?;
        let extglob = self.state.shopt.extglob;
        let nocasematch = self.state.shopt.nocasematch;

        let mut stdout = String::new();
        let mut stderr = String::new();
        let mut exit_code = 0;
        let mut fall_through = false;

        for item in &cmd.items {
            let matched = if fall_through {
                true
            } else {
                let mut m = false;
                for pattern in &item.patterns {
                    let pattern_val = self.expand_word(pattern)?;
                    if glob_match(&pattern_val, &word_val, extglob, nocasematch) {
                        m = true;
                        break;
                    }
                }
                m
            };

            if matched {
                let out = self.execute_statements(&item.body)?;
                stdout.push_str(&out.stdout);
                stderr.push_str(&out.stderr);
                exit_code = out.exit_code;
                match item.terminator {
                    CaseTerminator::Break => {
                        return self.apply_compound_redirections(
                            Ok(ExecOutput { stdout, stderr, exit_code }),
                            &cmd.redirections,
                        );
                    }
                    CaseTerminator::FallThrough => {
                        fall_through = true;
                    }
                    CaseTerminator::Continue => {
                        fall_through = false;
                    }
                }
            }
        }

        self.apply_compound_redirections(
            Ok(ExecOutput { stdout, stderr, exit_code }),
            &cmd.redirections,
        )
    }

    fn execute_subshell(&mut self, cmd: &SubshellCmd) -> InterpResult {
        let saved_state = self.state.clone();
        self.state.subshell_depth += 1;
        let result = self.execute_statements(&cmd.body);
        self.state = saved_state;

        let inner = match result {
            Ok(out) => Ok(out),
            Err(ShellSignal::Flow(ControlFlow::Exit { code, stdout, stderr })) => {
                Ok(ExecOutput { stdout, stderr, exit_code: code })
            }
            Err(e) => Err(e),
        };
        self.apply_compound_redirections(inner, &cmd.redirections)
    }

    fn execute_group(&mut self, cmd: &GroupCmd, _stdin: &str) -> InterpResult {
        let result = self.execute_statements(&cmd.body);
        self.apply_compound_redirections(result, &cmd.redirections)
    }

    fn execute_arithmetic_cmd(&mut self, cmd: &ArithmeticCmd) -> InterpResult {
        let value = self.evaluate_arith(&cmd.expression)?;
        let exit_code = i32::from(value == 0);
        Ok(ExecOutput {
            stdout: String::new(),
            stderr: String::new(),
            exit_code,
        })
    }

    fn execute_conditional_cmd(&mut self, cmd: &ConditionalCmd) -> InterpResult {
        self.state.in_condition = true;
        let result = self.evaluate_cond(&cmd.expression)?;
        self.state.in_condition = false;
        Ok(ExecOutput {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: i32::from(!result),
        })
    }

    pub(super) fn execute_statements(&mut self, stmts: &[Statement]) -> InterpResult {
        let mut stdout = String::new();
        let mut stderr = String::new();
        let mut exit_code = 0;
        for stmt in stmts {
            match self.execute_statement(stmt) {
                Ok(out) => {
                    stdout.push_str(&out.stdout);
                    stderr.push_str(&out.stderr);
                    exit_code = out.exit_code;
                }
                Err(ShellSignal::Flow(ControlFlow::Return { code, stdout: rs, stderr: re })) => {
                    stdout.push_str(&rs);
                    stderr.push_str(&re);
                    return Err(ShellSignal::Flow(ControlFlow::Return { code, stdout, stderr }));
                }
                Err(ShellSignal::Flow(ControlFlow::Exit { code, stdout: es, stderr: ee })) => {
                    stdout.push_str(&es);
                    stderr.push_str(&ee);
                    return Err(ShellSignal::Flow(ControlFlow::Exit { code, stdout, stderr }));
                }
                Err(ShellSignal::Flow(ControlFlow::Break { n, stdout: bs, stderr: be })) => {
                    stdout.push_str(&bs);
                    stderr.push_str(&be);
                    return Err(ShellSignal::Flow(ControlFlow::Break { n, stdout, stderr }));
                }
                Err(ShellSignal::Flow(ControlFlow::Continue { n, stdout: cs, stderr: ce })) => {
                    stdout.push_str(&cs);
                    stderr.push_str(&ce);
                    return Err(ShellSignal::Flow(ControlFlow::Continue { n, stdout, stderr }));
                }
                Err(e) => return Err(e),
            }
        }
        Ok(ExecOutput { stdout, stderr, exit_code })
    }

    pub(super) fn evaluate_cond(&mut self, expr: &crate::ast::conditional::CondExpr) -> Result<bool, ShellSignal> {
        use crate::ast::conditional::{CondBinaryOp, CondExpr, CondUnaryOp};
        match expr {
            CondExpr::Binary { op, left, right } => {
                let l = self.expand_word(left)?;
                let r = self.expand_word(right)?;
                let extglob = self.state.shopt.extglob;
                let nocasematch = self.state.shopt.nocasematch;
                Ok(match op {
                    CondBinaryOp::Eq => glob_match(&r, &l, extglob, nocasematch),
                    CondBinaryOp::Ne => !glob_match(&r, &l, extglob, nocasematch),
                    CondBinaryOp::StrLt => l < r,
                    CondBinaryOp::StrGt => l > r,
                    CondBinaryOp::IntEq => l.parse::<i64>().unwrap_or(0) == r.parse::<i64>().unwrap_or(0),
                    CondBinaryOp::IntNe => l.parse::<i64>().unwrap_or(0) != r.parse::<i64>().unwrap_or(0),
                    CondBinaryOp::IntLt => l.parse::<i64>().unwrap_or(0) < r.parse::<i64>().unwrap_or(0),
                    CondBinaryOp::IntLe => l.parse::<i64>().unwrap_or(0) <= r.parse::<i64>().unwrap_or(0),
                    CondBinaryOp::IntGt => l.parse::<i64>().unwrap_or(0) > r.parse::<i64>().unwrap_or(0),
                    CondBinaryOp::IntGe => l.parse::<i64>().unwrap_or(0) >= r.parse::<i64>().unwrap_or(0),
                    CondBinaryOp::RegexMatch => {
                        match regex::Regex::new(&r) {
                            Ok(re) => {
                                if let Some(caps) = re.captures(&l) {
                                    let rematch: Vec<String> = (0..caps.len())
                                        .map(|i| caps.get(i).map_or("", |m| m.as_str()).to_string())
                                        .collect();
                                    self.state.arrays.insert("BASH_REMATCH".to_string(), rematch);
                                    true
                                } else {
                                    self.state.arrays.insert("BASH_REMATCH".to_string(), Vec::new());
                                    false
                                }
                            }
                            Err(_) => false,
                        }
                    }
                    CondBinaryOp::NewerThan | CondBinaryOp::OlderThan | CondBinaryOp::SameFile => false,
                })
            }
            CondExpr::Unary { op, operand } => {
                let val = self.expand_word(operand)?;
                Ok(match op {
                    CondUnaryOp::StringEmpty => val.is_empty(),
                    CondUnaryOp::StringNonEmpty => !val.is_empty(),
                    CondUnaryOp::FileExists | CondUnaryOp::IsFile => {
                        let path = self.resolve_path(&val);
                        match self.fs.stat(&path) {
                            Ok(m) => match op {
                                CondUnaryOp::IsFile => m.is_file(),
                                _ => true,
                            },
                            Err(_) => false,
                        }
                    }
                    CondUnaryOp::IsDirectory => {
                        let path = self.resolve_path(&val);
                        self.fs.stat(&path).is_ok_and(|m| m.is_dir())
                    }
                    CondUnaryOp::IsSymlink => {
                        let path = self.resolve_path(&val);
                        self.fs.lstat(&path).is_ok_and(|m| m.is_symlink())
                    }
                    CondUnaryOp::IsReadable | CondUnaryOp::IsWritable | CondUnaryOp::IsExecutable => {
                        let path = self.resolve_path(&val);
                        self.fs.exists(&path)
                    }
                    CondUnaryOp::NonEmpty => {
                        let path = self.resolve_path(&val);
                        self.fs.stat(&path).is_ok_and(|m| m.size > 0)
                    }
                    CondUnaryOp::VariableSet => self.state.get_var(&val).is_some(),
                    _ => false,
                })
            }
            CondExpr::Not(inner) => Ok(!self.evaluate_cond(inner)?),
            CondExpr::And(l, r) => Ok(self.evaluate_cond(l)? && self.evaluate_cond(r)?),
            CondExpr::Or(l, r) => Ok(self.evaluate_cond(l)? || self.evaluate_cond(r)?),
            CondExpr::Group(inner) => self.evaluate_cond(inner),
        }
    }
}
