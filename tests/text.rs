use vbash::Shell;

// cut

#[test]
fn cut_fields() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'a:b:c' | cut -d : -f 2").unwrap();
    assert_eq!(r.stdout, "b\n");
}

#[test]
fn cut_chars() {
    let mut shell = Shell::new();
    let r = shell.exec("echo hello | cut -c 1-3").unwrap();
    assert_eq!(r.stdout, "hel\n");
}

#[test]
fn cut_multiple_fields() {
    let mut shell = Shell::builder()
        .file("/tmp/data.csv", "a,b,c\nd,e,f\n")
        .build();
    let r = shell.exec("cut -d , -f 1,3 /tmp/data.csv").unwrap();
    assert_eq!(r.stdout, "a,c\nd,f\n");
}

#[test]
fn cut_only_delimited_suppresses() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'no-delim' | cut -d : -f 1 -s").unwrap();
    assert_eq!(r.stdout, "");
}

#[test]
fn cut_no_spec_fails() {
    let mut shell = Shell::new();
    let r = shell.exec("echo hello | cut").unwrap();
    assert_ne!(r.exit_code, 0);
    assert!(r.stderr.contains("you must specify"));
}

// tr

#[test]
fn tr_translate_case() {
    let mut shell = Shell::new();
    let r = shell.exec("echo hello | tr 'a-z' 'A-Z'").unwrap();
    assert_eq!(r.stdout, "HELLO\n");
}

#[test]
fn tr_delete_digits() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'hello 123' | tr -d '0-9'").unwrap();
    assert_eq!(r.stdout, "hello \n");
}

#[test]
fn tr_squeeze() {
    let mut shell = Shell::new();
    let r = shell.exec("echo aabbbcc | tr -s 'b'").unwrap();
    assert_eq!(r.stdout, "aabcc\n");
}

#[test]
fn tr_complement_delete() {
    let mut shell = Shell::new();
    let r = shell.exec("echo hello123 | tr -cd '0-9'").unwrap();
    assert_eq!(r.stdout, "123");
}

#[test]
fn tr_missing_operand_fails() {
    let mut shell = Shell::new();
    let r = shell.exec("echo hello | tr 'a-z'").unwrap();
    assert_ne!(r.exit_code, 0);
    assert_eq!(r.stderr, "tr: missing operand\n");
}

// uniq

#[test]
fn uniq_basic_dedup() {
    let mut shell = Shell::builder()
        .file("/tmp/data.txt", "a\na\nb\nb\nc\n")
        .build();
    let r = shell.exec("uniq /tmp/data.txt").unwrap();
    assert_eq!(r.stdout, "a\nb\nc\n");
}

#[test]
fn uniq_count_exact_format() {
    let mut shell = Shell::builder()
        .file("/tmp/data.txt", "a\na\nb\nc\nc\nc\n")
        .build();
    let r = shell.exec("uniq -c /tmp/data.txt").unwrap();
    assert_eq!(r.stdout, "      2 a\n      1 b\n      3 c\n");
}

#[test]
fn uniq_duplicates_only() {
    let mut shell = Shell::builder()
        .file("/tmp/data.txt", "a\na\nb\nc\nc\n")
        .build();
    let r = shell.exec("uniq -d /tmp/data.txt").unwrap();
    assert_eq!(r.stdout, "a\nc\n");
}

#[test]
fn uniq_unique_only() {
    let mut shell = Shell::builder()
        .file("/tmp/data.txt", "a\na\nb\nc\nc\n")
        .build();
    let r = shell.exec("uniq -u /tmp/data.txt").unwrap();
    assert_eq!(r.stdout, "b\n");
}

// rev / tac

#[test]
fn rev_reverses_chars() {
    let mut shell = Shell::new();
    let r = shell.exec("echo hello | rev").unwrap();
    assert_eq!(r.stdout, "olleh\n");
}

#[test]
fn tac_reverses_lines() {
    let mut shell = Shell::builder()
        .file("/tmp/data.txt", "a\nb\nc\n")
        .build();
    let r = shell.exec("tac /tmp/data.txt").unwrap();
    assert_eq!(r.stdout, "c\nb\na\n");
}

// paste

#[test]
fn paste_two_files() {
    let mut shell = Shell::builder()
        .file("/tmp/a.txt", "1\n2\n3\n")
        .file("/tmp/b.txt", "a\nb\nc\n")
        .build();
    let r = shell.exec("paste /tmp/a.txt /tmp/b.txt").unwrap();
    assert_eq!(r.stdout, "1\ta\n2\tb\n3\tc\n");
}

#[test]
fn paste_serial() {
    let mut shell = Shell::builder()
        .file("/tmp/a.txt", "1\n2\n3\n")
        .build();
    let r = shell.exec("paste -s /tmp/a.txt").unwrap();
    assert_eq!(r.stdout, "1\t2\t3\n");
}

#[test]
fn paste_custom_delimiter() {
    let mut shell = Shell::builder()
        .file("/tmp/a.txt", "1\n2\n")
        .file("/tmp/b.txt", "a\nb\n")
        .build();
    let r = shell.exec("paste -d , /tmp/a.txt /tmp/b.txt").unwrap();
    assert_eq!(r.stdout, "1,a\n2,b\n");
}

// fold

#[test]
fn fold_wraps_at_width() {
    let mut shell = Shell::builder()
        .file("/tmp/data.txt", "abcdefghijklmnopqrst")
        .build();
    let r = shell.exec("fold -w 10 /tmp/data.txt").unwrap();
    assert_eq!(r.stdout, "abcdefghij\nklmnopqrst\n");
}

// expand

#[test]
fn expand_tabs_default() {
    let mut shell = Shell::builder()
        .file("/tmp/data.txt", "a\tb")
        .build();
    let r = shell.exec("expand /tmp/data.txt").unwrap();
    assert!(!r.stdout.contains('\t'));
    assert_eq!(r.stdout, "a       b\n");
}

#[test]
fn expand_tabs_custom_width() {
    let mut shell = Shell::builder()
        .file("/tmp/data.txt", "\tx")
        .build();
    let r = shell.exec("expand -t 4 /tmp/data.txt").unwrap();
    assert_eq!(r.stdout, "    x\n");
}

// column

#[test]
fn column_table_alignment() {
    let mut shell = Shell::builder()
        .file("/tmp/data.txt", "a bb ccc\ndddd ee f\n")
        .build();
    let r = shell.exec("column -t /tmp/data.txt").unwrap();
    assert_eq!(r.stdout, "a     bb  ccc\ndddd  ee  f\n");
}

// comm

#[test]
fn comm_three_columns() {
    let mut shell = Shell::builder()
        .file("/tmp/a.txt", "a\nb\nc\n")
        .file("/tmp/b.txt", "b\nc\nd\n")
        .build();
    let r = shell.exec("comm /tmp/a.txt /tmp/b.txt").unwrap();
    assert_eq!(r.stdout, "a\n\t\tb\n\t\tc\n\td\n");
}

#[test]
fn comm_suppress_columns() {
    let mut shell = Shell::builder()
        .file("/tmp/a.txt", "a\nb\nc\n")
        .file("/tmp/b.txt", "b\nc\nd\n")
        .build();
    let r = shell.exec("comm -12 /tmp/a.txt /tmp/b.txt").unwrap();
    assert_eq!(r.stdout, "b\nc\n");
}

#[test]
fn comm_missing_operand_fails() {
    let mut shell = Shell::new();
    let r = shell.exec("comm /tmp/a.txt").unwrap();
    assert_ne!(r.exit_code, 0);
    assert!(r.stderr.contains("missing operand"));
}

// join

#[test]
fn join_on_key() {
    let mut shell = Shell::builder()
        .file("/tmp/a.txt", "1 alice\n2 bob\n")
        .file("/tmp/b.txt", "1 NY\n2 LA\n")
        .build();
    let r = shell.exec("join /tmp/a.txt /tmp/b.txt").unwrap();
    assert_eq!(r.stdout, "1 alice NY\n2 bob LA\n");
}

#[test]
fn join_missing_operand_fails() {
    let mut shell = Shell::new();
    let r = shell.exec("join /tmp/a.txt").unwrap();
    assert_ne!(r.exit_code, 0);
    assert!(r.stderr.contains("missing operand"));
}

// nl

#[test]
fn nl_numbers_nonempty_lines() {
    let mut shell = Shell::builder()
        .file("/tmp/data.txt", "hello\nworld\n")
        .build();
    let r = shell.exec("nl /tmp/data.txt").unwrap();
    assert_eq!(r.stdout, "     1\thello\n     2\tworld\n");
}

#[test]
fn nl_body_all_numbers_blank_lines() {
    let mut shell = Shell::builder()
        .file("/tmp/data.txt", "a\n\nb\n")
        .build();
    let r = shell.exec("nl -b a /tmp/data.txt").unwrap();
    let lines: Vec<&str> = r.stdout.lines().collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "     1\ta");
    assert_eq!(lines[1], "     2\t");
    assert_eq!(lines[2], "     3\tb");
}

// od

#[test]
fn od_hex_exact_output() {
    let mut shell = Shell::builder()
        .file("/tmp/data.txt", "AB")
        .build();
    let r = shell.exec("od -t x1 /tmp/data.txt").unwrap();
    let lines: Vec<&str> = r.stdout.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "0000000 41 42");
    assert_eq!(lines[1], "0000002");
}

#[test]
fn od_octal_default() {
    let mut shell = Shell::builder()
        .file("/tmp/data.txt", "A")
        .build();
    let r = shell.exec("od /tmp/data.txt").unwrap();
    let lines: Vec<&str> = r.stdout.lines().collect();
    assert_eq!(lines[0], "0000000 000101");
    assert_eq!(lines[1], "0000001");
}

// base64

#[test]
fn base64_encode() {
    let mut shell = Shell::new();
    let r = shell.exec("echo -n hello | base64").unwrap();
    assert_eq!(r.stdout, "aGVsbG8=\n");
}

#[test]
fn base64_decode() {
    let mut shell = Shell::new();
    let r = shell.exec("echo -n 'aGVsbG8=' | base64 -d").unwrap();
    assert_eq!(r.stdout, "hello");
}

#[test]
fn base64_roundtrip() {
    let mut shell = Shell::new();
    let r = shell.exec("echo -n 'test data 123' | base64 | base64 -d").unwrap();
    assert_eq!(r.stdout, "test data 123");
}

// strings

#[test]
fn strings_extracts_printable() {
    let mut shell = Shell::builder()
        .file("/tmp/data.txt", "hello world")
        .build();
    let r = shell.exec("strings /tmp/data.txt").unwrap();
    assert_eq!(r.stdout, "hello world\n");
}

#[test]
fn strings_min_length_filter() {
    let mut shell = Shell::builder()
        .file("/tmp/data.txt", "ab cd efghij")
        .build();
    let r = shell.exec("strings -n 6 /tmp/data.txt").unwrap();
    assert_eq!(r.stdout, "ab cd efghij\n");
}

// diff

#[test]
fn diff_identical_no_output() {
    let mut shell = Shell::builder()
        .file("/tmp/a.txt", "same\ncontent\n")
        .file("/tmp/b.txt", "same\ncontent\n")
        .build();
    let r = shell.exec("diff /tmp/a.txt /tmp/b.txt").unwrap();
    assert_eq!(r.exit_code, 0);
    assert_eq!(r.stdout, "");
}

#[test]
fn diff_different_exit_code_and_headers() {
    let mut shell = Shell::builder()
        .file("/tmp/a.txt", "line1\nline2\nline3\nline4\n")
        .file("/tmp/b.txt", "line1\nchanged\nline3\nline4\n")
        .build();
    let r = shell.exec("diff /tmp/a.txt /tmp/b.txt").unwrap();
    assert_eq!(r.exit_code, 1);
    let lines: Vec<&str> = r.stdout.lines().collect();
    assert_eq!(lines[0], "--- /tmp/a.txt");
    assert_eq!(lines[1], "+++ /tmp/b.txt");
    assert!(lines[2].starts_with("@@") && lines[2].ends_with("@@"));
    assert!(lines.iter().any(|l| l.starts_with('-') && !l.starts_with("---")));
    assert!(lines.iter().any(|l| l.starts_with('+') && !l.starts_with("+++")));
}

// tee

#[test]
fn tee_copies_to_file_and_stdout() {
    let mut shell = Shell::new();
    let r = shell.exec("echo hello | tee /tmp/out.txt").unwrap();
    assert_eq!(r.stdout, "hello\n");
    let r2 = shell.exec("cat /tmp/out.txt").unwrap();
    assert_eq!(r2.stdout, "hello\n");
}

#[test]
fn tee_append_mode() {
    let mut shell = Shell::builder()
        .file("/tmp/out.txt", "first\n")
        .build();
    shell.exec("echo second | tee -a /tmp/out.txt").unwrap();
    let r = shell.exec("cat /tmp/out.txt").unwrap();
    assert_eq!(r.stdout, "first\nsecond\n");
}
