use vbash::Shell;
use vbash::error::Error;

#[test]
fn var_simple() {
    let mut shell = Shell::new();
    let r = shell.exec("X=hello; echo $X").unwrap();
    assert_eq!(r.stdout, "hello\n");
}

#[test]
fn var_braced() {
    let mut shell = Shell::new();
    let r = shell.exec("X=world; echo ${X}").unwrap();
    assert_eq!(r.stdout, "world\n");
}

#[test]
fn var_adjacent() {
    let mut shell = Shell::new();
    let r = shell.exec("X=he; echo ${X}llo").unwrap();
    assert_eq!(r.stdout, "hello\n");
}

#[test]
fn var_unset_empty() {
    let mut shell = Shell::new();
    let r = shell.exec("echo \"${UNSET}\"").unwrap();
    assert_eq!(r.stdout, "\n");
}


#[test]
fn var_default_colon() {
    let mut shell = Shell::new();
    let r = shell.exec("echo ${UNSET:-fallback}").unwrap();
    assert_eq!(r.stdout, "fallback\n");
}

#[test]
fn var_default_no_colon() {
    let mut shell = Shell::new();
    let r = shell.exec("X=''; echo ${X-fallback}").unwrap();
    // X is set (to empty), so -fallback should NOT be used
    assert_eq!(r.stdout, "\n");
}

#[test]
fn var_assign_default() {
    let mut shell = Shell::new();
    let r = shell.exec("echo ${NEW:=assigned}; echo $NEW").unwrap();
    assert_eq!(r.stdout, "assigned\nassigned\n");
}

#[test]
fn var_error_unset() {
    let mut shell = Shell::new();
    let r = shell.exec("set -u; echo $NOPE 2>&1");
    let err = r.unwrap_err();
    assert!(matches!(err, Error::Exec(vbash::error::ExecError::UnboundVariable(_))));
}

#[test]
fn var_alternative() {
    let mut shell = Shell::new();
    let r = shell.exec("X=yes; echo ${X:+alt}").unwrap();
    assert_eq!(r.stdout, "alt\n");
}

#[test]
fn var_alternative_unset() {
    let mut shell = Shell::new();
    let r = shell.exec("echo \"${UNSET:+alt}\"").unwrap();
    assert_eq!(r.stdout, "\n");
}


#[test]
fn var_length_string() {
    let mut shell = Shell::new();
    let r = shell.exec("X=hello; echo ${#X}").unwrap();
    assert_eq!(r.stdout, "5\n");
}

#[test]
fn var_substring() {
    let mut shell = Shell::new();
    let r = shell.exec("X=hello; echo ${X:1:3}").unwrap();
    assert_eq!(r.stdout, "ell\n");
}

#[test]
fn var_substring_from_offset() {
    let mut shell = Shell::new();
    let r = shell.exec("X=hello; echo ${X:2}").unwrap();
    assert_eq!(r.stdout, "llo\n");
}


#[test]
fn var_prefix_short() {
    let mut shell = Shell::new();
    let r = shell.exec("X=hello; echo ${X#hel}").unwrap();
    assert_eq!(r.stdout, "lo\n");
}

#[test]
fn var_prefix_long() {
    let mut shell = Shell::new();
    let r = shell.exec("X=/a/b/c; echo ${X##*/}").unwrap();
    assert_eq!(r.stdout, "c\n");
}

#[test]
fn var_suffix_short() {
    let mut shell = Shell::new();
    let r = shell.exec("X=hello.txt; echo ${X%.txt}").unwrap();
    assert_eq!(r.stdout, "hello\n");
}

#[test]
fn var_suffix_long() {
    let mut shell = Shell::new();
    // %% is greedy: the pattern /* matches "/a/b/c" (everything from the first /),
    // leaving the empty prefix before the initial /.
    let r = shell.exec("X=/a/b/c; echo ${X%%/*}").unwrap();
    assert_eq!(r.stdout, "\n");
}


#[test]
fn var_replace_first() {
    let mut shell = Shell::new();
    let r = shell.exec("X=aabaa; echo ${X/a/x}").unwrap();
    assert_eq!(r.stdout, "xabaa\n");
}

#[test]
fn var_replace_all() {
    let mut shell = Shell::new();
    let r = shell.exec("X=aabaa; echo ${X//a/x}").unwrap();
    assert_eq!(r.stdout, "xxbxx\n");
}


#[test]
fn var_upper_first() {
    let mut shell = Shell::new();
    let r = shell.exec("X=hello; echo ${X^}").unwrap();
    assert_eq!(r.stdout, "Hello\n");
}

#[test]
fn var_upper_all() {
    let mut shell = Shell::new();
    let r = shell.exec("X=hello; echo ${X^^}").unwrap();
    assert_eq!(r.stdout, "HELLO\n");
}

#[test]
fn var_lower_first() {
    let mut shell = Shell::new();
    let r = shell.exec("X=HELLO; echo ${X,}").unwrap();
    assert_eq!(r.stdout, "hELLO\n");
}

#[test]
fn var_lower_all() {
    let mut shell = Shell::new();
    let r = shell.exec("X=HELLO; echo ${X,,}").unwrap();
    assert_eq!(r.stdout, "hello\n");
}


#[test]
fn var_indirect() {
    let mut shell = Shell::new();
    let r = shell.exec("X=hello; Y=X; echo ${!Y}").unwrap();
    assert_eq!(r.stdout, "hello\n");
}


#[test]
fn var_exit_code() {
    let mut shell = Shell::new();
    let r = shell.exec("false; echo $?").unwrap();
    assert_eq!(r.stdout, "1\n");
}

#[test]
fn var_pid() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $$").unwrap();
    let pid = r.stdout.trim();
    assert!(pid.parse::<u64>().is_ok(), "expected numeric PID, got: {pid}");
}

#[test]
fn var_param_count() {
    let mut shell = Shell::new();
    let r = shell.exec("set -- a b c; echo $#").unwrap();
    assert_eq!(r.stdout, "3\n");
}

#[test]
fn var_at_expansion() {
    let mut shell = Shell::new();
    let r = shell.exec("set -- x y z; echo $@").unwrap();
    assert_eq!(r.stdout, "x y z\n");
}

#[test]
fn var_star_expansion() {
    let mut shell = Shell::new();
    let r = shell.exec("set -- x y z; echo $*").unwrap();
    assert_eq!(r.stdout, "x y z\n");
}

// === Expansion edge case tests ===

macro_rules! t {
    ($name:ident, $cmd:expr, $out:expr) => {
        #[test]
        fn $name() {
            let mut sh = Shell::new();
            let r = sh.exec($cmd).unwrap();
            assert_eq!(r.stdout, $out, "stderr={:?}", r.stderr);
        }
    };
}

// Default with colon distinction
t!(exp_default_unset, "echo ${UNSET:-fallback}", "fallback\n");
t!(exp_default_empty, "X=''; echo ${X:-fallback}", "fallback\n");
t!(exp_default_no_colon_empty, "X=''; echo ${X-fallback}", "\n");
t!(exp_default_no_colon_unset, "echo ${UNSET-fallback}", "fallback\n");
t!(exp_default_empty_default, "echo ${UNSET:-}", "\n");

// Assign default
t!(exp_assign_default, "echo ${NEW:=hello}; echo $NEW", "hello\nhello\n");
t!(exp_assign_already_set, "X=old; echo ${X:=new}", "old\n");

// Error on unset
#[test]
fn exp_error_message() {
    let mut sh = Shell::new();
    let r = sh.exec("(echo ${UNSET:?custom error}) 2>&1");
    match r {
        Ok(res) => assert!(
            res.stdout.contains("custom error") || res.stderr.contains("custom error"),
            "expected 'custom error' in output. stdout={:?} stderr={:?}", res.stdout, res.stderr
        ),
        Err(e) => assert!(
            format!("{e:?}").contains("custom error"),
            "expected 'custom error' in error: {e:?}"
        ),
    }
}

// Alternative
t!(exp_alt_set, "X=yes; echo ${X:+replaced}", "replaced\n");
t!(exp_alt_unset, "echo ${UNSET:+replaced}", "\n");
t!(exp_alt_empty, "X=''; echo ${X:+replaced}", "\n");
t!(exp_alt_no_colon_empty, "X=''; echo ${X+replaced}", "replaced\n");

// Length
t!(exp_length_string, "X=hello; echo ${#X}", "5\n");
t!(exp_length_empty, "X=''; echo ${#X}", "0\n");
t!(exp_length_array, "arr=(a b c d); echo ${#arr[@]}", "4\n");

// Substring
t!(exp_substr_from_start, "X=hello; echo ${X:0:3}", "hel\n");
t!(exp_substr_middle, "X=hello; echo ${X:2:2}", "ll\n");
t!(exp_substr_to_end, "X=hello; echo ${X:2}", "llo\n");
#[test]
fn exp_substr_negative() {
    let mut sh = Shell::new();
    let r = sh.exec("X=hello; echo ${X: -3}").unwrap();
    assert_eq!(r.stdout, "llo\n");
}
t!(exp_substr_beyond, "X=hi; echo ${X:0:100}", "hi\n");

// Pattern removal
t!(exp_prefix_short, "X=/usr/local/bin; echo ${X#*/}", "usr/local/bin\n");
t!(exp_prefix_long, "X=/usr/local/bin; echo ${X##*/}", "bin\n");
t!(exp_suffix_short, "X=file.tar.gz; echo ${X%.*}", "file.tar\n");
t!(exp_suffix_long, "X=file.tar.gz; echo ${X%%.*}", "file\n");
t!(exp_pattern_no_match, "X=hello; echo ${X#xyz}", "hello\n");

// Replacement
t!(exp_replace_first, "X=banana; echo ${X/a/o}", "bonana\n");
t!(exp_replace_all, "X=banana; echo ${X//a/o}", "bonono\n");
t!(exp_replace_delete, "X=hello; echo ${X//l/}", "heo\n");
t!(exp_replace_anchor_start, "X=hello; echo ${X/#hel/HEL}", "HELlo\n");
t!(exp_replace_anchor_end, "X=hello; echo ${X/%llo/LLO}", "heLLO\n");

// Case modification
t!(exp_case_upper_first, "X=hello; echo ${X^}", "Hello\n");
t!(exp_case_upper_all, "X=hello; echo ${X^^}", "HELLO\n");
t!(exp_case_lower_first, "X=HELLO; echo ${X,}", "hELLO\n");
t!(exp_case_lower_all, "X=HELLO; echo ${X,,}", "hello\n");

// Indirection
t!(exp_indirect, "X=hello; Y=X; echo ${!Y}", "hello\n");

// Special variables
t!(exp_question_mark, "true; echo $?", "0\n");
t!(exp_question_after_false, "false; echo $?", "1\n");

#[test]
fn exp_dollar_dollar() {
    let mut sh = Shell::new();
    let r = sh.exec("echo $$").unwrap();
    let pid = r.stdout.trim();
    assert!(pid.parse::<u64>().is_ok(), "expected numeric PID, got {pid:?}");
}

t!(exp_at_vs_star, "set -- \"a b\" c; for x in \"$@\"; do echo \"[$x]\"; done", "[a b]\n[c]\n");
t!(exp_star_joined, "set -- a b c; echo \"$*\"", "a b c\n");
t!(exp_hash_count, "set -- x y z; echo $#", "3\n");

// IFS
t!(exp_ifs_custom, "IFS=:; X=\"a:b:c\"; for x in $X; do echo $x; done", "a\nb\nc\n");
t!(exp_ifs_default, "X=\"a  b  c\"; for x in $X; do echo $x; done", "a\nb\nc\n");

// Array operations
t!(exp_array_basic, "arr=(one two three); echo ${arr[1]}", "two\n");
t!(exp_array_all, "arr=(one two three); echo ${arr[@]}", "one two three\n");
t!(exp_array_star, "arr=(one two three); echo ${arr[*]}", "one two three\n");
t!(exp_array_append, "arr=(a b); arr+=(c); echo ${arr[@]}", "a b c\n");
t!(exp_array_length_element, "arr=(hello world); echo ${#arr[0]}", "5\n");

// Nested expansions
t!(exp_nested_default_in_string, "echo \"${UNSET:-hello} world\"", "hello world\n");
t!(exp_nested_arith_in_default, "echo ${UNSET:-$((2+3))}", "5\n");

// Empty and whitespace edge cases
t!(exp_empty_var_in_string, "X=''; echo \"a${X}b\"", "ab\n");
t!(exp_unset_in_string, "echo \"a${UNSET}b\"", "ab\n");

// Tilde expansion
#[test]
fn exp_tilde_home() {
    let mut sh = Shell::new();
    let r = sh.exec("echo ~").unwrap();
    assert_eq!(r.stdout, "/home/user\n");
}

// Arithmetic in expansion context
t!(exp_arith_vars, "X=10; Y=3; echo $((X+Y))", "13\n");
t!(exp_arith_modulo, "echo $((17%5))", "2\n");
#[test]
fn exp_arith_shift() {
    let mut sh = Shell::new();
    let r = sh.exec("echo $((1<<4))").unwrap();
    assert_eq!(r.stdout, "16\n");
}
t!(exp_arith_ternary, "echo $(( 5 > 3 ? 1 : 0 ))", "1\n");
t!(exp_arith_increment, "X=5; echo $((X++))", "5\n");
t!(exp_arith_pre_increment, "X=5; echo $((++X))", "6\n");
t!(exp_arith_compound, "echo $(( (2+3) * 4 ))", "20\n");
t!(exp_arith_negative, "echo $(( -5 + 3 ))", "-2\n");
#[test]
fn exp_arith_bitwise_and() {
    let mut sh = Shell::new();
    let r = sh.exec("echo $((12 & 10))").unwrap();
    assert_eq!(r.stdout, "8\n");
}

#[test]
fn exp_arith_bitwise_or() {
    let mut sh = Shell::new();
    let r = sh.exec("echo $((12 | 3))").unwrap();
    assert_eq!(r.stdout, "15\n");
}
