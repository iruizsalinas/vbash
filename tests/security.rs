use vbash::{ExecutionLimits, Shell};
use vbash::error::{Error, LimitKind};

fn assert_limit(r: Result<vbash::ExecResult, Error>, expected: LimitKind) {
    match r {
        Err(Error::LimitExceeded(kind)) => assert_eq!(kind, expected),
        Err(e) => panic!("expected LimitExceeded({expected:?}), got: {e:?}"),
        Ok(out) => panic!("expected LimitExceeded({expected:?}), got Ok with exit_code={}", out.exit_code),
    }
}

#[test]
fn security_loop_limit() {
    let mut shell = Shell::builder()
        .limits(ExecutionLimits {
            max_loop_iterations: 10,
            ..ExecutionLimits::default()
        })
        .build();
    let r = shell.exec("while true; do echo x; done");
    assert_limit(r, LimitKind::LoopIterations);
}

#[test]
fn security_loop_at_exact_limit() {
    let mut shell = Shell::builder()
        .limits(ExecutionLimits {
            max_loop_iterations: 10,
            ..ExecutionLimits::default()
        })
        .build();
    let r = shell.exec("for i in 1 2 3 4 5 6 7 8 9 10; do echo $i; done").unwrap();
    assert_eq!(r.exit_code, 0);
    assert_eq!(r.stdout.lines().count(), 10);
}

#[test]
fn security_loop_over_limit() {
    let mut shell = Shell::builder()
        .limits(ExecutionLimits {
            max_loop_iterations: 10,
            ..ExecutionLimits::default()
        })
        .build();
    let r = shell.exec("for i in 1 2 3 4 5 6 7 8 9 10 11; do echo $i; done");
    assert_limit(r, LimitKind::LoopIterations);
}

#[test]
fn security_command_count() {
    let mut shell = Shell::builder()
        .limits(ExecutionLimits {
            max_command_count: 5,
            ..ExecutionLimits::default()
        })
        .build();
    let r = shell.exec("echo 1; echo 2; echo 3; echo 4; echo 5; echo 6; echo 7; echo 8");
    assert_limit(r, LimitKind::CommandCount);
}

#[test]
fn security_command_count_at_exact_limit() {
    let mut shell = Shell::builder()
        .limits(ExecutionLimits {
            max_command_count: 5,
            ..ExecutionLimits::default()
        })
        .build();
    let r = shell.exec("echo 1; echo 2; echo 3; echo 4; echo 5").unwrap();
    assert_eq!(r.exit_code, 0);
    assert_eq!(r.stdout, "1\n2\n3\n4\n5\n");
}

#[test]
fn security_call_depth() {
    let mut shell = Shell::builder()
        .limits(ExecutionLimits {
            max_call_depth: 5,
            ..ExecutionLimits::default()
        })
        .build();
    let r = shell.exec("f() { f; }; f");
    assert_limit(r, LimitKind::CallDepth);
}

#[test]
fn security_brace_limit() {
    let mut shell = Shell::builder()
        .limits(ExecutionLimits {
            max_brace_expansion: 50,
            ..ExecutionLimits::default()
        })
        .build();
    let r = shell.exec("echo {1..99999}");
    assert_limit(r, LimitKind::BraceExpansion);
}

#[test]
fn security_substitution_depth() {
    let mut shell = Shell::builder()
        .limits(ExecutionLimits {
            max_substitution_depth: 3,
            ..ExecutionLimits::default()
        })
        .build();
    let r = shell.exec("echo $(echo $(echo $(echo $(echo deep))))");
    assert_limit(r, LimitKind::SubstitutionDepth);
}

#[test]
fn security_output_size() {
    let mut shell = Shell::builder()
        .limits(ExecutionLimits {
            max_output_size: 100,
            ..ExecutionLimits::default()
        })
        .build();
    let r = shell.exec("for i in $(seq 1000); do echo 'xxxxxxxxxxxxxxxxxxxx'; done");
    assert_limit(r, LimitKind::OutputSize);
}

#[test]
fn security_input_size() {
    let mut shell = Shell::builder()
        .limits(ExecutionLimits {
            max_input_size: 50,
            ..ExecutionLimits::default()
        })
        .build();
    let long_input = "echo ".to_string() + &"x".repeat(200);
    let r = shell.exec(&long_input);
    assert_limit(r, LimitKind::InputSize);
}

#[test]
fn security_source_depth() {
    let mut shell = Shell::builder()
        .limits(ExecutionLimits {
            max_source_depth: 3,
            ..ExecutionLimits::default()
        })
        .build();
    shell
        .exec("echo 'source /tmp/loop.sh' > /tmp/loop.sh")
        .unwrap();
    let r = shell.exec("source /tmp/loop.sh");
    assert_limit(r, LimitKind::SourceDepth);
}

#[test]
fn security_string_length() {
    let mut shell = Shell::builder()
        .limits(ExecutionLimits {
            max_string_length: 100,
            ..ExecutionLimits::default()
        })
        .build();
    let r = shell.exec("X=$(printf 'x%.0s' {1..200}); echo $X");
    assert_limit(r, LimitKind::StringLength);
}

#[test]
fn security_eval_depth() {
    let mut shell = Shell::builder()
        .limits(ExecutionLimits {
            max_call_depth: 5,
            ..ExecutionLimits::default()
        })
        .build();
    let r = shell.exec("f() { eval 'f'; }; f");
    assert_limit(r, LimitKind::CallDepth);
}

#[test]
fn security_sleep_cap() {
    let mut shell = Shell::new();
    let start = std::time::Instant::now();
    let _ = shell.exec("sleep 0.01");
    assert!(start.elapsed().as_secs() < 2);
}

#[test]
fn security_sed_branch_limit() {
    let mut shell = Shell::new();
    let r = shell.exec("echo x | sed ':l; b l'");
    let err = r.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("branch loop limit"), "expected branch loop limit error, got: {msg}");
}

#[test]
fn security_array_index_cap() {
    let mut shell = Shell::new();
    let r = shell.exec("a[9999999]=x; echo done");
    assert!(r.is_err(), "large array index should produce an error");
}

#[test]
fn security_fs_write_limit() {
    let mut shell = Shell::builder()
        .limits(ExecutionLimits {
            max_output_size: 500,
            ..ExecutionLimits::default()
        })
        .build();
    let r = shell.exec("for i in $(seq 500); do echo 'xxxxxxxxxxxxxxxxxxxx'; done > /tmp/big.txt");
    assert_limit(r, LimitKind::OutputSize);
}

#[test]
fn security_combined_limits() {
    let mut shell = Shell::builder()
        .limits(ExecutionLimits {
            max_loop_iterations: 100,
            max_output_size: 500,
            max_command_count: 200,
            ..ExecutionLimits::default()
        })
        .build();
    let r = shell.exec("while true; do echo 'xxxxxxxxxx'; done");
    assert_limit(r, LimitKind::CommandCount);
}
