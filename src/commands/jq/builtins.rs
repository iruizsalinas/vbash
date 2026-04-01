use std::collections::HashMap;

use serde_json::Value;

use super::ast::JqExpr;
use super::collection_builtins;
use super::eval::{Evaluator, MAX_ITERATIONS};
use super::helpers::{
    is_truthy, json_float, json_number, recurse_value, value_type,
};
use super::string_builtins;

impl Evaluator {
    pub(super) fn eval_func_call(&self, name: &str, args: &[JqExpr], input: &Value, vars: &HashMap<String, Value>, depth: usize) -> Result<Vec<Value>, String> {
        match name {

            "length" => collection_builtins::eval_length(input),
            "utf8bytelength" => collection_builtins::eval_utf8bytelength(input),
            "keys" | "keys_unsorted" => collection_builtins::eval_keys(name, input),
            "values" => collection_builtins::eval_values(input),
            "has" => collection_builtins::eval_has(self, args, input, vars, depth),
            "in" => collection_builtins::eval_in(self, args, input, vars, depth),
            "contains" => collection_builtins::eval_contains(self, args, input, vars, depth),
            "inside" => collection_builtins::eval_inside(self, args, input, vars, depth),
            "map" => collection_builtins::eval_map(self, args, input, vars, depth),
            "map_values" => collection_builtins::eval_map_values(self, args, input, vars, depth),
            "select" => collection_builtins::eval_select(self, args, input, vars, depth),
            "add" => collection_builtins::eval_add(input),
            "any" => collection_builtins::eval_any(self, args, input, vars, depth),
            "all" => collection_builtins::eval_all(self, args, input, vars, depth),
            "unique" => collection_builtins::eval_unique(input),
            "unique_by" => collection_builtins::eval_unique_by(self, args, input, vars, depth),
            "group_by" => collection_builtins::eval_group_by(self, args, input, vars, depth),
            "sort_by" => collection_builtins::eval_sort_by(self, args, input, vars, depth),
            "sort" => collection_builtins::eval_sort(input),
            "min" => collection_builtins::eval_min(input),
            "min_by" => collection_builtins::eval_min_by(self, args, input, vars, depth),
            "max" => collection_builtins::eval_max(input),
            "max_by" => collection_builtins::eval_max_by(self, args, input, vars, depth),
            "flatten" => collection_builtins::eval_flatten(self, args, input, vars, depth),
            "reverse" => collection_builtins::eval_reverse(input),
            "to_entries" => collection_builtins::eval_to_entries(input),
            "from_entries" => collection_builtins::eval_from_entries(input),
            "with_entries" => collection_builtins::eval_with_entries(self, args, input, vars, depth),
            "paths" => collection_builtins::eval_paths(self, args, input, vars, depth),
            "leaf_paths" => Ok(collection_builtins::eval_leaf_paths(input)),
            "path" => collection_builtins::eval_path(self, args, input, vars, depth),
            "getpath" => collection_builtins::eval_getpath(self, args, input, vars, depth),
            "setpath" => collection_builtins::eval_setpath(self, args, input, vars, depth),
            "delpaths" => collection_builtins::eval_delpaths(self, args, input, vars, depth),
            "del" => collection_builtins::eval_del(self, args, input, vars, depth),


            "ascii_downcase" => string_builtins::eval_ascii_downcase(input),
            "ascii_upcase" => string_builtins::eval_ascii_upcase(input),
            "ltrimstr" => string_builtins::eval_ltrimstr(self, args, input, vars, depth),
            "rtrimstr" => string_builtins::eval_rtrimstr(self, args, input, vars, depth),
            "startswith" => string_builtins::eval_startswith(self, args, input, vars, depth),
            "endswith" => string_builtins::eval_endswith(self, args, input, vars, depth),
            "split" => string_builtins::eval_split(self, args, input, vars, depth),
            "join" => string_builtins::eval_join(self, args, input, vars, depth),
            "test" => string_builtins::eval_test(self, args, input, vars, depth),
            "match" => string_builtins::eval_match(self, args, input, vars, depth),
            "capture" => string_builtins::eval_capture(self, args, input, vars, depth),
            "scan" => string_builtins::eval_scan(self, args, input, vars, depth),
            "sub" => string_builtins::eval_sub(self, args, input, vars, depth),
            "gsub" => string_builtins::eval_gsub(self, args, input, vars, depth),
            "explode" => string_builtins::eval_explode(input),
            "implode" => string_builtins::eval_implode(input),
            "tostring" => Ok(string_builtins::eval_tostring(input)),
            "tonumber" => string_builtins::eval_tonumber(input),
            "tojson" => Ok(string_builtins::eval_tojson(input)),
            "fromjson" => string_builtins::eval_fromjson(input),
            "indices" | "index" | "rindex" => string_builtins::eval_indices(self, name, args, input, vars, depth),


            "type" => Ok(vec![Value::String(value_type(input).to_string())]),
            "empty" => Ok(vec![]),
            "not" => Ok(vec![Value::Bool(!is_truthy(input))]),
            "recurse" => {
                if args.is_empty() {
                    let mut results = Vec::new();
                    recurse_value(input, &mut results, 0)?;
                    Ok(results)
                } else {
                    let mut results = vec![input.clone()];
                    let mut current = vec![input.clone()];
                    let mut count = 0;
                    loop {
                        if count >= MAX_ITERATIONS {
                            break;
                        }
                        let mut next = Vec::new();
                        for c in &current {
                            if let Ok(vals) = self.eval_inner(&args[0], c, vars, depth + 1) {
                                for v in vals {
                                    if v != Value::Null {
                                        next.push(v);
                                    }
                                }
                            }
                        }
                        if next.is_empty() {
                            break;
                        }
                        results.extend(next.clone());
                        current = next;
                        count += 1;
                    }
                    Ok(results)
                }
            }
            "env" => Ok(vec![Value::Object(serde_json::Map::new())]),
            "range" => {
                match args.len() {
                    1 => {
                        let nv = self.eval_inner(&args[0], input, vars, depth + 1)?;
                        let n = nv.into_iter().next().and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let mut results = Vec::new();
                        let mut i = 0.0f64;
                        let mut count = 0;
                        while i < n && count < MAX_ITERATIONS {
                            results.push(json_float(i));
                            i += 1.0;
                            count += 1;
                        }
                        Ok(results)
                    }
                    2 => {
                        let av = self.eval_inner(&args[0], input, vars, depth + 1)?;
                        let bv = self.eval_inner(&args[1], input, vars, depth + 1)?;
                        let a = av.into_iter().next().and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let b = bv.into_iter().next().and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let mut results = Vec::new();
                        let mut i = a;
                        let mut count = 0;
                        while i < b && count < MAX_ITERATIONS {
                            results.push(json_float(i));
                            i += 1.0;
                            count += 1;
                        }
                        Ok(results)
                    }
                    3 => {
                        let av = self.eval_inner(&args[0], input, vars, depth + 1)?;
                        let bv = self.eval_inner(&args[1], input, vars, depth + 1)?;
                        let sv = self.eval_inner(&args[2], input, vars, depth + 1)?;
                        let a = av.into_iter().next().and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let b = bv.into_iter().next().and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let step = sv.into_iter().next().and_then(|v| v.as_f64()).unwrap_or(1.0);
                        if step == 0.0 {
                            return Err("range step cannot be zero".to_string());
                        }
                        let mut results = Vec::new();
                        let mut i = a;
                        let mut count = 0;
                        if step > 0.0 {
                            while i < b && count < MAX_ITERATIONS {
                                results.push(json_float(i));
                                i += step;
                                count += 1;
                            }
                        } else {
                            while i > b && count < MAX_ITERATIONS {
                                results.push(json_float(i));
                                i += step;
                                count += 1;
                            }
                        }
                        Ok(results)
                    }
                    _ => Err("range requires 1-3 arguments".to_string()),
                }
            }
            "limit" => {
                if args.len() < 2 {
                    return Err("limit requires 2 arguments".to_string());
                }
                let nv = self.eval_inner(&args[0], input, vars, depth + 1)?;
                let n = nv.into_iter().next().and_then(|v| v.as_i64()).unwrap_or(0);
                if n <= 0 {
                    return Ok(vec![]);
                }
                let vals = self.eval_inner(&args[1], input, vars, depth + 1)?;
                Ok(vals.into_iter().take(n as usize).collect())
            }
            "first" => {
                if args.is_empty() {
                    match input {
                        Value::Array(arr) => Ok(vec![arr.first().cloned().unwrap_or(Value::Null)]),
                        _ => Ok(vec![input.clone()]),
                    }
                } else {
                    let vals = self.eval_inner(&args[0], input, vars, depth + 1)?;
                    Ok(vals.into_iter().take(1).collect())
                }
            }
            "last" => {
                if args.is_empty() {
                    match input {
                        Value::Array(arr) => Ok(vec![arr.last().cloned().unwrap_or(Value::Null)]),
                        _ => Ok(vec![input.clone()]),
                    }
                } else {
                    let vals = self.eval_inner(&args[0], input, vars, depth + 1)?;
                    Ok(vals.into_iter().last().into_iter().collect())
                }
            }
            "nth" => {
                if args.is_empty() {
                    return Err("nth requires at least 1 argument".to_string());
                }
                let nv = self.eval_inner(&args[0], input, vars, depth + 1)?;
                let n = nv.into_iter().next().and_then(|v| v.as_i64()).unwrap_or(0);
                if args.len() > 1 {
                    let vals = self.eval_inner(&args[1], input, vars, depth + 1)?;
                    Ok(vals.into_iter().nth(n as usize).into_iter().collect())
                } else {
                    match input {
                        Value::Array(arr) => Ok(vec![arr.get(n as usize).cloned().unwrap_or(Value::Null)]),
                        _ => Err("nth on non-array without generator".to_string()),
                    }
                }
            }
            "input" | "inputs" => Ok(vec![]),
            "debug" => {
                Ok(vec![input.clone()])
            }
            "ascii" => {
                match input {
                    Value::Number(n) => {
                        let code = n.as_u64().unwrap_or(0);
                        if code < 128 {
                            Ok(vec![Value::String((code as u8 as char).to_string())])
                        } else {
                            Err("ascii value out of range".to_string())
                        }
                    }
                    _ => Err("ascii requires number input".to_string()),
                }
            }
            "infinite" => Ok(vec![json_float(f64::INFINITY)]),
            "nan" => Ok(vec![json_float(f64::NAN)]),
            "isinfinite" => {
                let f = input.as_f64().unwrap_or(0.0);
                Ok(vec![Value::Bool(f.is_infinite())])
            }
            "isnan" => {
                let f = input.as_f64().unwrap_or(0.0);
                Ok(vec![Value::Bool(f.is_nan())])
            }
            "isnormal" => {
                let f = input.as_f64().unwrap_or(0.0);
                Ok(vec![Value::Bool(f.is_normal())])
            }
            "error" => {
                if args.is_empty() {
                    match input {
                        Value::String(s) => Err(s.clone()),
                        _ => Err(serde_json::to_string(input).unwrap_or_default()),
                    }
                } else {
                    let msg_vals = self.eval_inner(&args[0], input, vars, depth + 1)?;
                    let msg = msg_vals.into_iter().next().and_then(|v| v.as_str().map(String::from))
                        .unwrap_or_else(|| "error".to_string());
                    Err(msg)
                }
            }
            "builtins" => {
                let names = vec![
                    "length", "utf8bytelength", "type", "keys", "keys_unsorted", "values", "has", "in",
                    "contains", "inside", "empty", "not", "map", "map_values", "select",
                    "recurse", "env", "path", "getpath", "setpath", "delpaths",
                    "to_entries", "from_entries", "with_entries", "paths", "leaf_paths",
                    "flatten", "range", "add", "any", "all", "unique", "unique_by",
                    "group_by", "sort_by", "sort", "min", "min_by", "max", "max_by",
                    "reverse", "limit", "first", "last", "nth", "indices", "index", "rindex",
                    "input", "inputs", "debug", "ascii_downcase", "ascii_upcase",
                    "ltrimstr", "rtrimstr", "startswith", "endswith", "split", "join",
                    "test", "match", "capture", "scan", "sub", "gsub",
                    "ascii", "explode", "implode", "tojson", "fromjson",
                    "tostring", "tonumber", "infinite", "nan", "isinfinite", "isnan", "isnormal",
                    "error", "builtins", "input_line_number",
                    "repeat", "until", "while", "isempty", "del",
                    "null", "true", "false",
                ];
                Ok(vec![Value::Array(names.into_iter().map(|n| Value::String(format!("{n}/0"))).collect())])
            }
            "input_line_number" => Ok(vec![json_number(0)]),
            "repeat" => {
                if args.is_empty() {
                    return Err("repeat requires 1 argument".to_string());
                }
                let mut results = Vec::new();
                let mut current = vec![input.clone()];
                let mut count = 0;
                loop {
                    if count >= MAX_ITERATIONS || results.len() >= MAX_ITERATIONS {
                        break;
                    }
                    let mut next = Vec::new();
                    for c in &current {
                        match self.eval_inner(&args[0], c, vars, depth + 1) {
                            Ok(vals) => next.extend(vals),
                            Err(_) => return Ok(results),
                        }
                    }
                    if next.is_empty() {
                        break;
                    }
                    results.extend(next.clone());
                    current = next;
                    count += 1;
                }
                Ok(results)
            }
            "until" => {
                if args.len() < 2 {
                    return Err("until requires 2 arguments".to_string());
                }
                let mut current = input.clone();
                let mut count = 0;
                loop {
                    if count >= MAX_ITERATIONS {
                        return Err("until iteration limit".to_string());
                    }
                    let cond_vals = self.eval_inner(&args[0], &current, vars, depth + 1)?;
                    if cond_vals.iter().any(is_truthy) {
                        return Ok(vec![current]);
                    }
                    let update_vals = self.eval_inner(&args[1], &current, vars, depth + 1)?;
                    current = update_vals.into_iter().next().unwrap_or(Value::Null);
                    count += 1;
                }
            }
            "while" => {
                if args.len() < 2 {
                    return Err("while requires 2 arguments".to_string());
                }
                let mut results = Vec::new();
                let mut current = input.clone();
                let mut count = 0;
                loop {
                    if count >= MAX_ITERATIONS {
                        break;
                    }
                    let cond_vals = self.eval_inner(&args[0], &current, vars, depth + 1)?;
                    if !cond_vals.iter().any(is_truthy) {
                        break;
                    }
                    results.push(current.clone());
                    let update_vals = self.eval_inner(&args[1], &current, vars, depth + 1)?;
                    current = update_vals.into_iter().next().unwrap_or(Value::Null);
                    count += 1;
                }
                Ok(results)
            }
            "isempty" => {
                if args.is_empty() {
                    return Err("isempty requires 1 argument".to_string());
                }
                let vals = self.eval_inner(&args[0], input, vars, depth + 1)?;
                Ok(vec![Value::Bool(vals.is_empty())])
            }
            "abs" => {
                match input {
                    Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            Ok(vec![json_number(i.abs())])
                        } else if let Some(f) = n.as_f64() {
                            Ok(vec![json_float(f.abs())])
                        } else {
                            Ok(vec![input.clone()])
                        }
                    }
                    _ => Err(format!("abs requires number, got {}", value_type(input))),
                }
            }
            "floor" => {
                match input {
                    Value::Number(n) => {
                        let f = n.as_f64().unwrap_or(0.0);
                        Ok(vec![json_number(f.floor() as i64)])
                    }
                    _ => Err("floor requires number".to_string()),
                }
            }
            "ceil" => {
                match input {
                    Value::Number(n) => {
                        let f = n.as_f64().unwrap_or(0.0);
                        Ok(vec![json_number(f.ceil() as i64)])
                    }
                    _ => Err("ceil requires number".to_string()),
                }
            }
            "round" => {
                match input {
                    Value::Number(n) => {
                        let f = n.as_f64().unwrap_or(0.0);
                        Ok(vec![json_number(f.round() as i64)])
                    }
                    _ => Err("round requires number".to_string()),
                }
            }
            "sqrt" => {
                match input {
                    Value::Number(n) => {
                        let f = n.as_f64().unwrap_or(0.0);
                        Ok(vec![json_float(f.sqrt())])
                    }
                    _ => Err("sqrt requires number".to_string()),
                }
            }
            "log" | "log2" | "log10" | "exp" | "exp2" | "exp10"
            | "sin" | "cos" | "tan" | "asin" | "acos" | "atan"
            | "fabs" | "cbrt" => {
                match input {
                    Value::Number(n) => {
                        let f = n.as_f64().unwrap_or(0.0);
                        let result = match name {
                            "log" => f.ln(),
                            "log2" => f.log2(),
                            "log10" => f.log10(),
                            "exp" => f.exp(),
                            "exp2" => f.exp2(),
                            "exp10" => (10.0f64).powf(f),
                            "sin" => f.sin(),
                            "cos" => f.cos(),
                            "tan" => f.tan(),
                            "asin" => f.asin(),
                            "acos" => f.acos(),
                            "atan" => f.atan(),
                            "fabs" => f.abs(),
                            "cbrt" => f.cbrt(),
                            _ => f,
                        };
                        Ok(vec![json_float(result)])
                    }
                    _ => Err(format!("{name} requires number")),
                }
            }
            "pow" => {
                if args.len() < 2 {
                    return Err("pow requires 2 arguments".to_string());
                }
                let base_vals = self.eval_inner(&args[0], input, vars, depth + 1)?;
                let exp_vals = self.eval_inner(&args[1], input, vars, depth + 1)?;
                let base = base_vals.into_iter().next().and_then(|v| v.as_f64()).unwrap_or(0.0);
                let exp = exp_vals.into_iter().next().and_then(|v| v.as_f64()).unwrap_or(0.0);
                Ok(vec![json_float(base.powf(exp))])
            }
            "atan2" => {
                if args.len() < 2 {
                    return Err("atan2 requires 2 arguments".to_string());
                }
                let y_vals = self.eval_inner(&args[0], input, vars, depth + 1)?;
                let x_vals = self.eval_inner(&args[1], input, vars, depth + 1)?;
                let y = y_vals.into_iter().next().and_then(|v| v.as_f64()).unwrap_or(0.0);
                let x = x_vals.into_iter().next().and_then(|v| v.as_f64()).unwrap_or(0.0);
                Ok(vec![json_float(y.atan2(x))])
            }
            "transpose" => {
                match input {
                    Value::Array(arr) => {
                        let max_len = arr.iter().filter_map(|v| v.as_array().map(Vec::len)).max().unwrap_or(0);
                        let mut result = Vec::new();
                        for i in 0..max_len {
                            let row: Vec<Value> = arr.iter().map(|v| {
                                v.as_array().and_then(|a| a.get(i).cloned()).unwrap_or(Value::Null)
                            }).collect();
                            result.push(Value::Array(row));
                        }
                        Ok(vec![Value::Array(result)])
                    }
                    _ => Err("transpose requires array input".to_string()),
                }
            }
            _ => {
                Err(format!("{name}/{} is not defined", args.len()))
            }
        }
    }
}
