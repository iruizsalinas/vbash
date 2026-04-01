use vbash::{InMemoryFs, Shell, VirtualFs};

fn main() {
    let fs = InMemoryFs::new();
    fs.mkdir("/project", true).unwrap();
    fs.mkdir("/project/src", true).unwrap();
    fs.write_file("/project/src/main.rs", b"fn main() {}").unwrap();
    fs.write_file("/project/src/lib.rs", b"pub fn hello() {}").unwrap();
    fs.write_file("/project/Cargo.toml", b"[package]\nname = \"demo\"").unwrap();
    fs.write_file("/project/README.md", b"# Demo Project").unwrap();

    let mut shell = Shell::builder()
        .fs(fs)
        .cwd("/project")
        .build();

    let r = shell.exec("find . -type f | sort").unwrap();
    println!("files:\n{}", r.stdout);

    let r = shell.exec("grep -r 'fn' src/").unwrap();
    println!("grep results:\n{}", r.stdout);

    shell.exec(r#"
        mkdir -p /project/docs
        echo "API documentation" > /project/docs/api.md
        ls -la /project/docs/
    "#).unwrap();

    let content = shell.read_file("/project/docs/api.md").unwrap();
    println!("api.md: {content}");
}
