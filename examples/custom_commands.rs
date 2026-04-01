use std::collections::HashMap;
use std::fmt::Write;
use vbash::{CommandContext, ExecResult, Shell};

fn uptime_cmd(_args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, vbash::Error> {
    let mut stdout = String::new();
    let _ = writeln!(stdout, "up 42 days, 3:14");
    let _ = writeln!(stdout, "load average: 0.01, 0.02, 0.00");
    let file_count = ctx.fs.readdir("/tmp").map_or(0, |e| e.len());
    let _ = writeln!(stdout, "files in /tmp: {file_count}");
    Ok(ExecResult {
        stdout,
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
    })
}

fn main() {
    let mut shell = Shell::builder()
        .command("uptime", uptime_cmd)
        .build();

    let r = shell.exec("uptime").unwrap();
    print!("{}", r.stdout);

    let r = shell.exec("uptime | grep load").unwrap();
    print!("{}", r.stdout);
}
