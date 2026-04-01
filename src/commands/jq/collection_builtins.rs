use std::collections::HashMap;

use serde_json::Value;

use super::ast::{BinOp, JqExpr};
use super::eval::Evaluator;
use super::helpers::{
    apply_binop, collect_all_paths, collect_leaf_paths, del_path, flatten_array, get_path,
    is_truthy, json_number, set_path, value_cmp, value_contains, value_type,
};

/// `keys` / `keys_unsorted`: extract keys from objects or indices from arrays.
pub(super) fn eval_keys(name: &str, input: &Value) -> Result<Vec<Value>, String> {
    match input {
        Value::Object(map) => {
            let mut keys: Vec<String> = map.keys().cloned().collect();
            if name == "keys" {
                keys.sort();
            }
            Ok(vec![Value::Array(
                keys.into_iter().map(Value::String).collect(),
            )])
        }
        Value::Array(arr) => Ok(vec![Value::Array(
            (0..arr.len()).map(|i| json_number(i as i64)).collect(),
        )]),
        _ => Err(format!("{} has no keys", value_type(input))),
    }
}

/// values: extract values from objects or return array elements.
pub(super) fn eval_values(input: &Value) -> Result<Vec<Value>, String> {
    match input {
        Value::Object(map) => Ok(vec![Value::Array(map.values().cloned().collect())]),
        Value::Array(arr) => Ok(vec![Value::Array(arr.clone())]),
        _ => Err(format!("{} has no values", value_type(input))),
    }
}

/// has: test if an object has a key or array has an index.
pub(super) fn eval_has(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("has requires 1 argument".to_string());
    }
    let key_vals = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
    let mut results = Vec::new();
    for kv in &key_vals {
        let result = match (input, kv) {
            (Value::Object(map), Value::String(k)) => map.contains_key(k),
            (Value::Array(arr), Value::Number(n)) => {
                let idx = n.as_i64().unwrap_or(-1);
                idx >= 0 && (idx as usize) < arr.len()
            }
            _ => false,
        };
        results.push(Value::Bool(result));
    }
    Ok(results)
}

/// in: test if key is in an object or index is in an array.
pub(super) fn eval_in(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("in requires 1 argument".to_string());
    }
    let obj_vals = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
    let mut results = Vec::new();
    for obj in &obj_vals {
        let result = match (input, obj) {
            (Value::String(k), Value::Object(map)) => map.contains_key(k),
            (Value::Number(n), Value::Array(arr)) => {
                let idx = n.as_i64().unwrap_or(-1);
                idx >= 0 && (idx as usize) < arr.len()
            }
            _ => false,
        };
        results.push(Value::Bool(result));
    }
    Ok(results)
}

/// contains: test if input contains the given value.
pub(super) fn eval_contains(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("contains requires 1 argument".to_string());
    }
    let other_vals = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
    let other = other_vals.into_iter().next().unwrap_or(Value::Null);
    Ok(vec![Value::Bool(value_contains(input, &other))])
}

/// inside: test if input is contained in the given value.
pub(super) fn eval_inside(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("inside requires 1 argument".to_string());
    }
    let other_vals = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
    let other = other_vals.into_iter().next().unwrap_or(Value::Null);
    Ok(vec![Value::Bool(value_contains(&other, input))])
}

/// length: return the length of an array, object, string, or number.
pub(super) fn eval_length(input: &Value) -> Result<Vec<Value>, String> {
    match input {
        Value::Array(arr) => Ok(vec![json_number(arr.len() as i64)]),
        Value::Object(map) => Ok(vec![json_number(map.len() as i64)]),
        Value::String(s) => Ok(vec![json_number(s.chars().count() as i64)]),
        Value::Number(n) => {
            let f = n.as_f64().unwrap_or(0.0).abs();
            Ok(vec![super::helpers::json_float(f)])
        }
        Value::Null => Ok(vec![json_number(0)]),
        Value::Bool(_) => Err("boolean has no length".to_string()),
    }
}

/// utf8bytelength: return the byte length of a string.
pub(super) fn eval_utf8bytelength(input: &Value) -> Result<Vec<Value>, String> {
    match input {
        Value::String(s) => Ok(vec![json_number(s.len() as i64)]),
        _ => Err(format!(
            "{} has no utf8bytelength",
            value_type(input)
        )),
    }
}

/// add: sum an array.
pub(super) fn eval_add(input: &Value) -> Result<Vec<Value>, String> {
    match input {
        Value::Array(arr) if arr.is_empty() => Ok(vec![Value::Null]),
        Value::Array(arr) => {
            let mut acc = arr[0].clone();
            for item in &arr[1..] {
                acc = apply_binop(&BinOp::Add, &acc, item)?;
            }
            Ok(vec![acc])
        }
        _ => Err("add requires array input".to_string()),
    }
}

/// any: test if any element satisfies a condition.
pub(super) fn eval_any(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        match input {
            Value::Array(arr) => Ok(vec![Value::Bool(arr.iter().any(is_truthy))]),
            _ => Err("any requires array input".to_string()),
        }
    } else {
        match input {
            Value::Array(arr) => {
                for item in arr {
                    let vals =
                        evaluator.eval_inner(&args[0], item, vars, depth + 1)?;
                    if vals.iter().any(is_truthy) {
                        return Ok(vec![Value::Bool(true)]);
                    }
                }
                Ok(vec![Value::Bool(false)])
            }
            _ => Err("any requires array input".to_string()),
        }
    }
}

/// all: test if all elements satisfy a condition.
pub(super) fn eval_all(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        match input {
            Value::Array(arr) => Ok(vec![Value::Bool(arr.iter().all(is_truthy))]),
            _ => Err("all requires array input".to_string()),
        }
    } else {
        match input {
            Value::Array(arr) => {
                for item in arr {
                    let vals =
                        evaluator.eval_inner(&args[0], item, vars, depth + 1)?;
                    if !vals.iter().all(is_truthy) {
                        return Ok(vec![Value::Bool(false)]);
                    }
                }
                Ok(vec![Value::Bool(true)])
            }
            _ => Err("all requires array input".to_string()),
        }
    }
}

/// unique: remove duplicates from an array (sorted).
pub(super) fn eval_unique(input: &Value) -> Result<Vec<Value>, String> {
    match input {
        Value::Array(arr) => {
            let mut seen = Vec::new();
            for item in arr {
                if !seen.contains(item) {
                    seen.push(item.clone());
                }
            }
            seen.sort_by(value_cmp);
            Ok(vec![Value::Array(seen)])
        }
        _ => Err("unique requires array input".to_string()),
    }
}

/// `unique_by`: remove duplicates based on a key function.
pub(super) fn eval_unique_by(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("unique_by requires 1 argument".to_string());
    }
    match input {
        Value::Array(arr) => {
            let mut seen_keys = Vec::new();
            let mut result = Vec::new();
            for item in arr {
                let key_vals =
                    evaluator.eval_inner(&args[0], item, vars, depth + 1)?;
                let key = key_vals.into_iter().next().unwrap_or(Value::Null);
                if !seen_keys.contains(&key) {
                    seen_keys.push(key);
                    result.push(item.clone());
                }
            }
            Ok(vec![Value::Array(result)])
        }
        _ => Err("unique_by requires array input".to_string()),
    }
}

/// `group_by`: group array elements by a key function.
pub(super) fn eval_group_by(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("group_by requires 1 argument".to_string());
    }
    match input {
        Value::Array(arr) => {
            let mut keyed: Vec<(Value, Value)> = Vec::new();
            for item in arr {
                let key_vals =
                    evaluator.eval_inner(&args[0], item, vars, depth + 1)?;
                let key = key_vals.into_iter().next().unwrap_or(Value::Null);
                keyed.push((key, item.clone()));
            }
            keyed.sort_by(|a, b| value_cmp(&a.0, &b.0));
            let mut groups: Vec<Value> = Vec::new();
            let mut current_key: Option<Value> = None;
            let mut current_group: Vec<Value> = Vec::new();
            for (key, val) in keyed {
                if current_key.as_ref() == Some(&key) {
                    current_group.push(val);
                } else {
                    if !current_group.is_empty() {
                        groups.push(Value::Array(std::mem::take(
                            &mut current_group,
                        )));
                    }
                    current_key = Some(key);
                    current_group.push(val);
                }
            }
            if !current_group.is_empty() {
                groups.push(Value::Array(current_group));
            }
            Ok(vec![Value::Array(groups)])
        }
        _ => Err("group_by requires array input".to_string()),
    }
}

/// `sort_by`: sort array elements by a key function.
pub(super) fn eval_sort_by(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("sort_by requires 1 argument".to_string());
    }
    match input {
        Value::Array(arr) => {
            let mut keyed: Vec<(Value, Value)> = Vec::new();
            for item in arr {
                let key_vals =
                    evaluator.eval_inner(&args[0], item, vars, depth + 1)?;
                let key = key_vals.into_iter().next().unwrap_or(Value::Null);
                keyed.push((key, item.clone()));
            }
            keyed.sort_by(|a, b| value_cmp(&a.0, &b.0));
            Ok(vec![Value::Array(
                keyed.into_iter().map(|(_, v)| v).collect(),
            )])
        }
        _ => Err("sort_by requires array input".to_string()),
    }
}

/// sort: sort an array.
pub(super) fn eval_sort(input: &Value) -> Result<Vec<Value>, String> {
    match input {
        Value::Array(arr) => {
            let mut sorted = arr.clone();
            sorted.sort_by(value_cmp);
            Ok(vec![Value::Array(sorted)])
        }
        _ => Err("sort requires array input".to_string()),
    }
}

/// min: return the minimum element.
pub(super) fn eval_min(input: &Value) -> Result<Vec<Value>, String> {
    match input {
        Value::Array(arr) if arr.is_empty() => Ok(vec![Value::Null]),
        Value::Array(arr) => {
            let mut m = &arr[0];
            for item in &arr[1..] {
                if value_cmp(item, m) == std::cmp::Ordering::Less {
                    m = item;
                }
            }
            Ok(vec![m.clone()])
        }
        _ => Err("min requires array input".to_string()),
    }
}

/// `min_by`: return the minimum element by a key function.
pub(super) fn eval_min_by(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("min_by requires 1 argument".to_string());
    }
    match input {
        Value::Array(arr) if arr.is_empty() => Ok(vec![Value::Null]),
        Value::Array(arr) => {
            let mut min_item = &arr[0];
            let mut min_key = evaluator
                .eval_inner(&args[0], &arr[0], vars, depth + 1)?
                .into_iter()
                .next()
                .unwrap_or(Value::Null);
            for item in &arr[1..] {
                let key = evaluator
                    .eval_inner(&args[0], item, vars, depth + 1)?
                    .into_iter()
                    .next()
                    .unwrap_or(Value::Null);
                if value_cmp(&key, &min_key) == std::cmp::Ordering::Less {
                    min_item = item;
                    min_key = key;
                }
            }
            Ok(vec![min_item.clone()])
        }
        _ => Err("min_by requires array input".to_string()),
    }
}

/// max: return the maximum element.
pub(super) fn eval_max(input: &Value) -> Result<Vec<Value>, String> {
    match input {
        Value::Array(arr) if arr.is_empty() => Ok(vec![Value::Null]),
        Value::Array(arr) => {
            let mut m = &arr[0];
            for item in &arr[1..] {
                if value_cmp(item, m) == std::cmp::Ordering::Greater {
                    m = item;
                }
            }
            Ok(vec![m.clone()])
        }
        _ => Err("max requires array input".to_string()),
    }
}

/// `max_by`: return the maximum element by a key function.
pub(super) fn eval_max_by(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("max_by requires 1 argument".to_string());
    }
    match input {
        Value::Array(arr) if arr.is_empty() => Ok(vec![Value::Null]),
        Value::Array(arr) => {
            let mut max_item = &arr[0];
            let mut max_key = evaluator
                .eval_inner(&args[0], &arr[0], vars, depth + 1)?
                .into_iter()
                .next()
                .unwrap_or(Value::Null);
            for item in &arr[1..] {
                let key = evaluator
                    .eval_inner(&args[0], item, vars, depth + 1)?
                    .into_iter()
                    .next()
                    .unwrap_or(Value::Null);
                if value_cmp(&key, &max_key) == std::cmp::Ordering::Greater {
                    max_item = item;
                    max_key = key;
                }
            }
            Ok(vec![max_item.clone()])
        }
        _ => Err("max_by requires array input".to_string()),
    }
}

/// flatten: flatten nested arrays.
pub(super) fn eval_flatten(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    let max_depth = if args.is_empty() {
        i64::MAX
    } else {
        let dv = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
        dv.into_iter()
            .next()
            .and_then(|v| v.as_i64())
            .unwrap_or(i64::MAX)
    };
    match input {
        Value::Array(arr) => {
            let mut result = Vec::new();
            flatten_array(arr, max_depth, 0, &mut result);
            Ok(vec![Value::Array(result)])
        }
        _ => Err("cannot flatten non-array".to_string()),
    }
}

/// reverse: reverse an array or string.
pub(super) fn eval_reverse(input: &Value) -> Result<Vec<Value>, String> {
    match input {
        Value::Array(arr) => {
            let mut r = arr.clone();
            r.reverse();
            Ok(vec![Value::Array(r)])
        }
        Value::String(s) => Ok(vec![Value::String(s.chars().rev().collect())]),
        _ => Err(format!("cannot reverse {}", value_type(input))),
    }
}

/// `to_entries`: convert object to array of `{key, value}` entries.
pub(super) fn eval_to_entries(input: &Value) -> Result<Vec<Value>, String> {
    match input {
        Value::Object(map) => {
            let entries: Vec<Value> = map
                .iter()
                .map(|(k, v)| {
                    let mut entry = serde_json::Map::new();
                    entry.insert("key".to_string(), Value::String(k.clone()));
                    entry.insert("value".to_string(), v.clone());
                    Value::Object(entry)
                })
                .collect();
            Ok(vec![Value::Array(entries)])
        }
        _ => Err(format!(
            "{} cannot be converted to entries",
            value_type(input)
        )),
    }
}

/// `from_entries`: convert array of `{key, value}` entries to object.
pub(super) fn eval_from_entries(input: &Value) -> Result<Vec<Value>, String> {
    match input {
        Value::Array(arr) => {
            let mut map = serde_json::Map::new();
            for entry in arr {
                if let Value::Object(e) = entry {
                    let key = e
                        .get("key")
                        .or_else(|| e.get("name"))
                        .map(|k| match k {
                            Value::String(s) => s.clone(),
                            other => super::format_value(
                                other,
                                super::FormatOpts(super::FormatOpts::COMPACT),
                            ),
                        })
                        .unwrap_or_default();
                    let val = e.get("value").cloned().unwrap_or(Value::Null);
                    map.insert(key, val);
                }
            }
            Ok(vec![Value::Object(map)])
        }
        _ => Err("from_entries requires array input".to_string()),
    }
}

/// `with_entries`: transform object entries.
pub(super) fn eval_with_entries(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("with_entries requires 1 argument".to_string());
    }
    match input {
        Value::Object(map) => {
            let entries: Vec<Value> = map
                .iter()
                .map(|(k, v)| {
                    let mut entry = serde_json::Map::new();
                    entry.insert("key".to_string(), Value::String(k.clone()));
                    entry.insert("value".to_string(), v.clone());
                    Value::Object(entry)
                })
                .collect();
            let mut new_map = serde_json::Map::new();
            for entry in &entries {
                let transformed =
                    evaluator.eval_inner(&args[0], entry, vars, depth + 1)?;
                for t in &transformed {
                    if let Value::Object(e) = t {
                        let key = e
                            .get("key")
                            .or_else(|| e.get("name"))
                            .map(|k| match k {
                                Value::String(s) => s.clone(),
                                other => super::format_value(
                                    other,
                                    super::FormatOpts(super::FormatOpts::COMPACT),
                                ),
                            })
                            .unwrap_or_default();
                        let val =
                            e.get("value").cloned().unwrap_or(Value::Null);
                        new_map.insert(key, val);
                    }
                }
            }
            Ok(vec![Value::Object(new_map)])
        }
        _ => Err("with_entries requires object input".to_string()),
    }
}

/// map: apply a function to each element of an array.
pub(super) fn eval_map(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("map requires 1 argument".to_string());
    }
    match input {
        Value::Array(arr) => {
            let mut results = Vec::new();
            for item in arr {
                results
                    .extend(evaluator.eval_inner(&args[0], item, vars, depth + 1)?);
            }
            Ok(vec![Value::Array(results)])
        }
        _ => Err(format!("cannot map over {}", value_type(input))),
    }
}

/// `map_values`: apply a function to each value of an object or array.
pub(super) fn eval_map_values(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("map_values requires 1 argument".to_string());
    }
    match input {
        Value::Object(map) => {
            let mut new_map = serde_json::Map::new();
            for (k, v) in map {
                let vals =
                    evaluator.eval_inner(&args[0], v, vars, depth + 1)?;
                if let Some(nv) = vals.into_iter().next() {
                    new_map.insert(k.clone(), nv);
                }
            }
            Ok(vec![Value::Object(new_map)])
        }
        Value::Array(arr) => {
            let mut new_arr = Vec::new();
            for v in arr {
                let vals =
                    evaluator.eval_inner(&args[0], v, vars, depth + 1)?;
                if let Some(nv) = vals.into_iter().next() {
                    new_arr.push(nv);
                }
            }
            Ok(vec![Value::Array(new_arr)])
        }
        _ => Err(format!("cannot map_values over {}", value_type(input))),
    }
}

/// select: filter values by a condition.
pub(super) fn eval_select(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("select requires 1 argument".to_string());
    }
    let cond_vals = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
    if cond_vals.iter().any(is_truthy) {
        Ok(vec![input.clone()])
    } else {
        Ok(vec![])
    }
}

/// del: delete paths from input.
pub(super) fn eval_del(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("del requires 1 argument".to_string());
    }
    let paths = evaluator.get_paths(&args[0], input, vars, depth)?;
    let mut result = input.clone();
    let mut sorted_paths = paths;
    sorted_paths.sort_by_key(|b| std::cmp::Reverse(b.len()));
    for path in &sorted_paths {
        result = del_path(&result, path);
    }
    Ok(vec![result])
}

/// path: output the path to the selected value.
pub(super) fn eval_path(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("path requires 1 argument".to_string());
    }
    let paths = evaluator.get_paths(&args[0], input, vars, depth)?;
    Ok(paths.into_iter().map(Value::Array).collect())
}

/// paths: output all paths in the input.
pub(super) fn eval_paths(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        let mut paths = Vec::new();
        collect_leaf_paths(input, &mut vec![], &mut paths, false);
        Ok(paths.into_iter().map(Value::Array).collect())
    } else {
        let mut paths = Vec::new();
        collect_all_paths(input, &mut vec![], &mut paths);
        let mut results = Vec::new();
        for p in &paths {
            if p.is_empty() {
                continue;
            }
            let val = get_path(input, p);
            let filter_vals =
                evaluator.eval_inner(&args[0], &val, vars, depth + 1)?;
            if filter_vals.iter().any(is_truthy) {
                results.push(Value::Array(p.clone()));
            }
        }
        Ok(results)
    }
}

/// `leaf_paths`: output all leaf paths.
pub(super) fn eval_leaf_paths(input: &Value) -> Vec<Value> {
    let mut paths = Vec::new();
    collect_leaf_paths(input, &mut vec![], &mut paths, true);
    paths.into_iter().map(Value::Array).collect()
}

/// getpath: get value at a given path.
pub(super) fn eval_getpath(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("getpath requires 1 argument".to_string());
    }
    let path_vals = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
    let mut results = Vec::new();
    for pv in &path_vals {
        if let Value::Array(path) = pv {
            results.push(get_path(input, path));
        } else {
            return Err("path must be an array".to_string());
        }
    }
    Ok(results)
}

/// setpath: set value at a given path.
pub(super) fn eval_setpath(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.len() < 2 {
        return Err("setpath requires 2 arguments".to_string());
    }
    let path_vals = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
    let val_vals = evaluator.eval_inner(&args[1], input, vars, depth + 1)?;
    let path = path_vals.into_iter().next().unwrap_or(Value::Null);
    let val = val_vals.into_iter().next().unwrap_or(Value::Null);
    if let Value::Array(p) = &path {
        Ok(vec![set_path(input, p, &val)])
    } else {
        Err("path must be an array".to_string())
    }
}

/// delpaths: delete multiple paths.
pub(super) fn eval_delpaths(
    evaluator: &Evaluator,
    args: &[JqExpr],
    input: &Value,
    vars: &HashMap<String, Value>,
    depth: usize,
) -> Result<Vec<Value>, String> {
    if args.is_empty() {
        return Err("delpaths requires 1 argument".to_string());
    }
    let paths_vals = evaluator.eval_inner(&args[0], input, vars, depth + 1)?;
    let paths_val = paths_vals.into_iter().next().unwrap_or(Value::Null);
    if let Value::Array(paths) = paths_val {
        let mut result = input.clone();
        let mut path_vecs: Vec<Vec<Value>> = Vec::new();
        for p in &paths {
            if let Value::Array(pv) = p {
                path_vecs.push(pv.clone());
            }
        }
        path_vecs.sort_by_key(|b| std::cmp::Reverse(b.len()));
        for path in &path_vecs {
            result = del_path(&result, path);
        }
        Ok(vec![result])
    } else {
        Err("delpaths argument must be an array".to_string())
    }
}
