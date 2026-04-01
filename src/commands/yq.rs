use std::fmt::Write;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;
use std::collections::HashMap;

fn props_to_json(input: &str) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue;
        }
        let (key, value) = if let Some(eq_pos) = line.find('=') {
            (line[..eq_pos].trim(), line[eq_pos + 1..].trim())
        } else if let Some(col_pos) = line.find(':') {
            (line[..col_pos].trim(), line[col_pos + 1..].trim())
        } else {
            continue;
        };
        map.insert(key.to_string(), serde_json::Value::String(value.to_string()));
    }
    serde_json::Value::Object(map)
}

fn json_to_props(val: &serde_json::Value) -> String {
    let mut out = String::new();
    if let serde_json::Value::Object(map) = val {
        let mut keys: Vec<&String> = map.keys().collect();
        keys.sort();
        for key in keys {
            if let Some(v) = map.get(key) {
                let _ = writeln!(out, "{key}={}", value_to_prop_string(v));
            }
        }
    }
    out
}

fn value_to_prop_string(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

fn evaluate_filter(val: &serde_json::Value, filter: &str) -> Result<Vec<serde_json::Value>, String> {
    let filter = filter.trim();

    if filter.is_empty() || filter == "." {
        return Ok(vec![val.clone()]);
    }

    if filter.contains('|') {
        let segments = split_pipe(filter);
        let mut current = vec![val.clone()];
        for seg in &segments {
            let seg = seg.trim();
            let mut next = Vec::new();
            for item in &current {
                let results = evaluate_filter(item, seg)?;
                next.extend(results);
            }
            current = next;
        }
        return Ok(current);
    }

    if filter == ".[]" {
        return match val {
            serde_json::Value::Array(arr) => Ok(arr.clone()),
            serde_json::Value::Object(map) => {
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                Ok(keys.into_iter().filter_map(|k| map.get(k).cloned()).collect())
            }
            _ => Err(format!("cannot iterate over {}", type_name(val))),
        };
    }

    if let Some(rest) = filter.strip_prefix(".[]") {
        let arr_results = evaluate_filter(val, ".[]")?;
        let mut out = Vec::new();
        for item in &arr_results {
            let sub = evaluate_filter(item, &format!(".{rest}"))?;
            out.extend(sub);
        }
        return Ok(out);
    }

    if filter == "length" {
        let len = match val {
            serde_json::Value::Array(a) => a.len(),
            serde_json::Value::Object(m) => m.len(),
            serde_json::Value::String(s) => s.len(),
            serde_json::Value::Null => 0,
            _ => return Err(format!("{} has no length", type_name(val))),
        };
        return Ok(vec![serde_json::Value::Number(serde_json::Number::from(len))]);
    }

    if filter == "keys" || filter == "keys[]" {
        let keys = match val {
            serde_json::Value::Object(map) => {
                let mut ks: Vec<String> = map.keys().cloned().collect();
                ks.sort();
                ks
            }
            serde_json::Value::Array(arr) => {
                (0..arr.len()).map(|i| i.to_string()).collect()
            }
            _ => return Err(format!("{} has no keys", type_name(val))),
        };
        let arr: Vec<serde_json::Value> = keys.into_iter().map(serde_json::Value::String).collect();
        if filter == "keys[]" {
            return Ok(arr);
        }
        return Ok(vec![serde_json::Value::Array(arr)]);
    }

    if filter == "values" || filter == "values[]" {
        let vals = match val {
            serde_json::Value::Object(map) => {
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                keys.into_iter().filter_map(|k| map.get(k).cloned()).collect::<Vec<_>>()
            }
            serde_json::Value::Array(arr) => arr.clone(),
            _ => return Err(format!("{} has no values", type_name(val))),
        };
        if filter == "values[]" {
            return Ok(vals);
        }
        return Ok(vec![serde_json::Value::Array(vals)]);
    }

    if filter == "type" {
        let t = type_name(val);
        return Ok(vec![serde_json::Value::String(t.to_string())]);
    }

    if filter == "not" {
        let b = matches!(val, serde_json::Value::Bool(false) | serde_json::Value::Null);
        return Ok(vec![serde_json::Value::Bool(b)]);
    }

    if filter == "empty" {
        return Ok(vec![]);
    }

    if filter == "reverse" {
        return match val {
            serde_json::Value::Array(arr) => {
                let mut reversed = arr.clone();
                reversed.reverse();
                Ok(vec![serde_json::Value::Array(reversed)])
            }
            serde_json::Value::String(s) => {
                Ok(vec![serde_json::Value::String(s.chars().rev().collect())])
            }
            _ => Err(format!("{} cannot be reversed", type_name(val))),
        };
    }

    if filter == "flatten" {
        if let serde_json::Value::Array(arr) = val {
            let mut flat = Vec::new();
            for item in arr {
                if let serde_json::Value::Array(inner) = item {
                    flat.extend(inner.iter().cloned());
                } else {
                    flat.push(item.clone());
                }
            }
            return Ok(vec![serde_json::Value::Array(flat)]);
        }
        return Ok(vec![val.clone()]);
    }

    if filter == "sort" {
        if let serde_json::Value::Array(arr) = val {
            let mut sorted = arr.clone();
            sorted.sort_by(json_cmp);
            return Ok(vec![serde_json::Value::Array(sorted)]);
        }
        return Err(format!("{} cannot be sorted", type_name(val)));
    }

    if filter == "unique" {
        if let serde_json::Value::Array(arr) = val {
            let mut sorted = arr.clone();
            sorted.sort_by(json_cmp);
            sorted.dedup_by(|a, b| json_cmp(a, b) == std::cmp::Ordering::Equal);
            return Ok(vec![serde_json::Value::Array(sorted)]);
        }
        return Err(format!("{} cannot be uniqued", type_name(val)));
    }

    if filter == "add" {
        if let serde_json::Value::Array(arr) = val {
            if arr.is_empty() {
                return Ok(vec![serde_json::Value::Null]);
            }
            if arr.iter().all(serde_json::Value::is_number) {
                let sum: f64 = arr.iter()
                    .filter_map(serde_json::Value::as_f64)
                    .sum();
                if sum.fract() == 0.0 {
                    if let Some(i) = serde_json::Number::from_f64(sum) {
                        return Ok(vec![serde_json::Value::Number(i)]);
                    }
                }
                return Ok(vec![serde_json::json!(sum)]);
            }
            if arr.iter().all(serde_json::Value::is_string) {
                let concat: String = arr.iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect();
                return Ok(vec![serde_json::Value::String(concat)]);
            }
            return Ok(vec![serde_json::Value::Null]);
        }
        return Err(format!("{} cannot be added", type_name(val)));
    }

    if filter == "to_entries" {
        if let serde_json::Value::Object(map) = val {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let entries: Vec<serde_json::Value> = keys.into_iter().map(|k| {
                serde_json::json!({"key": k, "value": map.get(k)})
            }).collect();
            return Ok(vec![serde_json::Value::Array(entries)]);
        }
        return Err("not an object".to_string());
    }

    if filter == "from_entries" {
        if let serde_json::Value::Array(arr) = val {
            let mut map = serde_json::Map::new();
            for entry in arr {
                if let (Some(k), Some(v)) = (
                    entry.get("key").or_else(|| entry.get("name")).and_then(|k| {
                        if let serde_json::Value::String(s) = k { Some(s.clone()) } else { None }
                    }),
                    entry.get("value"),
                ) {
                    map.insert(k, v.clone());
                }
            }
            return Ok(vec![serde_json::Value::Object(map)]);
        }
        return Err("not an array".to_string());
    }

    if filter == "tonumber" {
        return match val {
            serde_json::Value::Number(_) => Ok(vec![val.clone()]),
            serde_json::Value::String(s) => {
                if let Ok(n) = s.parse::<i64>() {
                    Ok(vec![serde_json::json!(n)])
                } else if let Ok(n) = s.parse::<f64>() {
                    Ok(vec![serde_json::json!(n)])
                } else {
                    Err(format!("cannot convert {s:?} to number"))
                }
            }
            _ => Err(format!("{} cannot be converted to number", type_name(val))),
        };
    }

    if filter == "tostring" {
        return match val {
            serde_json::Value::String(_) => Ok(vec![val.clone()]),
            _ => Ok(vec![serde_json::Value::String(format_value(val, false))]),
        };
    }

    if let Some(idx_str) = filter.strip_prefix(".[").and_then(|s| s.strip_suffix(']')) {
        if let Ok(idx) = idx_str.parse::<i64>() {
            if let serde_json::Value::Array(arr) = val {
                let actual_idx = if idx < 0 {
                    usize::try_from(i64::try_from(arr.len()).unwrap_or(0) + idx).unwrap_or(0)
                } else {
                    usize::try_from(idx).unwrap_or(0)
                };
                return Ok(vec![arr.get(actual_idx).cloned().unwrap_or(serde_json::Value::Null)]);
            }
            return Ok(vec![serde_json::Value::Null]);
        }
        let key = idx_str.trim_matches('"');
        if let serde_json::Value::Object(map) = val {
            return Ok(vec![map.get(key).cloned().unwrap_or(serde_json::Value::Null)]);
        }
        return Ok(vec![serde_json::Value::Null]);
    }

    if let Some(field_path) = filter.strip_prefix('.') {
        let (first, rest) = match field_path.find('.') {
            Some(pos) => (&field_path[..pos], Some(&field_path[pos..])),
            None => {
                if let Some(bracket_pos) = field_path.find('[') {
                    (&field_path[..bracket_pos], Some(&field_path[bracket_pos..]))
                } else {
                    (field_path, None)
                }
            }
        };

        let first = first.trim_matches('"');
        let intermediate = if let serde_json::Value::Object(map) = val {
            map.get(first).cloned().unwrap_or(serde_json::Value::Null)
        } else {
            serde_json::Value::Null
        };

        return if let Some(remaining) = rest {
            let remaining = if remaining.starts_with('[') {
                format!(".{remaining}")
            } else {
                remaining.to_string()
            };
            evaluate_filter(&intermediate, &remaining)
        } else {
            Ok(vec![intermediate])
        };
    }

    if let Ok(v) = serde_json::from_str::<serde_json::Value>(filter) {
        return Ok(vec![v]);
    }

    Err(format!("unsupported filter: {filter}"))
}

fn split_pipe(filter: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;

    for ch in filter.chars() {
        if escape {
            current.push(ch);
            escape = false;
            continue;
        }
        if ch == '\\' && in_string {
            current.push(ch);
            escape = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            current.push(ch);
            continue;
        }
        if in_string {
            current.push(ch);
            continue;
        }
        match ch {
            '(' | '[' | '{' => { depth += 1; current.push(ch); }
            ')' | ']' | '}' => { depth -= 1; current.push(ch); }
            '|' if depth == 0 => {
                segments.push(current.clone());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        segments.push(current);
    }
    segments
}

fn type_name(val: &serde_json::Value) -> &'static str {
    match val {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn json_cmp(a: &serde_json::Value, b: &serde_json::Value) -> std::cmp::Ordering {
    match (a, b) {
        (serde_json::Value::Number(na), serde_json::Value::Number(nb)) => {
            let fa = na.as_f64().unwrap_or(0.0);
            let fb = nb.as_f64().unwrap_or(0.0);
            fa.partial_cmp(&fb).unwrap_or(std::cmp::Ordering::Equal)
        }
        (serde_json::Value::String(sa), serde_json::Value::String(sb)) => sa.cmp(sb),
        _ => {
            let ta = type_order(a);
            let tb = type_order(b);
            ta.cmp(&tb)
        }
    }
}

fn type_order(val: &serde_json::Value) -> u8 {
    match val {
        serde_json::Value::Null => 0,
        serde_json::Value::Bool(false) => 1,
        serde_json::Value::Bool(true) => 2,
        serde_json::Value::Number(_) => 3,
        serde_json::Value::String(_) => 4,
        serde_json::Value::Array(_) => 5,
        serde_json::Value::Object(_) => 6,
    }
}

fn format_value(val: &serde_json::Value, compact: bool) -> String {
    if compact {
        serde_json::to_string(val).unwrap_or_else(|_| "null".to_string())
    } else {
        serde_json::to_string_pretty(val).unwrap_or_else(|_| "null".to_string())
    }
}

fn format_output(
    results: &[serde_json::Value],
    raw_output: bool,
    compact: bool,
) -> String {
    let mut out = String::new();
    for val in results {
        if raw_output {
            if let serde_json::Value::String(s) = val {
                let _ = writeln!(out, "{s}");
                continue;
            }
        }
        let formatted = format_value(val, compact);
        let _ = writeln!(out, "{formatted}");
    }
    out
}

pub fn yq(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut raw_output = false;
    let mut compact = false;
    let mut inplace = false;
    let mut slurp = false;
    let mut input_format = "json";
    let mut output_format = "json";
    let mut filter: Option<&str> = None;
    let mut file_args: Vec<&str> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-r" | "--raw-output" => raw_output = true,
            "-c" | "--compact" | "--compact-output" => compact = true,
            "-i" | "--inplace" | "--in-place" => inplace = true,
            "-s" | "--slurp" => slurp = true,
            "-p" if i + 1 < args.len() => {
                i += 1;
                input_format = args[i];
            }
            "-o" if i + 1 < args.len() => {
                i += 1;
                output_format = args[i];
            }
            arg if arg.starts_with("--input-format=") => {
                if let Some(fmt) = arg.strip_prefix("--input-format=") {
                    input_format = fmt;
                }
            }
            arg if arg.starts_with("--output-format=") => {
                if let Some(fmt) = arg.strip_prefix("--output-format=") {
                    output_format = fmt;
                }
            }
            arg if !arg.starts_with('-') => {
                if filter.is_none() {
                    filter = Some(arg);
                } else {
                    file_args.push(arg);
                }
            }
            _ => {}
        }
        i += 1;
    }

    let filter_str = filter.unwrap_or(".");

    let inputs: Vec<(Option<String>, String)> = if file_args.is_empty() {
        vec![(None, ctx.stdin.to_string())]
    } else {
        let mut v = Vec::new();
        for path in &file_args {
            let resolved = crate::fs::path::resolve(ctx.cwd, path);
            match ctx.fs.read_file_string(&resolved) {
                Ok(content) => v.push((Some(resolved), content)),
                Err(e) => {
                    return Ok(ExecResult {
                        stdout: String::new(),
                        stderr: format!("yq: {e}\n"),
                        exit_code: 1,
                        env: HashMap::new(),
});
                }
            }
        }
        v
    };

    let parsed: Vec<(Option<String>, serde_json::Value)> = {
        let mut v = Vec::new();
        for (path, content) in &inputs {
            let val = match input_format {
                "props" | "properties" => props_to_json(content),
                _ => match serde_json::from_str(content.trim()) {
                    Ok(val) => val,
                    Err(e) => {
                        return Ok(ExecResult {
                            stdout: String::new(),
                            stderr: format!("yq: parse error: {e}\n"),
                            exit_code: 1,
                            env: HashMap::new(),
});
                    }
                },
            };
            v.push((path.clone(), val));
        }
        v
    };

    if slurp {
        let all_values: Vec<serde_json::Value> = parsed.into_iter().map(|(_, v)| v).collect();
        let combined = serde_json::Value::Array(all_values);
        let results = match evaluate_filter(&combined, filter_str) {
            Ok(r) => r,
            Err(e) => {
                return Ok(ExecResult {
                    stdout: String::new(),
                    stderr: format!("yq: {e}\n"),
                    exit_code: 1,
                    env: HashMap::new(),
});
            }
        };

        let stdout = render_results(&results, raw_output, compact, output_format);
        return Ok(ExecResult { stdout, stderr: String::new(), exit_code: 0, env: HashMap::new() });
    }

    let mut stdout = String::new();
    for (path, val) in &parsed {
        let results = match evaluate_filter(val, filter_str) {
            Ok(r) => r,
            Err(e) => {
                return Ok(ExecResult {
                    stdout: String::new(),
                    stderr: format!("yq: {e}\n"),
                    exit_code: 1,
                    env: HashMap::new(),
});
            }
        };

        let rendered = render_results(&results, raw_output, compact, output_format);

        if inplace {
            if let Some(p) = path {
                if let Err(e) = ctx.fs.write_file(p, rendered.as_bytes()) {
                    return Ok(ExecResult {
                        stdout: String::new(),
                        stderr: format!("yq: {e}\n"),
                        exit_code: 1,
                        env: HashMap::new(),
});
                }
            }
        } else {
            stdout.push_str(&rendered);
        }
    }

    Ok(ExecResult { stdout, stderr: String::new(), exit_code: 0, env: HashMap::new() })
}

fn render_results(
    results: &[serde_json::Value],
    raw_output: bool,
    compact: bool,
    output_format: &str,
) -> String {
    match output_format {
        "props" | "properties" => {
            let mut out = String::new();
            for val in results {
                out.push_str(&json_to_props(val));
            }
            out
        }
        _ => format_output(results, raw_output, compact),
    }
}
