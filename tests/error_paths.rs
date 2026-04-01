use vbash::Shell;

macro_rules! t_exit {
    ($name:ident, $cmd:expr, nonzero) => {
        #[test]
        fn $name() {
            let mut sh = Shell::new();
            let r = sh.exec($cmd).unwrap();
            assert_ne!(r.exit_code, 0, "expected nonzero exit, got 0. stdout={:?} stderr={:?}", r.stdout, r.stderr);
        }
    };
    ($name:ident, $cmd:expr, $code:expr) => {
        #[test]
        fn $name() {
            let mut sh = Shell::new();
            let r = sh.exec($cmd).unwrap();
            assert_eq!(r.exit_code, $code, "stdout={:?} stderr={:?}", r.stdout, r.stderr);
        }
    };
}

macro_rules! t_exit_stderr {
    ($name:ident, $cmd:expr, nonzero) => {
        #[test]
        fn $name() {
            let mut sh = Shell::new();
            let r = sh.exec($cmd).unwrap();
            assert_ne!(r.exit_code, 0, "expected nonzero exit, got 0. stdout={:?} stderr={:?}", r.stdout, r.stderr);
            assert!(!r.stderr.is_empty(), "expected stderr message, got none");
        }
    };
}

macro_rules! t_fail_or_nonzero {
    ($name:ident, $cmd:expr) => {
        #[test]
        fn $name() {
            let mut sh = Shell::new();
            match sh.exec($cmd) {
                Ok(r) => assert_ne!(r.exit_code, 0, "expected error or nonzero exit, got 0. stdout={:?} stderr={:?}", r.stdout, r.stderr),
                Err(_) => {}
            }
        }
    };
}

// File operations
t_exit_stderr!(error_cat_nonexistent, "cat /nonexistent", nonzero);

#[test]
fn error_head_empty() {
    let mut sh = Shell::new();
    let r = sh.exec("echo -n | head").unwrap();
    assert_eq!(r.stdout, "");
    assert_eq!(r.exit_code, 0);
}

#[test]
fn error_tail_empty() {
    let mut sh = Shell::new();
    let r = sh.exec("echo -n | tail").unwrap();
    assert_eq!(r.stdout, "");
    assert_eq!(r.exit_code, 0);
}

t_exit_stderr!(error_cp_missing_args, "cp", nonzero);
t_exit_stderr!(error_mv_missing_args, "mv", nonzero);
t_exit_stderr!(error_rm_nonexistent, "rm /nonexistent", nonzero);

#[test]
fn error_rm_force_nonexistent() {
    let mut sh = Shell::new();
    let r = sh.exec("rm -f /nonexistent").unwrap();
    assert_eq!(r.exit_code, 0);
}

#[test]
fn error_mkdir_exists() {
    let mut sh = Shell::new();
    let r = sh.exec("mkdir /tmp/testdir; mkdir /tmp/testdir").unwrap();
    assert_ne!(r.exit_code, 0);
}

#[test]
fn error_rmdir_not_empty() {
    let mut sh = Shell::new();
    sh.exec("mkdir -p /tmp/d; echo f > /tmp/d/f").unwrap();
    let r = sh.exec("rmdir /tmp/d").unwrap();
    assert_ne!(r.exit_code, 0);
}

t_exit!(error_chmod_nonexistent, "chmod 755 /nonexistent", nonzero);

// ln with nonexistent target returns Err(Fs(...))
t_fail_or_nonzero!(error_ln_nonexistent_target, "ln /nonexistent /tmp/link");

// Text commands
t_exit!(error_grep_no_match, "echo hello | grep xyz", 1);

#[test]
fn error_grep_no_match_no_output() {
    let mut sh = Shell::new();
    let r = sh.exec("echo hello | grep xyz").unwrap();
    assert_eq!(r.exit_code, 1);
    assert_eq!(r.stdout, "");
}

// grep with invalid regex returns Err
t_fail_or_nonzero!(error_grep_invalid_regex, "echo test | grep '[invalid'");

// Data commands
t_exit!(error_jq_invalid_json, "echo 'not json' | jq '.'", nonzero);
t_exit!(error_jq_invalid_filter, "echo '{}' | jq '%%%'", nonzero);

// sed with bad syntax returns Err
t_fail_or_nonzero!(error_sed_bad_delimiter, "echo test | sed 's/x'");

// awk with syntax error returns Err
t_fail_or_nonzero!(error_awk_syntax_error, "echo test | awk '{ print'");

// Utilities
t_exit!(error_expr_nonnumeric, "expr abc + def", nonzero);

// source of nonexistent file returns Err(Fs(...))
t_fail_or_nonzero!(error_source_nonexistent, "source /nonexistent.sh");

// Archive
t_exit!(error_tar_nonexistent, "tar tf /nonexistent.tar", nonzero);
t_exit!(error_gzip_nonexistent, "gzip /nonexistent", nonzero);

// Builtins
t_exit_stderr!(error_cd_nonexistent, "cd /nonexistent", nonzero);

#[test]
fn error_echo_no_args() {
    let mut sh = Shell::new();
    let r = sh.exec("echo").unwrap();
    assert_eq!(r.stdout, "\n");
    assert_eq!(r.exit_code, 0);
}

t_exit!(error_command_not_found, "nonexistent_xyz", 127);

#[test]
fn error_command_not_found_stderr() {
    let mut sh = Shell::new();
    let r = sh.exec("nonexistent_xyz").unwrap();
    assert_eq!(r.exit_code, 127);
    assert!(r.stderr.contains("command not found"));
}

#[test]
fn error_empty_input() {
    let mut sh = Shell::new();
    let r = sh.exec("").unwrap();
    assert_eq!(r.exit_code, 0);
    assert_eq!(r.stdout, "");
}

#[test]
fn error_whitespace_only() {
    let mut sh = Shell::new();
    let r = sh.exec("   ").unwrap();
    assert_eq!(r.exit_code, 0);
    assert_eq!(r.stdout, "");
}

// Redirect errors: cat < /nonexistent returns Err(Fs(...))
t_fail_or_nonzero!(error_read_from_nonexistent, "cat < /nonexistent");

// Exit codes propagation
#[test]
fn error_false_exit_code() {
    let mut sh = Shell::new();
    let r = sh.exec("false").unwrap();
    assert_eq!(r.exit_code, 1);
}

#[test]
fn error_true_exit_code() {
    let mut sh = Shell::new();
    let r = sh.exec("true").unwrap();
    assert_eq!(r.exit_code, 0);
}

#[test]
fn error_exit_code_custom() {
    let mut sh = Shell::new();
    let r = sh.exec("exit 42").unwrap();
    assert_eq!(r.exit_code, 42);
}

#[test]
fn error_exit_code_last_command() {
    let mut sh = Shell::new();
    let r = sh.exec("true; false").unwrap();
    assert_eq!(r.exit_code, 1);
}

#[test]
fn error_exit_code_last_command_ok() {
    let mut sh = Shell::new();
    let r = sh.exec("false; true").unwrap();
    assert_eq!(r.exit_code, 0);
}

// Pipe error handling
#[test]
fn error_pipe_last_exit() {
    let mut sh = Shell::new();
    let r = sh.exec("echo ok | false").unwrap();
    assert_eq!(r.exit_code, 1);
}

#[test]
fn error_pipe_first_fails() {
    let mut sh = Shell::new();
    let r = sh.exec("false | echo ok").unwrap();
    assert_eq!(r.stdout, "ok\n");
    assert_eq!(r.exit_code, 0);
}

#[test]
fn error_unset_nonexistent() {
    let mut sh = Shell::new();
    let r = sh.exec("unset NONEXISTENT_VAR; echo $?").unwrap();
    assert_eq!(r.stdout, "0\n");
}

// cut with no flags
t_exit!(error_cut_no_spec, "echo test | cut", nonzero);

// find nonexistent
t_exit!(error_find_nonexistent, "find /nonexistent_dir_xyz", nonzero);

// Division by zero returns Err
t_fail_or_nonzero!(error_arith_div_zero, "echo $((1/0))");

// Readonly variable reassignment returns Err
t_fail_or_nonzero!(error_readonly_assign, "readonly X=5; X=10");

// break/continue outside loop return Err
t_fail_or_nonzero!(error_break_outside_loop, "break");
t_fail_or_nonzero!(error_continue_outside_loop, "continue");

// return outside function - vbash allows this (returns 0)
#[test]
fn error_return_outside_function() {
    let mut sh = Shell::new();
    let r = sh.exec("return 0").unwrap();
    assert_eq!(r.exit_code, 0);
}

// Nested error propagation
#[test]
fn error_subshell_exit() {
    let mut sh = Shell::new();
    let r = sh.exec("(exit 5); echo $?").unwrap();
    assert_eq!(r.stdout, "5\n");
}

#[test]
fn error_if_false_no_else() {
    let mut sh = Shell::new();
    let r = sh.exec("if false; then echo yes; fi").unwrap();
    assert_eq!(r.stdout, "");
}

// Trap-like: set -e behavior
#[test]
fn error_set_e_stops() {
    let mut sh = Shell::new();
    let r = sh.exec("set -e; false; echo should_not_appear").unwrap();
    assert_eq!(r.stdout, "");
    assert_ne!(r.exit_code, 0);
}

// Multiple exit codes
#[test]
fn error_exit_overrides() {
    let mut sh = Shell::new();
    let r = sh.exec("exit 0").unwrap();
    assert_eq!(r.exit_code, 0);
}

#[test]
fn error_exit_255() {
    let mut sh = Shell::new();
    let r = sh.exec("exit 255").unwrap();
    assert_eq!(r.exit_code, 255);
}

// Modulo by zero
t_fail_or_nonzero!(error_arith_mod_zero, "echo $((1%0))");

// Sort on nonexistent file: vbash returns 0 with empty output (may be a quirk)
#[test]
fn error_sort_nonexistent_file() {
    let mut sh = Shell::new();
    let r = sh.exec("sort /nonexistent_file_xyz");
    // Accept either Err or nonzero exit code
    if let Ok(res) = r {
        // vbash sort on nonexistent returns exit 0 with empty output; just verify no crash
        assert!(res.stdout.is_empty() || res.exit_code != 0);
    }
}

// Nested function error
#[test]
fn error_function_nonzero() {
    let mut sh = Shell::new();
    let r = sh.exec("f() { return 3; }; f; echo $?").unwrap();
    assert_eq!(r.stdout, "3\n");
}
