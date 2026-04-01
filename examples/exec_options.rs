use std::collections::HashMap;
use vbash::{ExecOptions, Shell};

fn main() {
    let mut shell = Shell::new();

    // pass stdin to a command
    let r = shell.exec_with(
        "cat | tr a-z A-Z",
        ExecOptions {
            stdin: Some("hello world"),
            ..Default::default()
        },
    ).unwrap();
    println!("{}", r.stdout.trim());

    // override env vars for one call
    let mut env = HashMap::new();
    env.insert("API_KEY".to_string(), "sk-1234".to_string());
    let r = shell.exec_with(
        "echo $API_KEY",
        ExecOptions {
            env: Some(&env),
            ..Default::default()
        },
    ).unwrap();
    println!("{}", r.stdout.trim());

    // the override doesn't persist
    let r = shell.exec("echo ${API_KEY:-unset}").unwrap();
    println!("{}", r.stdout.trim());

    // override cwd
    shell.exec("mkdir -p /data && echo content > /data/file.txt").unwrap();
    let r = shell.exec_with(
        "cat file.txt",
        ExecOptions {
            cwd: Some("/data"),
            ..Default::default()
        },
    ).unwrap();
    println!("{}", r.stdout.trim());
}
