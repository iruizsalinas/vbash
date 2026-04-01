use vbash::Shell;

fn main() {
    let mut shell = Shell::new();

    let r = shell.exec("echo hello world").unwrap();
    println!("{}", r.stdout.trim());

    // filesystem persists between calls
    shell.exec("echo 'some data' > /tmp/out.txt").unwrap();
    let r = shell.exec("cat /tmp/out.txt").unwrap();
    println!("{}", r.stdout.trim());

    // pipelines, variables, loops all work
    let r = shell.exec(r#"
        for lang in rust python go; do
            echo "$lang"
        done | sort | head -2
    "#).unwrap();
    print!("{}", r.stdout);
}
