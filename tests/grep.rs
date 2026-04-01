use vbash::Shell;

#[test]
fn grep_match() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "foo\nbar\nbaz\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | grep bar").unwrap();
    assert_eq!(r.stdout, "bar\n");
    assert_eq!(r.exit_code, 0);
}

#[test]
fn grep_no_match() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'hello' | grep xyz").unwrap();
    assert_eq!(r.stdout, "");
    assert_eq!(r.exit_code, 1);
}

#[test]
fn grep_ignore_case() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'Hello' | grep -i hello").unwrap();
    assert_eq!(r.stdout, "Hello\n");
}

#[test]
fn grep_invert() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "foo\nbar\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | grep -v foo").unwrap();
    assert_eq!(r.stdout, "bar\n");
}

#[test]
fn grep_count() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "a\nb\na\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | grep -c a").unwrap();
    assert_eq!(r.stdout, "2\n");
}

#[test]
fn grep_line_number() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "foo\nbar\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | grep -n bar").unwrap();
    assert_eq!(r.stdout, "2:bar\n");
}

#[test]
fn grep_only_matching() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'abc123def' | grep -o '[0-9]+'").unwrap();
    assert_eq!(r.stdout, "123\n");
}

#[test]
fn grep_word_regexp() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "cat\ncatch\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | grep -w cat").unwrap();
    assert_eq!(r.stdout, "cat\n");
}

#[test]
fn grep_fixed_string() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'a.b' | grep -F 'a.b'").unwrap();
    assert_eq!(r.stdout, "a.b\n");
}

#[test]
fn grep_fixed_string_no_regex() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'axb' | grep -F 'a.b'").unwrap();
    assert_eq!(r.stdout, "");
    assert_eq!(r.exit_code, 1);
}

#[test]
fn grep_recursive() {
    let mut shell = Shell::builder()
        .file("/data/dir1/file1.txt", "hello world\n")
        .file("/data/dir1/file2.txt", "goodbye world\n")
        .file("/data/dir2/file3.txt", "hello again\n")
        .build();
    let r = shell.exec("grep -r hello /data").unwrap();
    assert_eq!(r.exit_code, 0);
    let mut lines: Vec<&str> = r.stdout.trim().lines().collect();
    lines.sort_unstable();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "/data/dir1/file1.txt:hello world");
    assert_eq!(lines[1], "/data/dir2/file3.txt:hello again");
}

#[test]
fn grep_files_with_matches() {
    let mut shell = Shell::builder()
        .file("/data/a.txt", "hello\n")
        .file("/data/b.txt", "world\n")
        .build();
    let r = shell.exec("grep -l hello /data/a.txt /data/b.txt").unwrap();
    assert_eq!(r.stdout, "/data/a.txt\n");
}

#[test]
fn grep_files_without_match() {
    let mut shell = Shell::builder()
        .file("/data/a.txt", "hello\n")
        .file("/data/b.txt", "world\n")
        .build();
    let r = shell.exec("grep -L hello /data/a.txt /data/b.txt").unwrap();
    assert_eq!(r.stdout, "/data/b.txt\n");
}

#[test]
fn grep_quiet() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "hello\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | grep -q hello").unwrap();
    assert_eq!(r.stdout, "");
    assert_eq!(r.exit_code, 0);
}

#[test]
fn grep_quiet_no_match() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'hello' | grep -q xyz").unwrap();
    assert_eq!(r.stdout, "");
    assert_eq!(r.exit_code, 1);
}

#[test]
fn grep_max_count() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "a\na\na\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | grep -m 1 a").unwrap();
    assert_eq!(r.stdout, "a\n");
}

#[test]
fn grep_context_after() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "aaa\nBBB\nccc\nddd\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | grep -A 1 BBB").unwrap();
    assert_eq!(r.stdout, "BBB\nccc\n");
}

#[test]
fn grep_context_before() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "aaa\nBBB\nccc\nddd\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | grep -B 1 BBB").unwrap();
    assert_eq!(r.stdout, "aaa\nBBB\n");
}

#[test]
fn grep_context_overlap() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "aaa\nBBB\nccc\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | grep -A 1 -B 1 BBB").unwrap();
    assert_eq!(r.stdout, "aaa\nBBB\nccc\n");
}

#[test]
fn grep_include() {
    let mut shell = Shell::builder()
        .file("/data/file.txt", "findme\n")
        .file("/data/file.log", "findme\n")
        .build();
    let r = shell.exec("grep -r --include='*.txt' findme /data").unwrap();
    assert_eq!(r.stdout, "/data/file.txt:findme\n");
}

#[test]
fn grep_exclude() {
    let mut shell = Shell::builder()
        .file("/data/file.txt", "findme\n")
        .file("/data/file.log", "findme\n")
        .build();
    let r = shell.exec("grep -r --exclude='*.log' findme /data").unwrap();
    assert_eq!(r.stdout, "/data/file.txt:findme\n");
}

#[test]
fn egrep_basic() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'aab' | egrep 'a+b'").unwrap();
    assert_eq!(r.stdout, "aab\n");
}

#[test]
fn fgrep_basic() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'a+b' | fgrep 'a+b'").unwrap();
    assert_eq!(r.stdout, "a+b\n");
}

#[test]
fn fgrep_no_regex() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'aab' | fgrep 'a+b'").unwrap();
    assert_eq!(r.stdout, "");
    assert_eq!(r.exit_code, 1);
}

#[test]
fn grep_multiple_e_last_wins() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "apple\nbanana\ncherry\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | grep -e apple -e cherry").unwrap();
    assert_eq!(r.stdout, "cherry\n");
    assert_eq!(r.exit_code, 0);
}

#[test]
fn grep_regex_anchors() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "hello world\nworld hello\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | grep '^hello'").unwrap();
    assert_eq!(r.stdout, "hello world\n");
}

#[test]
fn grep_regex_end_anchor() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "hello world\nworld hello\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | grep 'hello$'").unwrap();
    assert_eq!(r.stdout, "world hello\n");
}

#[test]
fn grep_empty_input() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '' | grep foo").unwrap();
    assert_eq!(r.stdout, "");
    assert_eq!(r.exit_code, 1);
}

#[test]
fn grep_no_pattern_error() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'hello' | grep").unwrap();
    assert_eq!(r.exit_code, 2);
    assert_eq!(r.stderr, "grep: no pattern\n");
}

// === Edge case tests ===

#[test]
fn grep_first_line() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "match\nno\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | grep match").unwrap();
    assert_eq!(r.stdout, "match\n");
}

#[test]
fn grep_last_line() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "no\nmatch\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | grep match").unwrap();
    assert_eq!(r.stdout, "match\n");
}

#[test]
fn grep_empty_line() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "a\n\nb\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | grep -c ''").unwrap();
    assert_eq!(r.stdout, "3\n");
}

#[test]
fn grep_all_lines() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "a\nb\nc\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | grep '.'").unwrap();
    assert_eq!(r.stdout, "a\nb\nc\n");
}

#[test]
fn grep_no_output_on_fail() {
    let mut sh = Shell::new();
    let r = sh.exec("echo hello | grep xyz").unwrap();
    assert_eq!(r.stdout, "");
    assert_eq!(r.exit_code, 1);
}

#[test]
fn grep_case_preserves() {
    let mut sh = Shell::new();
    let r = sh.exec("echo Hello | grep -i hello").unwrap();
    assert_eq!(r.stdout, "Hello\n");
}

#[test]
fn grep_multi_file_prefix() {
    let mut sh = Shell::builder()
        .file("/tmp/f1.txt", "pattern here\n")
        .file("/tmp/f2.txt", "pattern there\n")
        .build();
    let r = sh.exec("grep pattern /tmp/f1.txt /tmp/f2.txt").unwrap();
    assert!(r.stdout.contains("/tmp/f1.txt:"));
    assert!(r.stdout.contains("/tmp/f2.txt:"));
}

#[test]
fn grep_multi_file_count() {
    let mut sh = Shell::builder()
        .file("/tmp/fc1.txt", "aa\nbb\naa\n")
        .file("/tmp/fc2.txt", "aa\ncc\n")
        .build();
    let r = sh.exec("grep -c aa /tmp/fc1.txt /tmp/fc2.txt").unwrap();
    assert!(r.stdout.contains("/tmp/fc1.txt:2"));
    assert!(r.stdout.contains("/tmp/fc2.txt:1"));
}

#[test]
fn grep_h_no_filename() {
    let mut sh = Shell::builder()
        .file("/tmp/h1.txt", "line\n")
        .file("/tmp/h2.txt", "line\n")
        .build();
    let r = sh.exec("grep -h line /tmp/h1.txt /tmp/h2.txt").unwrap();
    assert_eq!(r.stdout, "line\nline\n");
}

#[test]
fn grep_f_literal_dot() {
    let mut sh = Shell::new();
    let r = sh.exec("echo 'a.b' | grep -F 'a.b'").unwrap();
    assert_eq!(r.stdout, "a.b\n");
}

#[test]
fn grep_f_literal_star() {
    let mut sh = Shell::new();
    let r = sh.exec("echo 'a*b' | grep -F 'a*b'").unwrap();
    assert_eq!(r.stdout, "a*b\n");
}

#[test]
fn grep_f_literal_bracket() {
    let mut sh = Shell::new();
    let r = sh.exec("echo 'a[b' | grep -F 'a[b'").unwrap();
    assert_eq!(r.stdout, "a[b\n");
}

#[test]
fn grep_w_word() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "cat\ncatch\nthe cat\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | grep -w cat").unwrap();
    assert_eq!(r.stdout, "cat\nthe cat\n");
}

#[test]
fn grep_w_number() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "file1\n1\ntest1test\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | grep -w 1").unwrap();
    assert_eq!(r.stdout, "1\n");
}

#[test]
fn grep_r_finds_in_subdir() {
    let mut sh = Shell::builder()
        .file("/tmp/rdir/sub/a.txt", "findme\n")
        .file("/tmp/rdir/b.txt", "findme\n")
        .build();
    let r = sh.exec("grep -r findme /tmp/rdir").unwrap();
    assert_eq!(r.exit_code, 0);
    let lines: Vec<&str> = r.stdout.trim().lines().collect();
    assert!(lines.len() >= 2, "expected at least 2 matches, got: {lines:?}");
}

#[test]
fn grep_r_respects_include() {
    let mut sh = Shell::builder()
        .file("/tmp/incdir/a.txt", "target\n")
        .file("/tmp/incdir/b.log", "target\n")
        .build();
    let r = sh.exec("grep -r --include='*.txt' target /tmp/incdir").unwrap();
    assert!(r.stdout.contains("a.txt"));
    assert!(!r.stdout.contains("b.log"));
}

#[test]
fn grep_r_respects_exclude() {
    let mut sh = Shell::builder()
        .file("/tmp/exdir/a.txt", "target\n")
        .file("/tmp/exdir/b.log", "target\n")
        .build();
    let r = sh.exec("grep -r --exclude='*.log' target /tmp/exdir").unwrap();
    assert!(r.stdout.contains("a.txt"));
    assert!(!r.stdout.contains("b.log"));
}

#[test]
fn grep_a_after() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "a\nb\nc\nd\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | grep -A1 b").unwrap();
    assert_eq!(r.stdout, "b\nc\n");
}

#[test]
fn grep_b_before() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "a\nb\nc\nd\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | grep -B1 c").unwrap();
    assert_eq!(r.stdout, "b\nc\n");
}

#[test]
fn grep_c_both() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "a\nb\nc\nd\ne\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | grep -C1 c").unwrap();
    assert_eq!(r.stdout, "b\nc\nd\n");
}

#[test]
fn grep_exit_0_match() {
    let mut sh = Shell::new();
    let r = sh.exec("echo hello | grep hello").unwrap();
    assert_eq!(r.exit_code, 0);
}

#[test]
fn grep_exit_1_no_match() {
    let mut sh = Shell::new();
    let r = sh.exec("echo hello | grep xyz").unwrap();
    assert_eq!(r.exit_code, 1);
}

#[test]
fn grep_exit_2_error() {
    let mut sh = Shell::new();
    let result = sh.exec("echo hello | grep '[invalid'");
    if let Ok(r) = result {
        assert_eq!(r.exit_code, 2);
    }
}

#[test]
fn grep_quiet_match() {
    let mut sh = Shell::new();
    let r = sh.exec("echo hello | grep -q hello").unwrap();
    assert_eq!(r.stdout, "");
    assert_eq!(r.exit_code, 0);
}

#[test]
fn grep_quiet_no_match_edge() {
    let mut sh = Shell::new();
    let r = sh.exec("echo hello | grep -q xyz").unwrap();
    assert_eq!(r.stdout, "");
    assert_eq!(r.exit_code, 1);
}

#[test]
fn grep_max_first_only() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "a\na\na\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | grep -m1 a").unwrap();
    assert_eq!(r.stdout, "a\n");
}

#[test]
fn grep_max_with_count() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "a\na\na\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | grep -m2 -c a").unwrap();
    assert_eq!(r.stdout, "2\n");
}

#[test]
fn grep_o_extract() {
    let mut sh = Shell::new();
    let r = sh.exec("echo 'abc123def456' | grep -oE '[0-9]+'").unwrap();
    assert_eq!(r.stdout, "123\n456\n");
}

#[test]
fn grep_v_invert() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "yes\nno\nyes\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | grep -v yes").unwrap();
    assert_eq!(r.stdout, "no\n");
}

#[test]
fn grep_l_files_with_matches() {
    let mut sh = Shell::builder()
        .file("/tmp/lf1.txt", "apple\n")
        .file("/tmp/lf2.txt", "banana\n")
        .build();
    let r = sh.exec("grep -l apple /tmp/lf1.txt /tmp/lf2.txt").unwrap();
    assert_eq!(r.stdout.trim(), "/tmp/lf1.txt");
}
