use vbash::Shell;

#[test]
fn yq_identity() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '{\"a\":1}' | yq -c '.'").unwrap();
    assert_eq!(r.stdout.trim(), "{\"a\":1}");
}

#[test]
fn yq_field() {
    let mut shell = Shell::new();
    let r = shell
        .exec("echo '{\"name\":\"alice\"}' | yq '.name'")
        .unwrap();
    assert_eq!(r.stdout.trim(), "\"alice\"");
}

#[test]
fn yq_raw_output() {
    let mut shell = Shell::new();
    let r = shell
        .exec("echo '{\"name\":\"alice\"}' | yq -r '.name'")
        .unwrap();
    assert_eq!(r.stdout.trim(), "alice");
}

#[test]
fn yq_compact() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '{\"a\":1}' | yq -c '.'").unwrap();
    assert_eq!(r.stdout.trim(), "{\"a\":1}");
}

#[test]
fn yq_length() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '[1,2,3]' | yq 'length'").unwrap();
    assert_eq!(r.stdout.trim(), "3");
}

#[test]
fn yq_keys() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '{\"b\":1,\"a\":2}' | yq -c 'keys'").unwrap();
    assert_eq!(r.stdout.trim(), "[\"a\",\"b\"]");
}

#[test]
fn yq_properties_output() {
    let mut shell = Shell::new();
    let r = shell
        .exec("echo '{\"a\":\"1\",\"b\":\"2\"}' | yq -o props '.'")
        .unwrap();
    assert_eq!(r.stdout, "a=1\nb=2\n");
}

#[test]
fn yq_slurp() {
    let mut shell = Shell::builder()
        .file("/tmp/a.json", "{\"x\":1}")
        .file("/tmp/b.json", "{\"x\":2}")
        .build();
    let r = shell.exec("yq -s -c '.' /tmp/a.json /tmp/b.json").unwrap();
    assert_eq!(r.stdout.trim(), "[{\"x\":1},{\"x\":2}]");
}

#[test]
fn yq_nested_field() {
    let mut shell = Shell::new();
    let r = shell
        .exec("echo '{\"a\":{\"b\":42}}' | yq '.a.b'")
        .unwrap();
    assert_eq!(r.stdout.trim(), "42");
}

#[test]
fn yq_invalid_input() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'not json' | yq '.'").unwrap();
    assert_eq!(r.exit_code, 1);
    assert!(r.stderr.contains("parse error"));
}
