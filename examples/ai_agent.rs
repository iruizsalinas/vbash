use vbash::{Error, ExecutionLimits, SessionLimits, Shell};

fn main() {
    let mut shell = Shell::builder()
        .limits(ExecutionLimits {
            max_command_count: 500,
            max_loop_iterations: 1_000,
            max_output_size: 64 * 1024,
            ..ExecutionLimits::default()
        })
        .session_limits(SessionLimits {
            max_exec_calls: 50,
            max_total_commands: 10_000,
        })
        .build();

    // normal scripts run fine
    let r = shell.exec(r#"
        echo '{"users": [{"name": "alice"}, {"name": "bob"}]}' \
            | jq -r '.users[].name' \
            | sort
    "#).unwrap();
    println!("users: {}", r.stdout.trim());

    // infinite loops get stopped
    let r = shell.exec("while true; do echo x; done");
    match r {
        Err(Error::LimitExceeded(kind)) => {
            println!("caught: {kind}");
        }
        other => println!("unexpected: {other:?}"),
    }

    // non-zero exit is not an error, just a code
    let r = shell.exec("grep nosuchpattern /dev/null").unwrap();
    println!("grep exit code: {}", r.exit_code);

    // scripts can't read host env or filesystem
    let r = shell.exec("echo $SECRET_KEY").unwrap();
    println!("SECRET_KEY: {:?}", r.stdout.trim()); // empty
}
