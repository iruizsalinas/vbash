//! Core end-to-end smoke tests verifying the vbash system works as a whole.
//! Detailed command-specific and feature-specific tests are in their own files.

use vbash::{ExecOptions, ExecutionLimits, Shell};

#[test]
fn simple_echo() {
    let mut shell = Shell::new();
    let r = shell.exec("echo hello world").unwrap();
    assert_eq!(r.stdout, "hello world\n");
    assert_eq!(r.exit_code, 0);
}

#[test]
fn variable_assignment_and_expansion() {
    let mut shell = Shell::new();
    let r = shell.exec("FOO=bar; echo $FOO").unwrap();
    assert_eq!(r.stdout, "bar\n");
}

#[test]
fn pipe_chain() {
    let mut shell = Shell::builder()
        .file("/data/input.txt", "cherry\napple\nbanana\n")
        .build();
    let r = shell.exec("cat /data/input.txt | sort | head -n 2").unwrap();
    assert_eq!(r.stdout, "apple\nbanana\n");
}

#[test]
fn if_else() {
    let mut shell = Shell::new();
    let r = shell.exec("if false; then echo yes; else echo no; fi").unwrap();
    assert_eq!(r.stdout, "no\n");
}

#[test]
fn for_loop() {
    let mut shell = Shell::new();
    let r = shell.exec("for x in a b c; do echo $x; done").unwrap();
    assert_eq!(r.stdout, "a\nb\nc\n");
}

#[test]
fn function_with_args() {
    let mut shell = Shell::new();
    let r = shell.exec("greet() { echo hello $1; }; greet world").unwrap();
    assert_eq!(r.stdout, "hello world\n");
}

#[test]
fn command_substitution() {
    let mut shell = Shell::new();
    let r = shell.exec("echo the answer is $(echo 42)").unwrap();
    assert_eq!(r.stdout, "the answer is 42\n");
}

#[test]
fn redirect_to_file_and_read_back() {
    let mut shell = Shell::new();
    shell.exec("echo hello > /tmp/out.txt").unwrap();
    let r = shell.exec("cat /tmp/out.txt").unwrap();
    assert_eq!(r.stdout, "hello\n");
}

#[test]
fn filesystem_persists_across_exec() {
    let mut shell = Shell::new();
    shell.exec("echo data > /tmp/persist.txt").unwrap();
    let r = shell.exec("cat /tmp/persist.txt").unwrap();
    assert_eq!(r.stdout, "data\n");
}

#[test]
fn builder_preloaded_files() {
    let mut shell = Shell::builder()
        .file("/app/config.txt", "key=value")
        .build();
    let r = shell.exec("cat /app/config.txt").unwrap();
    assert_eq!(r.stdout, "key=value");
}

#[test]
fn exec_with_stdin() {
    let mut shell = Shell::new();
    let r = shell
        .exec_with(
            "cat",
            ExecOptions {
                stdin: Some("hello from stdin"),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(r.stdout, "hello from stdin");
}

#[test]
fn exec_returns_env() {
    let mut shell = Shell::new();
    let r = shell.exec("export MY_VAR=hello").unwrap();
    assert_eq!(r.env.get("MY_VAR").map(String::as_str), Some("hello"));
}

#[test]
fn and_or_operators() {
    let mut shell = Shell::new();
    let r = shell.exec("true && echo yes || echo no").unwrap();
    assert_eq!(r.stdout, "yes\n");
    let r = shell.exec("false && echo yes || echo no").unwrap();
    assert_eq!(r.stdout, "no\n");
}

#[test]
fn subshell_isolation() {
    let mut shell = Shell::new();
    let r = shell.exec("X=outer; (X=inner; echo $X); echo $X").unwrap();
    assert_eq!(r.stdout, "inner\nouter\n");
}

#[test]
fn command_not_found() {
    let mut shell = Shell::new();
    let r = shell.exec("nonexistent_command_xyz").unwrap();
    assert_eq!(r.exit_code, 127);
    assert!(r.stderr.contains("command not found"));
}

#[test]
fn execution_limit_prevents_infinite_loop() {
    let mut shell = Shell::builder()
        .limits(ExecutionLimits {
            max_loop_iterations: 10,
            ..ExecutionLimits::default()
        })
        .build();
    let r = shell.exec("while true; do echo x; done");
    assert!(r.is_err());
}
