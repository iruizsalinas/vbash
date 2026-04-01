use std::fmt::Write;

use regex::Regex;

use super::ast::{Expr, Program};
use super::builtins::{
    awk_regex_replace_all, awk_regex_replace_first, awk_sprintf, looks_numeric,
};
use super::eval::ControlSignal;
use super::value::AwkValue;

impl super::eval::Interpreter {
    pub(super) fn eval(&mut self, expr: &Expr, program: &Program) -> Result<AwkValue, String> {
        match expr {
            Expr::Num(n) => Ok(AwkValue::Num(*n)),
            Expr::Str(s) => Ok(AwkValue::Str(s.clone())),
            Expr::Regex(r) => {
                let re = Regex::new(r).map_err(|e| format!("invalid regex: {e}"))?;
                let field0 = self.fields.first().cloned().unwrap_or_default();
                Ok(AwkValue::Num(if re.is_match(&field0) { 1.0 } else { 0.0 }))
            }
            Expr::Var(name) => {
                Ok(self.vars.get(name).cloned().unwrap_or(AwkValue::Uninit))
            }
            Expr::Field(e) => {
                let idx = self.eval(e, program)?.to_num() as usize;
                if idx < self.fields.len() {
                    Ok(AwkValue::Str(self.fields[idx].clone()))
                } else {
                    Ok(AwkValue::Str(String::new()))
                }
            }
            Expr::Array(name, subs) => {
                let key = self.build_array_key(subs, program)?;
                let val = self
                    .arrays
                    .get(name)
                    .and_then(|a| a.get(&key))
                    .cloned()
                    .unwrap_or(AwkValue::Uninit);
                Ok(val)
            }
            Expr::Assign(target, value) => {
                let val = self.eval(value, program)?;
                self.assign_to(target, val.clone(), program)?;
                Ok(val)
            }
            Expr::CompoundAssign(op, target, value) => {
                let old = self.eval(target, program)?;
                let rhs = self.eval(value, program)?;
                let result = Self::arith_op(op, &old, &rhs)?;
                self.assign_to(target, result.clone(), program)?;
                Ok(result)
            }
            Expr::BinOp(op, left, right) => {
                match op.as_str() {
                    "&&" => {
                        let l = self.eval(left, program)?;
                        if !l.is_true() {
                            return Ok(AwkValue::Num(0.0));
                        }
                        let r = self.eval(right, program)?;
                        Ok(AwkValue::Num(if r.is_true() { 1.0 } else { 0.0 }))
                    }
                    "||" => {
                        let l = self.eval(left, program)?;
                        if l.is_true() {
                            return Ok(AwkValue::Num(1.0));
                        }
                        let r = self.eval(right, program)?;
                        Ok(AwkValue::Num(if r.is_true() { 1.0 } else { 0.0 }))
                    }
                    "==" | "!=" | "<" | "<=" | ">" | ">=" => {
                        let l = self.eval(left, program)?;
                        let r = self.eval(right, program)?;
                        Ok(AwkValue::Num(if Self::compare(op, &l, &r) { 1.0 } else { 0.0 }))
                    }
                    _ => {
                        let l = self.eval(left, program)?;
                        let r = self.eval(right, program)?;
                        Self::arith_op(op, &l, &r)
                    }
                }
            }
            Expr::UnaryOp(op, e) => {
                let val = self.eval(e, program)?;
                match op.as_str() {
                    "!" => Ok(AwkValue::Num(if val.is_true() { 0.0 } else { 1.0 })),
                    "-" => Ok(AwkValue::Num(-val.to_num())),
                    "+" => Ok(AwkValue::Num(val.to_num())),
                    _ => Ok(val),
                }
            }
            Expr::PreInc(e) => {
                let val = self.eval(e, program)?.to_num() + 1.0;
                let result = AwkValue::Num(val);
                self.assign_to(e, result.clone(), program)?;
                Ok(result)
            }
            Expr::PreDec(e) => {
                let val = self.eval(e, program)?.to_num() - 1.0;
                let result = AwkValue::Num(val);
                self.assign_to(e, result.clone(), program)?;
                Ok(result)
            }
            Expr::PostInc(e) => {
                let old = self.eval(e, program)?.to_num();
                let new_val = AwkValue::Num(old + 1.0);
                self.assign_to(e, new_val, program)?;
                Ok(AwkValue::Num(old))
            }
            Expr::PostDec(e) => {
                let old = self.eval(e, program)?.to_num();
                let new_val = AwkValue::Num(old - 1.0);
                self.assign_to(e, new_val, program)?;
                Ok(AwkValue::Num(old))
            }
            Expr::Ternary(cond, then_e, else_e) => {
                let v = self.eval(cond, program)?;
                if v.is_true() {
                    self.eval(then_e, program)
                } else {
                    self.eval(else_e, program)
                }
            }
            Expr::MatchOp(left, right) => {
                let s = self.eval(left, program)?.to_str();
                let pat = match right.as_ref() {
                    Expr::Regex(r) => r.clone(),
                    _ => self.eval(right, program)?.to_str(),
                };
                let re = Regex::new(&pat).map_err(|e| format!("invalid regex: {e}"))?;
                Ok(AwkValue::Num(if re.is_match(&s) { 1.0 } else { 0.0 }))
            }
            Expr::NotMatchOp(left, right) => {
                let s = self.eval(left, program)?.to_str();
                let pat = match right.as_ref() {
                    Expr::Regex(r) => r.clone(),
                    _ => self.eval(right, program)?.to_str(),
                };
                let re = Regex::new(&pat).map_err(|e| format!("invalid regex: {e}"))?;
                Ok(AwkValue::Num(if re.is_match(&s) { 0.0 } else { 1.0 }))
            }
            Expr::Concat(left, right) => {
                let l = self.eval(left, program)?.to_str();
                let r = self.eval(right, program)?.to_str();
                Ok(AwkValue::Str(l + &r))
            }
            Expr::InArray(arr, subs) => {
                let key = self.build_array_key(subs, program)?;
                let exists = self
                    .arrays
                    .get(arr)
                    .is_some_and(|a| a.contains_key(&key));
                Ok(AwkValue::Num(if exists { 1.0 } else { 0.0 }))
            }
            Expr::Getline => {
                Ok(AwkValue::Num(0.0))
            }
            Expr::Call(name, args) => {
                self.call_function(name, args, program)
            }
            Expr::Sprintf(args) => {
                if args.is_empty() {
                    return Ok(AwkValue::Str(String::new()));
                }
                let vals: Vec<AwkValue> = args
                    .iter()
                    .map(|e| self.eval(e, program))
                    .collect::<Result<Vec<_>, _>>()?;
                let fmt = vals[0].to_str();
                Ok(AwkValue::Str(awk_sprintf(&fmt, &vals[1..])))
            }
        }
    }

    pub(super) fn assign_to(&mut self, target: &Expr, val: AwkValue, program: &Program) -> Result<(), String> {
        match target {
            Expr::Var(name) => {
                self.vars.insert(name.clone(), val);
                if name == "FS" {
                    self.fs_pattern = self.get_var_str("FS");
                }
            }
            Expr::Field(idx_expr) => {
                let idx = self.eval(idx_expr, program)?.to_num() as usize;
                let s = val.to_str();
                while self.fields.len() <= idx {
                    self.fields.push(String::new());
                }
                self.fields[idx] = s;
                if idx != 0 {
                    self.rebuild_record();
                }
                let nf = self.fields.len() - 1;
                self.vars.insert("NF".to_string(), AwkValue::Num(nf as f64));
            }
            Expr::Array(name, subs) => {
                let key = self.build_array_key(subs, program)?;
                self.arrays
                    .entry(name.clone())
                    .or_default()
                    .insert(key, val);
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn compare(op: &str, left: &AwkValue, right: &AwkValue) -> bool {
        let both_numeric = matches!(
            (left, right),
            (AwkValue::Num(_), AwkValue::Num(_))
                | (AwkValue::Num(_), AwkValue::Uninit)
                | (AwkValue::Uninit, AwkValue::Num(_))
                | (AwkValue::Uninit, AwkValue::Uninit)
        );

        let use_numeric = both_numeric || {
            let ls = left.to_str();
            let rs = right.to_str();
            looks_numeric(&ls) && looks_numeric(&rs)
        };

        if use_numeric {
            let ln = left.to_num();
            let rn = right.to_num();
            match op {
                "==" => (ln - rn).abs() < f64::EPSILON,
                "!=" => (ln - rn).abs() >= f64::EPSILON,
                "<" => ln < rn,
                "<=" => ln <= rn,
                ">" => ln > rn,
                ">=" => ln >= rn,
                _ => false,
            }
        } else {
            let ls = left.to_str();
            let rs = right.to_str();
            match op {
                "==" => ls == rs,
                "!=" => ls != rs,
                "<" => ls < rs,
                "<=" => ls <= rs,
                ">" => ls > rs,
                ">=" => ls >= rs,
                _ => false,
            }
        }
    }

    pub(super) fn arith_op(op: &str, left: &AwkValue, right: &AwkValue) -> Result<AwkValue, String> {
        let l = left.to_num();
        let r = right.to_num();
        let result = match op {
            "+" => l + r,
            "-" => l - r,
            "*" => l * r,
            "/" => {
                if r == 0.0 {
                    return Err("division by zero".to_string());
                }
                l / r
            }
            "%" => {
                if r == 0.0 {
                    return Err("division by zero".to_string());
                }
                l % r
            }
            "^" => l.powf(r),
            _ => return Err(format!("unknown operator: {op}")),
        };
        Ok(AwkValue::Num(result))
    }

    pub(super) fn eval_regex_as_string(&mut self, expr: &Expr, program: &Program) -> Result<AwkValue, String> {
        if let Expr::Regex(r) = expr {
            return Ok(AwkValue::Str(r.clone()));
        }
        self.eval(expr, program)
    }

    pub(super) fn call_function(&mut self, name: &str, args: &[Expr], program: &Program) -> Result<AwkValue, String> {
        let arg_vals: Vec<AwkValue> = args
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let as_regex = matches!(
                    (name, i),
                    ("sub" | "gsub", 0) | ("match", 1) | ("split", 2)
                );
                if as_regex {
                    self.eval_regex_as_string(a, program)
                } else {
                    self.eval(a, program)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        if let Some(func) = program.functions.get(name).cloned() {
            let saved_vars: Vec<(String, Option<AwkValue>)> = func
                .params
                .iter()
                .map(|p| (p.clone(), self.vars.remove(p)))
                .collect();

            for (i, param) in func.params.iter().enumerate() {
                let val = arg_vals.get(i).cloned().unwrap_or(AwkValue::Uninit);
                self.vars.insert(param.clone(), val);
            }

            let result = match self.exec_stmts(&func.body, program) {
                Ok(()) => AwkValue::Num(0.0),
                Err(ControlSignal::Return(v)) => v,
                Err(other) => {
                    for (name, old) in saved_vars {
                        match old {
                            Some(v) => { self.vars.insert(name, v); }
                            None => { self.vars.remove(&name); }
                        }
                    }
                    return match other {
                        ControlSignal::Exit(c) => Err(format!("exit {c}")),
                        ControlSignal::Next => Err("next in function".to_string()),
                        _ => Ok(AwkValue::Num(0.0)),
                    };
                }
            };

            for (name, old) in saved_vars {
                match old {
                    Some(v) => { self.vars.insert(name, v); }
                    None => { self.vars.remove(&name); }
                }
            }

            return Ok(result);
        }

        match name {
            "length" => {
                let s = arg_vals.first().map_or_else(
                    || self.fields.first().cloned().unwrap_or_default(),
                    AwkValue::to_str,
                );
                Ok(AwkValue::Num(s.len() as f64))
            }
            "substr" => {
                let s = arg_vals.first().map_or_else(String::new, AwkValue::to_str);
                let start = arg_vals.get(1).map_or(1.0, AwkValue::to_num);
                let start_idx = (start as isize - 1).max(0) as usize;
                if let Some(len_val) = arg_vals.get(2) {
                    let len = len_val.to_num().max(0.0) as usize;
                    let end = (start_idx + len).min(s.len());
                    Ok(AwkValue::Str(s.get(start_idx..end).unwrap_or("").to_string()))
                } else {
                    Ok(AwkValue::Str(s.get(start_idx..).unwrap_or("").to_string()))
                }
            }
            "index" => {
                let s = arg_vals.first().map_or_else(String::new, AwkValue::to_str);
                let t = arg_vals.get(1).map_or_else(String::new, AwkValue::to_str);
                let pos = s.find(&t).map_or(0, |p| p + 1);
                Ok(AwkValue::Num(pos as f64))
            }
            "split" => {
                let s = arg_vals.first().map_or_else(String::new, AwkValue::to_str);
                let arr_name = match args.get(1) {
                    Some(Expr::Var(n)) => n.clone(),
                    _ => return Err("split: second arg must be array name".to_string()),
                };
                let sep = arg_vals.get(2).map_or_else(
                    || self.get_var_str("FS"),
                    AwkValue::to_str,
                );
                let parts: Vec<&str> = if sep == " " {
                    s.split_whitespace().collect()
                } else if let Ok(re) = Regex::new(&sep) {
                    re.split(&s).collect()
                } else {
                    s.split(&sep).collect()
                };
                let arr = self.arrays.entry(arr_name).or_default();
                arr.clear();
                for (i, part) in parts.iter().enumerate() {
                    arr.insert((i + 1).to_string(), AwkValue::Str((*part).to_string()));
                }
                Ok(AwkValue::Num(parts.len() as f64))
            }
            "sub" => {
                let pat = arg_vals.first().map_or_else(String::new, AwkValue::to_str);
                let repl = arg_vals.get(1).map_or_else(String::new, AwkValue::to_str);
                let re = Regex::new(&pat).map_err(|e| format!("sub: {e}"))?;

                let target_expr = args.get(2);
                let target_str = if let Some(expr) = target_expr {
                    self.eval(expr, program)?.to_str()
                } else {
                    self.fields.first().cloned().unwrap_or_default()
                };

                let replaced = awk_regex_replace_first(&re, &target_str, &repl);
                let changed = replaced != target_str;

                if let Some(expr) = target_expr {
                    self.assign_to(expr, AwkValue::Str(replaced), program)?;
                } else {
                    if !self.fields.is_empty() {
                        self.fields[0] = replaced;
                    }
                    self.resplit_from_record();
                }

                Ok(AwkValue::Num(if changed { 1.0 } else { 0.0 }))
            }
            "gsub" => {
                let pat = arg_vals.first().map_or_else(String::new, AwkValue::to_str);
                let repl = arg_vals.get(1).map_or_else(String::new, AwkValue::to_str);
                let re = Regex::new(&pat).map_err(|e| format!("gsub: {e}"))?;

                let target_expr = args.get(2);
                let target_str = if let Some(expr) = target_expr {
                    self.eval(expr, program)?.to_str()
                } else {
                    self.fields.first().cloned().unwrap_or_default()
                };

                let (replaced, count) = awk_regex_replace_all(&re, &target_str, &repl);

                if let Some(expr) = target_expr {
                    self.assign_to(expr, AwkValue::Str(replaced), program)?;
                } else {
                    if !self.fields.is_empty() {
                        self.fields[0] = replaced;
                    }
                    self.resplit_from_record();
                }

                Ok(AwkValue::Num(count as f64))
            }
            "match" => {
                let s = arg_vals.first().map_or_else(String::new, AwkValue::to_str);
                let pat = arg_vals.get(1).map_or_else(String::new, AwkValue::to_str);
                let re = Regex::new(&pat).map_err(|e| format!("match: {e}"))?;
                if let Some(m) = re.find(&s) {
                    let start = m.start() + 1;
                    let length = m.len();
                    self.vars.insert("RSTART".to_string(), AwkValue::Num(start as f64));
                    self.vars.insert("RLENGTH".to_string(), AwkValue::Num(length as f64));
                    Ok(AwkValue::Num(start as f64))
                } else {
                    self.vars.insert("RSTART".to_string(), AwkValue::Num(0.0));
                    self.vars.insert("RLENGTH".to_string(), AwkValue::Num(-1.0));
                    Ok(AwkValue::Num(0.0))
                }
            }
            "sprintf" => {
                if arg_vals.is_empty() {
                    return Ok(AwkValue::Str(String::new()));
                }
                let fmt = arg_vals[0].to_str();
                Ok(AwkValue::Str(awk_sprintf(&fmt, &arg_vals[1..])))
            }
            "tolower" => {
                let s = arg_vals.first().map_or_else(String::new, AwkValue::to_str);
                Ok(AwkValue::Str(s.to_lowercase()))
            }
            "toupper" => {
                let s = arg_vals.first().map_or_else(String::new, AwkValue::to_str);
                Ok(AwkValue::Str(s.to_uppercase()))
            }
            "int" => {
                let n = arg_vals.first().map_or(0.0, AwkValue::to_num);
                Ok(AwkValue::Num(n.trunc()))
            }
            "sqrt" => {
                let n = arg_vals.first().map_or(0.0, AwkValue::to_num);
                Ok(AwkValue::Num(n.sqrt()))
            }
            "sin" => {
                let n = arg_vals.first().map_or(0.0, AwkValue::to_num);
                Ok(AwkValue::Num(n.sin()))
            }
            "cos" => {
                let n = arg_vals.first().map_or(0.0, AwkValue::to_num);
                Ok(AwkValue::Num(n.cos()))
            }
            "atan2" => {
                let y = arg_vals.first().map_or(0.0, AwkValue::to_num);
                let x = arg_vals.get(1).map_or(0.0, AwkValue::to_num);
                Ok(AwkValue::Num(y.atan2(x)))
            }
            "exp" => {
                let n = arg_vals.first().map_or(0.0, AwkValue::to_num);
                Ok(AwkValue::Num(n.exp()))
            }
            "log" => {
                let n = arg_vals.first().map_or(0.0, AwkValue::to_num);
                Ok(AwkValue::Num(n.ln()))
            }
            "rand" => {
                let r = self.next_rand();
                Ok(AwkValue::Num(r))
            }
            "srand" => {
                let old = self.rng_state;
                if let Some(v) = arg_vals.first() {
                    self.rng_state = v.to_num() as u64;
                } else {
                    self.rng_state = 42;
                }
                Ok(AwkValue::Num(old as f64))
            }
            "print" => {
                let ofs = self.get_var_str("OFS");
                let ors = self.get_var_str("ORS");
                let vals: Vec<String> = arg_vals.iter().map(AwkValue::to_str).collect();
                let _ = write!(self.output, "{}{ors}", vals.join(&ofs));
                Ok(AwkValue::Num(0.0))
            }
            "system" => {
                Ok(AwkValue::Num(0.0))
            }
            _ => Err(format!("unknown function: {name}")),
        }
    }
}
