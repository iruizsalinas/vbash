use vbash::Shell;

// ls

#[test]
fn ls_basic_sorted() {
    let mut shell = Shell::builder()
        .file("/tmp/b.txt", "world")
        .file("/tmp/a.txt", "hello")
        .build();
    let r = shell.exec("ls -1 /tmp").unwrap();
    assert_eq!(r.stdout, "a.txt\nb.txt\n");
}

#[test]
fn ls_long_format_permission_prefix() {
    let mut shell = Shell::builder()
        .file("/tmp/file.txt", "data")
        .build();
    let r = shell.exec("ls -l /tmp").unwrap();
    let line = r.stdout.lines().next().unwrap();
    assert!(line.starts_with("-rw"), "expected permission prefix, got: {line}");
    assert!(line.ends_with("file.txt"), "expected filename at end, got: {line}");
}

#[test]
fn ls_long_format_directory_prefix() {
    let mut shell = Shell::builder()
        .file("/tmp/sub/child.txt", "x")
        .build();
    let r = shell.exec("ls -l /tmp").unwrap();
    let line = r.stdout.lines().next().unwrap();
    assert!(line.starts_with('d'), "expected dir prefix 'd', got: {line}");
}

#[test]
fn ls_all_hidden_included() {
    let mut shell = Shell::builder()
        .file("/tmp/.hidden", "secret")
        .file("/tmp/visible", "public")
        .build();
    let without = shell.exec("ls -1 /tmp").unwrap();
    assert_eq!(without.stdout, "visible\n");
    let with = shell.exec("ls -1 -a /tmp").unwrap();
    let lines: Vec<&str> = with.stdout.lines().collect();
    assert!(lines.contains(&".hidden"));
    assert!(lines.contains(&"visible"));
}

#[test]
fn ls_sort_by_size_order() {
    let mut shell = Shell::builder()
        .file("/tmp/small.txt", "a")
        .file("/tmp/big.txt", "aaaaabbbbb")
        .build();
    let r = shell.exec("ls -1 -S /tmp").unwrap();
    assert_eq!(r.stdout, "big.txt\nsmall.txt\n");
}

#[test]
fn ls_reverse_order() {
    let mut shell = Shell::builder()
        .file("/tmp/aaa.txt", "1")
        .file("/tmp/zzz.txt", "2")
        .build();
    let r = shell.exec("ls -1 -r /tmp").unwrap();
    assert_eq!(r.stdout, "zzz.txt\naaa.txt\n");
}

#[test]
fn ls_classify_dir_and_symlink() {
    let mut shell = Shell::builder()
        .file("/tmp/dir1/child.txt", "x")
        .file("/tmp/file.txt", "y")
        .build();
    shell.exec("ln -s /tmp/file.txt /tmp/link").unwrap();
    let r = shell.exec("ls -1 -F /tmp").unwrap();
    let lines: Vec<&str> = r.stdout.lines().collect();
    assert!(lines.contains(&"dir1/"));
    assert!(lines.contains(&"link@"));
    assert!(lines.contains(&"file.txt"));
}

#[test]
fn ls_one_per_line_count() {
    let mut shell = Shell::builder()
        .file("/tmp/a", "1")
        .file("/tmp/b", "2")
        .file("/tmp/c", "3")
        .build();
    let r = shell.exec("ls -1 /tmp").unwrap();
    assert_eq!(r.stdout, "a\nb\nc\n");
}

#[test]
fn ls_directory_flag() {
    let mut shell = Shell::builder()
        .file("/tmp/inner/f.txt", "data")
        .build();
    let r = shell.exec("ls -d /tmp").unwrap();
    assert_eq!(r.stdout.trim(), "tmp");
}

#[test]
fn ls_recursive_lists_subdirs() {
    let mut shell = Shell::builder()
        .file("/tmp/d1/f1.txt", "one")
        .file("/tmp/d2/f2.txt", "two")
        .build();
    let r = shell.exec("ls -1 -R /tmp").unwrap();
    let lines: Vec<&str> = r.stdout.lines().collect();
    assert!(lines.contains(&"/tmp:"));
    assert!(lines.contains(&"f1.txt"));
    assert!(lines.contains(&"f2.txt"));
}

#[test]
fn ls_nonexistent_fails() {
    let mut shell = Shell::new();
    let r = shell.exec("ls /no/such/path").unwrap();
    assert_ne!(r.exit_code, 0);
    assert!(!r.stderr.is_empty());
}

// stat

#[test]
fn stat_default_output_fields() {
    let mut shell = Shell::builder()
        .file("/tmp/f.txt", "hello")
        .build();
    let r = shell.exec("stat /tmp/f.txt").unwrap();
    let lines: Vec<&str> = r.stdout.lines().collect();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0], "  File: /tmp/f.txt");
    assert!(lines[1].starts_with("  Size: 5\t"));
    assert!(lines[2].starts_with("Access: ("));
}

#[test]
fn stat_format_size_and_name() {
    let mut shell = Shell::builder()
        .file("/tmp/f.txt", "hello")
        .build();
    let r = shell.exec("stat -c '%s %n' /tmp/f.txt").unwrap();
    assert_eq!(r.stdout, "5 /tmp/f.txt\n");
}

#[test]
fn stat_format_permissions() {
    let mut shell = Shell::builder()
        .file("/tmp/f.txt", "data")
        .build();
    shell.exec("chmod 755 /tmp/f.txt").unwrap();
    let r = shell.exec("stat -c '%a' /tmp/f.txt").unwrap();
    assert_eq!(r.stdout, "755\n");
}

#[test]
fn stat_nonexistent_fails() {
    let mut shell = Shell::new();
    let r = shell.exec("stat /no/such/file").unwrap();
    assert_ne!(r.exit_code, 0);
    assert!(!r.stderr.is_empty());
}

// chmod

#[test]
fn chmod_numeric() {
    let mut shell = Shell::builder()
        .file("/tmp/f.txt", "data")
        .build();
    shell.exec("chmod 755 /tmp/f.txt").unwrap();
    let r = shell.exec("stat -c '%a' /tmp/f.txt").unwrap();
    assert_eq!(r.stdout, "755\n");
}

#[test]
fn chmod_recursive() {
    let mut shell = Shell::builder()
        .file("/tmp/d/a.txt", "1")
        .file("/tmp/d/b.txt", "2")
        .build();
    shell.exec("chmod -R 700 /tmp/d").unwrap();
    let ra = shell.exec("stat -c '%a' /tmp/d/a.txt").unwrap();
    let rb = shell.exec("stat -c '%a' /tmp/d/b.txt").unwrap();
    assert_eq!(ra.stdout, "700\n");
    assert_eq!(rb.stdout, "700\n");
}

#[test]
fn chmod_invalid_mode_fails() {
    let mut shell = Shell::builder()
        .file("/tmp/f.txt", "data")
        .build();
    let r = shell.exec("chmod xyz /tmp/f.txt").unwrap();
    assert_ne!(r.exit_code, 0);
    assert!(r.stderr.contains("invalid mode"));
}

// ln / readlink

#[test]
fn ln_hard_link_shares_content() {
    let mut shell = Shell::builder()
        .file("/tmp/orig.txt", "content")
        .build();
    shell.exec("ln /tmp/orig.txt /tmp/link.txt").unwrap();
    let r = shell.exec("cat /tmp/link.txt").unwrap();
    assert_eq!(r.stdout, "content");
}

#[test]
fn ln_symbolic_readlink() {
    let mut shell = Shell::builder()
        .file("/tmp/target.txt", "data")
        .build();
    shell.exec("ln -s /tmp/target.txt /tmp/slink").unwrap();
    let r = shell.exec("readlink /tmp/slink").unwrap();
    assert_eq!(r.stdout, "/tmp/target.txt\n");
}

#[test]
fn ln_force_overwrites() {
    let mut shell = Shell::builder()
        .file("/tmp/t1.txt", "first")
        .file("/tmp/t2.txt", "second")
        .file("/tmp/mylink", "placeholder")
        .build();
    shell.exec("ln -s -f /tmp/t2.txt /tmp/mylink").unwrap();
    let r = shell.exec("readlink /tmp/mylink").unwrap();
    assert_eq!(r.stdout, "/tmp/t2.txt\n");
}

#[test]
fn readlink_canonicalize() {
    let mut shell = Shell::builder()
        .file("/tmp/real.txt", "data")
        .build();
    shell.exec("ln -s /tmp/real.txt /tmp/sym1").unwrap();
    let r = shell.exec("readlink -f /tmp/sym1").unwrap();
    assert_eq!(r.stdout, "/tmp/real.txt\n");
}

#[test]
fn ln_missing_operand_fails() {
    let mut shell = Shell::new();
    let r = shell.exec("ln").unwrap();
    assert_ne!(r.exit_code, 0);
    assert!(r.stderr.contains("missing operand"));
}

// rmdir

#[test]
fn rmdir_empty_dir() {
    let mut shell = Shell::new();
    shell.exec("mkdir /tmp/emptydir").unwrap();
    let r = shell.exec("rmdir /tmp/emptydir").unwrap();
    assert_eq!(r.exit_code, 0);
    let check = shell.exec("ls /tmp/emptydir").unwrap();
    assert_ne!(check.exit_code, 0);
}

#[test]
fn rmdir_nonempty_fails() {
    let mut shell = Shell::builder()
        .file("/tmp/notempty/f.txt", "data")
        .build();
    let r = shell.exec("rmdir /tmp/notempty").unwrap();
    assert_ne!(r.exit_code, 0);
}

#[test]
fn rmdir_parents() {
    let mut shell = Shell::new();
    shell.exec("mkdir -p /tmp/a/b/c").unwrap();
    let r = shell.exec("rmdir -p /tmp/a/b/c").unwrap();
    assert_eq!(r.exit_code, 0);
}

// tree

#[test]
fn tree_basic_structure() {
    let mut shell = Shell::builder()
        .file("/tmp/tree/a.txt", "1")
        .file("/tmp/tree/sub/b.txt", "2")
        .build();
    let r = shell.exec("tree /tmp/tree").unwrap();
    let lines: Vec<&str> = r.stdout.lines().collect();
    assert_eq!(lines[0], "/tmp/tree");
    assert!(lines.iter().any(|l| l.contains("a.txt")));
    assert!(lines.iter().any(|l| l.contains("sub")));
    assert!(lines.iter().any(|l| l.contains("b.txt")));
    assert!(lines.last().unwrap().contains("directories"));
}

#[test]
fn tree_depth_limit() {
    let mut shell = Shell::builder()
        .file("/tmp/tree/a/b/deep.txt", "x")
        .build();
    let r = shell.exec("tree -L 1 /tmp/tree").unwrap();
    assert!(r.stdout.contains('a'));
    assert!(!r.stdout.contains("deep.txt"));
}

#[test]
fn tree_dirs_only_no_files() {
    let mut shell = Shell::builder()
        .file("/tmp/tree/sub/f.txt", "x")
        .build();
    let r = shell.exec("tree -d /tmp/tree").unwrap();
    assert!(r.stdout.contains("sub"));
    assert!(!r.stdout.contains("f.txt"));
    let last = r.stdout.lines().last().unwrap();
    assert!(last.contains("directories"));
    assert!(!last.contains("files"));
}

// file

#[test]
fn file_text_detection() {
    let mut shell = Shell::builder()
        .file("/tmp/t.txt", "hello world")
        .build();
    let r = shell.exec("file /tmp/t.txt").unwrap();
    assert_eq!(r.stdout, "/tmp/t.txt: ASCII text\n");
}

#[test]
fn file_directory_detection() {
    let mut shell = Shell::new();
    shell.exec("mkdir /tmp/mydir").unwrap();
    let r = shell.exec("file /tmp/mydir").unwrap();
    assert_eq!(r.stdout, "/tmp/mydir: directory\n");
}

#[test]
fn file_empty_detection() {
    let mut shell = Shell::builder()
        .file("/tmp/empty.txt", "")
        .build();
    let r = shell.exec("file /tmp/empty.txt").unwrap();
    assert_eq!(r.stdout, "/tmp/empty.txt: empty\n");
}

#[test]
fn file_json_detection() {
    let mut shell = Shell::builder()
        .file("/tmp/data.json", "{\"key\": \"value\"}")
        .build();
    let r = shell.exec("file /tmp/data.json").unwrap();
    assert_eq!(r.stdout, "/tmp/data.json: JSON text\n");
}

#[test]
fn file_missing_operand_fails() {
    let mut shell = Shell::new();
    let r = shell.exec("file").unwrap();
    assert_ne!(r.exit_code, 0);
    assert_eq!(r.stderr, "file: missing operand\n");
}

#[test]
fn file_nonexistent_fails() {
    let mut shell = Shell::new();
    let r = shell.exec("file /no/such/file").unwrap();
    assert_ne!(r.exit_code, 0);
    assert!(!r.stderr.is_empty());
}

// split

#[test]
fn split_by_lines() {
    let mut shell = Shell::builder()
        .file("/tmp/data.txt", "1\n2\n3\n4\n5\n6\n")
        .build();
    shell.exec("split -l 2 /tmp/data.txt /tmp/out").unwrap();
    let r1 = shell.exec("cat /tmp/outaa").unwrap();
    assert_eq!(r1.stdout, "1\n2\n");
    let r2 = shell.exec("cat /tmp/outab").unwrap();
    assert_eq!(r2.stdout, "3\n4\n");
    let r3 = shell.exec("cat /tmp/outac").unwrap();
    assert_eq!(r3.stdout, "5\n6\n");
}

#[test]
fn split_by_bytes() {
    let mut shell = Shell::builder()
        .file("/tmp/data.txt", "abcdefghijklmnopqrstuvwxyz")
        .build();
    shell.exec("split -b 10 /tmp/data.txt /tmp/s").unwrap();
    let r1 = shell.exec("cat /tmp/saa").unwrap();
    assert_eq!(r1.stdout, "abcdefghij");
    let r2 = shell.exec("cat /tmp/sab").unwrap();
    assert_eq!(r2.stdout, "klmnopqrst");
}

// basename / dirname

#[test]
fn basename_path() {
    let mut shell = Shell::new();
    let r = shell.exec("basename /a/b/c.txt").unwrap();
    assert_eq!(r.stdout, "c.txt\n");
}

#[test]
fn basename_suffix_removal() {
    let mut shell = Shell::new();
    let r = shell.exec("basename /a/b/c.txt .txt").unwrap();
    assert_eq!(r.stdout, "c\n");
}

#[test]
fn dirname_path() {
    let mut shell = Shell::new();
    let r = shell.exec("dirname /a/b/c.txt").unwrap();
    assert_eq!(r.stdout, "/a/b\n");
}

// tail

#[test]
fn tail_default_last_10() {
    let content = (1..=15).map(|i| format!("line{i}")).collect::<Vec<_>>().join("\n");
    let mut shell = Shell::builder()
        .file("/tmp/f.txt", &content)
        .build();
    let r = shell.exec("tail /tmp/f.txt").unwrap();
    let lines: Vec<&str> = r.stdout.lines().collect();
    assert_eq!(lines.len(), 10);
    assert_eq!(lines[0], "line6");
    assert_eq!(lines[9], "line15");
}

#[test]
fn tail_n_specific() {
    let content = (1..=10).map(|i| format!("line{i}")).collect::<Vec<_>>().join("\n");
    let mut shell = Shell::builder()
        .file("/tmp/f.txt", &content)
        .build();
    let r = shell.exec("tail -n 3 /tmp/f.txt").unwrap();
    assert_eq!(r.stdout, "line8\nline9\nline10\n");
}

#[test]
fn tail_from_start() {
    let mut shell = Shell::builder()
        .file("/tmp/f.txt", "a\nb\nc\nd\ne")
        .build();
    let r = shell.exec("tail -n +2 /tmp/f.txt").unwrap();
    assert_eq!(r.stdout, "b\nc\nd\ne\n");
}
