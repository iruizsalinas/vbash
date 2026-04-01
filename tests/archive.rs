use vbash::Shell;

#[test]
fn tar_create_extract() {
    let mut shell = Shell::builder()
        .file("/tmp/src/a.txt", "alpha")
        .file("/tmp/src/b.txt", "bravo")
        .build();
    shell.exec("tar cf /tmp/archive.tar /tmp/src/a.txt /tmp/src/b.txt").unwrap();
    shell.exec("rm /tmp/src/a.txt /tmp/src/b.txt").unwrap();
    shell.exec("tar xf /tmp/archive.tar").unwrap();
    let a = shell.exec("cat /tmp/src/a.txt").unwrap();
    let b = shell.exec("cat /tmp/src/b.txt").unwrap();
    assert_eq!(a.stdout, "alpha");
    assert_eq!(b.stdout, "bravo");
}

#[test]
fn tar_list() {
    let mut shell = Shell::builder()
        .file("/tmp/src/a.txt", "alpha")
        .file("/tmp/src/b.txt", "bravo")
        .build();
    shell.exec("tar cf /tmp/archive.tar /tmp/src/a.txt /tmp/src/b.txt").unwrap();
    let r = shell.exec("tar tf /tmp/archive.tar").unwrap();
    let mut lines: Vec<&str> = r.stdout.trim().lines().collect();
    lines.sort_unstable();
    assert_eq!(lines, vec!["/tmp/src/a.txt", "/tmp/src/b.txt"]);
}

#[test]
fn tar_gzip() {
    let mut shell = Shell::builder()
        .file("/tmp/src/file.txt", "content")
        .build();
    shell.exec("tar czf /tmp/archive.tar.gz /tmp/src").unwrap();
    let r = shell.exec("tar tzf /tmp/archive.tar.gz").unwrap();
    assert!(r.stdout.contains("file.txt"));
    assert_eq!(r.exit_code, 0);
}

#[test]
fn tar_verbose() {
    let mut shell = Shell::builder()
        .file("/tmp/src/v.txt", "verbose")
        .build();
    shell.exec("tar cf /tmp/archive.tar /tmp/src/v.txt").unwrap();
    let r = shell.exec("tar tvf /tmp/archive.tar").unwrap();
    let line = r.stdout.trim();
    assert!(line.contains("v.txt"));
    assert!(
        line.starts_with('-') || line.starts_with('d'),
        "verbose listing should start with file type char, got: {line}"
    );
    assert!(
        line.contains("rw") || line.contains("r--"),
        "verbose listing should contain permission bits, got: {line}"
    );
}

#[test]
fn tar_change_dir() {
    let mut shell = Shell::builder()
        .file("/tmp/cdir/hello.txt", "world")
        .build();
    shell.exec("tar cf /tmp/archive.tar -C /tmp/cdir hello.txt").unwrap();
    let r = shell.exec("tar tf /tmp/archive.tar").unwrap();
    assert_eq!(r.stdout.trim(), "hello.txt");
}

#[test]
fn tar_strip() {
    let mut shell = Shell::builder()
        .file("/tmp/src/sub/strip.txt", "stripped")
        .build();
    shell.exec("tar cf /tmp/archive.tar /tmp/src/sub/strip.txt").unwrap();
    shell.exec("mkdir -p /tmp/out").unwrap();
    shell.exec("tar --strip-components=1 -x -f /tmp/archive.tar -C /tmp/out").unwrap();
    let r = shell.exec("find /tmp/out -name 'strip.txt'").unwrap();
    assert!(r.stdout.contains("strip.txt"));
}

#[test]
fn tar_multiple() {
    let mut shell = Shell::builder()
        .file("/tmp/m1.txt", "one")
        .file("/tmp/m2.txt", "two")
        .file("/tmp/m3.txt", "three")
        .build();
    shell.exec("tar cf /tmp/archive.tar /tmp/m1.txt /tmp/m2.txt /tmp/m3.txt").unwrap();
    let r = shell.exec("tar tf /tmp/archive.tar").unwrap();
    let mut lines: Vec<&str> = r.stdout.trim().lines().collect();
    lines.sort_unstable();
    assert_eq!(lines, vec!["/tmp/m1.txt", "/tmp/m2.txt", "/tmp/m3.txt"]);
}

#[test]
fn gzip_compress_decompress() {
    let mut shell = Shell::new();
    shell.exec("echo hello > /tmp/gz.txt").unwrap();
    shell.exec("gzip /tmp/gz.txt").unwrap();
    shell.exec("gunzip /tmp/gz.txt.gz").unwrap();
    let r = shell.exec("cat /tmp/gz.txt").unwrap();
    assert_eq!(r.stdout.trim(), "hello");
}

#[test]
fn gzip_stdout() {
    let mut shell = Shell::new();
    shell.exec("echo hello > /tmp/gzstdout.txt").unwrap();
    let r = shell.exec("gzip -c /tmp/gzstdout.txt").unwrap();
    assert_eq!(r.exit_code, 0);
    assert!(!r.stdout.is_empty());
}

#[test]
fn gzip_keep() {
    let mut shell = Shell::new();
    shell.exec("echo hello > /tmp/gzkeep.txt").unwrap();
    shell.exec("gzip -k /tmp/gzkeep.txt").unwrap();
    let r = shell
        .exec("[[ -f /tmp/gzkeep.txt ]] && echo exists")
        .unwrap();
    assert_eq!(r.stdout.trim(), "exists");
    let r2 = shell
        .exec("[[ -f /tmp/gzkeep.txt.gz ]] && echo exists")
        .unwrap();
    assert_eq!(r2.stdout.trim(), "exists");
}

#[test]
fn zcat_basic() {
    let mut shell = Shell::new();
    shell.exec("echo 'zcat content' > /tmp/zc.txt").unwrap();
    shell.exec("gzip -k /tmp/zc.txt").unwrap();
    let r = shell.exec("zcat /tmp/zc.txt.gz").unwrap();
    assert_eq!(r.stdout.trim(), "zcat content");
    let check = shell
        .exec("[[ -f /tmp/zc.txt.gz ]] && echo exists")
        .unwrap();
    assert_eq!(check.stdout.trim(), "exists");
}

#[test]
fn tar_nonexistent_file() {
    let mut shell = Shell::new();
    let r = shell.exec("tar cf /tmp/bad.tar /tmp/nosuchfile.txt").unwrap();
    assert_eq!(r.exit_code, 2);
    assert!(r.stderr.contains("tar:"));
}

#[test]
fn gzip_empty_file() {
    let mut shell = Shell::new();
    shell.exec("touch /tmp/empty.txt").unwrap();
    shell.exec("gzip /tmp/empty.txt").unwrap();
    shell.exec("gunzip /tmp/empty.txt.gz").unwrap();
    let r = shell.exec("cat /tmp/empty.txt").unwrap();
    assert_eq!(r.stdout, "");
    assert_eq!(r.exit_code, 0);
}
