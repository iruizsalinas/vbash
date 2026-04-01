use std::collections::HashMap;

use serde_json::Value;

use super::ast::{BinOp, JqExpr, StringPart, UnaryOp, UpdateOp};
use super::helpers::{
    apply_binop, apply_format, as_index, collect_all_paths, get_path, is_truthy, json_float,
    json_number, normalize_index, recurse_value, set_path, value_type,
};

pub(super) struct Evaluator {
    _sort_keys: bool,
}

pub(super) const MAX_ITERATIONS: usize = 100_000;

pub(super) struct FuncDefRef<'a> {
    pub(super) name: &'a str,
    pub(super) args: &'a [String],
    pub(super) body: &'a JqExpr,
}

#[derive(Debug)]
struct FuncClosure {
    args: Vec<String>,
    _body: JqExpr,
}

impl std::fmt::Display for FuncClosure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "func({:?})", self.args)
    }
}

impl Evaluator {
    pub(super) fn new(sort_keys: bool) -> Self {
        Self { _sort_keys: sort_keys }
    }

    pub(super) fn evaluate(&self, expr: &JqExpr, input: &Value, vars: &HashMap<String, Value>) -> Result<Vec<Value>, String> {
        self.eval_inner(expr, input, vars, 0)
    }

    pub(super) fn eval_inner(&self, expr: &JqExpr, input: &Value, vars: &HashMap<String, Value>, depth: usize) -> Result<Vec<Value>, String> {
        if depth > 1000 {
            return Err("recursion limit exceeded".to_string());
        }
        match expr {
            JqExpr::Identity => Ok(vec![input.clone()]),
            JqExpr::Field(name) => {
                match input {
                    Value::Object(map) => {
                        Ok(vec![map.get(name).cloned().unwrap_or(Value::Null)])
                    }
                    Value::Null => Ok(vec![Value::Null]),
                    _ => Err(format!("null and object are not of type \"{name}\" (input was {})", value_type(input))),
                }
            }
            JqExpr::OptionalField(name) => {
                match input {
                    Value::Object(map) => Ok(vec![map.get(name).cloned().unwrap_or(Value::Null)]),
                    _ => Ok(vec![]),
                }
            }
            JqExpr::Index(idx_expr) => {
                let indices = self.eval_inner(idx_expr, input, vars, depth + 1)?;
                let mut results = Vec::new();
                for idx in &indices {
                    match (input, idx) {
                        (Value::Array(arr), Value::Number(n)) => {
                            let i = n.as_i64().unwrap_or(0);
                            let actual = if i < 0 { arr.len() as i64 + i } else { i };
                            results.push(arr.get(actual as usize).cloned().unwrap_or(Value::Null));
                        }
                        (Value::Object(map), Value::String(key)) => {
                            results.push(map.get(key).cloned().unwrap_or(Value::Null));
                        }
                        (Value::Null, _) => results.push(Value::Null),
                        _ => return Err(format!("cannot index {} with {}", value_type(input), value_type(idx))),
                    }
                }
                Ok(results)
            }
            JqExpr::Slice(start_expr, end_expr) => {
                match input {
                    Value::Array(arr) => {
                        let len = arr.len() as i64;
                        let start = if let Some(se) = start_expr {
                            let vals = self.eval_inner(se, input, vars, depth + 1)?;
                            as_index(&vals).unwrap_or(0)
                        } else { 0 };
                        let end = if let Some(ee) = end_expr {
                            let vals = self.eval_inner(ee, input, vars, depth + 1)?;
                            as_index(&vals).unwrap_or(len)
                        } else { len };
                        let start = normalize_index(start, len);
                        let end = normalize_index(end, len);
                        let start = start.max(0) as usize;
                        let end = end.max(0) as usize;
                        let end = end.min(arr.len());
                        let start = start.min(end);
                        Ok(vec![Value::Array(arr[start..end].to_vec())])
                    }
                    Value::String(s) => {
                        let chars: Vec<char> = s.chars().collect();
                        let len = chars.len() as i64;
                        let start = if let Some(se) = start_expr {
                            let vals = self.eval_inner(se, input, vars, depth + 1)?;
                            as_index(&vals).unwrap_or(0)
                        } else { 0 };
                        let end = if let Some(ee) = end_expr {
                            let vals = self.eval_inner(ee, input, vars, depth + 1)?;
                            as_index(&vals).unwrap_or(len)
                        } else { len };
                        let start = normalize_index(start, len).max(0) as usize;
                        let end = normalize_index(end, len).max(0) as usize;
                        let end = end.min(chars.len());
                        let start = start.min(end);
                        let sliced: String = chars[start..end].iter().collect();
                        Ok(vec![Value::String(sliced)])
                    }
                    _ => Err(format!("cannot slice {}", value_type(input))),
                }
            }
            JqExpr::Iterate => {
                match input {
                    Value::Array(arr) => Ok(arr.clone()),
                    Value::Object(map) => Ok(map.values().cloned().collect()),
                    Value::Null => Ok(vec![]),
                    _ => Err(format!("{} is not iterable", value_type(input))),
                }
            }
            JqExpr::OptionalIterate => {
                match input {
                    Value::Array(arr) => Ok(arr.clone()),
                    Value::Object(map) => Ok(map.values().cloned().collect()),
                    _ => Ok(vec![]),
                }
            }
            JqExpr::Pipe(left, right) => {
                let left_results = self.eval_inner(left, input, vars, depth + 1)?;
                let mut results = Vec::new();
                for val in &left_results {
                    let right_results = self.eval_inner(right, val, vars, depth + 1)?;
                    results.extend(right_results);
                }
                Ok(results)
            }
            JqExpr::Comma(left, right) => {
                let mut results = self.eval_inner(left, input, vars, depth + 1)?;
                results.extend(self.eval_inner(right, input, vars, depth + 1)?);
                Ok(results)
            }
            JqExpr::Literal(val) => Ok(vec![val.clone()]),
            JqExpr::ArrayConstruct(inner) => {
                match inner {
                    Some(expr) => {
                        let vals = self.eval_inner(expr, input, vars, depth + 1)?;
                        Ok(vec![Value::Array(vals)])
                    }
                    None => Ok(vec![Value::Array(Vec::new())]),
                }
            }
            JqExpr::ObjectConstruct(pairs) => {
                let mut obj_results: Vec<serde_json::Map<String, Value>> = vec![serde_json::Map::new()];
                for (key_expr, val_expr) in pairs {
                    let keys = self.eval_inner(key_expr, input, vars, depth + 1)?;
                    let vals = self.eval_inner(val_expr, input, vars, depth + 1)?;
                    let mut new_results = Vec::new();
                    for map in &obj_results {
                        for k in &keys {
                            let key_str = match k {
                                Value::String(s) => s.clone(),
                                _ => super::format_value(k, super::FormatOpts(super::FormatOpts::COMPACT)),
                            };
                            for v in &vals {
                                let mut new_map = map.clone();
                                new_map.insert(key_str.clone(), v.clone());
                                new_results.push(new_map);
                            }
                        }
                    }
                    obj_results = new_results;
                }
                Ok(obj_results.into_iter().map(Value::Object).collect())
            }
            JqExpr::Paren(inner) => self.eval_inner(inner, input, vars, depth + 1),
            JqExpr::BinaryOp(op, left, right) => {
                match op {
                    BinOp::And => {
                        let left_vals = self.eval_inner(left, input, vars, depth + 1)?;
                        let mut results = Vec::new();
                        for lv in &left_vals {
                            if is_truthy(lv) {
                                let right_vals = self.eval_inner(right, input, vars, depth + 1)?;
                                for rv in &right_vals {
                                    results.push(Value::Bool(is_truthy(rv)));
                                }
                            } else {
                                results.push(Value::Bool(false));
                            }
                        }
                        Ok(results)
                    }
                    BinOp::Or => {
                        let left_vals = self.eval_inner(left, input, vars, depth + 1)?;
                        let mut results = Vec::new();
                        for lv in &left_vals {
                            if is_truthy(lv) {
                                results.push(Value::Bool(true));
                            } else {
                                let right_vals = self.eval_inner(right, input, vars, depth + 1)?;
                                for rv in &right_vals {
                                    results.push(Value::Bool(is_truthy(rv)));
                                }
                            }
                        }
                        Ok(results)
                    }
                    BinOp::Alt => {
                        let left_vals = self.eval_inner(left, input, vars, depth + 1)?;
                        let non_null: Vec<Value> = left_vals.into_iter().filter(|v| !v.is_null() && *v != Value::Bool(false)).collect();
                        if non_null.is_empty() {
                            self.eval_inner(right, input, vars, depth + 1)
                        } else {
                            Ok(non_null)
                        }
                    }
                    _ => {
                        let left_vals = self.eval_inner(left, input, vars, depth + 1)?;
                        let right_vals = self.eval_inner(right, input, vars, depth + 1)?;
                        let mut results = Vec::new();
                        for lv in &left_vals {
                            for rv in &right_vals {
                                results.push(apply_binop(op, lv, rv)?);
                            }
                        }
                        Ok(results)
                    }
                }
            }
            JqExpr::UnaryOp(op, inner) => {
                let vals = self.eval_inner(inner, input, vars, depth + 1)?;
                let mut results = Vec::new();
                for v in &vals {
                    match op {
                        UnaryOp::Not => results.push(Value::Bool(!is_truthy(v))),
                        UnaryOp::Neg => {
                            match v {
                                Value::Number(n) => {
                                    if let Some(i) = n.as_i64() {
                                        results.push(json_number(-i));
                                    } else if let Some(f) = n.as_f64() {
                                        results.push(json_float(-f));
                                    } else {
                                        return Err("cannot negate".to_string());
                                    }
                                }
                                _ => return Err(format!("cannot negate {}", value_type(v))),
                            }
                        }
                    }
                }
                Ok(results)
            }
            JqExpr::Conditional { cond, then_branch, elif_branches, else_branch } => {
                let cond_vals = self.eval_inner(cond, input, vars, depth + 1)?;
                let mut results = Vec::new();
                for cv in &cond_vals {
                    if is_truthy(cv) {
                        results.extend(self.eval_inner(then_branch, input, vars, depth + 1)?);
                    } else {
                        let mut handled = false;
                        for (econd, ebody) in elif_branches {
                            let econd_vals = self.eval_inner(econd, input, vars, depth + 1)?;
                            if econd_vals.iter().any(is_truthy) {
                                results.extend(self.eval_inner(ebody, input, vars, depth + 1)?);
                                handled = true;
                                break;
                            }
                        }
                        if !handled {
                            if let Some(eb) = else_branch {
                                results.extend(self.eval_inner(eb, input, vars, depth + 1)?);
                            } else {
                                results.push(input.clone());
                            }
                        }
                    }
                }
                Ok(results)
            }
            JqExpr::TryCatch { try_expr, catch_expr } => {
                match self.eval_inner(try_expr, input, vars, depth + 1) {
                    Ok(vals) => {
                        let filtered: Vec<Value> = vals.into_iter().collect();
                        if filtered.is_empty() {
                            Ok(vec![])
                        } else {
                            Ok(filtered)
                        }
                    }
                    Err(e) => {
                        if let Some(ce) = catch_expr {
                            let err_val = Value::String(e);
                            self.eval_inner(ce, &err_val, vars, depth + 1)
                        } else {
                            Ok(vec![])
                        }
                    }
                }
            }
            JqExpr::VarRef(name) => {
                match vars.get(name) {
                    Some(val) => Ok(vec![val.clone()]),
                    None => {
                        if name == "ENV" || name == "__loc__" {
                            Ok(vec![Value::Object(serde_json::Map::new())])
                        } else {
                            Err(format!("${name} is not defined"))
                        }
                    }
                }
            }
            JqExpr::VarBind { expr, name, body } => {
                let vals = self.eval_inner(expr, input, vars, depth + 1)?;
                let mut results = Vec::new();
                for val in &vals {
                    let mut new_vars = vars.clone();
                    new_vars.insert(name.clone(), val.clone());
                    results.extend(self.eval_inner(body, input, &new_vars, depth + 1)?);
                }
                Ok(results)
            }
            JqExpr::Reduce { expr, name, init, update } => {
                let init_vals = self.eval_inner(init, input, vars, depth + 1)?;
                let mut acc = init_vals.into_iter().next().unwrap_or(Value::Null);
                let items = self.eval_inner(expr, input, vars, depth + 1)?;
                for (count, item) in items.iter().enumerate() {
                    if count >= MAX_ITERATIONS {
                        return Err("reduce iteration limit".to_string());
                    }
                    let mut new_vars = vars.clone();
                    new_vars.insert(name.clone(), item.clone());
                    let update_vals = self.eval_inner(update, &acc, &new_vars, depth + 1)?;
                    acc = update_vals.into_iter().next().unwrap_or(Value::Null);
                }
                Ok(vec![acc])
            }
            JqExpr::Foreach { expr, name, init, update, extract } => {
                let init_vals = self.eval_inner(init, input, vars, depth + 1)?;
                let mut acc = init_vals.into_iter().next().unwrap_or(Value::Null);
                let items = self.eval_inner(expr, input, vars, depth + 1)?;
                let mut results = Vec::new();
                for (count, item) in items.iter().enumerate() {
                    if count >= MAX_ITERATIONS {
                        return Err("foreach iteration limit".to_string());
                    }
                    let mut new_vars = vars.clone();
                    new_vars.insert(name.clone(), item.clone());
                    let update_vals = self.eval_inner(update, &acc, &new_vars, depth + 1)?;
                    acc = update_vals.into_iter().next().unwrap_or(Value::Null);
                    if let Some(ext) = extract {
                        results.extend(self.eval_inner(ext, &acc, &new_vars, depth + 1)?);
                    } else {
                        results.push(acc.clone());
                    }
                }
                Ok(results)
            }
            JqExpr::Recurse => {
                let mut results = Vec::new();
                recurse_value(input, &mut results, 0)?;
                Ok(results)
            }
            JqExpr::StringInterp(parts) => {
                let mut combined = Vec::new();
                self.eval_string_interp(parts, input, vars, depth, &mut combined)?;
                Ok(combined.into_iter().map(Value::String).collect())
            }
            JqExpr::UpdateOp(op, path_expr, val_expr) => {
                self.eval_update(op, path_expr, val_expr, input, vars, depth)
            }
            JqExpr::Optional(inner) => {
                match self.eval_inner(inner, input, vars, depth + 1) {
                    Ok(vals) => Ok(vals),
                    Err(_) => Ok(vec![]),
                }
            }
            JqExpr::Label(name, body) => {
                match self.eval_inner(body, input, vars, depth + 1) {
                    Ok(vals) => Ok(vals),
                    Err(e) if e == format!("break:{name}") => Ok(vec![]),
                    Err(e) => Err(e),
                }
            }
            JqExpr::BreakExpr(label) => {
                Err(format!("break:{label}"))
            }
            JqExpr::FuncDef { name, args, body, next } => {
                let mut new_vars = vars.clone();
                let closure = FuncClosure {
                    args: args.clone(),
                    _body: (**body).clone(),
                };
                new_vars.insert(format!("__func__{name}"), serde_json::to_value(closure.to_string()).unwrap_or(Value::Null));
                if let Some(next_expr) = next {
                    let func_def = FuncDefRef { name, args, body };
                    self.eval_with_func(next_expr, input, &new_vars, depth + 1, &func_def)
                } else {
                    Ok(vec![input.clone()])
                }
            }
            JqExpr::FuncCall(name, call_args) => {
                self.eval_func_call(name, call_args, input, vars, depth)
            }
            JqExpr::Format(name, arg) => {
                let vals = if let Some(a) = arg {
                    self.eval_inner(a, input, vars, depth + 1)?
                } else {
                    vec![input.clone()]
                };
                let mut results = Vec::new();
                for v in &vals {
                    results.push(Value::String(apply_format(name, v)?));
                }
                Ok(results)
            }
        }
    }

    pub(super) fn eval_string_interp(&self, parts: &[StringPart], input: &Value, vars: &HashMap<String, Value>, depth: usize, out: &mut Vec<String>) -> Result<(), String> {
        let mut current = vec![String::new()];
        for part in parts {
            match part {
                StringPart::Literal(s) => {
                    for c in &mut current {
                        c.push_str(s);
                    }
                }
                StringPart::Expr(expr) => {
                    let vals = self.eval_inner(expr, input, vars, depth + 1)?;
                    let mut new_current = Vec::new();
                    for base in &current {
                        for v in &vals {
                            let s = match v {
                                Value::String(s) => s.clone(),
                                Value::Null => "null".to_string(),
                                Value::Bool(b) => b.to_string(),
                                Value::Number(n) => n.to_string(),
                                _ => serde_json::to_string(v).unwrap_or_default(),
                            };
                            let mut combined = base.clone();
                            combined.push_str(&s);
                            new_current.push(combined);
                        }
                    }
                    current = new_current;
                }
            }
        }
        out.extend(current);
        Ok(())
    }

    pub(super) fn eval_update(&self, op: &UpdateOp, path_expr: &JqExpr, val_expr: &JqExpr, input: &Value, vars: &HashMap<String, Value>, depth: usize) -> Result<Vec<Value>, String> {
        match op {
            UpdateOp::Assign => {
                let new_vals = self.eval_inner(val_expr, input, vars, depth + 1)?;
                let new_val = new_vals.into_iter().next().unwrap_or(Value::Null);
                let paths = self.get_paths(path_expr, input, vars, depth)?;
                let mut result = input.clone();
                for path in &paths {
                    result = set_path(&result, path, &new_val);
                }
                Ok(vec![result])
            }
            UpdateOp::Update => {
                let paths = self.get_paths(path_expr, input, vars, depth)?;
                let mut result = input.clone();
                for path in &paths {
                    let current = get_path(&result, path);
                    let new_vals = self.eval_inner(val_expr, &current, vars, depth + 1)?;
                    let new_val = new_vals.into_iter().next().unwrap_or(Value::Null);
                    result = set_path(&result, path, &new_val);
                }
                Ok(vec![result])
            }
            UpdateOp::AddUpdate => self.eval_arithmetic_update(path_expr, val_expr, input, vars, depth, &BinOp::Add),
            UpdateOp::SubUpdate => self.eval_arithmetic_update(path_expr, val_expr, input, vars, depth, &BinOp::Sub),
            UpdateOp::MulUpdate => self.eval_arithmetic_update(path_expr, val_expr, input, vars, depth, &BinOp::Mul),
            UpdateOp::DivUpdate => self.eval_arithmetic_update(path_expr, val_expr, input, vars, depth, &BinOp::Div),
            UpdateOp::ModUpdate => self.eval_arithmetic_update(path_expr, val_expr, input, vars, depth, &BinOp::Mod),
        }
    }

    fn eval_arithmetic_update(&self, path_expr: &JqExpr, val_expr: &JqExpr, input: &Value, vars: &HashMap<String, Value>, depth: usize, op: &BinOp) -> Result<Vec<Value>, String> {
        let paths = self.get_paths(path_expr, input, vars, depth)?;
        let mut result = input.clone();
        for path in &paths {
            let current = get_path(&result, path);
            let rhs_vals = self.eval_inner(val_expr, input, vars, depth + 1)?;
            let rhs = rhs_vals.into_iter().next().unwrap_or(Value::Null);
            let new_val = apply_binop(op, &current, &rhs)?;
            result = set_path(&result, path, &new_val);
        }
        Ok(vec![result])
    }

    pub(super) fn get_paths(&self, expr: &JqExpr, input: &Value, vars: &HashMap<String, Value>, depth: usize) -> Result<Vec<Vec<Value>>, String> {
        match expr {
            JqExpr::Identity => Ok(vec![vec![]]),
            JqExpr::Field(name) => Ok(vec![vec![Value::String(name.clone())]]),
            JqExpr::OptionalField(name) => Ok(vec![vec![Value::String(name.clone())]]),
            JqExpr::Index(idx_expr) => {
                let indices = self.eval_inner(idx_expr, input, vars, depth + 1)?;
                Ok(indices.into_iter().map(|i| vec![i]).collect())
            }
            JqExpr::Iterate => {
                match input {
                    Value::Array(arr) => {
                        Ok((0..arr.len()).map(|i| vec![json_number(i as i64)]).collect())
                    }
                    Value::Object(map) => {
                        Ok(map.keys().map(|k| vec![Value::String(k.clone())]).collect())
                    }
                    _ => Ok(vec![]),
                }
            }
            JqExpr::Pipe(left, right) => {
                let left_paths = self.get_paths(left, input, vars, depth)?;
                let mut results = Vec::new();
                for lp in &left_paths {
                    let intermediate = get_path(input, lp);
                    let right_paths = self.get_paths(right, &intermediate, vars, depth)?;
                    for rp in &right_paths {
                        let mut combined = lp.clone();
                        combined.extend(rp.iter().cloned());
                        results.push(combined);
                    }
                }
                Ok(results)
            }
            JqExpr::Comma(left, right) => {
                let mut paths = self.get_paths(left, input, vars, depth)?;
                paths.extend(self.get_paths(right, input, vars, depth)?);
                Ok(paths)
            }
            JqExpr::Recurse => {
                let mut paths = Vec::new();
                collect_all_paths(input, &mut vec![], &mut paths);
                Ok(paths)
            }
            JqExpr::FuncCall(name, args) if name == "select" => {
                if let Some(filter) = args.first() {
                    let vals = self.eval_inner(filter, input, vars, depth + 1)?;
                    if vals.iter().any(is_truthy) {
                        Ok(vec![vec![]])
                    } else {
                        Ok(vec![])
                    }
                } else {
                    Ok(vec![vec![]])
                }
            }
            _ => {
                let vals = self.eval_inner(expr, input, vars, depth + 1)?;
                if vals.is_empty() {
                    Ok(vec![])
                } else {
                    Ok(vec![vec![]])
                }
            }
        }
    }

    pub(super) fn eval_with_func(&self, expr: &JqExpr, input: &Value, vars: &HashMap<String, Value>, depth: usize, func_def: &FuncDefRef<'_>) -> Result<Vec<Value>, String> {
        match expr {
            JqExpr::FuncCall(name, call_args) if name == func_def.name => {
                let mut new_vars = vars.clone();
                for (i, param) in func_def.args.iter().enumerate() {
                    if let Some(arg_expr) = call_args.get(i) {
                        if let Some(stripped) = param.strip_prefix('$') {
                            let val = self.eval_inner(arg_expr, input, &new_vars, depth + 1)?;
                            new_vars.insert(stripped.to_string(), val.into_iter().next().unwrap_or(Value::Null));
                        }
                    }
                }
                self.eval_with_func(func_def.body, input, &new_vars, depth + 1, func_def)
            }
            JqExpr::Pipe(left, right) => {
                let left_results = self.eval_with_func(left, input, vars, depth + 1, func_def)?;
                let mut results = Vec::new();
                for val in &left_results {
                    let right_results = self.eval_with_func(right, val, vars, depth + 1, func_def)?;
                    results.extend(right_results);
                }
                Ok(results)
            }
            JqExpr::Comma(left, right) => {
                let mut results = self.eval_with_func(left, input, vars, depth + 1, func_def)?;
                results.extend(self.eval_with_func(right, input, vars, depth + 1, func_def)?);
                Ok(results)
            }
            JqExpr::FuncCall(name, call_args) if name == "map" || name == "select" || name == "map_values"
                || name == "sort_by" || name == "group_by" || name == "unique_by" || name == "min_by"
                || name == "max_by" || name == "any" || name == "all" || name == "with_entries"
                || name == "del" || name == "limit" || name == "first" || name == "last"
                || name == "until" || name == "while" || name == "repeat" || name == "isempty"
                || name == "recurse" || name == "path" || name == "paths" => {
                let wrapped_args: Vec<JqExpr> = call_args.iter().map(|a| {
                    Self::wrap_func_calls(a, func_def.name)
                }).collect();
                self.eval_func_call(name, &wrapped_args, input, vars, depth)
            }
            JqExpr::ArrayConstruct(inner) => {
                match inner {
                    Some(expr) => {
                        let vals = self.eval_with_func(expr, input, vars, depth + 1, func_def)?;
                        Ok(vec![Value::Array(vals)])
                    }
                    None => Ok(vec![Value::Array(Vec::new())]),
                }
            }
            JqExpr::Conditional { cond, then_branch, elif_branches, else_branch } => {
                let cond_vals = self.eval_with_func(cond, input, vars, depth + 1, func_def)?;
                let mut results = Vec::new();
                for cv in &cond_vals {
                    if is_truthy(cv) {
                        results.extend(self.eval_with_func(then_branch, input, vars, depth + 1, func_def)?);
                    } else {
                        let mut handled = false;
                        for (econd, ebody) in elif_branches {
                            let econd_vals = self.eval_with_func(econd, input, vars, depth + 1, func_def)?;
                            if econd_vals.iter().any(is_truthy) {
                                results.extend(self.eval_with_func(ebody, input, vars, depth + 1, func_def)?);
                                handled = true;
                                break;
                            }
                        }
                        if !handled {
                            if let Some(eb) = else_branch {
                                results.extend(self.eval_with_func(eb, input, vars, depth + 1, func_def)?);
                            } else {
                                results.push(input.clone());
                            }
                        }
                    }
                }
                Ok(results)
            }
            JqExpr::FuncDef { name: inner_name, args: inner_args, body: inner_body, next } => {
                if let Some(next_expr) = next {
                    let inner_def = FuncDefRef { name: inner_name, args: inner_args, body: inner_body };
                    self.eval_with_func(next_expr, input, vars, depth + 1, &inner_def)
                } else {
                    Ok(vec![input.clone()])
                }
            }
            _ => self.eval_inner(expr, input, vars, depth),
        }
    }

    fn wrap_func_calls(expr: &JqExpr, _func_name: &str) -> JqExpr {
        expr.clone()
    }
}
