use std::collections::HashMap;

use serde_json::Value;

use super::ast::JqExpr;
use super::eval::Evaluator;
use super::helpers::{
    build_match_object, build_regex_pattern, json_number, value_type,
};

/// `ascii_downcase`: lowercases a string.
pub(super) fn eval_ascii_downcase(input: &Value) -> Result<Vec<Value>, String> {
    match input {
        Value::String(s) => Ok(vec![Value::String(s.to_lowercase())]),
        _ => Err(format!("{} cannot be lowercased", value_type(input))),
    }
}

/// `ascii_upcase`: uppercases a string.
pub(super) fn eval_ascii_upcase(input: &Value) -> Result<Vec<Value>, String> {
    match input {
        Value::String(s) => Ok(vec![Value::String(s.to_uppercase())]),
        _ => Err(format!("{} cannot be uppercased", value_type(input))),
    }
}

/// ltrimstr: strip a prefix from a string.
pub(super) fn eval_ltrimstr(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("ltrimstr requires 1 argument".to_string());
    }
    let sv = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
    let prefix = sv
        .into_iter()
        .next()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    match input {
        Value::String(s) => Ok(vec![Value::String(
            s.strip_prefix(prefix.as_str())
                .unwrap_or(s)
                .to_string(),
        )]),
        _ => Ok(vec![input.clone()]),
    }
}

/// rtrimstr: strip a suffix from a string.
pub(super) fn eval_rtrimstr(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("rtrimstr requires 1 argument".to_string());
    }
    let sv = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
    let suffix = sv
        .into_iter()
        .next()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    match input {
        Value::String(s) => Ok(vec![Value::String(
            s.strip_suffix(suffix.as_str())
                .unwrap_or(s)
                .to_string(),
        )]),
        _ => Ok(vec![input.clone()]),
    }
}

/// startswith: test if a string starts with a prefix.
pub(super) fn eval_startswith(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("startswith requires 1 argument".to_string());
    }
    let sv = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
    let prefix = sv
        .into_iter()
        .next()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    match input {
        Value::String(s) => Ok(vec![Value::Bool(s.starts_with(&prefix))]),
        _ => Ok(vec![Value::Bool(false)]),
    }
}

/// endswith: test if a string ends with a suffix.
pub(super) fn eval_endswith(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("endswith requires 1 argument".to_string());
    }
    let sv = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
    let suffix = sv
        .into_iter()
        .next()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    match input {
        Value::String(s) => Ok(vec![Value::Bool(s.ends_with(&suffix))]),
        _ => Ok(vec![Value::Bool(false)]),
    }
}

/// split: split a string by separator.
pub(super) fn eval_split(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("split requires 1 argument".to_string());
    }
    let sv = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
    let sep = sv
        .into_iter()
        .next()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    match input {
        Value::String(s) => {
            let parts: Vec<Value> = if sep.is_empty() {
                s.chars().map(|c| Value::String(c.to_string())).collect()
            } else {
                s.split(&sep)
                    .map(|p| Value::String(p.to_string()))
                    .collect()
            };
            Ok(vec![Value::Array(parts)])
        }
        _ => Err(format!("{} cannot be split", value_type(input))),
    }
}

/// join: join array elements into a string.
pub(super) fn eval_join(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("join requires 1 argument".to_string());
    }
    let sv = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
    let sep = sv
        .into_iter()
        .next()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    match input {
        Value::Array(arr) => {
            let parts: Vec<String> = arr
                .iter()
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    Value::Null => String::new(),
                    other => super::format_value(
                        other,
                        super::FormatOpts(super::FormatOpts::COMPACT),
                    ),
                })
                .collect();
            Ok(vec![Value::String(parts.join(&sep))])
        }
        _ => Err("join requires array input".to_string()),
    }
}

/// test: test if input matches a regex.
pub(super) fn eval_test(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("test requires 1 argument".to_string());
    }
    let rv = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
    let pattern = rv
        .into_iter()
        .next()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    let flags = if args.len() > 1 {
        let fv = evaluator.eval_inner(&args[1], input, vars, depth + 1)?;
        fv.into_iter()
            .next()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default()
    } else {
        String::new()
    };
    let pat = build_regex_pattern(&pattern, &flags);
    let re = regex::Regex::new(&pat).map_err(|e| format!("invalid regex: {e}"))?;
    match input {
        Value::String(s) => Ok(vec![Value::Bool(re.is_match(s))]),
        _ => Err(format!(
            "test requires string input, got {}",
            value_type(input)
        )),
    }
}

/// match: find regex match in a string.
pub(super) fn eval_match(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("match requires 1 argument".to_string());
    }
    let rv = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
    let pattern = rv
        .into_iter()
        .next()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    let flags = if args.len() > 1 {
        let fv = evaluator.eval_inner(&args[1], input, vars, depth + 1)?;
        fv.into_iter()
            .next()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default()
    } else {
        String::new()
    };
    let global = flags.contains('g');
    let pat = build_regex_pattern(&pattern, &flags);
    let re = regex::Regex::new(&pat).map_err(|e| format!("invalid regex: {e}"))?;
    match input {
        Value::String(s) => {
            let mut results = Vec::new();
            if global {
                for m in re.find_iter(s) {
                    results.push(build_match_object(&re, s, m.start(), m.end()));
                }
            } else if let Some(m) = re.find(s) {
                results.push(build_match_object(&re, s, m.start(), m.end()));
            } else {
                return Err("match: no match".to_string());
            }
            Ok(results)
        }
        _ => Err("match requires string input".to_string()),
    }
}

/// capture: named capture groups from a regex match.
pub(super) fn eval_capture(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("capture requires 1 argument".to_string());
    }
    let rv = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
    let pattern = rv
        .into_iter()
        .next()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    let flags = if args.len() > 1 {
        let fv = evaluator.eval_inner(&args[1], input, vars, depth + 1)?;
        fv.into_iter()
            .next()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default()
    } else {
        String::new()
    };
    let pat = build_regex_pattern(&pattern, &flags);
    let re = regex::Regex::new(&pat).map_err(|e| format!("invalid regex: {e}"))?;
    match input {
        Value::String(s) => {
            if let Some(caps) = re.captures(s) {
                let mut obj = serde_json::Map::new();
                for name in re.capture_names().flatten() {
                    if let Some(m) = caps.name(name) {
                        obj.insert(
                            name.to_string(),
                            Value::String(m.as_str().to_string()),
                        );
                    } else {
                        obj.insert(name.to_string(), Value::Null);
                    }
                }
                Ok(vec![Value::Object(obj)])
            } else {
                Err("capture: no match".to_string())
            }
        }
        _ => Err("capture requires string input".to_string()),
    }
}

/// scan: find all regex matches.
pub(super) fn eval_scan(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("scan requires 1 argument".to_string());
    }
    let rv = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
    let pattern = rv
        .into_iter()
        .next()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    let flags = if args.len() > 1 {
        let fv = evaluator.eval_inner(&args[1], input, vars, depth + 1)?;
        fv.into_iter()
            .next()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default()
    } else {
        String::new()
    };
    let pat = build_regex_pattern(&pattern, &flags);
    let re = regex::Regex::new(&pat).map_err(|e| format!("invalid regex: {e}"))?;
    match input {
        Value::String(s) => {
            let mut results = Vec::new();
            for caps in re.captures_iter(s) {
                if re.captures_len() > 1 {
                    let groups: Vec<Value> = (1..caps.len())
                        .map(|i| {
                            caps.get(i).map_or(Value::Null, |m| {
                                Value::String(m.as_str().to_string())
                            })
                        })
                        .collect();
                    results.push(Value::Array(groups));
                } else {
                    results.push(Value::Array(vec![Value::String(
                        caps[0].to_string(),
                    )]));
                }
            }
            Ok(results)
        }
        _ => Err("scan requires string input".to_string()),
    }
}

/// sub: replace first regex match.
pub(super) fn eval_sub(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.len() < 2 {
        return Err("sub requires 2 arguments".to_string());
    }
    let rv = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
    let pattern = rv
        .into_iter()
        .next()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    let repl_vals = evaluator.eval_inner(&args[1], input, vars, depth + 1)?;
    let replacement = repl_vals
        .into_iter()
        .next()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    let flags = if args.len() > 2 {
        let fv = evaluator.eval_inner(&args[2], input, vars, depth + 1)?;
        fv.into_iter()
            .next()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default()
    } else {
        String::new()
    };
    let pat = build_regex_pattern(&pattern, &flags);
    let re = regex::Regex::new(&pat).map_err(|e| format!("invalid regex: {e}"))?;
    match input {
        Value::String(s) => {
            let result = re.replacen(s, 1, replacement.as_str());
            Ok(vec![Value::String(result.into_owned())])
        }
        _ => Err("sub requires string input".to_string()),
    }
}

/// gsub: replace all regex matches.
pub(super) fn eval_gsub(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.len() < 2 {
        return Err("gsub requires 2 arguments".to_string());
    }
    let rv = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
    let pattern = rv
        .into_iter()
        .next()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    let repl_vals = evaluator.eval_inner(&args[1], input, vars, depth + 1)?;
    let replacement = repl_vals
        .into_iter()
        .next()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    let flags = if args.len() > 2 {
        let fv = evaluator.eval_inner(&args[2], input, vars, depth + 1)?;
        fv.into_iter()
            .next()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default()
    } else {
        String::new()
    };
    let pat = build_regex_pattern(&pattern, &flags);
    let re = regex::Regex::new(&pat).map_err(|e| format!("invalid regex: {e}"))?;
    match input {
        Value::String(s) => {
            let result = re.replace_all(s, replacement.as_str());
            Ok(vec![Value::String(result.into_owned())])
        }
        _ => Err("gsub requires string input".to_string()),
    }
}

/// explode: convert string to array of codepoints.
pub(super) fn eval_explode(input: &Value) -> Result<Vec<Value>, String> {
    match input {
        Value::String(s) => {
            let codepoints: Vec<Value> =
                s.chars().map(|c| json_number(c as i64)).collect();
            Ok(vec![Value::Array(codepoints)])
        }
        _ => Err("explode requires string input".to_string()),
    }
}

/// implode: convert array of codepoints to string.
pub(super) fn eval_implode(input: &Value) -> Result<Vec<Value>, String> {
    match input {
        Value::Array(arr) => {
            let mut s = String::new();
            for v in arr {
                if let Some(n) = v.as_u64() {
                    if let Some(c) = char::from_u32(n as u32) {
                        s.push(c);
                    }
                }
            }
            Ok(vec![Value::String(s)])
        }
        _ => Err("implode requires array input".to_string()),
    }
}

/// tostring: convert value to string.
pub(super) fn eval_tostring(input: &Value) -> Vec<Value> {
    match input {
        Value::String(_) => vec![input.clone()],
        _ => vec![Value::String(
            serde_json::to_string(input).unwrap_or_default(),
        )],
    }
}

/// tonumber: convert value to number.
pub(super) fn eval_tonumber(input: &Value) -> Result<Vec<Value>, String> {
    match input {
        Value::Number(_) => Ok(vec![input.clone()]),
        Value::String(s) => {
            if let Ok(i) = s.parse::<i64>() {
                Ok(vec![super::helpers::json_number(i)])
            } else if let Ok(f) = s.parse::<f64>() {
                Ok(vec![super::helpers::json_float(f)])
            } else {
                Err(format!("cannot convert {s:?} to number"))
            }
        }
        _ => Err(format!(
            "cannot convert {} to number",
            value_type(input)
        )),
    }
}

/// tojson: serialize value to JSON string.
pub(super) fn eval_tojson(input: &Value) -> Vec<Value> {
    vec![Value::String(
        serde_json::to_string(input).unwrap_or_default(),
    )]
}

/// fromjson: parse JSON string to value.
pub(super) fn eval_fromjson(input: &Value) -> Result<Vec<Value>, String> {
    match input {
        Value::String(s) => {
            let val: Value =
                serde_json::from_str(s).map_err(|e| format!("fromjson: {e}"))?;
            Ok(vec![val])
        }
        _ => Err("fromjson requires string input".to_string()),
    }
}

/// indices/index/rindex: find occurrences of a substring or element.
pub(super) fn eval_indices(
    evaluator: &Evaluator,
    name: &str,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err(format!("{name} requires 1 argument"));
    }
    let needle_vals = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
    let needle = needle_vals.into_iter().next().unwrap_or(Value::Null);
    match input {
        Value::Array(arr) => {
            if let Value::Array(sub) = &needle {
                let mut indices = Vec::new();
                if sub.len() <= arr.len() {
                    for i in 0..=(arr.len() - sub.len()) {
                        if arr[i..i + sub.len()] == sub[..] {
                            indices.push(json_number(i as i64));
                        }
                    }
                }
                match name {
                    "index" => Ok(vec![indices
                        .into_iter()
                        .next()
                        .unwrap_or(Value::Null)]),
                    "rindex" => Ok(vec![indices
                        .into_iter()
                        .last()
                        .unwrap_or(Value::Null)]),
                    _ => Ok(vec![Value::Array(indices)]),
                }
            } else {
                let mut indices = Vec::new();
                for (i, item) in arr.iter().enumerate() {
                    if item == &needle {
                        indices.push(json_number(i as i64));
                    }
                }
                match name {
                    "index" => Ok(vec![indices
                        .into_iter()
                        .next()
                        .unwrap_or(Value::Null)]),
                    "rindex" => Ok(vec![indices
                        .into_iter()
                        .last()
                        .unwrap_or(Value::Null)]),
                    _ => Ok(vec![Value::Array(indices)]),
                }
            }
        }
        Value::String(s) => {
            if let Value::String(pat) = &needle {
                let mut indices = Vec::new();
                let mut start = 0;
                while let Some(pos) = s[start..].find(pat.as_str()) {
                    indices.push(json_number((start + pos) as i64));
                    start += pos + 1;
                    if start >= s.len() {
                        break;
                    }
                }
                match name {
                    "index" => Ok(vec![indices
                        .into_iter()
                        .next()
                        .unwrap_or(Value::Null)]),
                    "rindex" => Ok(vec![indices
                        .into_iter()
                        .last()
                        .unwrap_or(Value::Null)]),
                    _ => Ok(vec![Value::Array(indices)]),
                }
            } else {
                Ok(vec![Value::Null])
            }
        }
        _ => Ok(vec![Value::Null]),
    }
}
