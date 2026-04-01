use vbash::Shell;

macro_rules! t {
    ($name:ident, $cmd:expr, $out:expr) => {
        #[test]
        fn $name() {
            let mut sh = Shell::new();
            let r = sh.exec($cmd).unwrap();
            assert_eq!(r.stdout, $out);
        }
    };
    ($name:ident, $cmd:expr, $out:expr, exit $code:expr) => {
        #[test]
        fn $name() {
            let mut sh = Shell::new();
            let r = sh.exec($cmd).unwrap();
            assert_eq!(r.stdout, $out);
            assert_eq!(r.exit_code, $code);
        }
    };
}

// Quoting
t!(syntax_single_quote_literal, "echo '$HOME'", "$HOME\n");
t!(syntax_double_quote_expands, "HOME=/test; echo \"$HOME\"", "/test\n");
t!(syntax_adjacent_quotes, "echo \"hello\"'world'", "helloworld\n");
t!(syntax_empty_string_arg, "echo \"\"", "\n");
t!(syntax_escaped_dollar, "echo \\$HOME", "$HOME\n");
t!(syntax_escaped_space, "echo hello\\ world", "hello world\n");
t!(syntax_backslash_in_double, "echo \"hello\\\\world\"", "hello\\world\n");
t!(syntax_single_in_double, "echo \"it's\"", "it's\n");

// Command substitution
t!(syntax_cmd_subst_dollar, "echo $(echo hello)", "hello\n");
t!(syntax_cmd_subst_backtick, "echo `echo hello`", "hello\n");
t!(syntax_cmd_subst_nested, "echo $(echo $(echo deep))", "deep\n");
t!(syntax_cmd_subst_in_string, "echo \"hello $(echo world)\"", "hello world\n");

#[test]
fn syntax_cmd_subst_strips_newline() {
    let mut sh = Shell::new();
    let r = sh.exec("X=$(echo -e 'a\\nb'); echo \"$X\"").unwrap();
    assert_eq!(r.stdout, "a\nb\n");
}

#[test]
fn syntax_cmd_subst_exit_code() {
    let mut sh = Shell::new();
    let r = sh.exec("echo \"$(false)\"; echo $?").unwrap();
    assert_eq!(r.stdout, "\n0\n");
}

// Separators
t!(syntax_semicolons, "echo a; echo b", "a\nb\n");
t!(syntax_comment, "echo hello # this is ignored", "hello\n");
t!(syntax_comment_not_in_string, "echo \"hello # world\"", "hello # world\n");

#[test]
fn syntax_newlines() {
    let mut sh = Shell::new();
    let r = sh.exec("echo a\necho b").unwrap();
    assert_eq!(r.stdout, "a\nb\n");
}

// Brace expansion
t!(syntax_brace_list, "echo {a,b,c}", "a b c\n");
t!(syntax_brace_range_num, "echo {1..5}", "1 2 3 4 5\n");
t!(syntax_brace_range_step, "echo {0..10..3}", "0 3 6 9\n");
t!(syntax_brace_prefix_suffix, "echo pre{A,B}post", "preApost preBpost\n");
t!(syntax_brace_nested, "echo {a,b}{1,2}", "a1 a2 b1 b2\n");

// Arithmetic
t!(syntax_arith_expansion, "echo $((2+3))", "5\n");
t!(syntax_arith_in_string, "echo \"result is $((5*6))\"", "result is 30\n");
t!(syntax_arith_command_true, "(( 5 > 3 )) && echo yes", "yes\n");
t!(syntax_arith_command_false, "(( 0 )) || echo no", "no\n");

// Assignment
t!(syntax_assign_simple, "X=hello; echo $X", "hello\n");
t!(syntax_assign_with_spaces, "X=\"hello world\"; echo \"$X\"", "hello world\n");

#[test]
fn syntax_assign_no_command() {
    let mut sh = Shell::new();
    let r = sh.exec("X=5").unwrap();
    assert_eq!(r.exit_code, 0);
    assert_eq!(r.stdout, "");
}

t!(syntax_multiple_assign, "A=1 B=2 C=3; echo $A $B $C", "1 2 3\n");
t!(syntax_assign_export, "export X=test; printenv X", "test\n");

// Subshell vs group
t!(syntax_subshell, "(echo hello)", "hello\n");
t!(syntax_subshell_var_isolation, "X=1; (X=2; echo $X); echo $X", "2\n1\n");
t!(syntax_group, "{ echo hello; }", "hello\n");
t!(syntax_group_shares_state, "X=1; { X=2; }; echo $X", "2\n");

// Here-string
t!(syntax_herestring, "cat <<< \"hello\"", "hello\n");
t!(syntax_herestring_var, "X=world; cat <<< \"hello $X\"", "hello world\n");

// Pipeline
t!(syntax_pipe_simple, "echo hello | cat", "hello\n");
t!(syntax_pipe_exit_last, "false | true; echo $?", "0\n");
t!(syntax_pipe_negate, "! false; echo $?", "0\n");
t!(syntax_pipe_negate_true, "! true; echo $?", "1\n");

// Operators
#[test]
fn syntax_and_short_circuit() {
    let mut sh = Shell::new();
    let r = sh.exec("false && echo no").unwrap();
    assert_eq!(r.stdout, "");
}

#[test]
fn syntax_or_short_circuit() {
    let mut sh = Shell::new();
    let r = sh.exec("true || echo no").unwrap();
    assert_eq!(r.stdout, "");
}

t!(syntax_and_chain, "true && true && echo yes", "yes\n");
t!(syntax_or_chain, "false || false || echo fallback", "fallback\n");

// Special
t!(syntax_colon_noop, ": ; echo $?", "0\n");
t!(syntax_true_false, "true; echo $?; false; echo $?", "0\n1\n");

// Quoting extras
t!(syntax_dollar_single_escape, "echo $'hello\\nworld'", "hello\nworld\n");
t!(syntax_empty_single_quotes, "echo ''", "\n");
t!(syntax_backslash_newline_continuation, "echo hel\\\nlo", "hello\n");

// Nested braces
t!(syntax_brace_range_alpha, "echo {a..e}", "a b c d e\n");

// More pipes
t!(syntax_multi_pipe, "echo hello world | tr ' ' '\\n' | sort", "hello\nworld\n");

// Compound
t!(syntax_while_loop, "X=0; while [ $X -lt 3 ]; do echo $X; X=$((X+1)); done", "0\n1\n2\n");
t!(syntax_until_loop, "X=0; until [ $X -ge 3 ]; do echo $X; X=$((X+1)); done", "0\n1\n2\n");
t!(syntax_case_simple, "X=hello; case $X in hello) echo matched;; *) echo no;; esac", "matched\n");
t!(syntax_case_wildcard, "X=test; case $X in foo) echo foo;; *) echo default;; esac", "default\n");

// Redirection
#[test]
fn syntax_redirect_append() {
    let mut sh = Shell::new();
    sh.exec("echo first > /tmp/out.txt").unwrap();
    sh.exec("echo second >> /tmp/out.txt").unwrap();
    let r = sh.exec("cat /tmp/out.txt").unwrap();
    assert_eq!(r.stdout, "first\nsecond\n");
}

#[test]
fn syntax_redirect_stderr() {
    let mut sh = Shell::new();
    let r = sh.exec("echo hello 2>/dev/null").unwrap();
    assert_eq!(r.stdout, "hello\n");
}

#[test]
fn syntax_redirect_input() {
    let mut sh = Shell::new();
    sh.exec("echo hello > /tmp/in.txt").unwrap();
    let r = sh.exec("cat < /tmp/in.txt").unwrap();
    assert_eq!(r.stdout, "hello\n");
}

// Function
t!(syntax_function_def, "f() { echo inside; }; f", "inside\n");
t!(syntax_function_args, "f() { echo $1 $2; }; f hello world", "hello world\n");
t!(syntax_function_return, "f() { return 42; }; f; echo $?", "42\n");

// Test / bracket
t!(syntax_test_string_eq, "[ \"hello\" = \"hello\" ] && echo yes", "yes\n");
t!(syntax_test_string_neq, "[ \"hello\" != \"world\" ] && echo yes", "yes\n");
t!(syntax_test_int_lt, "[ 3 -lt 5 ] && echo yes", "yes\n");
t!(syntax_double_bracket, "[[ \"hello\" == \"hello\" ]] && echo yes", "yes\n");

// === Behavioral tests ===

#[test]
fn beh_true_exit() {
    let mut sh = Shell::new();
    let r = sh.exec("true").unwrap();
    assert_eq!(r.exit_code, 0);
}

#[test]
fn beh_false_exit() {
    let mut sh = Shell::new();
    let r = sh.exec("false").unwrap();
    assert_eq!(r.exit_code, 1);
}

#[test]
fn beh_last_cmd_wins() {
    let mut sh = Shell::new();
    let r = sh.exec("true; false").unwrap();
    assert_eq!(r.exit_code, 1);
}

#[test]
fn beh_last_cmd_wins2() {
    let mut sh = Shell::new();
    let r = sh.exec("false; true").unwrap();
    assert_eq!(r.exit_code, 0);
}

#[test]
fn beh_subshell_exit() {
    let mut sh = Shell::new();
    let r = sh.exec("(exit 42)").unwrap();
    assert_eq!(r.exit_code, 42);
}

#[test]
fn beh_not_found_127() {
    let mut sh = Shell::new();
    let r = sh.exec("nonexistent_cmd").unwrap();
    assert_eq!(r.exit_code, 127);
}

#[test]
fn beh_assignment_exit_zero() {
    let mut sh = Shell::new();
    let r = sh.exec("X=5").unwrap();
    assert_eq!(r.exit_code, 0);
}

#[test]
fn beh_empty_input() {
    let mut sh = Shell::new();
    let r = sh.exec("").unwrap();
    assert_eq!(r.exit_code, 0);
}

#[test]
fn beh_unset_expands_empty() {
    let mut sh = Shell::new();
    let r = sh.exec("echo \"[$UNSET]\"").unwrap();
    assert_eq!(r.stdout, "[]\n");
}

#[test]
fn beh_empty_var() {
    let mut sh = Shell::new();
    let r = sh.exec("X=''; echo \"[$X]\"").unwrap();
    assert_eq!(r.stdout, "[]\n");
}

#[test]
fn beh_empty_vs_unset_set() {
    let mut sh = Shell::new();
    let r = sh.exec("X=''; echo \"${X-default}\"").unwrap();
    assert_eq!(r.stdout, "\n");
}

#[test]
fn beh_empty_vs_unset_unset() {
    let mut sh = Shell::new();
    let r = sh.exec("echo \"${UNSET_VAR_XYZ-default}\"").unwrap();
    assert_eq!(r.stdout, "default\n");
}

#[test]
fn beh_readonly_assign_fails() {
    let mut sh = Shell::new();
    let result = sh.exec("readonly X=5; X=6");
    match result {
        Ok(r) => {
            assert_ne!(r.exit_code, 0);
            assert!(!r.stderr.is_empty());
        }
        Err(e) => {
            let msg = format!("{e}");
            assert!(msg.contains("readonly"), "expected readonly error, got: {msg}");
        }
    }
}

#[test]
fn beh_readonly_unset_fails() {
    let mut sh = Shell::new();
    let r = sh.exec("readonly X=5; unset X").unwrap();
    assert_ne!(r.exit_code, 0);
    assert!(!r.stderr.is_empty());
}

#[test]
fn beh_multiple_assignment() {
    let mut sh = Shell::new();
    let r = sh.exec("A=1 B=2 C=3; echo \"$A $B $C\"").unwrap();
    assert_eq!(r.stdout, "1 2 3\n");
}

#[test]
fn beh_export_visible() {
    let mut sh = Shell::new();
    let r = sh.exec("export X=hello; printenv X").unwrap();
    assert_eq!(r.stdout, "hello\n");
}

#[test]
fn beh_arith_unset_is_zero() {
    let mut sh = Shell::new();
    let r = sh.exec("echo $((UNSET_VAR))").unwrap();
    assert_eq!(r.stdout, "0\n");
}

#[test]
fn beh_arith_string_is_zero() {
    let mut sh = Shell::new();
    let r = sh.exec("X=abc; echo $((X))").unwrap();
    assert_eq!(r.stdout, "0\n");
}

#[test]
fn beh_arith_numeric_string() {
    let mut sh = Shell::new();
    let r = sh.exec("X=42; echo $((X+1))").unwrap();
    assert_eq!(r.stdout, "43\n");
}

#[test]
fn beh_unquoted_splits() {
    let mut sh = Shell::new();
    let r = sh.exec("X=\"a   b   c\"; for x in $X; do echo $x; done").unwrap();
    assert_eq!(r.stdout, "a\nb\nc\n");
}

#[test]
fn beh_quoted_no_split() {
    let mut sh = Shell::new();
    let r = sh.exec("X=\"a   b   c\"; for x in \"$X\"; do echo \"$x\"; done").unwrap();
    assert_eq!(r.stdout, "a   b   c\n");
}

#[test]
fn beh_empty_field_removal() {
    let mut sh = Shell::new();
    let r = sh.exec("X=''; echo $X end").unwrap();
    assert_eq!(r.stdout, "end\n");
}

#[test]
fn beh_quoted_empty_preserved() {
    let mut sh = Shell::new();
    let r = sh.exec("X=''; echo \"$X\" end").unwrap();
    assert_eq!(r.stdout, " end\n");
}

#[test]
fn beh_concatenation() {
    let mut sh = Shell::new();
    let r = sh.exec("X=hello; echo ${X}world").unwrap();
    assert_eq!(r.stdout, "helloworld\n");
}

#[test]
fn beh_adjacent_strings() {
    let mut sh = Shell::new();
    let r = sh.exec("echo \"a\"\"b\"'c'").unwrap();
    assert_eq!(r.stdout, "abc\n");
}

#[test]
fn beh_redirect_creates_file() {
    let mut sh = Shell::new();
    sh.exec("echo hi > /tmp/beh.txt").unwrap();
    let r = sh.exec("cat /tmp/beh.txt").unwrap();
    assert_eq!(r.stdout, "hi\n");
}

#[test]
fn beh_redirect_truncates() {
    let mut sh = Shell::new();
    sh.exec("echo first > /tmp/beh2.txt").unwrap();
    sh.exec("echo second > /tmp/beh2.txt").unwrap();
    let r = sh.exec("cat /tmp/beh2.txt").unwrap();
    assert_eq!(r.stdout, "second\n");
}

#[test]
fn beh_append_creates() {
    let mut sh = Shell::new();
    sh.exec("echo a >> /tmp/beh3.txt").unwrap();
    sh.exec("echo b >> /tmp/beh3.txt").unwrap();
    let r = sh.exec("cat /tmp/beh3.txt").unwrap();
    assert_eq!(r.stdout, "a\nb\n");
}

#[test]
fn beh_dev_null_write() {
    let mut sh = Shell::new();
    let r = sh.exec("echo test > /dev/null").unwrap();
    assert_eq!(r.stdout, "");
    assert_eq!(r.exit_code, 0);
}

#[test]
fn beh_read_dev_null() {
    let mut sh = Shell::new();
    let r = sh.exec("cat /dev/null").unwrap();
    assert_eq!(r.stdout, "");
    assert_eq!(r.exit_code, 0);
}

#[test]
fn beh_semicolons_same_as_newlines() {
    let mut sh = Shell::new();
    let r1 = sh.exec("echo a; echo b").unwrap();
    let r2 = sh.exec("echo a\necho b").unwrap();
    assert_eq!(r1.stdout, r2.stdout);
}

#[test]
fn beh_trailing_semicolon() {
    let mut sh = Shell::new();
    let r = sh.exec("echo hi;").unwrap();
    assert_eq!(r.stdout, "hi\n");
}

#[test]
fn beh_hash_in_word() {
    let mut sh = Shell::new();
    let r = sh.exec("echo hello#world").unwrap();
    assert_eq!(r.stdout, "hello#world\n");
}

#[test]
fn beh_comment_standalone() {
    let mut sh = Shell::new();
    let r = sh.exec("# just a comment").unwrap();
    assert_eq!(r.stdout, "");
    assert_eq!(r.exit_code, 0);
}

#[test]
fn beh_tilde_home() {
    let mut sh = Shell::new();
    let r = sh.exec("echo ~").unwrap();
    assert_eq!(r.stdout, "/home/user\n");
}

#[test]
fn beh_tilde_plus() {
    let mut sh = Shell::new();
    let r = sh.exec("echo ~+").unwrap();
    assert_eq!(r.stdout, "/home/user\n");
}

#[test]
fn beh_pid_is_number() {
    let mut sh = Shell::new();
    let r = sh.exec("echo $$").unwrap();
    let trimmed = r.stdout.trim();
    assert!(trimmed.parse::<u64>().is_ok(), "expected numeric PID, got: {trimmed}");
}

#[test]
fn beh_seconds_small() {
    let mut sh = Shell::new();
    let r = sh.exec("echo $SECONDS").unwrap();
    let val: u64 = r.stdout.trim().parse().expect("SECONDS should be numeric");
    assert!(val <= 2, "SECONDS should be small, got: {val}");
}

#[test]
fn beh_while_loop_basic() {
    let mut sh = Shell::new();
    let r = sh.exec("i=0; while [ $i -lt 3 ]; do echo $i; i=$((i+1)); done").unwrap();
    assert_eq!(r.stdout, "0\n1\n2\n");
}

#[test]
fn beh_case_statement() {
    let mut sh = Shell::new();
    let r = sh.exec("X=hello; case $X in hello) echo matched;; *) echo nope;; esac").unwrap();
    assert_eq!(r.stdout, "matched\n");
}

#[test]
fn beh_nested_command_substitution() {
    let mut sh = Shell::new();
    let r = sh.exec("echo $(echo $(echo deep))").unwrap();
    assert_eq!(r.stdout, "deep\n");
}

#[test]
fn beh_backtick_substitution() {
    let mut sh = Shell::new();
    let r = sh.exec("echo `echo backtick`").unwrap();
    assert_eq!(r.stdout, "backtick\n");
}
