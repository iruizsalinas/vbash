mod lexer;
mod ast;
mod parser;
mod eval;
mod builtins;
mod collection_builtins;
mod string_builtins;
mod helpers;

use std::collections::HashMap;
use std::fmt::Write;

use serde_json::Value;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;

use lexer::lex;
use parser::parse_tokens;
use eval::Evaluator;

pub fn jq_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut raw_output = false;
    let mut compact = false;
    let mut exit_status = false;
    let mut slurp = false;
    let mut null_input = false;
    let mut join_output = false;
    let mut sort_keys = false;
    let mut tab_indent = false;
    let mut variables: HashMap<String, Value> = HashMap::new();
    let mut filter_str = None;
    let mut file_args = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-r" | "--raw-output" => raw_output = true,
            "-c" | "--compact-output" => compact = true,
            "-e" | "--exit-status" => exit_status = true,
            "-s" | "--slurp" => slurp = true,
            "-n" | "--null-input" => null_input = true,
            "-j" | "--join-output" => { join_output = true; raw_output = true; }
            "-S" | "--sort-keys" => sort_keys = true,
            "--tab" => tab_indent = true,
            "--arg" => {
                if i + 2 < args.len() {
                    let name = args[i + 1].to_string();
                    let val = Value::String(args[i + 2].to_string());
                    variables.insert(name, val);
                    i += 2;
                } else {
                    return Ok(ExecResult {
                        stdout: String::new(),
                        stderr: "jq: --arg requires NAME VALUE\n".to_string(),
                        exit_code: 2,
                        env: HashMap::new(),
});
                }
            }
            "--argjson" => {
                if i + 2 < args.len() {
                    let name = args[i + 1].to_string();
                    match serde_json::from_str(args[i + 2]) {
                        Ok(val) => { variables.insert(name, val); }
                        Err(e) => {
                            return Ok(ExecResult {
                                stdout: String::new(),
                                stderr: format!("jq: --argjson: invalid JSON: {e}\n"),
                                exit_code: 2,
                                env: HashMap::new(),
});
                        }
                    }
                    i += 2;
                } else {
                    return Ok(ExecResult {
                        stdout: String::new(),
                        stderr: "jq: --argjson requires NAME JSON\n".to_string(),
                        exit_code: 2,
                        env: HashMap::new(),
});
                }
            }
            arg if filter_str.is_none() && !arg.starts_with('-') => {
                filter_str = Some(arg);
            }
            arg if filter_str.is_none() && arg == "--" => {}
            _ => {
                if filter_str.is_some() {
                    file_args.push(args[i]);
                } else {
                    filter_str = Some(args[i]);
                }
            }
        }
        i += 1;
    }

    let filter_str = filter_str.unwrap_or(".");

    let tokens = match lex(filter_str) {
        Ok(t) => t,
        Err(e) => {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: format!("jq: compile error: {e}\n"),
                exit_code: 3,
                env: HashMap::new(),
});
        }
    };

    let expr = match parse_tokens(&tokens) {
        Ok(e) => e,
        Err(e) => {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: format!("jq: compile error: {e}\n"),
                exit_code: 3,
                env: HashMap::new(),
});
        }
    };

    let input_str = if file_args.is_empty() {
        ctx.stdin.to_string()
    } else {
        let mut combined = String::new();
        for path in &file_args {
            let resolved = crate::fs::path::resolve(ctx.cwd, path);
            match ctx.fs.read_file_string(&resolved) {
                Ok(content) => combined.push_str(&content),
                Err(e) => {
                    return Ok(ExecResult {
                        stdout: String::new(),
                        stderr: format!("jq: {e}\n"),
                        exit_code: 2,
                        env: HashMap::new(),
});
                }
            }
        }
        combined
    };

    let inputs: Vec<Value> = if null_input {
        vec![Value::Null]
    } else if slurp {
        let mut arr = Vec::new();
        for val in JsonStream::new(&input_str) {
            match val {
                Ok(v) => arr.push(v),
                Err(e) => {
                    return Ok(ExecResult {
                        stdout: String::new(),
                        stderr: format!("jq: parse error: {e}\n"),
                        exit_code: 2,
                        env: HashMap::new(),
});
                }
            }
        }
        vec![Value::Array(arr)]
    } else {
        let mut vals = Vec::new();
        for val in JsonStream::new(&input_str) {
            match val {
                Ok(v) => vals.push(v),
                Err(e) => {
                    return Ok(ExecResult {
                        stdout: String::new(),
                        stderr: format!("jq: parse error: {e}\n"),
                        exit_code: 2,
                        env: HashMap::new(),
});
                }
            }
        }
        if vals.is_empty() && !null_input {
            vec![Value::Null]
        } else {
            vals
        }
    };

    let eval = Evaluator::new(sort_keys);
    let mut stdout = String::new();
    let mut all_outputs = Vec::new();
    let mut had_error = false;

    for input in &inputs {
        match eval.evaluate(&expr, input, &variables) {
            Ok(results) => {
                all_outputs.extend(results);
            }
            Err(e) => {
                if e != "break" {
                    had_error = true;
                    let _ = writeln!(ctx.stderr, "jq: error: {e}");
                }
            }
        }
    }

    let mut fmt_flags = 0u8;
    if raw_output { fmt_flags |= FormatOpts::RAW; }
    if compact { fmt_flags |= FormatOpts::COMPACT; }
    if tab_indent { fmt_flags |= FormatOpts::TAB; }
    if sort_keys { fmt_flags |= FormatOpts::SORT_KEYS; }
    let fmt_opts = FormatOpts(fmt_flags);

    for val in &all_outputs {
        let formatted = format_value(val, fmt_opts);
        let _ = write!(stdout, "{formatted}");
        if !join_output {
            let _ = writeln!(stdout);
        }
    }

    let exit_code = if had_error {
        5
    } else if exit_status {
        let last_false_or_null = all_outputs.last().is_none_or(|v| {
            v.is_null() || *v == Value::Bool(false)
        });
        i32::from(last_false_or_null)
    } else {
        0
    };

    Ok(ExecResult {
        stdout,
        stderr: String::new(),
        exit_code,
        env: HashMap::new(),
})
}

struct JsonStream<'a> {
    data: &'a str,
    pos: usize,
}

impl<'a> JsonStream<'a> {
    fn new(data: &'a str) -> Self {
        Self { data, pos: 0 }
    }
}

impl Iterator for JsonStream<'_> {
    type Item = Result<Value, String>;

    fn next(&mut self) -> Option<Self::Item> {
        let remaining = &self.data[self.pos..];
        let trimmed = remaining.trim_start();
        if trimmed.is_empty() {
            return None;
        }
        self.pos = self.data.len() - trimmed.len();
        let reader = serde_json::Deserializer::from_str(&self.data[self.pos..]);
        let mut stream = reader.into_iter::<Value>();
        match stream.next() {
            Some(Ok(val)) => {
                self.pos += stream.byte_offset();
                Some(Ok(val))
            }
            Some(Err(e)) => {
                self.pos = self.data.len();
                Some(Err(e.to_string()))
            }
            None => None,
        }
    }
}

#[derive(Clone, Copy)]
struct FormatOpts(u8);

impl FormatOpts {
    const RAW: u8 = 1;
    const COMPACT: u8 = 2;
    const TAB: u8 = 4;
    const SORT_KEYS: u8 = 8;

    fn raw(self) -> bool { self.0 & Self::RAW != 0 }
    fn compact(self) -> bool { self.0 & Self::COMPACT != 0 }
    fn tab(self) -> bool { self.0 & Self::TAB != 0 }
    fn sort_keys(self) -> bool { self.0 & Self::SORT_KEYS != 0 }
}

fn format_value(val: &Value, opts: FormatOpts) -> String {
    let (raw, compact, tab, sort_keys) = (opts.raw(), opts.compact(), opts.tab(), opts.sort_keys());
    if raw {
        if let Value::String(s) = val {
            return s.clone();
        }
    }
    if sort_keys {
        let sorted = sort_value_keys(val);
        if compact {
            return serde_json::to_string(&sorted).unwrap_or_default();
        }
        if tab {
            return format_with_indent(&sorted, "\t");
        }
        return serde_json::to_string_pretty(&sorted).unwrap_or_default();
    }
    if compact {
        return serde_json::to_string(val).unwrap_or_default();
    }
    if tab {
        return format_with_indent(val, "\t");
    }
    serde_json::to_string_pretty(val).unwrap_or_default()
}

fn format_with_indent(val: &Value, indent: &str) -> String {
    let pretty = serde_json::to_string_pretty(val).unwrap_or_default();
    if indent == "\t" {
        let mut result = String::new();
        for line in pretty.lines() {
            let trimmed = line.trim_start();
            let spaces = line.len() - trimmed.len();
            let tabs = spaces / 2;
            for _ in 0..tabs {
                result.push('\t');
            }
            result.push_str(trimmed);
            result.push('\n');
        }
        if result.ends_with('\n') {
            result.pop();
        }
        result
    } else {
        pretty
    }
}

fn sort_value_keys(val: &Value) -> Value {
    match val {
        Value::Object(map) => {
            let mut sorted: Vec<(String, Value)> = map
                .iter()
                .map(|(k, v)| (k.clone(), sort_value_keys(v)))
                .collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(arr) => Value::Array(arr.iter().map(sort_value_keys).collect()),
        other => other.clone(),
    }
}
