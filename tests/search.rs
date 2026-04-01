use vbash::Shell;

#[test]
fn find_name() {
    let mut shell = Shell::builder()
        .file("/tmp/hello.txt", "data")
        .file("/tmp/world.txt", "data")
        .file("/tmp/notes.log", "data")
        .build();
    let r = shell.exec("find /tmp -name '*.txt'").unwrap();
    let mut lines: Vec<&str> = r.stdout.trim().lines().collect();
    lines.sort_unstable();
    assert_eq!(lines, vec!["/tmp/hello.txt", "/tmp/world.txt"]);
}

#[test]
fn find_iname() {
    let mut shell = Shell::builder()
        .file("/tmp/Report.TXT", "data")
        .build();
    let r = shell.exec("find /tmp -iname '*.txt'").unwrap();
    assert_eq!(r.stdout.trim(), "/tmp/Report.TXT");
}

#[test]
fn find_type_file() {
    let mut shell = Shell::builder()
        .file("/tmp/sub/file.txt", "data")
        .build();
    let r = shell.exec("find /tmp -type f").unwrap();
    assert_eq!(r.stdout.trim(), "/tmp/sub/file.txt");
}

#[test]
fn find_type_dir() {
    let mut shell = Shell::builder()
        .file("/tmp/sub/file.txt", "data")
        .build();
    let r = shell.exec("find /tmp -type d").unwrap();
    let mut lines: Vec<&str> = r.stdout.trim().lines().collect();
    lines.sort_unstable();
    assert_eq!(lines, vec!["/tmp", "/tmp/sub"]);
}

#[test]
fn find_empty() {
    let mut shell = Shell::new();
    shell.exec("touch /tmp/empty.txt").unwrap();
    shell.exec("echo data > /tmp/notempty.txt").unwrap();
    let r = shell.exec("find /tmp -empty -type f").unwrap();
    assert!(r.stdout.contains("empty.txt"));
    assert!(!r.stdout.contains("notempty.txt"));
}

#[test]
fn find_maxdepth() {
    let mut shell = Shell::builder()
        .file("/tmp/a.txt", "data")
        .file("/tmp/sub/b.txt", "data")
        .build();
    let r = shell.exec("find /tmp -maxdepth 1 -name '*.txt'").unwrap();
    assert_eq!(r.stdout.trim(), "/tmp/a.txt");
}

#[test]
fn find_mindepth() {
    let mut shell = Shell::builder()
        .file("/tmp/top.txt", "data")
        .file("/tmp/sub/deep.txt", "data")
        .build();
    let r = shell.exec("find /tmp -mindepth 2 -name '*.txt'").unwrap();
    assert_eq!(r.stdout.trim(), "/tmp/sub/deep.txt");
}

#[test]
fn find_delete() {
    let mut shell = Shell::new();
    shell.exec("echo data > /tmp/todelete.txt").unwrap();
    shell.exec("find /tmp -name 'todelete.txt' -delete").unwrap();
    let r = shell
        .exec("[[ -f /tmp/todelete.txt ]] && echo exists || echo gone")
        .unwrap();
    assert_eq!(r.stdout.trim(), "gone");
}

#[test]
fn find_print0() {
    let mut shell = Shell::builder()
        .file("/tmp/a.txt", "data")
        .build();
    let r = shell.exec("find /tmp -name '*.txt' -print0").unwrap();
    assert!(r.stdout.contains('\0'));
    assert!(r.stdout.contains("a.txt"));
}

#[test]
fn find_path() {
    let mut shell = Shell::builder()
        .file("/tmp/sub/deep.txt", "data")
        .file("/tmp/other.txt", "data")
        .build();
    let r = shell.exec("find /tmp -path '/tmp/sub/*'").unwrap();
    assert_eq!(r.stdout.trim(), "/tmp/sub/deep.txt");
}

#[test]
fn find_size() {
    let mut shell = Shell::builder()
        .file("/tmp/nonempty.txt", "hello")
        .build();
    shell.exec("touch /tmp/zero.txt").unwrap();
    let r = shell.exec("find /tmp -type f -size +0c").unwrap();
    assert!(r.stdout.contains("nonempty.txt"));
    assert!(!r.stdout.contains("zero.txt"));
}

#[test]
fn find_no_results() {
    let mut shell = Shell::new();
    shell.exec("mkdir -p /tmp/emptydir").unwrap();
    let r = shell.exec("find /tmp/emptydir -type f").unwrap();
    assert_eq!(r.stdout, "");
    assert_eq!(r.exit_code, 0);
}

#[test]
fn which_echo() {
    let mut shell = Shell::new();
    let r = shell.exec("which echo").unwrap();
    assert_eq!(r.exit_code, 0);
    assert_eq!(r.stdout.trim(), "/usr/bin/echo");
}

#[test]
fn which_not_found() {
    let mut shell = Shell::new();
    let r = shell.exec("which nonexistent_xyz").unwrap();
    assert_eq!(r.exit_code, 1);
    assert_eq!(r.stdout, "");
}

#[test]
fn which_path_command() {
    let mut shell = Shell::new();
    let r = shell.exec("which cat").unwrap();
    assert_eq!(r.exit_code, 0);
    assert_eq!(r.stdout.trim(), "/usr/bin/cat");
}
