use std::fmt::Write;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;
use std::collections::HashMap;

#[allow(
    clippy::many_single_char_names,
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn compute_diff_edits(lines1: &[&str], lines2: &[&str]) -> Vec<(char, usize)> {
    let len_a = lines1.len();
    let len_b = lines2.len();
    let max = len_a + len_b;
    if max == 0 {
        return Vec::new();
    }

    let mut v = vec![0i64; 2 * max + 1];
    let mut traces: Vec<Vec<i64>> = Vec::new();
    let off = max as i64;

    'outer: for d in 0..=(max as i64) {
        traces.push(v.clone());
        let mut k = -d;
        while k <= d {
            let idx = (k + off) as usize;
            let mut x = if k == -d || (k != d && v[idx - 1] < v[idx + 1]) {
                v[idx + 1]
            } else {
                v[idx - 1] + 1
            };
            let mut y = x - k;
            while (x as usize) < len_a
                && (y as usize) < len_b
                && lines1[x as usize] == lines2[y as usize]
            {
                x += 1;
                y += 1;
            }
            v[idx] = x;
            if x as usize >= len_a && y as usize >= len_b {
                traces.push(v.clone());
                break 'outer;
            }
            k += 2;
        }
    }

    let mut edits: Vec<(char, usize)> = Vec::new();
    let mut cx = len_a as i64;
    let mut cy = len_b as i64;

    for d in (0..(traces.len().saturating_sub(1)) as i64).rev() {
        let k = cx - cy;
        let prev = &traces[d as usize];
        let idx = (k + off) as usize;

        let prev_k = if k == -d
            || (k != d && idx > 0 && idx + 1 < prev.len() && prev[idx - 1] < prev[idx + 1])
        {
            k + 1
        } else {
            k - 1
        };

        let prev_x = prev[(prev_k + off) as usize];
        let prev_y = prev_x - prev_k;

        while cx > prev_x && cy > prev_y {
            cx -= 1;
            cy -= 1;
            edits.push((' ', cx as usize));
        }

        if d > 0 {
            if cx == prev_x {
                edits.push(('+', cy as usize));
                cy -= 1;
            } else {
                edits.push(('-', cx as usize));
                cx -= 1;
            }
        }
    }

    edits.reverse();
    edits
}

pub fn diff(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut file_args = Vec::new();

    for arg in args {
        match *arg {
            "-u" | "--unified" => {}
            _ => file_args.push(*arg),
        }
    }

    if file_args.len() < 2 {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "diff: missing operand\n".to_string(),
            exit_code: 2,
            env: HashMap::new(),
});
    }

    let content1 = if file_args[0] == "-" {
        ctx.stdin.to_string()
    } else {
        let path = crate::fs::path::resolve(ctx.cwd, file_args[0]);
        match ctx.fs.read_file_string(&path) {
            Ok(c) => c,
            Err(e) => {
                return Ok(ExecResult {
                    stdout: String::new(),
                    stderr: format!("diff: {e}\n"),
                    exit_code: 2,
                    env: HashMap::new(),
})
            }
        }
    };

    let content2 = if file_args[1] == "-" {
        ctx.stdin.to_string()
    } else {
        let path = crate::fs::path::resolve(ctx.cwd, file_args[1]);
        match ctx.fs.read_file_string(&path) {
            Ok(c) => c,
            Err(e) => {
                return Ok(ExecResult {
                    stdout: String::new(),
                    stderr: format!("diff: {e}\n"),
                    exit_code: 2,
                    env: HashMap::new(),
})
            }
        }
    };

    let lines1: Vec<&str> = content1.lines().collect();
    let lines2: Vec<&str> = content2.lines().collect();

    if lines1 == lines2 {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: 0,
            env: HashMap::new(),
});
    }

    let edits = compute_diff_edits(&lines1, &lines2);

    let mut stdout = String::new();
    let _ = writeln!(stdout, "--- {}", file_args[0]);
    let _ = writeln!(stdout, "+++ {}", file_args[1]);

    let mut ei = 0;
    while ei < edits.len() {
        if edits[ei].0 == ' ' {
            ei += 1;
            continue;
        }

        let hunk_start = ei.saturating_sub(3);
        let mut hunk_end = ei;
        let mut last_change = ei;
        while hunk_end < edits.len() {
            if edits[hunk_end].0 != ' ' {
                last_change = hunk_end;
            }
            if hunk_end > last_change + 3 {
                break;
            }
            hunk_end += 1;
        }

        let mut old_start = 0u64;
        let mut old_count = 0u64;
        let mut new_start = 0u64;
        let mut new_count = 0u64;
        let mut first_old = true;
        let mut first_new = true;

        for edit in &edits[hunk_start..hunk_end] {
            match edit.0 {
                ' ' => {
                    if first_old {
                        old_start = edit.1 as u64 + 1;
                        first_old = false;
                    }
                    if first_new {
                        new_start = edit.1 as u64 + 1;
                        first_new = false;
                    }
                    old_count += 1;
                    new_count += 1;
                }
                '-' => {
                    if first_old {
                        old_start = edit.1 as u64 + 1;
                        first_old = false;
                    }
                    if first_new {
                        new_start = edit.1 as u64 + 1;
                        first_new = false;
                    }
                    old_count += 1;
                }
                '+' => {
                    if first_new {
                        new_start = edit.1 as u64 + 1;
                        first_new = false;
                    }
                    if first_old && old_start == 0 {
                        old_start = edit.1 as u64 + 1;
                    }
                    new_count += 1;
                }
                _ => {}
            }
        }

        if old_start == 0 {
            old_start = 1;
        }
        if new_start == 0 {
            new_start = 1;
        }

        let _ = writeln!(
            stdout,
            "@@ -{old_start},{old_count} +{new_start},{new_count} @@"
        );

        for edit in &edits[hunk_start..hunk_end] {
            match edit.0 {
                ' ' => {
                    let _ = writeln!(stdout, " {}", lines1[edit.1]);
                }
                '-' => {
                    let _ = writeln!(stdout, "-{}", lines1[edit.1]);
                }
                '+' => {
                    let _ = writeln!(stdout, "+{}", lines2[edit.1]);
                }
                _ => {}
            }
        }

        ei = hunk_end;
    }

    Ok(ExecResult {
        stdout,
        stderr: String::new(),
        exit_code: 1,
        env: HashMap::new(),
})
}
