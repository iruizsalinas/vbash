use std::collections::HashMap;
use std::fmt::Write;

use regex::Regex;

use super::ast::{Expr, Pattern, Program, Stmt};
use super::builtins::awk_sprintf;
use super::value::AwkValue;

pub(super) enum ControlSignal {
    Break,
    Continue,
    Next,
    Exit(i32),
    Return(AwkValue),
}

pub(super) struct Interpreter {
    pub(super) vars: HashMap<String, AwkValue>,
    pub(super) arrays: HashMap<String, HashMap<String, AwkValue>>,
    pub(super) fields: Vec<String>,
    pub output: String,
    pub(super) fs_pattern: String,
    pub(super) rng_state: u64,
    pub(super) range_active: HashMap<usize, bool>,
}

impl Interpreter {
    pub fn new(fs: String, pre_vars: Vec<(String, String)>, _program: &Program) -> Self {
        let mut vars = HashMap::new();
        vars.insert("FS".to_string(), AwkValue::Str(fs.clone()));
        vars.insert("OFS".to_string(), AwkValue::Str(" ".to_string()));
        vars.insert("ORS".to_string(), AwkValue::Str("\n".to_string()));
        vars.insert("RS".to_string(), AwkValue::Str("\n".to_string()));
        vars.insert("NR".to_string(), AwkValue::Num(0.0));
        vars.insert("NF".to_string(), AwkValue::Num(0.0));
        vars.insert("FNR".to_string(), AwkValue::Num(0.0));
        vars.insert("FILENAME".to_string(), AwkValue::Str(String::new()));
        vars.insert("SUBSEP".to_string(), AwkValue::Str("\x1c".to_string()));
        vars.insert("RSTART".to_string(), AwkValue::Num(0.0));
        vars.insert("RLENGTH".to_string(), AwkValue::Num(-1.0));
        vars.insert("ARGC".to_string(), AwkValue::Num(0.0));

        for (name, val) in pre_vars {
            vars.insert(name, AwkValue::Str(val));
        }

        Self {
            fs_pattern: fs,
            vars,
            arrays: HashMap::new(),
            fields: vec![String::new()],
            output: String::new(),
            rng_state: 42,
            range_active: HashMap::new(),
        }
    }

    pub fn run(&mut self, input: &str, program: &Program) -> Result<(), String> {
        for rule in &program.rules {
            if matches!(rule.pattern, Pattern::Begin) {
                match self.exec_stmts(&rule.action, program) {
                    Ok(()) | Err(ControlSignal::Next) => {}
                    Err(ControlSignal::Exit(_)) => return Ok(()),
                    Err(ControlSignal::Return(_)) => {}
                    Err(ControlSignal::Break | ControlSignal::Continue) => {}
                }
            }
        }

        let rs = self.get_var_str("RS");
        let lines: Vec<&str> = if rs == "\n" {
            input.lines().collect()
        } else if rs.is_empty() {
            input.split("\n\n")
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect()
        } else if rs.len() == 1 {
            input.split(rs.chars().next().unwrap_or('\n')).collect()
        } else {
            input.lines().collect()
        };

        for line in &lines {
            let nr = self.get_var_num("NR") + 1.0;
            self.vars.insert("NR".to_string(), AwkValue::Num(nr));
            let fnr = self.get_var_num("FNR") + 1.0;
            self.vars.insert("FNR".to_string(), AwkValue::Num(fnr));

            self.set_record(line);

            for (rule_idx, rule) in program.rules.iter().enumerate() {
                if matches!(rule.pattern, Pattern::Begin | Pattern::End) {
                    continue;
                }
                if self.pattern_matches(&rule.pattern, line, rule_idx, program)? {
                    match self.exec_stmts(&rule.action, program) {
                        Ok(()) => {}
                        Err(ControlSignal::Next) => break,
                        Err(ControlSignal::Exit(_)) => {
                            self.run_end(program);
                            return Ok(());
                        }
                        Err(ControlSignal::Return(_)) => {}
                        Err(ControlSignal::Break | ControlSignal::Continue) => {}
                    }
                }
            }
        }

        self.run_end(program);
        Ok(())
    }

    fn run_end(&mut self, program: &Program) {
        for rule in &program.rules {
            if matches!(rule.pattern, Pattern::End) {
                match self.exec_stmts(&rule.action, program) {
                    Ok(()) => {}
                    Err(ControlSignal::Exit(_)) => return,
                    _ => {}
                }
            }
        }
    }

    pub(super) fn set_record(&mut self, line: &str) {
        self.fields.clear();
        self.fields.push(line.to_string());

        let fs = self.get_var_str("FS");
        self.fs_pattern.clone_from(&fs);

        let parts: Vec<&str> = if fs == " " {
            line.split_whitespace().collect()
        } else if fs.len() == 1 {
            line.split(fs.chars().next().unwrap_or(' ')).collect()
        } else if let Ok(re) = Regex::new(&fs) {
            re.split(line).collect()
        } else {
            line.split(&fs).collect()
        };

        for p in &parts {
            self.fields.push((*p).to_string());
        }
        let nf = self.fields.len() - 1;
        self.vars.insert("NF".to_string(), AwkValue::Num(nf as f64));
    }

    pub(super) fn rebuild_record(&mut self) {
        let ofs = self.get_var_str("OFS");
        if self.fields.len() > 1 {
            self.fields[0] = self.fields[1..].join(&ofs);
        }
    }

    pub(super) fn get_var_str(&self, name: &str) -> String {
        self.vars.get(name).map_or_else(String::new, AwkValue::to_str)
    }

    pub(super) fn get_var_num(&self, name: &str) -> f64 {
        self.vars.get(name).map_or(0.0, AwkValue::to_num)
    }

    fn pattern_matches(&mut self, pat: &Pattern, line: &str, rule_idx: usize, program: &Program) -> Result<bool, String> {
        match pat {
            Pattern::All => Ok(true),
            Pattern::Begin | Pattern::End => Ok(false),
            Pattern::Regex(r) => {
                let re = Regex::new(r).map_err(|e| format!("invalid regex: {e}"))?;
                Ok(re.is_match(line))
            }
            Pattern::Expr(e) => {
                let val = self.eval(e, program)?;
                Ok(val.is_true())
            }
            Pattern::Range(start, end) => {
                let active = self.range_active.get(&rule_idx).copied().unwrap_or(false);
                if active {
                    let end_match = self.pattern_matches(end, line, usize::MAX, program)?;
                    if end_match {
                        self.range_active.insert(rule_idx, false);
                    }
                    Ok(true)
                } else {
                    let start_match = self.pattern_matches(start, line, usize::MAX, program)?;
                    if start_match {
                        self.range_active.insert(rule_idx, true);
                        Ok(true)
                    } else {
                        Ok(false)
                    }
                }
            }
        }
    }

    pub(super) fn exec_stmts(&mut self, stmts: &[Stmt], program: &Program) -> Result<(), ControlSignal> {
        for stmt in stmts {
            self.exec_stmt(stmt, program)?;
        }
        Ok(())
    }

    fn exec_stmt(&mut self, stmt: &Stmt, program: &Program) -> Result<(), ControlSignal> {
        match stmt {
            Stmt::Expr(e) => {
                self.eval(e, program).map_err(|e| ControlSignal::Exit(Self::report_err(&e)))?;
            }
            Stmt::Print(exprs, _redir) => {
                let ofs = self.get_var_str("OFS");
                let ors = self.get_var_str("ORS");
                if exprs.is_empty() {
                    let _ = write!(self.output, "{}{ors}", self.fields[0]);
                } else {
                    let vals: Vec<String> = exprs
                        .iter()
                        .map(|e| self.eval(e, program).map(|v| v.to_str()))
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|e| ControlSignal::Exit(Self::report_err(&e)))?;
                    let _ = write!(self.output, "{}{ors}", vals.join(&ofs));
                }
            }
            Stmt::Printf(exprs, _redir) => {
                if exprs.is_empty() {
                    return Ok(());
                }
                let vals: Vec<AwkValue> = exprs
                    .iter()
                    .map(|e| self.eval(e, program))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| ControlSignal::Exit(Self::report_err(&e)))?;
                let fmt = vals[0].to_str();
                let result = awk_sprintf(&fmt, &vals[1..]);
                let _ = write!(self.output, "{result}");
            }
            Stmt::If(cond, then_body, else_body) => {
                let v = self.eval(cond, program).map_err(|e| ControlSignal::Exit(Self::report_err(&e)))?;
                if v.is_true() {
                    self.exec_stmts(then_body, program)?;
                } else if let Some(els) = else_body {
                    self.exec_stmts(els, program)?;
                }
            }
            Stmt::While(cond, body) => {
                let mut iters = 0u32;
                loop {
                    let v = self.eval(cond, program).map_err(|e| ControlSignal::Exit(Self::report_err(&e)))?;
                    if !v.is_true() {
                        break;
                    }
                    match self.exec_stmts(body, program) {
                        Ok(()) => {}
                        Err(ControlSignal::Break) => break,
                        Err(ControlSignal::Continue) => {}
                        Err(other) => return Err(other),
                    }
                    iters += 1;
                    if iters > 100_000 {
                        return Err(ControlSignal::Exit(Self::report_err("loop iteration limit exceeded")));
                    }
                }
            }
            Stmt::DoWhile(body, cond) => {
                let mut iters = 0u32;
                loop {
                    match self.exec_stmts(body, program) {
                        Ok(()) => {}
                        Err(ControlSignal::Break) => break,
                        Err(ControlSignal::Continue) => {}
                        Err(other) => return Err(other),
                    }
                    let v = self.eval(cond, program).map_err(|e| ControlSignal::Exit(Self::report_err(&e)))?;
                    if !v.is_true() {
                        break;
                    }
                    iters += 1;
                    if iters > 100_000 {
                        return Err(ControlSignal::Exit(Self::report_err("loop iteration limit exceeded")));
                    }
                }
            }
            Stmt::For(init, cond, update, body) => {
                if let Some(init_stmt) = init {
                    self.exec_stmt(init_stmt, program)?;
                }
                let mut iters = 0u32;
                loop {
                    if let Some(c) = cond {
                        let v = self.eval(c, program).map_err(|e| ControlSignal::Exit(Self::report_err(&e)))?;
                        if !v.is_true() {
                            break;
                        }
                    }
                    match self.exec_stmts(body, program) {
                        Ok(()) => {}
                        Err(ControlSignal::Break) => break,
                        Err(ControlSignal::Continue) => {}
                        Err(other) => return Err(other),
                    }
                    if let Some(upd) = update {
                        self.exec_stmt(upd, program)?;
                    }
                    iters += 1;
                    if iters > 100_000 {
                        return Err(ControlSignal::Exit(Self::report_err("loop iteration limit exceeded")));
                    }
                }
            }
            Stmt::ForIn(var, arr_name, body) => {
                let keys: Vec<String> = self
                    .arrays
                    .get(arr_name)
                    .map(|a| a.keys().cloned().collect())
                    .unwrap_or_default();
                for key in keys {
                    self.vars.insert(var.clone(), AwkValue::Str(key));
                    match self.exec_stmts(body, program) {
                        Ok(()) => {}
                        Err(ControlSignal::Break) => break,
                        Err(ControlSignal::Continue) => {}
                        Err(other) => return Err(other),
                    }
                }
            }
            Stmt::Break => return Err(ControlSignal::Break),
            Stmt::Continue => return Err(ControlSignal::Continue),
            Stmt::Next => return Err(ControlSignal::Next),
            Stmt::Exit(code) => {
                let c = if let Some(e) = code {
                    self.eval(e, program).map_err(|e| ControlSignal::Exit(Self::report_err(&e)))?.to_num() as i32
                } else {
                    0
                };
                return Err(ControlSignal::Exit(c));
            }
            Stmt::Return(val) => {
                let v = if let Some(e) = val {
                    self.eval(e, program).map_err(|e| ControlSignal::Exit(Self::report_err(&e)))?
                } else {
                    AwkValue::Num(0.0)
                };
                return Err(ControlSignal::Return(v));
            }
            Stmt::Delete(name, subs) => {
                if subs.is_empty() {
                    self.arrays.remove(name);
                } else {
                    let key = self.build_array_key(subs, program)
                        .map_err(|e| ControlSignal::Exit(Self::report_err(&e)))?;
                    if let Some(arr) = self.arrays.get_mut(name) {
                        arr.remove(&key);
                    }
                }
            }
            Stmt::Block(stmts) => {
                self.exec_stmts(stmts, program)?;
            }
        }
        Ok(())
    }

    pub(super) fn report_err(_msg: &str) -> i32 {
        2
    }

    pub(super) fn build_array_key(&mut self, subs: &[Expr], program: &Program) -> Result<String, String> {
        let subsep = self.get_var_str("SUBSEP");
        let parts: Vec<String> = subs
            .iter()
            .map(|e| self.eval(e, program).map(|v| v.to_str()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(parts.join(&subsep))
    }

    pub(super) fn resplit_from_record(&mut self) {
        let record = self.fields.first().cloned().unwrap_or_default();
        self.set_record(&record);
    }

    pub(super) fn next_rand(&mut self) -> f64 {
        self.rng_state = self.rng_state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let bits = (self.rng_state >> 33) as f64;
        bits / (1u64 << 31) as f64
    }
}
