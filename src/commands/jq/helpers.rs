use std::fmt::Write;

use serde_json::Value;

use super::ast::BinOp;

pub(super) fn is_truthy(v: &Value) -> bool {
    !matches!(v, Value::Bool(false) | Value::Null)
}

pub(super) fn value_type(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

pub(super) fn json_number(n: i64) -> Value {
    Value::Number(serde_json::Number::from(n))
}

pub(super) fn json_float(f: f64) -> Value {
    if f.fract() == 0.0 && f.is_finite() && f >= i64::MIN as f64 && f <= i64::MAX as f64 {
        return json_number(f as i64);
    }
    serde_json::Number::from_f64(f).map_or(Value::Null, Value::Number)
}

pub(super) fn normalize_index(idx: i64, len: i64) -> i64 {
    if idx < 0 { len + idx } else { idx }
}

pub(super) fn as_index(vals: &[Value]) -> Option<i64> {
    vals.first().and_then(serde_json::Value::as_i64)
}

pub(super) fn apply_binop(op: &BinOp, left: &Value, right: &Value) -> Result<Value, String> {
    match op {
        BinOp::Add => {
            match (left, right) {
                (Value::Number(a), Value::Number(b)) => {
                    if let (Some(ai), Some(bi)) = (a.as_i64(), b.as_i64()) {
                        Ok(json_number(ai.wrapping_add(bi)))
                    } else {
                        let af = a.as_f64().unwrap_or(0.0);
                        let bf = b.as_f64().unwrap_or(0.0);
                        Ok(json_float(af + bf))
                    }
                }
                (Value::String(a), Value::String(b)) => {
                    let mut s = a.clone();
                    s.push_str(b);
                    Ok(Value::String(s))
                }
                (Value::Array(a), Value::Array(b)) => {
                    let mut r = a.clone();
                    r.extend(b.iter().cloned());
                    Ok(Value::Array(r))
                }
                (Value::Object(a), Value::Object(b)) => {
                    let mut r = a.clone();
                    for (k, v) in b {
                        r.insert(k.clone(), v.clone());
                    }
                    Ok(Value::Object(r))
                }
                (Value::Null, other) | (other, Value::Null) => Ok(other.clone()),
                _ => Err(format!("cannot add {} and {}", value_type(left), value_type(right))),
            }
        }
        BinOp::Sub => {
            match (left, right) {
                (Value::Number(a), Value::Number(b)) => {
                    if let (Some(ai), Some(bi)) = (a.as_i64(), b.as_i64()) {
                        Ok(json_number(ai.wrapping_sub(bi)))
                    } else {
                        let af = a.as_f64().unwrap_or(0.0);
                        let bf = b.as_f64().unwrap_or(0.0);
                        Ok(json_float(af - bf))
                    }
                }
                (Value::Array(a), Value::Array(b)) => {
                    let result: Vec<Value> = a.iter().filter(|v| !b.contains(v)).cloned().collect();
                    Ok(Value::Array(result))
                }
                _ => Err(format!("cannot subtract {} from {}", value_type(right), value_type(left))),
            }
        }
        BinOp::Mul => {
            match (left, right) {
                (Value::Number(a), Value::Number(b)) => {
                    if let (Some(ai), Some(bi)) = (a.as_i64(), b.as_i64()) {
                        Ok(json_number(ai.wrapping_mul(bi)))
                    } else {
                        let af = a.as_f64().unwrap_or(0.0);
                        let bf = b.as_f64().unwrap_or(0.0);
                        Ok(json_float(af * bf))
                    }
                }
                (Value::String(s), Value::Object(obj)) => {
                    let formatted = format_string_interpolation(s, obj);
                    Ok(Value::String(formatted))
                }
                (Value::Object(a), Value::Object(b)) => {
                    let mut result = a.clone();
                    for (k, v) in b {
                        if let Some(existing) = result.get(k) {
                            if let (Value::Object(eo), Value::Object(vo)) = (existing, v) {
                                let mut merged = eo.clone();
                                for (mk, mv) in vo {
                                    merged.insert(mk.clone(), mv.clone());
                                }
                                result.insert(k.clone(), Value::Object(merged));
                                continue;
                            }
                        }
                        result.insert(k.clone(), v.clone());
                    }
                    Ok(Value::Object(result))
                }
                _ => Err(format!("cannot multiply {} and {}", value_type(left), value_type(right))),
            }
        }
        BinOp::Div => {
            match (left, right) {
                (Value::Number(a), Value::Number(b)) => {
                    let bf = b.as_f64().unwrap_or(0.0);
                    if bf == 0.0 {
                        return Err("division by zero".to_string());
                    }
                    let af = a.as_f64().unwrap_or(0.0);
                    Ok(json_float(af / bf))
                }
                (Value::String(s), Value::String(sep)) => {
                    let parts: Vec<Value> = s.split(sep.as_str()).map(|p| Value::String(p.to_string())).collect();
                    Ok(Value::Array(parts))
                }
                _ => Err(format!("cannot divide {} by {}", value_type(left), value_type(right))),
            }
        }
        BinOp::Mod => {
            match (left, right) {
                (Value::Number(a), Value::Number(b)) => {
                    let bi = b.as_i64().unwrap_or(0);
                    if bi == 0 {
                        return Err("modulo by zero".to_string());
                    }
                    let ai = a.as_i64().unwrap_or(0);
                    Ok(json_number(ai % bi))
                }
                _ => Err(format!("cannot modulo {} by {}", value_type(left), value_type(right))),
            }
        }
        BinOp::Eq => Ok(Value::Bool(values_equal(left, right))),
        BinOp::Ne => Ok(Value::Bool(!values_equal(left, right))),
        BinOp::Lt => Ok(Value::Bool(value_cmp(left, right) == std::cmp::Ordering::Less)),
        BinOp::Le => Ok(Value::Bool(value_cmp(left, right) != std::cmp::Ordering::Greater)),
        BinOp::Gt => Ok(Value::Bool(value_cmp(left, right) == std::cmp::Ordering::Greater)),
        BinOp::Ge => Ok(Value::Bool(value_cmp(left, right) != std::cmp::Ordering::Less)),
        BinOp::And | BinOp::Or | BinOp::Alt => {
            Err("unexpected boolean op in apply_binop".to_string())
        }
    }
}

pub(super) fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(na), Value::Number(nb)) => na == nb,
        _ => a == b,
    }
}

pub(super) fn value_cmp(a: &Value, b: &Value) -> std::cmp::Ordering {
    let type_order = |v: &Value| -> u8 {
        match v {
            Value::Null => 0,
            Value::Bool(false) => 1,
            Value::Bool(true) => 2,
            Value::Number(_) => 3,
            Value::String(_) => 4,
            Value::Array(_) => 5,
            Value::Object(_) => 6,
        }
    };
    let ta = type_order(a);
    let tb = type_order(b);
    if ta != tb {
        return ta.cmp(&tb);
    }
    match (a, b) {
        (Value::Number(na), Value::Number(nb)) => {
            let fa = na.as_f64().unwrap_or(0.0);
            let fb = nb.as_f64().unwrap_or(0.0);
            fa.partial_cmp(&fb).unwrap_or(std::cmp::Ordering::Equal)
        }
        (Value::String(sa), Value::String(sb)) => sa.cmp(sb),
        (Value::Array(aa), Value::Array(ab)) => {
            for (x, y) in aa.iter().zip(ab.iter()) {
                let c = value_cmp(x, y);
                if c != std::cmp::Ordering::Equal {
                    return c;
                }
            }
            aa.len().cmp(&ab.len())
        }
        (Value::Object(ma), Value::Object(mb)) => {
            let mut ka: Vec<&String> = ma.keys().collect();
            let mut kb: Vec<&String> = mb.keys().collect();
            ka.sort();
            kb.sort();
            let key_cmp = ka.cmp(&kb);
            if key_cmp != std::cmp::Ordering::Equal {
                return key_cmp;
            }
            for k in &ka {
                let c = value_cmp(
                    ma.get(*k).unwrap_or(&Value::Null),
                    mb.get(*k).unwrap_or(&Value::Null),
                );
                if c != std::cmp::Ordering::Equal {
                    return c;
                }
            }
            std::cmp::Ordering::Equal
        }
        _ => std::cmp::Ordering::Equal,
    }
}

pub(super) fn value_contains(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Object(ma), Value::Object(mb)) => {
            mb.iter().all(|(k, bv)| {
                ma.get(k).is_some_and(|av| value_contains(av, bv))
            })
        }
        (Value::Array(aa), Value::Array(ab)) => {
            ab.iter().all(|bv| aa.iter().any(|av| value_contains(av, bv)))
        }
        (Value::String(sa), Value::String(sb)) => sa.contains(sb.as_str()),
        _ => a == b,
    }
}

pub(super) fn recurse_value(val: &Value, results: &mut Vec<Value>, depth: usize) -> Result<(), String> {
    if depth > 500 {
        return Err("recursion depth limit".to_string());
    }
    results.push(val.clone());
    match val {
        Value::Array(arr) => {
            for item in arr {
                recurse_value(item, results, depth + 1)?;
            }
        }
        Value::Object(map) => {
            for v in map.values() {
                recurse_value(v, results, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn collect_all_paths(val: &Value, current: &mut Vec<Value>, paths: &mut Vec<Vec<Value>>) {
    paths.push(current.clone());
    match val {
        Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                current.push(json_number(i as i64));
                collect_all_paths(item, current, paths);
                current.pop();
            }
        }
        Value::Object(map) => {
            for (k, v) in map {
                current.push(Value::String(k.clone()));
                collect_all_paths(v, current, paths);
                current.pop();
            }
        }
        _ => {}
    }
}

pub(super) fn collect_leaf_paths(val: &Value, current: &mut Vec<Value>, paths: &mut Vec<Vec<Value>>, leaves_only: bool) {
    match val {
        Value::Array(arr) if !arr.is_empty() => {
            if !leaves_only {
                paths.push(current.clone());
            }
            for (i, item) in arr.iter().enumerate() {
                current.push(json_number(i as i64));
                collect_leaf_paths(item, current, paths, leaves_only);
                current.pop();
            }
        }
        Value::Object(map) if !map.is_empty() => {
            if !leaves_only {
                paths.push(current.clone());
            }
            for (k, v) in map {
                current.push(Value::String(k.clone()));
                collect_leaf_paths(v, current, paths, leaves_only);
                current.pop();
            }
        }
        _ => {
            paths.push(current.clone());
        }
    }
}

pub(super) fn flatten_array(arr: &[Value], max_depth: i64, current_depth: i64, result: &mut Vec<Value>) {
    for item in arr {
        if current_depth < max_depth {
            if let Value::Array(inner) = item {
                flatten_array(inner, max_depth, current_depth + 1, result);
                continue;
            }
        }
        result.push(item.clone());
    }
}

pub(super) fn get_path(val: &Value, path: &[Value]) -> Value {
    let mut current = val.clone();
    for component in path {
        match (&current, component) {
            (Value::Object(map), Value::String(key)) => {
                current = map.get(key).cloned().unwrap_or(Value::Null);
            }
            (Value::Array(arr), Value::Number(n)) => {
                let idx = n.as_i64().unwrap_or(0);
                let actual = if idx < 0 { arr.len() as i64 + idx } else { idx };
                current = arr.get(actual as usize).cloned().unwrap_or(Value::Null);
            }
            _ => return Value::Null,
        }
    }
    current
}

pub(super) fn set_path(val: &Value, path: &[Value], new_val: &Value) -> Value {
    if path.is_empty() {
        return new_val.clone();
    }
    let head = &path[0];
    let rest = &path[1..];
    match head {
        Value::String(key) => {
            let mut map = match val {
                Value::Object(m) => m.clone(),
                _ => serde_json::Map::new(),
            };
            let current = map.get(key).cloned().unwrap_or(Value::Null);
            map.insert(key.clone(), set_path(&current, rest, new_val));
            Value::Object(map)
        }
        Value::Number(n) => {
            let idx = n.as_i64().unwrap_or(0);
            let mut arr = match val {
                Value::Array(a) => a.clone(),
                _ => Vec::new(),
            };
            let actual = if idx < 0 {
                (arr.len() as i64 + idx).max(0) as usize
            } else {
                idx as usize
            };
            while arr.len() <= actual {
                arr.push(Value::Null);
            }
            let current = arr[actual].clone();
            arr[actual] = set_path(&current, rest, new_val);
            Value::Array(arr)
        }
        _ => val.clone(),
    }
}

pub(super) fn del_path(val: &Value, path: &[Value]) -> Value {
    if path.is_empty() {
        return Value::Null;
    }
    if path.len() == 1 {
        match (&path[0], val) {
            (Value::String(key), Value::Object(map)) => {
                let mut new_map = map.clone();
                new_map.remove(key);
                Value::Object(new_map)
            }
            (Value::Number(n), Value::Array(arr)) => {
                let idx = n.as_i64().unwrap_or(0);
                let actual = if idx < 0 { (arr.len() as i64 + idx).max(0) as usize } else { idx as usize };
                if actual < arr.len() {
                    let mut new_arr = arr.clone();
                    new_arr.remove(actual);
                    Value::Array(new_arr)
                } else {
                    val.clone()
                }
            }
            _ => val.clone(),
        }
    } else {
        let head = &path[0];
        let rest = &path[1..];
        match (head, val) {
            (Value::String(key), Value::Object(map)) => {
                let mut new_map = map.clone();
                if let Some(inner) = map.get(key) {
                    new_map.insert(key.clone(), del_path(inner, rest));
                }
                Value::Object(new_map)
            }
            (Value::Number(n), Value::Array(arr)) => {
                let idx = n.as_i64().unwrap_or(0);
                let actual = if idx < 0 { (arr.len() as i64 + idx).max(0) as usize } else { idx as usize };
                if actual < arr.len() {
                    let mut new_arr = arr.clone();
                    new_arr[actual] = del_path(&arr[actual], rest);
                    Value::Array(new_arr)
                } else {
                    val.clone()
                }
            }
            _ => val.clone(),
        }
    }
}

fn format_string_interpolation(template: &str, _obj: &serde_json::Map<String, Value>) -> String {
    template.to_string()
}

pub(super) fn build_regex_pattern(pattern: &str, flags: &str) -> String {
    let mut prefix = String::from("(?");
    let mut has_flags = false;
    for c in flags.chars() {
        match c {
            'i' => { prefix.push('i'); has_flags = true; }
            'x' => { prefix.push('x'); has_flags = true; }
            's' => { prefix.push('s'); has_flags = true; }
            'm' => { prefix.push('m'); has_flags = true; }
            'g' => {}
            _ => {}
        }
    }
    if has_flags {
        prefix.push(')');
        format!("{prefix}{pattern}")
    } else {
        pattern.to_string()
    }
}

pub(super) fn build_match_object(re: &regex::Regex, s: &str, start: usize, end: usize) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("offset".to_string(), json_number(start as i64));
    obj.insert("length".to_string(), json_number((end - start) as i64));
    obj.insert("string".to_string(), Value::String(s[start..end].to_string()));
    if let Some(caps) = re.captures(&s[start..]) {
        let mut captures = Vec::new();
        for (i, cap_name) in re.capture_names().enumerate() {
            if i == 0 { continue; }
            if let Some(m) = caps.get(i) {
                let mut cap_obj = serde_json::Map::new();
                cap_obj.insert("offset".to_string(), json_number((start + m.start()) as i64));
                cap_obj.insert("length".to_string(), json_number(m.len() as i64));
                cap_obj.insert("string".to_string(), Value::String(m.as_str().to_string()));
                cap_obj.insert("name".to_string(), cap_name.map_or(Value::Null, |n| Value::String(n.to_string())));
                captures.push(Value::Object(cap_obj));
            } else {
                let mut cap_obj = serde_json::Map::new();
                cap_obj.insert("offset".to_string(), json_number(-1));
                cap_obj.insert("length".to_string(), json_number(0));
                cap_obj.insert("string".to_string(), Value::Null);
                cap_obj.insert("name".to_string(), cap_name.map_or(Value::Null, |n| Value::String(n.to_string())));
                captures.push(Value::Object(cap_obj));
            }
        }
        obj.insert("captures".to_string(), Value::Array(captures));
    } else {
        obj.insert("captures".to_string(), Value::Array(Vec::new()));
    }
    Value::Object(obj)
}

pub(super) fn apply_format(name: &str, val: &Value) -> Result<String, String> {
    match name {
        "text" => {
            match val {
                Value::String(s) => Ok(s.clone()),
                _ => Ok(serde_json::to_string(val).unwrap_or_default()),
            }
        }
        "json" => Ok(serde_json::to_string(val).unwrap_or_default()),
        "html" => {
            let s = match val {
                Value::String(s) => s.clone(),
                _ => serde_json::to_string(val).unwrap_or_default(),
            };
            let mut result = String::new();
            for c in s.chars() {
                match c {
                    '<' => result.push_str("&lt;"),
                    '>' => result.push_str("&gt;"),
                    '&' => result.push_str("&amp;"),
                    '\'' => result.push_str("&#39;"),
                    '"' => result.push_str("&quot;"),
                    other => result.push(other),
                }
            }
            Ok(result)
        }
        "uri" => {
            let s = match val {
                Value::String(s) => s.clone(),
                _ => serde_json::to_string(val).unwrap_or_default(),
            };
            let mut result = String::new();
            for byte in s.bytes() {
                if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
                    result.push(byte as char);
                } else {
                    let _ = write!(result, "%{byte:02X}");
                }
            }
            Ok(result)
        }
        "csv" => {
            match val {
                Value::Array(arr) => {
                    let parts: Vec<String> = arr.iter().map(|v| match v {
                        Value::String(s) => {
                            if s.contains(',') || s.contains('"') || s.contains('\n') {
                                format!("\"{}\"", s.replace('"', "\"\""))
                            } else {
                                s.clone()
                            }
                        }
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => if *b { "true".to_string() } else { "false".to_string() },
                        Value::Null => String::new(),
                        _ => serde_json::to_string(v).unwrap_or_default(),
                    }).collect();
                    Ok(parts.join(","))
                }
                _ => Err("@csv requires array input".to_string()),
            }
        }
        "tsv" => {
            match val {
                Value::Array(arr) => {
                    let parts: Vec<String> = arr.iter().map(|v| match v {
                        Value::String(s) => s.replace('\t', "\\t").replace('\n', "\\n").replace('\r', "\\r").replace('\\', "\\\\"),
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => if *b { "true".to_string() } else { "false".to_string() },
                        Value::Null => String::new(),
                        _ => serde_json::to_string(v).unwrap_or_default(),
                    }).collect();
                    Ok(parts.join("\t"))
                }
                _ => Err("@tsv requires array input".to_string()),
            }
        }
        "base64" => {
            let s = match val {
                Value::String(s) => s.clone(),
                _ => serde_json::to_string(val).unwrap_or_default(),
            };
            Ok(base64_encode(s.as_bytes()))
        }
        "base64d" => {
            let s = match val {
                Value::String(s) => s.clone(),
                _ => return Err("@base64d requires string input".to_string()),
            };
            let bytes = base64_decode(&s)?;
            String::from_utf8(bytes).map_err(|e| format!("@base64d: invalid utf8: {e}"))
        }
        "sh" => {
            match val {
                Value::String(s) => Ok(format!("'{}'", s.replace('\'', "'\\''"))),
                Value::Array(arr) => {
                    let parts: Vec<String> = arr.iter().map(|v| {
                        if let Value::String(s) = v {
                            format!("'{}'", s.replace('\'', "'\\''"))
                        } else {
                            let s = serde_json::to_string(v).unwrap_or_default();
                            format!("'{}'", s.replace('\'', "'\\''"))
                        }
                    }).collect();
                    Ok(parts.join(" "))
                }
                _ => {
                    let s = serde_json::to_string(val).unwrap_or_default();
                    Ok(format!("'{}'", s.replace('\'', "'\\''")))
                }
            }
        }
        _ => Err(format!("unknown format: @{name}")),
    }
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    let mut i = 0;
    while i < data.len() {
        let b0 = u32::from(data[i]);
        let b1 = if i + 1 < data.len() { u32::from(data[i + 1]) } else { 0 };
        let b2 = if i + 2 < data.len() { u32::from(data[i + 2]) } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if i + 1 < data.len() {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if i + 2 < data.len() {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        i += 3;
    }
    result
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    let input = input.trim();
    let mut result = Vec::new();
    let chars: Vec<u8> = input.bytes().filter(|b| *b != b'\n' && *b != b'\r' && *b != b' ').collect();
    if chars.len() % 4 != 0 {
        return Err("invalid base64 length".to_string());
    }
    let decode_char = |c: u8| -> Result<u32, String> {
        match c {
            b'A'..=b'Z' => Ok(u32::from(c - b'A')),
            b'a'..=b'z' => Ok(u32::from(c - b'a' + 26)),
            b'0'..=b'9' => Ok(u32::from(c - b'0' + 52)),
            b'+' => Ok(62),
            b'/' => Ok(63),
            b'=' => Ok(0),
            _ => Err(format!("invalid base64 character: {c}")),
        }
    };
    let mut i = 0;
    while i < chars.len() {
        let a = decode_char(chars[i])?;
        let b = decode_char(chars[i + 1])?;
        let c = decode_char(chars[i + 2])?;
        let d = decode_char(chars[i + 3])?;
        let triple = (a << 18) | (b << 12) | (c << 6) | d;
        result.push(((triple >> 16) & 0xFF) as u8);
        if chars[i + 2] != b'=' {
            result.push(((triple >> 8) & 0xFF) as u8);
        }
        if chars[i + 3] != b'=' {
            result.push((triple & 0xFF) as u8);
        }
        i += 4;
    }
    Ok(result)
}
