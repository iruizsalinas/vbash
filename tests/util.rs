use vbash::Shell;

#[test]
fn date_format() {
    let mut shell = Shell::new();
    let r = shell.exec("date +%Y").unwrap();
    let year = r.stdout.trim();
    assert_eq!(year.len(), 4);
    assert!(year.chars().all(|c| c.is_ascii_digit()));
    assert_eq!(r.exit_code, 0);
}

#[test]
fn date_utc() {
    let mut shell = Shell::new();
    let r = shell.exec("date -u '+%a %b %e %H:%M:%S %Z %Y'").unwrap();
    assert!(r.stdout.contains("UTC"));
    let parts: Vec<&str> = r.stdout.split_whitespace().collect();
    assert!(parts.len() >= 5, "expected date-like fields, got: {}", r.stdout);
}

#[test]
fn expr_add() {
    let mut shell = Shell::new();
    let r = shell.exec("expr 2 + 3").unwrap();
    assert_eq!(r.stdout.trim(), "5");
}

#[test]
fn expr_multiply() {
    let mut shell = Shell::new();
    let r = shell.exec("expr 4 '*' 5").unwrap();
    assert_eq!(r.stdout.trim(), "20");
}

#[test]
fn expr_compare() {
    let mut shell = Shell::new();
    let r = shell.exec("expr 5 '>' 3").unwrap();
    assert_eq!(r.stdout.trim(), "1");
}

#[test]
fn expr_string_length() {
    let mut shell = Shell::new();
    let r = shell.exec("expr length hello").unwrap();
    assert_eq!(r.stdout.trim(), "5");
}

#[test]
fn expr_string_match() {
    let mut shell = Shell::new();
    let r = shell.exec("expr match abc123 '[a-z]*'").unwrap();
    assert_eq!(r.stdout.trim(), "3");
}

#[test]
fn bc_basic() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '2+3' | bc").unwrap();
    assert_eq!(r.stdout.trim(), "5");
}

#[test]
fn bc_decimal() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '10/3' | bc").unwrap();
    assert!(
        r.stdout.trim().starts_with("3.3"),
        "expected 3.3..., got: {}",
        r.stdout.trim()
    );
}

#[test]
fn bc_parentheses() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '(2+3)*4' | bc").unwrap();
    assert_eq!(r.stdout.trim(), "20");
}

#[test]
fn seq_single() {
    let mut shell = Shell::new();
    let r = shell.exec("seq 5").unwrap();
    assert_eq!(r.stdout, "1\n2\n3\n4\n5\n");
}

#[test]
fn seq_range() {
    let mut shell = Shell::new();
    let r = shell.exec("seq 3 7").unwrap();
    assert_eq!(r.stdout, "3\n4\n5\n6\n7\n");
}

#[test]
fn seq_step() {
    let mut shell = Shell::new();
    let r = shell.exec("seq 1 2 10").unwrap();
    assert_eq!(r.stdout, "1\n3\n5\n7\n9\n");
}

#[test]
fn seq_reverse() {
    let mut shell = Shell::new();
    let r = shell.exec("seq 5 -1 1").unwrap();
    assert_eq!(r.stdout, "5\n4\n3\n2\n1\n");
}

#[test]
fn yes_limited() {
    let mut shell = Shell::new();
    let r = shell.exec("yes | head -n 3").unwrap();
    assert_eq!(r.stdout, "y\ny\ny\n");
}

#[test]
fn realpath_basic() {
    let mut shell = Shell::new();
    let r = shell.exec("realpath /tmp").unwrap();
    assert_eq!(r.stdout.trim(), "/tmp");
}

#[test]
fn mktemp_file() {
    let mut shell = Shell::new();
    let r = shell.exec("mktemp").unwrap();
    assert_eq!(r.exit_code, 0);
    let path = r.stdout.trim();
    assert!(path.starts_with("/tmp/tmp."));
    let check = shell
        .exec(&format!("[[ -f {path} ]] && echo exists"))
        .unwrap();
    assert_eq!(check.stdout.trim(), "exists");
}

#[test]
fn mktemp_dir() {
    let mut shell = Shell::new();
    let r = shell.exec("mktemp -d").unwrap();
    assert_eq!(r.exit_code, 0);
    let path = r.stdout.trim();
    assert!(path.starts_with("/tmp/tmp."));
    let check = shell
        .exec(&format!("[[ -d {path} ]] && echo exists"))
        .unwrap();
    assert_eq!(check.stdout.trim(), "exists");
}

#[test]
fn whoami_basic() {
    let mut shell = Shell::new();
    let r = shell.exec("whoami").unwrap();
    assert_eq!(r.stdout.trim(), "user");
}

#[test]
fn hostname_basic() {
    let mut shell = Shell::new();
    let r = shell.exec("hostname").unwrap();
    assert_eq!(r.stdout.trim(), "vbash");
}

#[test]
fn uname_sysname() {
    let mut shell = Shell::new();
    let r = shell.exec("uname -s").unwrap();
    assert_eq!(r.stdout.trim(), "Linux");
}

#[test]
fn uname_all() {
    let mut shell = Shell::new();
    let r = shell.exec("uname -a").unwrap();
    assert_eq!(r.stdout.trim(), "Linux vbash 5.15.0 x86_64");
}

#[test]
fn timeout_no_exec_fn() {
    let mut shell = Shell::new();
    let r = shell.exec("timeout 10 echo hello").unwrap();
    assert_eq!(r.exit_code, 126);
    assert_eq!(r.stderr, "timeout: cannot execute subcommand\n");
}

#[test]
fn nohup_no_exec_fn() {
    let mut shell = Shell::new();
    let r = shell.exec("nohup echo hello").unwrap();
    assert_eq!(r.exit_code, 126);
    assert_eq!(r.stderr, "nohup: cannot execute subcommand\n");
}

#[test]
fn xargs_no_exec_fn() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'a b c' | xargs echo").unwrap();
    assert_eq!(r.exit_code, 126);
    assert_eq!(r.stderr, "xargs: cannot execute subcommand\n");
}

#[test]
fn xargs_replace_no_exec_fn() {
    let mut shell = Shell::new();
    let r = shell
        .exec("echo /tmp | xargs -I X echo X")
        .unwrap();
    assert_eq!(r.exit_code, 126);
    assert_eq!(r.stderr, "xargs: cannot execute subcommand\n");
}

#[test]
fn du_basic() {
    let mut shell = Shell::builder()
        .file("/tmp/dufile.txt", "some content here")
        .build();
    let r = shell.exec("du /tmp").unwrap();
    assert_eq!(r.exit_code, 0);
    let lines: Vec<&str> = r.stdout.trim().lines().collect();
    assert!(lines.len() >= 2, "du should list files and dirs, got: {}", r.stdout);
    assert!(lines.iter().any(|l| l.contains("/tmp/dufile.txt")));
    assert!(lines.iter().any(|l| l.ends_with("/tmp")));
}

#[test]
fn du_summary() {
    let mut shell = Shell::builder()
        .file("/tmp/dufile.txt", "some content here")
        .build();
    let r = shell.exec("du -s /tmp").unwrap();
    let lines: Vec<&str> = r.stdout.trim().lines().collect();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].ends_with("\t/tmp"));
}

#[test]
fn du_human() {
    let mut shell = Shell::builder()
        .file("/tmp/dufile.txt", "some content here")
        .build();
    let r = shell.exec("du -h /tmp/dufile.txt").unwrap();
    let line = r.stdout.trim();
    assert!(line.ends_with("\t/tmp/dufile.txt"));
    let size_part = line.split('\t').next().unwrap();
    assert!(
        size_part.chars().any(|c| c.is_ascii_digit()),
        "human size should contain digits, got: {size_part}"
    );
}
