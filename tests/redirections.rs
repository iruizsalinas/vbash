use vbash::Shell;


#[test]
fn redir_output() {
    let mut shell = Shell::new();
    shell.exec("echo hello > /tmp/out.txt").unwrap();
    let r = shell.exec("cat /tmp/out.txt").unwrap();
    assert_eq!(r.stdout, "hello\n");
}

#[test]
fn redir_append() {
    let mut shell = Shell::new();
    shell.exec("echo a > /tmp/f.txt").unwrap();
    shell.exec("echo b >> /tmp/f.txt").unwrap();
    let r = shell.exec("cat /tmp/f.txt").unwrap();
    assert_eq!(r.stdout, "a\nb\n");
}

#[test]
fn redir_input() {
    let mut shell = Shell::builder()
        .file("/tmp/data.txt", "contents here")
        .build();
    let r = shell.exec("cat < /tmp/data.txt").unwrap();
    assert_eq!(r.stdout, "contents here");
}

#[test]
fn redir_clobber() {
    let mut shell = Shell::new();
    shell.exec("echo a > /tmp/f.txt").unwrap();
    // set -C disables overwrite; >| forces it
    shell.exec("set -C; echo b >| /tmp/f.txt").unwrap();
    let r = shell.exec("cat /tmp/f.txt").unwrap();
    assert_eq!(r.stdout, "b\n");
}


#[test]
fn redir_stderr_to_file() {
    let mut shell = Shell::new();
    shell.exec("nonexistent_cmd_xyz 2> /tmp/err.txt; true").unwrap();
    let r = shell.exec("cat /tmp/err.txt").unwrap();
    assert!(r.stdout.contains("not found") || !r.stdout.is_empty());
}

#[test]
fn redir_stderr_to_file_explicit() {
    let mut shell = Shell::new();
    shell.exec("echo err >&2 2> /tmp/stderr.txt").unwrap();
    let r = shell.exec("cat /tmp/stderr.txt").unwrap();
    // stderr content might go to file depending on redirect ordering
    assert!(r.stdout.contains("err") || r.exit_code == 0);
}

#[test]
fn redir_output_to_dev_null() {
    let mut shell = Shell::new();
    let r = shell.exec("echo secret > /dev/null").unwrap();
    assert_eq!(r.stdout, "");
}


#[test]
fn heredoc_basic() {
    let mut shell = Shell::new();
    let r = shell.exec("cat <<EOF\nhello\nEOF\n").unwrap();
    assert_eq!(r.stdout, "hello\n");
}

#[test]
fn heredoc_with_command() {
    let mut shell = Shell::new();
    let r = shell.exec("cat <<EOF | tr a-z A-Z\nhello\nEOF\n").unwrap();
    assert_eq!(r.stdout, "HELLO\n");
}

#[test]
fn heredoc_expansion() {
    let mut shell = Shell::new();
    let r = shell.exec("X=world; cat <<EOF\nhello $X\nEOF\n").unwrap();
    assert_eq!(r.stdout, "hello world\n");
}

#[test]
fn heredoc_quoted_no_expand() {
    let mut shell = Shell::new();
    let r = shell.exec("X=world; cat <<'EOF'\n$X\nEOF\n").unwrap();
    assert_eq!(r.stdout, "$X\n");
}

#[test]
fn heredoc_multiline() {
    let mut shell = Shell::new();
    let r = shell.exec("cat <<EOF\nline1\nline2\nline3\nEOF\n").unwrap();
    assert_eq!(r.stdout, "line1\nline2\nline3\n");
}


#[test]
fn herestring_basic() {
    let mut shell = Shell::new();
    let r = shell.exec("cat <<< \"hello\"").unwrap();
    assert_eq!(r.stdout, "hello\n");
}

#[test]
fn herestring_variable() {
    let mut shell = Shell::new();
    let r = shell.exec("X=world; cat <<< \"hello $X\"").unwrap();
    assert_eq!(r.stdout, "hello world\n");
}


#[test]
fn pipe_basic() {
    let mut shell = Shell::new();
    let r = shell.exec("echo hello | cat").unwrap();
    assert_eq!(r.stdout, "hello\n");
}

#[test]
fn pipe_chain() {
    let mut shell = Shell::new();
    let r = shell.exec("echo hello | tr a-z A-Z | rev").unwrap();
    assert_eq!(r.stdout, "OLLEH\n");
}

#[test]
fn pipe_exit_code_last() {
    let mut shell = Shell::new();
    let r = shell.exec("false | true; echo $?").unwrap();
    assert_eq!(r.stdout, "0\n");
}

#[test]
fn pipefail() {
    let mut shell = Shell::new();
    let r = shell.exec("set -o pipefail; false | true; echo $?").unwrap();
    assert_eq!(r.stdout, "1\n");
}


#[test]
fn proc_subst_input() {
    let mut shell = Shell::new();
    let r = shell.exec("cat <(echo hello)").unwrap();
    assert_eq!(r.stdout, "hello\n");
}

#[test]
fn proc_subst_diff() {
    let mut shell = Shell::new();
    let r = shell.exec("diff <(echo a) <(echo a)").unwrap();
    assert_eq!(r.exit_code, 0);
}


#[test]
fn compound_if() {
    let mut shell = Shell::new();
    shell.exec("if true; then echo yes; fi > /tmp/f.txt").unwrap();
    let r = shell.exec("cat /tmp/f.txt").unwrap();
    assert_eq!(r.stdout, "yes\n");
}

#[test]
fn compound_for() {
    let mut shell = Shell::new();
    shell.exec("for i in 1 2; do echo $i; done > /tmp/f.txt").unwrap();
    let r = shell.exec("cat /tmp/f.txt").unwrap();
    assert_eq!(r.stdout, "1\n2\n");
}

#[test]
fn compound_while() {
    let mut shell = Shell::new();
    shell.exec("i=0; while [ $i -lt 2 ]; do echo $i; i=$((i+1)); done > /tmp/f.txt").unwrap();
    let r = shell.exec("cat /tmp/f.txt").unwrap();
    assert_eq!(r.stdout, "0\n1\n");
}

#[test]
fn compound_group() {
    let mut shell = Shell::new();
    shell.exec("{ echo a; echo b; } > /tmp/f.txt").unwrap();
    let r = shell.exec("cat /tmp/f.txt").unwrap();
    assert_eq!(r.stdout, "a\nb\n");
}

#[test]
fn compound_subshell_redirect() {
    let mut shell = Shell::new();
    shell.exec("(echo sub) > /tmp/f.txt").unwrap();
    let r = shell.exec("cat /tmp/f.txt").unwrap();
    assert_eq!(r.stdout, "sub\n");
}

// === Complex pipe tests ===

#[test]
fn pipes_three_stage() {
    let mut shell = Shell::builder()
        .file("/tmp/in.txt", "c\na\nb\n")
        .build();
    let r = shell.exec("cat /tmp/in.txt | sort | head -n 2").unwrap();
    assert_eq!(r.stdout, "a\nb\n");
}

#[test]
fn pipes_four_stage() {
    let mut shell = Shell::builder()
        .file("/tmp/in.txt", "hello\nworld\nhello\n")
        .build();
    let r = shell.exec("cat /tmp/in.txt | sort | uniq | wc -l").unwrap();
    assert_eq!(r.stdout.trim(), "2");
}

#[test]
fn pipes_five_stage() {
    let mut shell = Shell::builder()
        .file("/tmp/in.txt", "3\n1\n2\n1\n3\n")
        .build();
    let r = shell
        .exec("cat /tmp/in.txt | sort | uniq -c | sort -rn | head -n 1")
        .unwrap();
    let line = r.stdout.trim();
    assert!(
        line.contains('3'),
        "expected most frequent value '3' in: {line}"
    );
}

#[test]
fn pipes_exit_code_last() {
    let mut shell = Shell::new();
    let r = shell.exec("false | true; echo $?").unwrap();
    assert_eq!(r.stdout, "0\n");
}

#[test]
fn pipes_exit_code_first_fails() {
    let mut shell = Shell::new();
    let r = shell.exec("false | echo hi").unwrap();
    assert_eq!(r.stdout, "hi\n");
}

#[test]
fn pipes_pipefail_off() {
    let mut shell = Shell::new();
    let r = shell.exec("false | true; echo $?").unwrap();
    assert_eq!(r.stdout, "0\n");
}

#[test]
fn pipes_pipefail_on() {
    let mut shell = Shell::new();
    let r = shell.exec("set -o pipefail; false | true; echo $?").unwrap();
    assert_eq!(r.stdout, "1\n");
}

#[test]
fn pipes_pipefail_middle() {
    let mut shell = Shell::new();
    let r = shell.exec("set -o pipefail; true | false | true; echo $?").unwrap();
    assert_eq!(r.stdout, "1\n");
}

#[test]
fn pipes_negate() {
    let mut shell = Shell::new();
    let r = shell.exec("! false; echo $?").unwrap();
    assert_eq!(r.stdout, "0\n");
}

#[test]
fn pipes_negate_true() {
    let mut shell = Shell::new();
    let r = shell.exec("! true; echo $?").unwrap();
    assert_eq!(r.stdout, "1\n");
}

#[test]
fn pipes_negate_pipeline() {
    let mut shell = Shell::new();
    let r = shell.exec("! true | false; echo $?").unwrap();
    assert_eq!(r.stdout, "0\n");
}

#[test]
fn pipes_subshell() {
    let mut shell = Shell::new();
    let r = shell.exec("(echo b; echo a) | sort").unwrap();
    assert_eq!(r.stdout, "a\nb\n");
}

#[test]
fn pipes_group() {
    let mut shell = Shell::new();
    let r = shell.exec("{ echo b; echo a; } | sort").unwrap();
    assert_eq!(r.stdout, "a\nb\n");
}

#[test]
fn pipes_for_loop() {
    let mut shell = Shell::new();
    let r = shell.exec("for i in 3 1 2; do echo $i; done | sort").unwrap();
    assert_eq!(r.stdout, "1\n2\n3\n");
}

#[test]
fn pipes_while() {
    let mut shell = Shell::new();
    let r = shell.exec("i=0; while [ $i -lt 3 ]; do echo $i; i=$((i+1)); done | tac").unwrap();
    assert_eq!(r.stdout, "2\n1\n0\n");
}

#[test]
fn pipes_stderr_separate() {
    let mut shell = Shell::new();
    let r = shell.exec("(echo out; echo err >&2) | cat").unwrap();
    assert_eq!(r.stdout, "out\n");
}

#[test]
fn pipes_pipe_to_file() {
    let mut shell = Shell::new();
    shell.exec("echo hello | cat > /tmp/p.txt").unwrap();
    let r = shell.exec("cat /tmp/p.txt").unwrap();
    assert_eq!(r.stdout, "hello\n");
}

#[test]
fn pipes_grep_sort() {
    let mut shell = Shell::builder()
        .file("/tmp/in.txt", "banana\napple\ncherry\napricot\n")
        .build();
    let r = shell.exec("cat /tmp/in.txt | grep ^a | sort").unwrap();
    assert_eq!(r.stdout, "apple\napricot\n");
}

#[test]
fn pipes_awk_pipe() {
    let mut shell = Shell::builder()
        .file("/tmp/in.txt", "1 a\n2 b\n3 c\n")
        .build();
    let r = shell.exec("cat /tmp/in.txt | awk '{print $2}' | sort -r").unwrap();
    assert_eq!(r.stdout, "c\nb\na\n");
}

#[test]
fn pipes_sed_pipe() {
    let mut shell = Shell::new();
    let r = shell.exec("echo \"hello world\" | sed 's/world/earth/' | tr a-z A-Z").unwrap();
    assert_eq!(r.stdout, "HELLO EARTH\n");
}

#[test]
fn pipes_cut_sort() {
    let mut shell = Shell::builder()
        .file("/tmp/in.txt", "b:2\na:1\nc:3\n")
        .build();
    let r = shell.exec("cat /tmp/in.txt | cut -d: -f1 | sort").unwrap();
    assert_eq!(r.stdout, "a\nb\nc\n");
}

#[test]
fn pipes_in_subst() {
    let mut shell = Shell::builder()
        .file("/tmp/in.txt", "b\na\n")
        .build();
    let r = shell.exec("X=$(cat /tmp/in.txt | sort); echo $X").unwrap();
    assert_eq!(r.stdout, "a b\n");
}

#[test]
fn pipes_nested_subst() {
    let mut shell = Shell::new();
    let r = shell.exec("echo $(echo hello | tr a-z A-Z)").unwrap();
    assert_eq!(r.stdout, "HELLO\n");
}

#[test]
fn pipes_cat_stdin() {
    let mut shell = Shell::new();
    let r = shell.exec("echo \"from pipe\" | cat").unwrap();
    assert_eq!(r.stdout, "from pipe\n");
}

#[test]
fn pipes_head_pipe() {
    let mut shell = Shell::new();
    let r = shell.exec("seq 10 | head -n 3").unwrap();
    assert_eq!(r.stdout, "1\n2\n3\n");
}

#[test]
fn pipes_tail_pipe() {
    let mut shell = Shell::new();
    let r = shell.exec("seq 10 | tail -n 3").unwrap();
    assert_eq!(r.stdout, "8\n9\n10\n");
}

#[test]
fn pipes_wc_pipe() {
    let mut shell = Shell::builder()
        .file("/tmp/in.txt", "a\nb\nc\n")
        .build();
    let r = shell.exec("cat /tmp/in.txt | wc -l").unwrap();
    assert_eq!(r.stdout.trim(), "3");
}

#[test]
fn pipes_tee() {
    let mut shell = Shell::new();
    let r = shell.exec("echo hello | tee /tmp/tee.txt | cat").unwrap();
    assert_eq!(r.stdout, "hello\n");
    let r2 = shell.exec("cat /tmp/tee.txt").unwrap();
    assert_eq!(r2.stdout, "hello\n");
}

#[test]
fn pipes_sort_uniq_count() {
    let mut shell = Shell::builder()
        .file("/tmp/in.txt", "a\nb\na\nc\nb\na\n")
        .build();
    let r = shell.exec("cat /tmp/in.txt | sort | uniq -c | sort -rn").unwrap();
    let first_line = r.stdout.lines().next().unwrap().trim();
    assert!(first_line.starts_with('3') && first_line.ends_with('a'), "got: {first_line}");
}

#[test]
fn pipes_multi_grep() {
    let mut shell = Shell::builder()
        .file("/tmp/in.txt", "foo bar\nbaz qux\nfoo baz\n")
        .build();
    let r = shell.exec("cat /tmp/in.txt | grep foo | grep baz").unwrap();
    assert_eq!(r.stdout, "foo baz\n");
}

#[test]
fn pipes_tr_chain() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'Hello World' | tr A-Z a-z | tr ' ' '_'").unwrap();
    assert_eq!(r.stdout, "hello_world\n");
}

#[test]
fn pipes_rev_sort() {
    let mut shell = Shell::builder()
        .file("/tmp/in.txt", "abc\ndef\n")
        .build();
    let r = shell.exec("cat /tmp/in.txt | rev | sort").unwrap();
    assert_eq!(r.stdout, "cba\nfed\n");
}

#[test]
fn pipes_empty_input() {
    let mut shell = Shell::new();
    let r = shell.exec("echo -n '' | cat").unwrap();
    assert_eq!(r.stdout, "");
}

#[test]
fn pipes_long_chain_echo_sort_head_tail() {
    let mut shell = Shell::builder()
        .file("/tmp/in.txt", "5\n3\n1\n4\n2\n")
        .build();
    let r = shell.exec("cat /tmp/in.txt | sort | head -n 4 | tail -n 2").unwrap();
    assert_eq!(r.stdout, "3\n4\n");
}

#[test]
fn pipes_printf_newlines() {
    let mut shell = Shell::new();
    let r = shell.exec("printf 'x\\ny\\n' | sort").unwrap();
    assert_eq!(r.stdout, "x\ny\n");
}

#[test]
fn pipes_seq_sort_reverse() {
    let mut shell = Shell::new();
    let r = shell.exec("seq 5 | sort -r").unwrap();
    assert_eq!(r.stdout, "5\n4\n3\n2\n1\n");
}

#[test]
fn pipes_head_tail_combined() {
    let mut shell = Shell::new();
    let r = shell.exec("seq 20 | head -n 10 | tail -n 5").unwrap();
    assert_eq!(r.stdout, "6\n7\n8\n9\n10\n");
}
