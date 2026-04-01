#![allow(clippy::unnecessary_wraps)]

use vbash::{CommandContext, ExecOptions, ExecResult, Shell, Error};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

#[test]
fn custom_command_via_builder() {
    fn hello(_args: &[&str], _ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
        Ok(ExecResult { stdout: "hello from custom\n".to_string(), stderr: String::new(), exit_code: 0, env: HashMap::new() })
    }
    let mut shell = Shell::builder().command("hello", hello).build();
    let r = shell.exec("hello").unwrap();
    assert_eq!(r.stdout, "hello from custom\n");
}

#[test]
fn custom_command_with_args() {
    fn greet(args: &[&str], _ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
        let name = args.first().copied().unwrap_or("world");
        Ok(ExecResult { stdout: format!("hello {name}\n"), stderr: String::new(), exit_code: 0, env: HashMap::new() })
    }
    let mut shell = Shell::builder().command("greet", greet).build();
    let r = shell.exec("greet alice").unwrap();
    assert_eq!(r.stdout, "hello alice\n");
}

#[test]
fn register_command_after_build() {
    fn late(_args: &[&str], _ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
        Ok(ExecResult { stdout: "late registration\n".to_string(), stderr: String::new(), exit_code: 0, env: HashMap::new() })
    }
    let mut shell = Shell::new();
    shell.register_command("late", late);
    let r = shell.exec("late").unwrap();
    assert_eq!(r.stdout, "late registration\n");
}

#[test]
fn custom_command_overrides_builtin() {
    fn my_echo(_args: &[&str], _ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
        Ok(ExecResult { stdout: "custom echo\n".to_string(), stderr: String::new(), exit_code: 0, env: HashMap::new() })
    }
    let mut shell = Shell::builder().command("echo", my_echo).build();
    let r = shell.exec("echo hello").unwrap();
    assert_eq!(r.stdout, "custom echo\n");
}

#[test]
fn cancellation_pre_cancelled() {
    let cancel = Arc::new(AtomicBool::new(true));
    let mut shell = Shell::new();
    let r = shell.exec_with("echo hello; echo world; echo three", ExecOptions {
        cancel: Some(cancel),
        ..Default::default()
    });
    assert!(r.is_err());
}

#[test]
fn cancellation_not_set_runs_normally() {
    let cancel = Arc::new(AtomicBool::new(false));
    let mut shell = Shell::new();
    let r = shell.exec_with("echo hello", ExecOptions {
        cancel: Some(cancel),
        ..Default::default()
    }).unwrap();
    assert_eq!(r.stdout, "hello\n");
}

#[test]
fn exec_with_timeout_completes_fast() {
    let mut shell = Shell::new();
    let r = shell.exec_with_timeout("echo fast", Duration::from_secs(5)).unwrap();
    assert_eq!(r.stdout, "fast\n");
}

#[test]
fn nocasematch_case_statement() {
    let mut shell = Shell::new();
    let r = shell.exec("shopt -s nocasematch; case HELLO in hello) echo match;; esac").unwrap();
    assert_eq!(r.stdout, "match\n");
}

#[test]
fn nocasematch_conditional() {
    let mut shell = Shell::new();
    let r = shell.exec("shopt -s nocasematch; [[ HELLO == hello ]] && echo yes").unwrap();
    assert_eq!(r.stdout, "yes\n");
}

#[test]
fn nocasematch_off_by_default() {
    let mut shell = Shell::new();
    let r = shell.exec("[[ HELLO == hello ]] && echo yes || echo no").unwrap();
    assert_eq!(r.stdout, "no\n");
}

#[test]
fn nocaseglob_match() {
    let mut shell = Shell::new();
    shell.exec("touch /tmp/Hello.TXT").unwrap();
    let r = shell.exec("shopt -s nocaseglob; echo /tmp/hello*").unwrap();
    assert!(r.stdout.contains("Hello.TXT"));
}

#[test]
fn nocaseglob_off_by_default() {
    let mut shell = Shell::new();
    shell.exec("touch /tmp/Hello.TXT").unwrap();
    let r = shell.exec("echo /tmp/hello*").unwrap();
    assert_eq!(r.stdout.trim(), "/tmp/hello*");
}
