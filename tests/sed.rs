use vbash::Shell;


#[test]
fn sed_substitute_basic() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'hello world' | sed 's/world/earth/'").unwrap();
    assert_eq!(r.stdout, "hello earth\n");
}

#[test]
fn sed_substitute_global() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'aaa' | sed 's/a/b/g'").unwrap();
    assert_eq!(r.stdout, "bbb\n");
}

#[test]
fn sed_substitute_case_insensitive() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'Hello' | sed 's/hello/hi/I'").unwrap();
    assert_eq!(r.stdout, "hi\n");
}

#[test]
fn sed_substitute_backreference() {
    let mut shell = Shell::new();
    let r = shell.exec(r"echo 'abc' | sed 's/\(.\)\(.\)/\2\1/'").unwrap();
    assert_eq!(r.stdout, "bac\n");
}

#[test]
fn sed_substitute_nth() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'aaa' | sed 's/a/b/2'").unwrap();
    assert_eq!(r.stdout, "aba\n");
}


#[test]
fn sed_delete() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "a\nb\nc\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | sed '2d'").unwrap();
    assert_eq!(r.stdout, "a\nc\n");
}

#[test]
fn sed_delete_range() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "a\nb\nc\nd\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | sed '2,3d'").unwrap();
    assert_eq!(r.stdout, "a\nd\n");
}

#[test]
fn sed_delete_pattern() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "foo\nbar\nbaz\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | sed '/bar/d'").unwrap();
    assert_eq!(r.stdout, "foo\nbaz\n");
}


#[test]
fn sed_print_with_suppress() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "a\nb\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | sed -n '2p'").unwrap();
    assert_eq!(r.stdout, "b\n");
}

#[test]
fn sed_print_line_number() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "a\nb\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | sed -n '='").unwrap();
    assert_eq!(r.stdout, "1\n2\n");
}


#[test]
fn sed_append() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "a\nb\n")
        .build();
    let r = shell.exec(r"cat /tmp/input.txt | sed '1a\inserted'").unwrap();
    assert_eq!(r.stdout, "a\ninserted\nb\n");
}

#[test]
fn sed_insert() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "a\nb\n")
        .build();
    let r = shell.exec(r"cat /tmp/input.txt | sed '1i\before'").unwrap();
    assert_eq!(r.stdout, "before\na\nb\n");
}

#[test]
fn sed_change() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "a\nb\nc\n")
        .build();
    let r = shell.exec(r"cat /tmp/input.txt | sed '2c\replaced'").unwrap();
    assert_eq!(r.stdout, "a\nreplaced\nc\n");
}


#[test]
fn sed_hold_exchange() {
    // Copy pattern to hold, replace pattern with X, print X, retrieve hold
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "a\nb\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | sed -n '{h;s/.*/X/;p;g;p}'").unwrap();
    // For each line: prints X then the original line
    assert_eq!(r.stdout, "X\na\nX\nb\n");
}

#[test]
fn sed_get_append() {
    // Join all lines via hold space
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "a\nb\nc\n")
        .build();
    let r = shell
        .exec(r"cat /tmp/input.txt | sed -n 'H;${x;s/^\n//;p}'")
        .unwrap();
    assert_eq!(r.stdout, "a\nb\nc\n");
}


#[test]
fn sed_line_address() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "a\nb\nc\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | sed '2s/b/B/'").unwrap();
    assert_eq!(r.stdout, "a\nB\nc\n");
}

#[test]
fn sed_regex_address() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "foo\nbar\nbaz\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | sed '/^b/s/b/B/'").unwrap();
    assert_eq!(r.stdout, "foo\nBar\nBaz\n");
}

#[test]
fn sed_last_line() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "a\nb\nc\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | sed '$s/c/C/'").unwrap();
    assert_eq!(r.stdout, "a\nb\nC\n");
}

#[test]
fn sed_range() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "a\nb\nc\nd\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | sed '2,3s/.*/X/'").unwrap();
    assert_eq!(r.stdout, "a\nX\nX\nd\n");
}


#[test]
fn sed_transliterate() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'hello' | sed 'y/helo/HELO/'").unwrap();
    assert_eq!(r.stdout, "HELLO\n");
}


#[test]
fn sed_branch_unconditional() {
    // Join all lines with spaces using N + branch loop
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "a\nb\nc\n")
        .build();
    let r = shell
        .exec(r"cat /tmp/input.txt | sed ':l;N;$!b l;s/\n/ /g'")
        .unwrap();
    assert_eq!(r.stdout, "a b c\n");
}

#[test]
fn sed_quit() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "a\nb\nc\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | sed '2q'").unwrap();
    assert_eq!(r.stdout, "a\nb\n");
}


#[test]
fn sed_multiple_e() {
    let mut shell = Shell::new();
    let r = shell
        .exec("echo 'abc' | sed -e 's/a/A/' -e 's/c/C/'")
        .unwrap();
    assert_eq!(r.stdout, "AbC\n");
}


#[test]
fn sed_in_place() {
    let mut shell = Shell::builder()
        .file("/tmp/data.txt", "old value\n")
        .build();
    shell.exec("sed -i 's/old/new/' /tmp/data.txt").unwrap();
    let content = shell.read_file("/tmp/data.txt").unwrap();
    assert_eq!(content, "new value");
}


#[test]
fn sed_extended_regex() {
    let mut shell = Shell::new();
    let r = shell
        .exec(r"echo 'aabb' | sed -E 's/(a+)(b+)/\2\1/'")
        .unwrap();
    assert_eq!(r.stdout, "bbaa\n");
}


#[test]
fn sed_substitute_with_different_delimiter() {
    let mut shell = Shell::new();
    let r = shell.exec("echo '/usr/bin' | sed 's|/usr/bin|/opt/bin|'").unwrap();
    assert_eq!(r.stdout, "/opt/bin\n");
}

#[test]
fn sed_negated_address() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "a\nb\nc\n")
        .build();
    // Delete all lines except line 2
    let r = shell.exec("cat /tmp/input.txt | sed '2!d'").unwrap();
    assert_eq!(r.stdout, "b\n");
}

#[test]
fn sed_substitute_empty_pattern_deletes() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'hello world' | sed 's/world//'").unwrap();
    assert_eq!(r.stdout, "hello \n");
}

#[test]
fn sed_substitute_ampersand() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'hello' | sed 's/hello/[&]/'").unwrap();
    assert_eq!(r.stdout, "[hello]\n");
}

#[test]
fn sed_multiple_commands_semicolon() {
    let mut shell = Shell::new();
    let r = shell.exec("echo 'abc' | sed 's/a/A/;s/b/B/'").unwrap();
    assert_eq!(r.stdout, "ABc\n");
}

#[test]
fn sed_print_duplicate() {
    // Without -n, 'p' causes each line to appear twice
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "hello\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | sed 'p'").unwrap();
    assert_eq!(r.stdout, "hello\nhello\n");
}

#[test]
fn sed_from_file() {
    let mut shell = Shell::builder()
        .file("/tmp/input.txt", "hello world\n")
        .file("/tmp/script.sed", "s/world/earth/\n")
        .build();
    let r = shell.exec("cat /tmp/input.txt | sed -f /tmp/script.sed").unwrap();
    assert_eq!(r.stdout, "hello earth\n");
}

// === Edge case tests ===

use vbash::ExecOptions;

fn sed(script: &str, input: &str) -> String {
    let mut shell = Shell::new();
    let cmd = format!("sed '{script}'");
    let r = shell
        .exec_with(
            &cmd,
            ExecOptions {
                stdin: Some(input),
                ..Default::default()
            },
        )
        .unwrap();
    r.stdout
}

fn sed_file(script: &str, content: &str) -> String {
    let mut shell = Shell::builder()
        .file("/tmp/data.txt", content)
        .build();
    let cmd = format!("sed '{script}' /tmp/data.txt");
    let r = shell.exec(&cmd).unwrap();
    r.stdout
}

fn sed_cmd(full_cmd: &str, input: &str) -> String {
    let mut shell = Shell::new();
    let r = shell
        .exec_with(
            full_cmd,
            ExecOptions {
                stdin: Some(input),
                ..Default::default()
            },
        )
        .unwrap();
    r.stdout
}

// Basic edge cases

#[test]
fn sed_empty_input() {
    assert_eq!(sed("s/x/y/", ""), "");
}

#[test]
fn sed_no_match() {
    assert_eq!(sed("s/xyz/abc/", "hello\n"), "hello\n");
}

#[test]
fn sed_substitute_all_occurrences() {
    assert_eq!(sed("s/a/b/g", "aaa\n"), "bbb\n");
}

#[test]
fn sed_empty_replacement() {
    assert_eq!(sed("s/l//g", "hello\n"), "heo\n");
}

#[test]
fn sed_delete_all() {
    assert_eq!(sed("d", "a\nb\nc\n"), "");
}

#[test]
fn sed_print_without_n() {
    assert_eq!(sed("p", "hello\n"), "hello\nhello\n");
}

// Address edge cases

#[test]
fn sed_last_line_dollar() {
    assert_eq!(sed_file("$d", "a\nb\nc\n"), "a\nb\n");
}

#[test]
fn sed_negated_address_edge() {
    assert_eq!(sed_file("2!d", "a\nb\nc\n"), "b\n");
}

#[test]
fn sed_regex_address_edge() {
    let out = sed("/.\\../ s/a/A/", "a.b\nccc\n");
    assert_eq!(out, "A.b\nccc\n");
}

#[test]
fn sed_range_same_line() {
    assert_eq!(sed_file("2,2d", "a\nb\nc\n"), "a\nc\n");
}

#[test]
fn sed_line_address_edge() {
    assert_eq!(sed_file("2d", "a\nb\nc\n"), "a\nc\n");
}

// Hold space

#[test]
fn sed_hold_get() {
    let out = sed_cmd("sed -n '1h;2{x;p}'", "first\nsecond\n");
    assert_eq!(out, "first\n");
}

#[test]
fn sed_hold_append_get() {
    let out = sed_cmd("sed -n 'H;${x;p}'", "a\nb\nc\n");
    // H appends to hold (initially empty), so hold = "\na\nb\nc"
    // x swaps, p prints. Output includes leading newline.
    assert_eq!(out, "\na\nb\nc\n");
}

#[test]
fn sed_exchange() {
    // Line 1: x swaps pattern(a)->hold, hold("")->pattern. -n suppresses.
    // Line 2: x swaps pattern(b)->hold, hold(a)->pattern. Then {x;p}: x swaps back, p prints b.
    let out = sed_cmd("sed -n 'x;2{x;p}'", "a\nb\n");
    assert_eq!(out, "b\n");
}

// N command (append next line)

#[test]
fn sed_n_join_lines() {
    let out = sed("N;s/\\n/ /", "a\nb\nc\nd\n");
    assert_eq!(out, "a b\nc d\n");
}

#[test]
fn sed_next_line() {
    let out = sed_cmd("sed -n '2{n;p}'", "a\nb\nc\n");
    assert_eq!(out, "c\n");
}

// Multiple commands

#[test]
fn sed_semicolon_commands() {
    assert_eq!(sed("s/a/A/;s/c/C/", "abc\n"), "AbC\n");
}

#[test]
fn sed_braces_grouping() {
    let out = sed_cmd("sed -n '/a/{s/a/A/;p}'", "abc\nxyz\n");
    assert_eq!(out, "Abc\n");
}

// Substitution flags

#[test]
fn sed_substitute_print_flag() {
    let out = sed_cmd("sed -n 's/hello/HELLO/p'", "hello\n");
    assert_eq!(out, "HELLO\n");
}

// Different delimiters

#[test]
fn sed_delimiter_pipe() {
    let out = sed("s|/usr|/opt|", "/usr/bin\n");
    assert_eq!(out, "/opt/bin\n");
}

#[test]
fn sed_delimiter_comma() {
    assert_eq!(sed("s,hello,world,", "hello\n"), "world\n");
}

// Backreferences

#[test]
fn sed_backreference_swap() {
    let out = sed("s/\\(..\\)\\(..\\)/\\2\\1/", "abcd\n");
    assert_eq!(out, "cdab\n");
}

#[test]
fn sed_ampersand_full_match() {
    assert_eq!(sed("s/ell/[&]/", "hello\n"), "h[ell]o\n");
}

// Quit

#[test]
fn sed_quit_early() {
    assert_eq!(sed_file("2q", "a\nb\nc\n"), "a\nb\n");
}

// Extended regex

#[test]
fn sed_ere_plus() {
    let out = sed_cmd("sed -E 's/a+/X/'", "aab\n");
    assert_eq!(out, "Xb\n");
}

#[test]
fn sed_ere_question() {
    let out = sed_cmd("sed -E 's/colou?r/color/'", "colour\n");
    assert_eq!(out, "color\n");
}

#[test]
fn sed_ere_alternation() {
    let out = sed_cmd("sed -E 's/cat|dog/pet/'", "cat\n");
    assert_eq!(out, "pet\n");
}

#[test]
fn sed_ere_groups() {
    let out = sed_cmd("sed -E 's/(hello) (world)/\\2 \\1/'", "hello world\n");
    assert_eq!(out, "world hello\n");
}

// In-place editing

#[test]
fn sed_inplace_modifies_file() {
    let mut shell = Shell::builder()
        .file("/tmp/edit.txt", "hello world\n")
        .build();
    shell.exec("sed -i 's/hello/goodbye/' /tmp/edit.txt").unwrap();
    let content = shell.read_file("/tmp/edit.txt").unwrap();
    assert!(content.contains("goodbye"), "got: {content}");
}

#[test]
fn sed_inplace_no_backup() {
    let mut shell = Shell::builder()
        .file("/tmp/edit2.txt", "aaa\n")
        .build();
    shell.exec("sed -i 's/a/b/g' /tmp/edit2.txt").unwrap();
    let content = shell.read_file("/tmp/edit2.txt").unwrap();
    assert!(content.contains("bbb"), "got: {content}");
}

// Multiline

#[test]
fn sed_multiline_substitute() {
    let out = sed("N;N;s/\\n/,/g", "a\nb\nc\n");
    assert_eq!(out, "a,b,c\n");
}

// Transliterate

#[test]
fn sed_transliterate_edge() {
    assert_eq!(sed("y/abc/ABC/", "abc\n"), "ABC\n");
}

// Insert and Append

#[test]
fn sed_insert_text() {
    let out = sed_file("2i\\inserted", "a\nb\nc\n");
    assert_eq!(out, "a\ninserted\nb\nc\n");
}

#[test]
fn sed_append_text() {
    let out = sed_file("2a\\appended", "a\nb\nc\n");
    assert_eq!(out, "a\nb\nappended\nc\n");
}

// Change command

#[test]
fn sed_change_line() {
    let out = sed_file("2c\\replaced", "a\nb\nc\n");
    assert_eq!(out, "a\nreplaced\nc\n");
}

// Print line number

#[test]
fn sed_print_line_number_edge() {
    let out = sed_cmd("sed -n '2='", "a\nb\nc\n");
    assert_eq!(out, "2\n");
}

// Range address

#[test]
fn sed_range_address() {
    let out = sed_file("2,3d", "a\nb\nc\nd\n");
    assert_eq!(out, "a\nd\n");
}

// Regex range

#[test]
fn sed_regex_range() {
    let out = sed_file("/start/,/end/d", "before\nstart\nmiddle\nend\nafter\n");
    assert_eq!(out, "before\nafter\n");
}

// BRE backreference repeat

#[test]
fn sed_bre_backreference() {
    let out = sed("s/\\(abc\\)/[\\1]/", "xabcy\n");
    assert_eq!(out, "x[abc]y\n");
}

// Suppress output with -n

#[test]
fn sed_suppress_output() {
    let out = sed_cmd("sed -n 'p'", "hello\n");
    assert_eq!(out, "hello\n");
}

#[test]
fn sed_suppress_no_print() {
    let out = sed_cmd("sed -n '2p'", "a\nb\nc\n");
    assert_eq!(out, "b\n");
}

// Step address

#[test]
fn sed_step_address() {
    let out = sed_file("1~2d", "a\nb\nc\nd\ne\n");
    assert_eq!(out, "b\nd\n");
}
