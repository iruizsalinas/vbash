use vbash::Shell;


#[test]
fn jq_identity() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '{"a":1}' | jq '.'"#).unwrap();
    // Pretty-printed identity
    assert!(r.stdout.contains("\"a\""));
    assert!(r.stdout.contains('1'));
}

#[test]
fn jq_field() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '{"name":"alice"}' | jq '.name'"#).unwrap();
    assert_eq!(r.stdout, "\"alice\"\n");
}

#[test]
fn jq_raw_output() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '{"name":"alice"}' | jq -r '.name'"#).unwrap();
    assert_eq!(r.stdout, "alice\n");
}

#[test]
fn jq_nested() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '{"a":{"b":1}}' | jq '.a.b'"#).unwrap();
    assert_eq!(r.stdout, "1\n");
}

#[test]
fn jq_array_index() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '[1,2,3]' | jq '.[1]'").unwrap();
    assert_eq!(r.stdout, "2\n");
}

#[test]
fn jq_array_iterate() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '[1,2,3]' | jq '.[]'").unwrap();
    assert_eq!(r.stdout, "1\n2\n3\n");
}

#[test]
fn jq_array_slice() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '[1,2,3,4,5]' | jq '.[2:4]'").unwrap();
    let trimmed = r.stdout.trim();
    // Should produce [3,4] in some format
    assert!(trimmed.contains('3'));
    assert!(trimmed.contains('4'));
    // Parse to verify exact contents
    let val: serde_json::Value = serde_json::from_str(trimmed).unwrap();
    assert_eq!(val, serde_json::json!([3, 4]));
}


#[test]
fn jq_pipe() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '{"a":{"b":1}}' | jq '.a | .b'"#).unwrap();
    assert_eq!(r.stdout, "1\n");
}

#[test]
fn jq_comma() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '{"a":1,"b":2}' | jq '.a, .b'"#).unwrap();
    assert_eq!(r.stdout, "1\n2\n");
}

#[test]
fn jq_add() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '[1,2,3]' | jq 'add'").unwrap();
    assert_eq!(r.stdout, "6\n");
}

#[test]
fn jq_length() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '[1,2,3]' | jq 'length'").unwrap();
    assert_eq!(r.stdout, "3\n");
}

#[test]
fn jq_keys() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '{"b":1,"a":2}' | jq 'keys'"#).unwrap();
    let val: serde_json::Value = serde_json::from_str(r.stdout.trim()).unwrap();
    assert_eq!(val, serde_json::json!(["a", "b"]));
}


#[test]
fn jq_select() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '[1,2,3,4,5]' | jq '[.[] | select(. > 3)]'").unwrap();
    let val: serde_json::Value = serde_json::from_str(r.stdout.trim()).unwrap();
    assert_eq!(val, serde_json::json!([4, 5]));
}

#[test]
fn jq_map() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '[1,2,3]' | jq '[.[] | . * 2]'").unwrap();
    let val: serde_json::Value = serde_json::from_str(r.stdout.trim()).unwrap();
    assert_eq!(val, serde_json::json!([2, 4, 6]));
}

#[test]
fn jq_sort() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '[3,1,2]' | jq 'sort'").unwrap();
    let val: serde_json::Value = serde_json::from_str(r.stdout.trim()).unwrap();
    assert_eq!(val, serde_json::json!([1, 2, 3]));
}

#[test]
fn jq_reverse() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '[1,2,3]' | jq 'reverse'").unwrap();
    let val: serde_json::Value = serde_json::from_str(r.stdout.trim()).unwrap();
    assert_eq!(val, serde_json::json!([3, 2, 1]));
}

#[test]
fn jq_unique() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '[1,2,2,3,3,3]' | jq 'unique'").unwrap();
    let val: serde_json::Value = serde_json::from_str(r.stdout.trim()).unwrap();
    assert_eq!(val, serde_json::json!([1, 2, 3]));
}

#[test]
fn jq_flatten() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '[[1,2],[3,[4]]]' | jq 'flatten'").unwrap();
    let val: serde_json::Value = serde_json::from_str(r.stdout.trim()).unwrap();
    assert_eq!(val, serde_json::json!([1, 2, 3, 4]));
}

#[test]
fn jq_group_by() {
    let mut shell = Shell::new();
    let r = shell
        .exec(r#"echo '[{"a":1,"b":"x"},{"a":1,"b":"y"},{"a":2,"b":"z"}]' | jq 'group_by(.a) | length'"#)
        .unwrap();
    assert_eq!(r.stdout, "2\n");
}

#[test]
fn jq_min() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '[3,1,2]' | jq 'min'").unwrap();
    assert_eq!(r.stdout, "1\n");
}

#[test]
fn jq_max() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '[3,1,2]' | jq 'max'").unwrap();
    assert_eq!(r.stdout, "3\n");
}

#[test]
fn jq_type() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '"hello"' | jq 'type'"#).unwrap();
    assert_eq!(r.stdout, "\"string\"\n");
}

#[test]
fn jq_has() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '{"a":1}' | jq 'has("a")'"#).unwrap();
    assert_eq!(r.stdout, "true\n");
}


#[test]
fn jq_split() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '"a,b,c"' | jq 'split(",")'  "#).unwrap();
    let val: serde_json::Value = serde_json::from_str(r.stdout.trim()).unwrap();
    assert_eq!(val, serde_json::json!(["a", "b", "c"]));
}

#[test]
fn jq_join() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '["a","b","c"]' | jq 'join(",")'  "#).unwrap();
    assert_eq!(r.stdout, "\"a,b,c\"\n");
}

#[test]
fn jq_ascii_downcase() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '"HELLO"' | jq 'ascii_downcase'"#).unwrap();
    assert_eq!(r.stdout, "\"hello\"\n");
}

#[test]
fn jq_startswith() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '"hello"' | jq 'startswith("hel")'"#).unwrap();
    assert_eq!(r.stdout, "true\n");
}

#[test]
fn jq_ltrimstr() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '"hello"' | jq 'ltrimstr("hel")'"#).unwrap();
    assert_eq!(r.stdout, "\"lo\"\n");
}

#[test]
fn jq_test() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '"abc123"' | jq 'test("[0-9]+")'"#).unwrap();
    assert_eq!(r.stdout, "true\n");
}

#[test]
fn jq_tostring() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '42' | jq 'tostring'").unwrap();
    assert_eq!(r.stdout, "\"42\"\n");
}

#[test]
fn jq_tonumber() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '"42"' | jq 'tonumber'"#).unwrap();
    assert_eq!(r.stdout, "42\n");
}


#[test]
fn jq_to_entries() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '{"a":1}' | jq 'to_entries'"#).unwrap();
    let val: serde_json::Value = serde_json::from_str(r.stdout.trim()).unwrap();
    assert_eq!(val, serde_json::json!([{"key": "a", "value": 1}]));
}

#[test]
fn jq_from_entries() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '[{"key":"a","value":1}]' | jq 'from_entries'"#).unwrap();
    let val: serde_json::Value = serde_json::from_str(r.stdout.trim()).unwrap();
    assert_eq!(val, serde_json::json!({"a": 1}));
}

#[test]
fn jq_with_entries() {
    let mut shell = Shell::new();
    let r = shell
        .exec(r#"echo '{"a":1,"b":2}' | jq 'with_entries(select(.value > 1))'"#)
        .unwrap();
    let val: serde_json::Value = serde_json::from_str(r.stdout.trim()).unwrap();
    assert_eq!(val, serde_json::json!({"b": 2}));
}


#[test]
fn jq_if_then() {
    let mut shell = Shell::new();
    let r = shell
        .exec(r#"echo '5' | jq 'if . > 3 then "big" else "small" end'"#)
        .unwrap();
    assert_eq!(r.stdout, "\"big\"\n");
}

#[test]
fn jq_try_catch() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo 'null' | jq 'try .foo catch "error"'"#).unwrap();
    // .foo on null produces null (not an error in jq), so try succeeds with null
    // The test verifies try/catch parses and runs without crashing
    assert_eq!(r.exit_code, 0);
}

#[test]
fn jq_alternative() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo 'null' | jq '.foo // "default"'"#).unwrap();
    assert_eq!(r.stdout, "\"default\"\n");
}


#[test]
fn jq_compact() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '{"a":1}' | jq -c '.'"#).unwrap();
    assert_eq!(r.stdout, "{\"a\":1}\n");
}

#[test]
fn jq_slurp() {
    let mut shell = Shell::builder()
        .file("/tmp/data.json", "1\n2\n3\n")
        .build();
    let r = shell.exec("cat /tmp/data.json | jq -s '.'").unwrap();
    let val: serde_json::Value = serde_json::from_str(r.stdout.trim()).unwrap();
    assert_eq!(val, serde_json::json!([1, 2, 3]));
}

#[test]
fn jq_null_input() {
    let mut shell = Shell::new();
    let r = shell.exec("jq -n '1+2'").unwrap();
    assert_eq!(r.stdout, "3\n");
}

#[test]
fn jq_sort_keys() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '{"b":1,"a":2}' | jq -S -c '.'"#).unwrap();
    assert_eq!(r.stdout, "{\"a\":2,\"b\":1}\n");
}

#[test]
fn jq_exit_status() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'null' | jq -e '.'").unwrap();
    // -e: exit code 1 when last output is null or false
    assert_eq!(r.exit_code, 1);
}


#[test]
fn jq_base64() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '"hello"' | jq '@base64'"#).unwrap();
    assert_eq!(r.stdout, "\"aGVsbG8=\"\n");
}

#[test]
fn jq_csv() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '[[1,"a"],[2,"b"]]' | jq '.[] | @csv'"#).unwrap();
    // Each sub-array formatted as CSV
    let lines: Vec<&str> = r.stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2);
    // CSV output is quoted as a JSON string
    assert!(lines[0].contains('1'));
    assert!(lines[0].contains('a'));
}

#[test]
fn jq_html() {
    let mut shell = Shell::new();
    let r = shell.exec(r#"echo '"<b>hi</b>"' | jq '@html'"#).unwrap();
    // HTML entities should escape < and >
    assert!(r.stdout.contains("&lt;"));
    assert!(r.stdout.contains("&gt;"));
}


#[test]
fn jq_null_input_empty_object() {
    let mut shell = Shell::new();
    let r = shell.exec("jq -n '{}'").unwrap();
    assert_eq!(r.stdout, "{}\n");
}

#[test]
fn jq_map_builtin() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '[1,2,3]' | jq 'map(. + 10)'").unwrap();
    let val: serde_json::Value = serde_json::from_str(r.stdout.trim()).unwrap();
    assert_eq!(val, serde_json::json!([11, 12, 13]));
}
