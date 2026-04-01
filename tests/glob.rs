use vbash::Shell;

#[test]
fn glob_star_matches_files() {
    let mut shell = Shell::builder()
        .file("/tmp/glob1/a.txt", "")
        .file("/tmp/glob1/b.txt", "")
        .file("/tmp/glob1/c.log", "")
        .build();
    let r = shell.exec("echo /tmp/glob1/*.txt").unwrap();
    let mut files: Vec<&str> = r.stdout.split_whitespace().collect();
    files.sort_unstable();
    assert_eq!(files, vec!["/tmp/glob1/a.txt", "/tmp/glob1/b.txt"]);
}

#[test]
fn glob_question_mark() {
    let mut shell = Shell::builder()
        .file("/tmp/glob2/ax.txt", "")
        .file("/tmp/glob2/ab.txt", "")
        .file("/tmp/glob2/abc.txt", "")
        .build();
    let r = shell.exec("echo /tmp/glob2/a?.txt").unwrap();
    let mut files: Vec<&str> = r.stdout.split_whitespace().collect();
    files.sort_unstable();
    assert_eq!(files, vec!["/tmp/glob2/ab.txt", "/tmp/glob2/ax.txt"]);
}

#[test]
fn glob_no_match_literal() {
    let mut shell = Shell::new();
    let r = shell.exec("echo /nonexistent/*.xyz").unwrap();
    assert_eq!(r.stdout.trim(), "/nonexistent/*.xyz");
}

#[test]
fn glob_empty_dir() {
    let mut shell = Shell::new();
    shell.exec("mkdir -p /tmp/emptyglob").unwrap();
    let r = shell.exec("echo /tmp/emptyglob/*").unwrap();
    assert_eq!(r.stdout.trim(), "/tmp/emptyglob/*");
}

#[test]
fn glob_char_class() {
    let mut shell = Shell::builder()
        .file("/tmp/glob3/fa.txt", "")
        .file("/tmp/glob3/fb.txt", "")
        .file("/tmp/glob3/fc.txt", "")
        .build();
    let r = shell.exec("echo /tmp/glob3/f[ab].txt").unwrap();
    let mut files: Vec<&str> = r.stdout.split_whitespace().collect();
    files.sort_unstable();
    assert_eq!(files, vec!["/tmp/glob3/fa.txt", "/tmp/glob3/fb.txt"]);
}

#[test]
fn glob_char_range() {
    let mut shell = Shell::builder()
        .file("/tmp/glob4/f1", "")
        .file("/tmp/glob4/f2", "")
        .file("/tmp/glob4/f3", "")
        .file("/tmp/glob4/fa", "")
        .file("/tmp/glob4/fb", "")
        .build();
    let r = shell.exec("echo /tmp/glob4/f[0-9]").unwrap();
    let mut files: Vec<&str> = r.stdout.split_whitespace().collect();
    files.sort_unstable();
    assert_eq!(files, vec!["/tmp/glob4/f1", "/tmp/glob4/f2", "/tmp/glob4/f3"]);
}

#[test]
fn glob_char_negation() {
    let mut shell = Shell::builder()
        .file("/tmp/glob5/fa", "")
        .file("/tmp/glob5/fb", "")
        .file("/tmp/glob5/fc", "")
        .build();
    let r = shell.exec("echo /tmp/glob5/f[!a]").unwrap();
    let mut files: Vec<&str> = r.stdout.split_whitespace().collect();
    files.sort_unstable();
    assert_eq!(files, vec!["/tmp/glob5/fb", "/tmp/glob5/fc"]);
}

#[test]
fn glob_star_no_hidden() {
    let mut shell = Shell::builder()
        .file("/tmp/glob6/.hidden", "")
        .file("/tmp/glob6/visible", "")
        .build();
    let r = shell.exec("echo /tmp/glob6/*").unwrap();
    assert_eq!(r.stdout.trim(), "/tmp/glob6/visible");
}

#[test]
fn glob_dotglob() {
    let mut shell = Shell::builder()
        .file("/tmp/glob7/.hidden", "")
        .file("/tmp/glob7/visible", "")
        .build();
    let r = shell.exec("shopt -s dotglob; echo /tmp/glob7/*").unwrap();
    let mut files: Vec<&str> = r.stdout.split_whitespace().collect();
    files.sort_unstable();
    assert_eq!(files, vec!["/tmp/glob7/.hidden", "/tmp/glob7/visible"]);
}

#[test]
fn glob_explicit_dot() {
    let mut shell = Shell::builder()
        .file("/tmp/glob8/.hidden", "")
        .file("/tmp/glob8/visible", "")
        .build();
    let r = shell.exec("echo /tmp/glob8/.*").unwrap();
    assert!(r.stdout.contains(".hidden"), "expected .hidden in: {}", r.stdout);
}

#[test]
fn glob_nullglob_off() {
    let mut shell = Shell::new();
    let r = shell.exec("echo /nonexistent_xyz/*").unwrap();
    assert_eq!(r.stdout.trim(), "/nonexistent_xyz/*");
}

#[test]
fn glob_nullglob_on() {
    let mut shell = Shell::new();
    let r = shell.exec("shopt -s nullglob; echo /nonexistent_xyz/*").unwrap();
    assert_eq!(r.stdout.trim(), "");
}

#[test]
fn glob_failglob() {
    let mut shell = Shell::new();
    let r = shell.exec("shopt -s failglob; echo /nonexistent_xyz/*");
    assert!(r.is_err() || r.unwrap().exit_code != 0);
}

#[test]
fn glob_in_single_quotes() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '/nonexistent_dir/*.txt'").unwrap();
    assert_eq!(r.stdout.trim(), "/nonexistent_dir/*.txt");
}

#[test]
fn glob_in_double_quotes() {
    let mut shell = Shell::new();
    let r = shell.exec("echo \"/nonexistent_dir/*.txt\"").unwrap();
    assert_eq!(r.stdout.trim(), "/nonexistent_dir/*.txt");
}

#[test]
fn glob_in_for() {
    let mut shell = Shell::builder()
        .file("/tmp/glob11/a.txt", "")
        .file("/tmp/glob11/b.txt", "")
        .build();
    let r = shell.exec("for f in /tmp/glob11/*.txt; do echo $f; done").unwrap();
    let mut lines: Vec<&str> = r.stdout.trim().lines().collect();
    lines.sort_unstable();
    assert_eq!(lines, vec!["/tmp/glob11/a.txt", "/tmp/glob11/b.txt"]);
}

#[test]
fn glob_in_case() {
    let mut shell = Shell::new();
    let r = shell.exec("case /tmp/a.txt in *.txt) echo yes;; esac").unwrap();
    assert_eq!(r.stdout, "yes\n");
}

#[test]
fn glob_subdir() {
    let mut shell = Shell::builder()
        .file("/tmp/glob12/sub/a.txt", "")
        .file("/tmp/glob12/sub/b.txt", "")
        .build();
    let r = shell.exec("echo /tmp/glob12/sub/*.txt").unwrap();
    let mut files: Vec<&str> = r.stdout.split_whitespace().collect();
    files.sort_unstable();
    assert_eq!(files, vec!["/tmp/glob12/sub/a.txt", "/tmp/glob12/sub/b.txt"]);
}

#[test]
fn glob_in_assignment() {
    let mut shell = Shell::builder()
        .file("/tmp/glob13/a.txt", "")
        .build();
    let r = shell.exec("X=/tmp/glob13/*.txt; echo $X").unwrap();
    assert!(
        r.stdout.trim() == "/tmp/glob13/a.txt" || r.stdout.trim() == "/tmp/glob13/*.txt",
        "got: {}",
        r.stdout.trim()
    );
}

#[test]
fn glob_multiple_patterns() {
    let mut shell = Shell::builder()
        .file("/tmp/glob14/a.txt", "")
        .file("/tmp/glob14/b.log", "")
        .build();
    let r = shell.exec("echo /tmp/glob14/*.txt /tmp/glob14/*.log").unwrap();
    let mut files: Vec<&str> = r.stdout.split_whitespace().collect();
    files.sort_unstable();
    assert_eq!(files, vec!["/tmp/glob14/a.txt", "/tmp/glob14/b.log"]);
}

#[test]
fn glob_star_in_middle() {
    let mut shell = Shell::builder()
        .file("/tmp/glob15/test_a.txt", "")
        .file("/tmp/glob15/test_b.txt", "")
        .file("/tmp/glob15/other.txt", "")
        .build();
    let r = shell.exec("echo /tmp/glob15/test_*.txt").unwrap();
    let mut files: Vec<&str> = r.stdout.split_whitespace().collect();
    files.sort_unstable();
    assert_eq!(files, vec!["/tmp/glob15/test_a.txt", "/tmp/glob15/test_b.txt"]);
}

#[test]
fn glob_extglob_at() {
    let mut shell = Shell::builder()
        .file("/tmp/glob16/a.txt", "")
        .file("/tmp/glob16/b.txt", "")
        .file("/tmp/glob16/c.log", "")
        .build();
    let r = shell.exec("shopt -s extglob; echo /tmp/glob16/@(a|b).txt").unwrap();
    let mut files: Vec<&str> = r.stdout.split_whitespace().collect();
    files.sort_unstable();
    assert_eq!(files, vec!["/tmp/glob16/a.txt", "/tmp/glob16/b.txt"]);
}

#[test]
fn glob_extglob_star_case() {
    let mut shell = Shell::new();
    let r = shell.exec("shopt -s extglob; case \"aaa\" in *(a)) echo match;; *) echo no;; esac").unwrap();
    assert_eq!(r.stdout, "match\n");
}

#[test]
fn glob_extglob_plus_case() {
    let mut shell = Shell::new();
    let r = shell.exec("shopt -s extglob; case \"abc\" in +(abc)) echo match;; *) echo no;; esac").unwrap();
    assert_eq!(r.stdout, "match\n");
}

#[test]
fn glob_extglob_question_case() {
    let mut shell = Shell::new();
    let r = shell.exec("shopt -s extglob; case \"\" in ?(x)) echo match;; *) echo no;; esac").unwrap();
    assert_eq!(r.stdout, "match\n");
}

#[test]
fn glob_extglob_not_case() {
    let mut shell = Shell::new();
    let r = shell.exec("shopt -s extglob; case \"xyz\" in !(abc)) echo match;; *) echo no;; esac").unwrap();
    assert_eq!(r.stdout, "match\n");
}

#[test]
fn glob_globstar_on() {
    let mut shell = Shell::builder()
        .file("/tmp/glob17/a.txt", "")
        .file("/tmp/glob17/sub/b.txt", "")
        .file("/tmp/glob17/sub/deep/c.txt", "")
        .build();
    let r = shell.exec("shopt -s globstar; echo /tmp/glob17/**/*.txt").unwrap();
    let mut files: Vec<&str> = r.stdout.split_whitespace().collect();
    files.sort_unstable();
    assert!(files.len() >= 2, "expected at least sub/b.txt and sub/deep/c.txt, got: {files:?}");
}

#[test]
fn glob_noglob() {
    let mut shell = Shell::builder()
        .file("/tmp/glob18/a.txt", "")
        .build();
    let r = shell.exec("set -f; echo /tmp/glob18/*.txt").unwrap();
    assert_eq!(r.stdout.trim(), "/tmp/glob18/*.txt");
}

#[test]
fn glob_question_single_char() {
    let mut shell = Shell::builder()
        .file("/tmp/glob19/a", "")
        .file("/tmp/glob19/bb", "")
        .build();
    let r = shell.exec("echo /tmp/glob19/?").unwrap();
    assert_eq!(r.stdout.trim(), "/tmp/glob19/a");
}

#[test]
fn glob_no_expand_in_assignment_rhs() {
    let mut shell = Shell::builder()
        .file("/tmp/glob20/x.txt", "")
        .build();
    let r = shell.exec("Y=/tmp/glob20/*.txt; echo \"$Y\"").unwrap();
    assert_eq!(r.stdout.trim(), "/tmp/glob20/*.txt");
}

#[test]
fn glob_star_with_prefix_and_suffix() {
    let mut shell = Shell::builder()
        .file("/tmp/glob21/abc_test_xyz.txt", "")
        .file("/tmp/glob21/abc_other_xyz.txt", "")
        .file("/tmp/glob21/abc_test_xyz.log", "")
        .build();
    let r = shell.exec("echo /tmp/glob21/abc_*_xyz.txt").unwrap();
    let mut files: Vec<&str> = r.stdout.split_whitespace().collect();
    files.sort_unstable();
    assert_eq!(files, vec!["/tmp/glob21/abc_other_xyz.txt", "/tmp/glob21/abc_test_xyz.txt"]);
}
