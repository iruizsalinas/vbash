use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;
use std::collections::HashMap;

pub fn date_cmd(args: &[&str], _ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let mut utc = false;
    let mut date_string: Option<&str> = None;
    let mut format_str: Option<&str> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-u" | "--utc" | "--universal" => {
                utc = true;
                i += 1;
            }
            "-d" | "--date" if i + 1 < args.len() => {
                date_string = Some(args[i + 1]);
                i += 2;
            }
            arg if arg.starts_with('+') => {
                format_str = Some(&arg[1..]);
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    let dt = if let Some(ds) = date_string {
        parse_date_string(ds, utc)
    } else if utc {
        DateVal::Utc(chrono::Utc::now())
    } else {
        DateVal::Local(chrono::Local::now())
    };

    let fmt = format_str.unwrap_or("%a %b %e %H:%M:%S %Z %Y");
    let output = match dt {
        DateVal::Utc(d) => d.format(fmt).to_string(),
        DateVal::Local(d) => d.format(fmt).to_string(),
    };

    Ok(ExecResult {
        stdout: format!("{output}\n"),
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}

enum DateVal {
    Utc(chrono::DateTime<chrono::Utc>),
    Local(chrono::DateTime<chrono::Local>),
}

fn parse_date_string(s: &str, utc: bool) -> DateVal {
    if let Some(ts) = s.strip_prefix('@') {
        if let Ok(secs) = ts.parse::<i64>() {
            if let Some(dt) = chrono::DateTime::from_timestamp(secs, 0) {
                if utc {
                    return DateVal::Utc(dt);
                }
                return DateVal::Local(dt.with_timezone(&chrono::Local));
            }
        }
    }

    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        if utc {
            return DateVal::Utc(dt.and_utc());
        }
        return DateVal::Local(
            dt.and_local_timezone(chrono::Local)
                .single()
                .unwrap_or_else(|| dt.and_utc().with_timezone(&chrono::Local)),
        );
    }

    if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let dt = d
            .and_hms_opt(0, 0, 0)
            .unwrap_or_default();
        if utc {
            return DateVal::Utc(dt.and_utc());
        }
        return DateVal::Local(
            dt.and_local_timezone(chrono::Local)
                .single()
                .unwrap_or_else(|| dt.and_utc().with_timezone(&chrono::Local)),
        );
    }

    if utc {
        DateVal::Utc(chrono::Utc::now())
    } else {
        DateVal::Local(chrono::Local::now())
    }
}

pub fn sleep_cmd(args: &[&str], _ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    if args.is_empty() {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "sleep: missing operand\n".to_string(),
            exit_code: 1,
            env: HashMap::new(),
});
    }

    let max_sleep = 300.0_f64;
    let secs: f64 = args[0].parse().unwrap_or(0.0);
    if secs < 0.0 {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "sleep: invalid time interval\n".to_string(),
            exit_code: 1,
            env: HashMap::new(),
});
    }

    let capped = secs.min(max_sleep);
    let duration = std::time::Duration::from_secs_f64(capped);
    std::thread::sleep(duration);

    Ok(ExecResult {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
})
}

pub fn timeout_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    if args.len() < 2 {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "timeout: missing operand\n".to_string(),
            exit_code: 1,
            env: HashMap::new(),
});
    }

    let cmd_parts = &args[1..];
    let command = cmd_parts.join(" ");

    if let Some(exec_fn) = ctx.exec_fn {
        exec_fn(&command)
    } else {
        Ok(ExecResult {
            stdout: String::new(),
            stderr: "timeout: cannot execute subcommand\n".to_string(),
            exit_code: 126,
            env: HashMap::new(),
})
    }
}

pub fn nohup_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    if args.is_empty() {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: "nohup: missing operand\n".to_string(),
            exit_code: 1,
            env: HashMap::new(),
});
    }

    let command = args.join(" ");
    if let Some(exec_fn) = ctx.exec_fn {
        exec_fn(&command)
    } else {
        Ok(ExecResult {
            stdout: String::new(),
            stderr: "nohup: cannot execute subcommand\n".to_string(),
            exit_code: 126,
            env: HashMap::new(),
})
    }
}
