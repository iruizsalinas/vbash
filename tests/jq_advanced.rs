use vbash::{ExecOptions, Shell};

fn jq(filter: &str, input: &str) -> String {
    let mut shell = Shell::new();
    let cmd = format!("jq '{filter}'");
    shell.exec_with(&cmd, ExecOptions { stdin: Some(input), ..Default::default() }).unwrap().stdout
}

fn jq_raw(filter: &str, input: &str) -> String {
    let mut shell = Shell::new();
    let cmd = format!("jq -r '{filter}'");
    shell.exec_with(&cmd, ExecOptions { stdin: Some(input), ..Default::default() }).unwrap().stdout
}

fn jq_cmd(full_cmd: &str, input: &str) -> (String, String, i32) {
    let mut shell = Shell::new();
    let r = shell.exec_with(full_cmd, ExecOptions { stdin: Some(input), ..Default::default() }).unwrap();
    (r.stdout, r.stderr, r.exit_code)
}

fn jq_compact(filter: &str, input: &str) -> String {
    let mut shell = Shell::new();
    let cmd = format!("jq -c '{filter}'");
    shell.exec_with(&cmd, ExecOptions { stdin: Some(input), ..Default::default() }).unwrap().stdout
}

// Null handling

#[test]
fn jq_null_field_access() {
    assert_eq!(jq(".foo", "null"), "null\n");
}

#[test]
fn jq_null_length() {
    assert_eq!(jq("length", "null"), "0\n");
}

#[test]
fn jq_null_addition() {
    assert_eq!(jq(". + 1", "null"), "1\n");
}

#[test]
fn jq_null_alternative() {
    assert_eq!(jq(". // \"default\"", "null"), "\"default\"\n");
}

#[test]
fn jq_false_alternative() {
    assert_eq!(jq(". // \"default\"", "false"), "\"default\"\n");
}

// Empty collections

#[test]
fn jq_empty_array_length() {
    assert_eq!(jq("length", "[]"), "0\n");
}

#[test]
fn jq_empty_object_keys() {
    assert_eq!(jq("keys", "{}"), "[]\n");
}

#[test]
fn jq_empty_array_add() {
    assert_eq!(jq("add", "[]"), "null\n");
}

#[test]
fn jq_empty_array_any() {
    assert_eq!(jq("any", "[]"), "false\n");
}

#[test]
fn jq_empty_array_all() {
    assert_eq!(jq("all", "[]"), "true\n");
}

// Recursive descent

#[test]
fn jq_recursive_nested() {
    let out = jq_compact("[.. | select(type == \"number\")]", "{\"a\":{\"b\":{\"c\":1}}}");
    assert_eq!(out, "[1]\n");
}

#[test]
fn jq_recursive_array() {
    let out = jq_compact("[.. | select(type == \"number\")]", "{\"a\":[1,{\"b\":2}]}");
    assert_eq!(out, "[1,2]\n");
}

// String interpolation

#[test]
fn jq_string_interp() {
    let out = jq_raw("\"Hello \\(.name)\"", "{\"name\":\"alice\"}");
    assert_eq!(out, "Hello alice\n");
}

#[test]
fn jq_string_interp_nested() {
    let out = jq_raw("\"x=\\(.a.b)\"", "{\"a\":{\"b\":\"val\"}}");
    assert_eq!(out, "x=val\n");
}

// Comparison edge cases

#[test]
fn jq_compare_string_number() {
    let (out, _, _) = jq_cmd("jq -n '\"1\" == 1'", "");
    assert_eq!(out, "false\n");
}

#[test]
fn jq_compare_null() {
    let (out, _, _) = jq_cmd("jq -n 'null == false'", "");
    assert_eq!(out, "false\n");
}

#[test]
fn jq_compare_arrays() {
    let (out, _, _) = jq_cmd("jq -n '[1,2] == [1,2]'", "");
    assert_eq!(out, "true\n");
}

#[test]
fn jq_sort_mixed() {
    let out = jq_compact("sort", "[3,1,2]");
    assert_eq!(out, "[1,2,3]\n");
}

// Object construction

#[test]
fn jq_object_computed_key() {
    let out = jq_compact("{(.key): .val}", "{\"key\":\"name\",\"val\":\"alice\"}");
    assert_eq!(out, "{\"name\":\"alice\"}\n");
}

#[test]
fn jq_object_multiple() {
    let out = jq_compact("{a:1, b:2, c:3}", "{}");
    assert_eq!(out, "{\"a\":1,\"b\":2,\"c\":3}\n");
}

// Reduce

#[test]
fn jq_reduce_sum() {
    assert_eq!(jq("reduce .[] as $x (0; . + $x)", "[1,2,3,4,5]"), "15\n");
}

#[test]
fn jq_reduce_string() {
    let out = jq_raw("reduce .[] as $x (\"\"; . + $x)", "[\"a\",\"b\",\"c\"]");
    assert_eq!(out, "abc\n");
}

// Error handling

#[test]
fn jq_type_error_field() {
    // .foo on a string produces an error (not null like real jq)
    let (_, _, code) = jq_cmd("jq '.foo'", "\"hello\"");
    assert_ne!(code, 0);
}

#[test]
fn jq_index_out_of_bounds() {
    assert_eq!(jq(".[10]", "[1,2,3]"), "null\n");
}

#[test]
fn jq_invalid_json() {
    let (_, stderr, code) = jq_cmd("jq '.'", "not json");
    assert_ne!(code, 0);
    assert!(!stderr.is_empty() || code != 0);
}

#[test]
fn jq_invalid_filter() {
    let (_, _, code) = jq_cmd("jq 'invalid syntax %%%'", "{}");
    assert_ne!(code, 0);
}

// Format strings

#[test]
fn jq_format_base64() {
    let out = jq_raw("@base64", "\"hello\"");
    assert_eq!(out, "aGVsbG8=\n");
}

#[test]
fn jq_format_uri() {
    let out = jq_raw("@uri", "\"hello world\"");
    assert_eq!(out, "hello%20world\n");
}

#[test]
fn jq_format_html() {
    let out = jq_raw("@html", "\"<b>hi</b>\"");
    assert_eq!(out, "&lt;b&gt;hi&lt;/b&gt;\n");
}

#[test]
fn jq_format_csv() {
    let out = jq_raw("@csv", "[\"a\",\"b\",\"c\"]");
    assert_eq!(out, "a,b,c\n");
}

#[test]
fn jq_format_sh() {
    let out = jq_raw("@sh", "\"hello world\"");
    assert_eq!(out, "'hello world'\n");
}

// Slicing

#[test]
fn jq_negative_index() {
    assert_eq!(jq(".[-1]", "[1,2,3,4,5]"), "5\n");
}

#[test]
fn jq_negative_slice() {
    let out = jq_compact(".[-2:]", "[1,2,3,4,5]");
    assert_eq!(out, "[4,5]\n");
}

#[test]
fn jq_slice_from() {
    let out = jq_compact(".[2:]", "[1,2,3,4,5]");
    assert_eq!(out, "[3,4,5]\n");
}

#[test]
fn jq_slice_to() {
    let out = jq_compact(".[:3]", "[1,2,3,4,5]");
    assert_eq!(out, "[1,2,3]\n");
}

// Options

#[test]
fn jq_arg_variable() {
    let (out, _, _) = jq_cmd("jq --arg name alice '$name'", "null");
    assert_eq!(out, "\"alice\"\n");
}

#[test]
fn jq_argjson_number() {
    let (out, _, _) = jq_cmd("jq --argjson x 42 '$x + 1'", "null");
    assert_eq!(out, "43\n");
}

#[test]
fn jq_join_output() {
    let (out, _, _) = jq_cmd("jq -j '.'", "1\n2");
    assert_eq!(out, "12");
}

// Arithmetic

#[test]
fn jq_modulo() {
    let (out, _, _) = jq_cmd("jq -n '17 % 5'", "");
    assert_eq!(out, "2\n");
}

#[test]
fn jq_negative_modulo() {
    let (out, _, _) = jq_cmd("jq -n '-7 % 3'", "");
    assert_eq!(out, "-1\n");
}

#[test]
fn jq_object_merge() {
    let out = jq_compact(". + {\"b\":2}", "{\"a\":1}");
    assert_eq!(out, "{\"a\":1,\"b\":2}\n");
}

#[test]
fn jq_object_override() {
    let out = jq_compact(". + {\"a\":2}", "{\"a\":1}");
    assert_eq!(out, "{\"a\":2}\n");
}

// Additional builtins

#[test]
fn jq_reverse() {
    let out = jq_compact("reverse", "[1,2,3]");
    assert_eq!(out, "[3,2,1]\n");
}

#[test]
fn jq_flatten() {
    let out = jq_compact("flatten", "[[1,2],[3,[4,5]]]");
    assert_eq!(out, "[1,2,3,4,5]\n");
}

#[test]
fn jq_unique() {
    let out = jq_compact("unique", "[3,1,2,1,3]");
    assert_eq!(out, "[1,2,3]\n");
}

#[test]
fn jq_to_entries() {
    let out = jq_compact("to_entries", "{\"a\":1}");
    assert_eq!(out, "[{\"key\":\"a\",\"value\":1}]\n");
}

#[test]
fn jq_from_entries() {
    let out = jq_compact("from_entries", "[{\"key\":\"a\",\"value\":1}]");
    assert_eq!(out, "{\"a\":1}\n");
}

#[test]
fn jq_type_function() {
    assert_eq!(jq_raw("type", "42"), "number\n");
    assert_eq!(jq_raw("type", "\"hi\""), "string\n");
    assert_eq!(jq_raw("type", "null"), "null\n");
    assert_eq!(jq_raw("type", "true"), "boolean\n");
}

#[test]
fn jq_select() {
    let out = jq_compact("[.[] | select(. > 2)]", "[1,2,3,4,5]");
    assert_eq!(out, "[3,4,5]\n");
}

#[test]
fn jq_map() {
    let out = jq_compact("map(. * 2)", "[1,2,3]");
    assert_eq!(out, "[2,4,6]\n");
}

#[test]
fn jq_has_key() {
    assert_eq!(jq("has(\"a\")", "{\"a\":1,\"b\":2}"), "true\n");
    assert_eq!(jq("has(\"c\")", "{\"a\":1,\"b\":2}"), "false\n");
}

#[test]
fn jq_abs() {
    let (out, _, _) = jq_cmd("jq -n '-5 | abs'", "");
    assert_eq!(out, "5\n");
}

#[test]
fn jq_floor_ceil() {
    let (out, _, _) = jq_cmd("jq -n '3.7 | floor'", "");
    assert_eq!(out, "3\n");
    let (out2, _, _) = jq_cmd("jq -n '3.2 | ceil'", "");
    assert_eq!(out2, "4\n");
}

#[test]
fn jq_ascii_case() {
    assert_eq!(jq_raw("ascii_downcase", "\"HELLO\""), "hello\n");
    assert_eq!(jq_raw("ascii_upcase", "\"hello\""), "HELLO\n");
}

#[test]
fn jq_split_join() {
    let out = jq_compact("split(\",\")", "\"a,b,c\"");
    assert_eq!(out, "[\"a\",\"b\",\"c\"]\n");
    let out2 = jq_raw("join(\"-\")", "[\"a\",\"b\",\"c\"]");
    assert_eq!(out2, "a-b-c\n");
}

#[test]
fn jq_startswith_endswith() {
    assert_eq!(jq("startswith(\"he\")", "\"hello\""), "true\n");
    assert_eq!(jq("endswith(\"lo\")", "\"hello\""), "true\n");
}

#[test]
fn jq_null_iterate() {
    // null | .[] should produce empty (null is handled as empty iterable)
    let out = jq(".[]", "null");
    assert_eq!(out, "");
}
