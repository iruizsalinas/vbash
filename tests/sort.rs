use vbash::Shell;

#[test]
fn sort_alphabetical() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "banana\napple\ncherry\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | sort").unwrap();
    assert_eq!(r.stdout, "apple\nbanana\ncherry\n");
}

#[test]
fn sort_already_sorted() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "a\nb\nc\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | sort").unwrap();
    assert_eq!(r.stdout, "a\nb\nc\n");
}

#[test]
fn sort_single_line() {
    let mut sh = Shell::new();
    let r = sh.exec("echo hello | sort").unwrap();
    assert_eq!(r.stdout, "hello\n");
}

#[test]
fn sort_empty_input() {
    let mut sh = Shell::new();
    let r = sh.exec("echo -n | sort").unwrap();
    assert_eq!(r.stdout, "");
}

#[test]
fn sort_numeric() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "10\n2\n1\n20\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | sort -n").unwrap();
    assert_eq!(r.stdout, "1\n2\n10\n20\n");
}

#[test]
fn sort_numeric_negative() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "5\n-3\n0\n-1\n2\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | sort -n").unwrap();
    assert_eq!(r.stdout, "-3\n-1\n0\n2\n5\n");
}

#[test]
fn sort_numeric_decimal() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "1.5\n1.2\n1.8\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | sort -n").unwrap();
    assert_eq!(r.stdout, "1.2\n1.5\n1.8\n");
}

#[test]
fn sort_numeric_leading_zeros() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "010\n2\n001\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | sort -n").unwrap();
    assert_eq!(r.stdout, "001\n2\n010\n");
}

#[test]
fn sort_reverse() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "a\nc\nb\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | sort -r").unwrap();
    assert_eq!(r.stdout, "c\nb\na\n");
}

#[test]
fn sort_unique() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "a\nb\na\nc\nb\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | sort -u").unwrap();
    assert_eq!(r.stdout, "a\nb\nc\n");
}

#[test]
fn sort_ignore_case() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "B\na\nC\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | sort -f").unwrap();
    let lines: Vec<&str> = r.stdout.trim().lines().collect();
    assert_eq!(lines.len(), 3);
    let lower: Vec<String> = lines.iter().map(|l| l.to_lowercase()).collect();
    assert!(lower[0] <= lower[1] && lower[1] <= lower[2],
        "expected case-insensitive sort, got: {lines:?}");
}

#[test]
fn sort_key_field() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "b 2\na 1\nc 3\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | sort -k1,1").unwrap();
    assert_eq!(r.stdout, "a 1\nb 2\nc 3\n");
}

#[test]
fn sort_key_numeric() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "b 20\na 3\nc 100\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | sort -k2,2n").unwrap();
    assert_eq!(r.stdout, "a 3\nb 20\nc 100\n");
}

#[test]
fn sort_key_reverse() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "a 1\nb 2\nc 3\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | sort -k2,2rn").unwrap();
    assert_eq!(r.stdout, "c 3\nb 2\na 1\n");
}

#[test]
fn sort_custom_separator() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "b:2\na:1\nc:3\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | sort -t: -k2,2n").unwrap();
    assert_eq!(r.stdout, "a:1\nb:2\nc:3\n");
}

#[test]
fn sort_multiple_keys() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "a 2\na 1\nb 1\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | sort -k1,1 -k2,2n").unwrap();
    assert_eq!(r.stdout, "a 1\na 2\nb 1\n");
}

#[test]
fn sort_check_sorted() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "a\nb\nc\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | sort -c").unwrap();
    assert_eq!(r.exit_code, 0);
}

#[test]
fn sort_check_unsorted() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "b\na\nc\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | sort -c").unwrap();
    assert_eq!(r.exit_code, 1);
}

#[test]
fn sort_output_file() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "b\na\n")
        .build();
    sh.exec("cat /tmp/in.txt | sort -o /tmp/sorted.txt").unwrap();
    let r = sh.exec("cat /tmp/sorted.txt").unwrap();
    assert_eq!(r.stdout, "a\nb\n");
}

#[test]
fn sort_stable() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "b 1\na 1\nc 1\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | sort -s -k2,2").unwrap();
    assert_eq!(r.stdout, "b 1\na 1\nc 1\n");
}

#[test]
fn sort_human() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "1K\n1M\n1G\n500\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | sort -h").unwrap();
    assert_eq!(r.stdout, "500\n1K\n1M\n1G\n");
}

#[test]
fn sort_duplicate_lines() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "a\na\na\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | sort").unwrap();
    assert_eq!(r.stdout, "a\na\na\n");
}

#[test]
fn sort_whitespace_lines() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "  b\n a\n  c\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | sort").unwrap();
    let lines: Vec<&str> = r.stdout.lines().collect();
    assert_eq!(lines.len(), 3);
    for i in 0..lines.len() - 1 {
        assert!(lines[i] <= lines[i + 1],
            "expected sorted order, got: {lines:?}");
    }
}

#[test]
fn sort_reverse_numeric() {
    let mut sh = Shell::builder()
        .file("/tmp/in.txt", "1\n3\n2\n")
        .build();
    let r = sh.exec("cat /tmp/in.txt | sort -rn").unwrap();
    assert_eq!(r.stdout, "3\n2\n1\n");
}
