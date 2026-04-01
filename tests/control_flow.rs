use vbash::Shell;

#[test]
fn cf_break_inner() {
    let mut shell = Shell::new();
    let r = shell.exec("for i in 1 2; do for j in a b c; do if [ $j = b ]; then break; fi; echo $i$j; done; done").unwrap();
    assert_eq!(r.stdout, "1a\n2a\n");
}

#[test]
fn cf_break_two() {
    let mut shell = Shell::new();
    let r = shell.exec("for i in 1 2 3; do for j in a b; do if [ $i = 2 ]; then break 2; fi; echo $i$j; done; done").unwrap();
    assert_eq!(r.stdout, "1a\n1b\n");
}

#[test]
fn cf_continue_inner() {
    let mut shell = Shell::new();
    let r = shell.exec("for i in 1 2; do for j in a b c; do if [ $j = b ]; then continue; fi; echo $i$j; done; done").unwrap();
    assert_eq!(r.stdout, "1a\n1c\n2a\n2c\n");
}

#[test]
fn cf_continue_two() {
    let mut shell = Shell::new();
    let r = shell.exec("for i in 1 2 3; do for j in a b; do if [ $j = b ]; then continue 2; fi; echo $i$j; done; done").unwrap();
    assert_eq!(r.stdout, "1a\n2a\n3a\n");
}

#[test]
fn cf_subshell_exit() {
    let mut shell = Shell::new();
    let r = shell.exec("(exit 42); echo $?").unwrap();
    assert_eq!(r.stdout, "42\n");
}

#[test]
fn cf_subshell_var() {
    let mut shell = Shell::new();
    let r = shell.exec("X=1; (X=2); echo $X").unwrap();
    assert_eq!(r.stdout, "1\n");
}

#[test]
fn cf_subshell_function() {
    let mut shell = Shell::new();
    let r = shell.exec("f() { echo inner; }; (f)").unwrap();
    assert_eq!(r.stdout, "inner\n");
}

#[test]
fn cf_nested_subshell() {
    let mut shell = Shell::new();
    let r = shell.exec("(echo a; (echo b))").unwrap();
    assert_eq!(r.stdout, "a\nb\n");
}

#[test]
fn cf_function_return() {
    let mut shell = Shell::new();
    let r = shell.exec("f() { return 5; }; f; echo $?").unwrap();
    assert_eq!(r.stdout, "5\n");
}

#[test]
fn cf_function_local() {
    let mut shell = Shell::new();
    let r = shell.exec("f() { local X=inner; echo $X; }; X=outer; f; echo $X").unwrap();
    assert_eq!(r.stdout, "inner\nouter\n");
}

#[test]
fn cf_function_recursive() {
    let mut shell = Shell::new();
    let r = shell.exec("f() { if [ $1 -le 0 ]; then echo done; return; fi; echo $1; f $(($1-1)); }; f 3").unwrap();
    assert_eq!(r.stdout, "3\n2\n1\ndone\n");
}

#[test]
fn cf_function_args() {
    let mut shell = Shell::new();
    let r = shell.exec("f() { echo $1 $2 $#; }; f a b c").unwrap();
    assert_eq!(r.stdout, "a b 3\n");
}

#[test]
fn cf_function_shift() {
    let mut shell = Shell::new();
    let r = shell.exec("f() { shift; echo $1; }; f a b c").unwrap();
    assert_eq!(r.stdout, "b\n");
}

#[test]
fn cf_case_glob() {
    let mut shell = Shell::new();
    let r = shell.exec("X=hello.txt; case $X in *.txt) echo text;; *.sh) echo script;; esac").unwrap();
    assert_eq!(r.stdout, "text\n");
}

#[test]
fn cf_case_multiple() {
    let mut shell = Shell::new();
    let r = shell.exec("X=b; case $X in a|b|c) echo letter;; esac").unwrap();
    assert_eq!(r.stdout, "letter\n");
}

#[test]
fn cf_case_default() {
    let mut shell = Shell::new();
    let r = shell.exec("X=z; case $X in a) echo a;; *) echo default;; esac").unwrap();
    assert_eq!(r.stdout, "default\n");
}

#[test]
fn cf_case_fallthrough() {
    let mut shell = Shell::new();
    let r = shell.exec("X=a; case $X in a) echo a;& b) echo b;; esac").unwrap();
    assert_eq!(r.stdout, "a\nb\n");
}

#[test]
fn cf_while_false_body() {
    let mut shell = Shell::new();
    let r = shell.exec("while false; do echo never; done; echo done").unwrap();
    assert_eq!(r.stdout, "done\n");
}

#[test]
fn cf_until_true_body() {
    let mut shell = Shell::new();
    let r = shell.exec("until true; do echo never; done; echo done").unwrap();
    assert_eq!(r.stdout, "done\n");
}

#[test]
fn cf_while_read() {
    let mut shell = Shell::builder()
        .file("/tmp/lines", "a\nb\nc\n")
        .build();
    let r = shell.exec("while read line; do echo \"[$line]\"; done < /tmp/lines").unwrap();
    assert_eq!(r.stdout, "[a]\n[b]\n[c]\n");
}

#[test]
fn cf_if_pipeline() {
    let mut shell = Shell::new();
    let r = shell.exec("if echo hello | grep -q hello; then echo found; fi").unwrap();
    assert_eq!(r.stdout, "found\n");
}

#[test]
fn cf_if_negated() {
    let mut shell = Shell::new();
    let r = shell.exec("if ! false; then echo yes; fi").unwrap();
    assert_eq!(r.stdout, "yes\n");
}

#[test]
fn cf_elif_chain() {
    let mut shell = Shell::new();
    let r = shell.exec("X=3; if [ $X -eq 1 ]; then echo one; elif [ $X -eq 2 ]; then echo two; elif [ $X -eq 3 ]; then echo three; fi").unwrap();
    assert_eq!(r.stdout, "three\n");
}

#[test]
fn cf_if_empty_then() {
    let mut shell = Shell::new();
    let r = shell.exec("if true; then :; fi; echo ok").unwrap();
    assert_eq!(r.stdout, "ok\n");
}

#[test]
fn cf_errexit_stops() {
    let mut shell = Shell::new();
    let r = shell.exec("set -e; false; echo should_not_print").unwrap();
    assert_eq!(r.exit_code, 1);
    assert!(!r.stdout.contains("should_not_print"));
}

#[test]
fn cf_errexit_if_suppressed() {
    let mut shell = Shell::new();
    let r = shell.exec("set -e; if false; then echo no; fi; echo ok").unwrap();
    assert_eq!(r.stdout, "ok\n");
}

#[test]
fn cf_errexit_and_suppressed() {
    let mut shell = Shell::new();
    let r = shell.exec("set -e; false && true; echo ok").unwrap();
    assert_eq!(r.stdout, "ok\n");
}

#[test]
fn cf_errexit_or_suppressed() {
    let mut shell = Shell::new();
    let r = shell.exec("set -e; true || false; echo ok").unwrap();
    assert_eq!(r.stdout, "ok\n");
}

#[test]
fn cf_trap_exit() {
    let mut shell = Shell::new();
    let r = shell.exec("trap 'echo bye' EXIT; echo hello").unwrap();
    assert_eq!(r.stdout, "hello\nbye\n");
}

#[test]
fn cf_trap_overwrite() {
    let mut shell = Shell::new();
    let r = shell.exec("trap 'echo first' EXIT; trap 'echo second' EXIT; echo hi").unwrap();
    assert_eq!(r.stdout, "hi\nsecond\n");
}

#[test]
fn cf_cfor_basic() {
    let mut shell = Shell::new();
    let r = shell.exec("for ((i=0; i<5; i++)); do echo $i; done").unwrap();
    assert_eq!(r.stdout, "0\n1\n2\n3\n4\n");
}

#[test]
fn cf_cfor_step() {
    let mut shell = Shell::new();
    let r = shell.exec("for ((i=0; i<10; i+=3)); do echo $i; done").unwrap();
    assert_eq!(r.stdout, "0\n3\n6\n9\n");
}

#[test]
fn cf_cfor_empty_body() {
    let mut shell = Shell::new();
    let r = shell.exec("for ((i=0; i<3; i++)); do :; done; echo $i").unwrap();
    assert_eq!(r.stdout, "3\n");
}

#[test]
fn cf_for_empty_list() {
    let mut shell = Shell::new();
    let r = shell.exec("for x in; do echo $x; done; echo done").unwrap();
    assert_eq!(r.stdout, "done\n");
}

#[test]
fn cf_nested_if_in_loop() {
    let mut shell = Shell::new();
    let r = shell.exec("for i in 1 2 3; do if [ $i -eq 2 ]; then echo found; fi; done").unwrap();
    assert_eq!(r.stdout, "found\n");
}

#[test]
fn cf_while_break() {
    let mut shell = Shell::new();
    let r = shell.exec("i=0; while true; do if [ $i -ge 3 ]; then break; fi; echo $i; i=$((i+1)); done").unwrap();
    assert_eq!(r.stdout, "0\n1\n2\n");
}

#[test]
fn cf_while_continue() {
    let mut shell = Shell::new();
    let r = shell.exec("i=0; while [ $i -lt 5 ]; do i=$((i+1)); if [ $i -eq 3 ]; then continue; fi; echo $i; done").unwrap();
    assert_eq!(r.stdout, "1\n2\n4\n5\n");
}

#[test]
fn cf_until_loop() {
    let mut shell = Shell::new();
    let r = shell.exec("i=0; until [ $i -ge 3 ]; do echo $i; i=$((i+1)); done").unwrap();
    assert_eq!(r.stdout, "0\n1\n2\n");
}

#[test]
fn cf_function_no_args() {
    let mut shell = Shell::new();
    let r = shell.exec("f() { echo hello; }; f").unwrap();
    assert_eq!(r.stdout, "hello\n");
}

#[test]
fn cf_subshell_cwd() {
    let mut shell = Shell::new();
    shell.exec("mkdir -p /tmp/sub").unwrap();
    let r = shell.exec("cd /tmp; (cd /tmp/sub; pwd); pwd").unwrap();
    assert_eq!(r.stdout, "/tmp/sub\n/tmp\n");
}

#[test]
fn cf_arithmetic_in_condition() {
    let mut shell = Shell::new();
    let r = shell.exec("if [ $((2+3)) -eq 5 ]; then echo yes; fi").unwrap();
    assert_eq!(r.stdout, "yes\n");
}

#[test]
fn cf_nested_function_call() {
    let mut shell = Shell::new();
    let r = shell.exec("a() { echo from_a; }; b() { a; echo from_b; }; b").unwrap();
    assert_eq!(r.stdout, "from_a\nfrom_b\n");
}

#[test]
fn cf_for_with_seq_subst() {
    let mut shell = Shell::new();
    let r = shell.exec("for i in $(seq 3); do echo $i; done").unwrap();
    assert_eq!(r.stdout, "1\n2\n3\n");
}

#[test]
fn cf_case_numeric() {
    let mut shell = Shell::new();
    let r = shell.exec("X=2; case $X in 1) echo one;; 2) echo two;; 3) echo three;; esac").unwrap();
    assert_eq!(r.stdout, "two\n");
}

#[test]
fn cf_if_and_or_combination() {
    let mut shell = Shell::new();
    let r = shell.exec("true && echo a; false && echo b; false || echo c").unwrap();
    assert_eq!(r.stdout, "a\nc\n");
}

#[test]
fn cf_nested_loops() {
    let mut shell = Shell::new();
    let r = shell.exec("for i in 1 2; do for j in a b; do echo $i$j; done; done").unwrap();
    assert_eq!(r.stdout, "1a\n1b\n2a\n2b\n");
}
