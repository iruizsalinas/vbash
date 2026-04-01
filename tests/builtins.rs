use vbash::{Error, ExecError, Shell};

#[test]
fn unset_variable() {
    let mut shell = Shell::new();
    let r = shell.exec("X=hello; unset X; echo ${X:-gone}").unwrap();
    assert_eq!(r.stdout, "gone\n");
}

#[test]
fn unset_multiple_variables() {
    let mut shell = Shell::new();
    let r = shell.exec("A=1; B=2; unset A B; echo \"${A:-gone}\" \"${B:-gone}\"").unwrap();
    assert_eq!(r.stdout, "gone gone\n");
}

#[test]
fn shift_basic() {
    let mut shell = Shell::new();
    let r = shell.exec("set -- a b c; shift; echo $1").unwrap();
    assert_eq!(r.stdout, "b\n");
}

#[test]
fn shift_multiple() {
    let mut shell = Shell::new();
    let r = shell.exec("set -- a b c d; shift 2; echo $1").unwrap();
    assert_eq!(r.stdout, "c\n");
}

#[test]
fn shift_all() {
    let mut shell = Shell::new();
    let r = shell.exec("set -- a b; shift 5; echo $#").unwrap();
    assert_eq!(r.stdout, "0\n");
}

#[test]
fn alias_define_and_list() {
    let mut shell = Shell::new();
    let r = shell.exec("alias foo='echo bar'; alias").unwrap();
    assert_eq!(r.stdout, "alias foo='echo bar'\n");
}

#[test]
fn unalias_basic() {
    let mut shell = Shell::new();
    let r = shell.exec("alias foo='echo x'; unalias foo; type foo").unwrap();
    assert_eq!(r.stdout, "type: foo: not found\n");
    assert_eq!(r.exit_code, 1);
}

#[test]
fn unalias_all() {
    let mut shell = Shell::new();
    let r = shell.exec("alias a='x'; alias b='y'; unalias -a; alias").unwrap();
    assert_eq!(r.stdout, "");
}

#[test]
fn type_builtin() {
    let mut shell = Shell::new();
    let r = shell.exec("type cd").unwrap();
    assert!(r.stdout.contains("builtin"));
}

#[test]
fn type_function() {
    let mut shell = Shell::new();
    let r = shell.exec("f() { :; }; type f").unwrap();
    assert!(r.stdout.contains("function"));
}

#[test]
fn type_not_found() {
    let mut shell = Shell::new();
    let r = shell.exec("type nonexistent_xyz123").unwrap();
    assert_eq!(r.stdout, "type: nonexistent_xyz123: not found\n");
    assert_eq!(r.exit_code, 1);
}

#[test]
fn command_runs_builtin() {
    let mut shell = Shell::new();
    let r = shell.exec("command echo hello").unwrap();
    assert_eq!(r.stdout, "hello\n");
}

#[test]
fn hash_noop() {
    let mut shell = Shell::new();
    let r = shell.exec("hash").unwrap();
    assert_eq!(r.exit_code, 0);
}

#[test]
fn popd_basic() {
    let mut shell = Shell::new();
    let r = shell.exec("pushd /tmp > /dev/null; popd > /dev/null; pwd").unwrap();
    assert_eq!(r.stdout, "/home/user\n");
}

#[test]
fn dirs_shows_stack() {
    let mut shell = Shell::new();
    let r = shell.exec("pushd /tmp > /dev/null; dirs").unwrap();
    assert!(r.stdout.contains("/tmp"));
}

#[test]
fn mapfile_from_file() {
    let mut shell = Shell::builder()
        .file("/tmp/lines.txt", "alpha\nbeta\ngamma\n")
        .build();
    let r = shell.exec("mapfile -t arr < /tmp/lines.txt; echo ${#arr[@]}").unwrap();
    assert_eq!(r.stdout, "3\n");
}

#[test]
fn readarray_alias() {
    let mut shell = Shell::builder()
        .file("/tmp/lines.txt", "one\ntwo\n")
        .build();
    let r = shell.exec("readarray -t arr < /tmp/lines.txt; echo ${#arr[@]}").unwrap();
    assert_eq!(r.stdout, "2\n");
}

#[test]
fn exit_with_code() {
    let mut shell = Shell::new();
    let r = shell.exec("exit 42").unwrap();
    assert_eq!(r.exit_code, 42);
}

#[test]
fn return_sets_code() {
    let mut shell = Shell::new();
    let r = shell.exec("f() { return 5; }; f; echo $?").unwrap();
    assert_eq!(r.stdout, "5\n");
}

#[test]
fn eval_basic() {
    let mut shell = Shell::new();
    let r = shell.exec("eval 'echo hello'").unwrap();
    assert_eq!(r.stdout, "hello\n");
}

#[test]
fn eval_variable() {
    let mut shell = Shell::new();
    let r = shell.exec("X='echo hi'; eval $X").unwrap();
    assert_eq!(r.stdout, "hi\n");
}

#[test]
fn readonly_value() {
    let mut shell = Shell::new();
    let r = shell.exec("readonly X=5; echo $X").unwrap();
    assert_eq!(r.stdout, "5\n");
}

#[test]
fn local_restores() {
    let mut shell = Shell::new();
    let r = shell.exec("X=outer; f() { local X=inner; }; f; echo $X").unwrap();
    assert_eq!(r.stdout, "outer\n");
}

#[test]
fn local_visible_in_function() {
    let mut shell = Shell::new();
    let r = shell.exec("f() { local X=local; echo $X; }; f").unwrap();
    assert_eq!(r.stdout, "local\n");
}

#[test]
fn local_outside_function_fails() {
    let mut shell = Shell::new();
    let r = shell.exec("local x=1").unwrap();
    assert_eq!(r.exit_code, 1);
    assert_eq!(r.stderr, "local: can only be used in a function\n");
}

#[test]
fn read_default_reply() {
    let mut shell = Shell::new();
    let r = shell.exec("read <<< 'hello'; echo $REPLY").unwrap();
    assert_eq!(r.stdout, "hello\n");
}

#[test]
fn read_split_vars() {
    let mut shell = Shell::new();
    let r = shell.exec("read x y z <<< 'a b c'; echo $y").unwrap();
    assert_eq!(r.stdout, "b\n");
}

#[test]
fn trap_on_exit() {
    let mut shell = Shell::new();
    let r = shell.exec("trap 'echo trapped' EXIT; echo main").unwrap();
    assert!(r.stdout.contains("main"));
    assert!(r.stdout.contains("trapped"));
}

#[test]
fn getopts_loop() {
    let mut shell = Shell::new();
    let r = shell.exec("set -- -a -b -c; while getopts 'abc' opt; do echo $opt; done").unwrap();
    assert_eq!(r.stdout, "a\nb\nc\n");
}

#[test]
fn declare_print_all() {
    let mut shell = Shell::new();
    let r = shell.exec("MY_VAR=hello; declare -p").unwrap();
    assert!(r.stdout.contains("MY_VAR"));
}

#[test]
fn declare_export() {
    let mut shell = Shell::new();
    let r = shell.exec("declare -x EXPORTED=value; printenv EXPORTED").unwrap();
    assert_eq!(r.stdout, "value\n");
}

#[test]
fn declare_array() {
    let mut shell = Shell::new();
    let r = shell.exec("declare -a arr; arr[0]=x; arr[1]=y; echo ${arr[1]}").unwrap();
    assert_eq!(r.stdout, "y\n");
}

#[test]
fn set_nounset() {
    let mut shell = Shell::new();
    let r = shell.exec("set -u; echo $UNBOUND_VAR_XYZ 2>&1");
    let err = r.unwrap_err();
    assert!(matches!(err, Error::Exec(ExecError::UnboundVariable(_))));
}

#[test]
fn set_positional_params() {
    let mut shell = Shell::new();
    let r = shell.exec("set -- x y z; echo $2").unwrap();
    assert_eq!(r.stdout, "y\n");
}

#[test]
fn printf_string_format() {
    let mut shell = Shell::new();
    let r = shell.exec("printf '%s=%s\\n' key value").unwrap();
    assert_eq!(r.stdout, "key=value\n");
}

#[test]
fn printf_padding() {
    let mut shell = Shell::new();
    let r = shell.exec("printf '%05d\\n' 42").unwrap();
    assert_eq!(r.stdout, "00042\n");
}
