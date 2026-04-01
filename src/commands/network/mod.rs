mod parse;
mod request;
mod security;

use std::collections::HashMap;
use std::fmt::Write;

use parse::parse_curl_args;
use request::execute_request;

use crate::ExecResult;
use crate::commands::CommandContext;
use crate::error::Error;

pub fn curl_cmd(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let opts = match parse_curl_args(args) {
        Ok(o) => o,
        Err(msg) => {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: format!("{msg}\n"),
                exit_code: 2,
                env: HashMap::new(),
            });
        }
    };

    let response = match execute_request(&opts, ctx.fs, ctx.cwd, ctx.network_policy) {
        Ok(r) => r,
        Err((msg, code)) => {
            let mut stderr = String::new();
            if !opts.silent || opts.show_error {
                stderr = format!("{msg}\n");
            }
            return Ok(ExecResult {
                stdout: String::new(),
                stderr,
                exit_code: code,
                env: HashMap::new(),
            });
        }
    };

    if opts.fail && response.status >= 400 {
        let mut stderr = String::new();
        if !opts.silent || opts.show_error {
            let _ = writeln!(
                stderr,
                "curl: (22) The requested URL returned error: {}",
                response.status
            );
        }
        return Ok(ExecResult {
            stdout: String::new(),
            stderr,
            exit_code: 22,
            env: HashMap::new(),
        });
    }

    let mut stdout = String::new();

    if opts.verbose {
        let _ = write!(ctx.stderr, "> {} {} HTTP/1.1\r\n", determine_display_method(&opts), opts.url);
        let _ = write!(ctx.stderr, "> Host: {}\r\n", extract_host(&opts.url));
        let _ = write!(ctx.stderr, "> User-Agent: {}\r\n", opts.user_agent.as_deref().unwrap_or("vbash-curl/0.1"));
        for (k, v) in &opts.headers {
            let _ = write!(ctx.stderr, "> {k}: {v}\r\n");
        }
        let _ = write!(ctx.stderr, "> \r\n");
        let _ = write!(
            ctx.stderr,
            "< HTTP/1.1 {} {}\r\n",
            response.status, response.status_text
        );
        for (k, v) in &response.headers {
            let _ = write!(ctx.stderr, "< {k}: {v}\r\n");
        }
        let _ = write!(ctx.stderr, "< \r\n");
    }

    if opts.include || opts.head {
        let _ = write!(
            stdout,
            "HTTP/1.1 {} {}\r\n",
            response.status, response.status_text
        );
        for (k, v) in &response.headers {
            let _ = write!(stdout, "{k}: {v}\r\n");
        }
        let _ = write!(stdout, "\r\n");
    }

    if !opts.head {
        let body_str = String::from_utf8_lossy(&response.body);

        if let Some(ref out_path) = opts.output {
            let resolved = crate::fs::path::resolve(ctx.cwd, out_path);
            if let Some(parent_end) = resolved.rfind('/') {
                if parent_end > 0 {
                    let _ = ctx.fs.mkdir(&resolved[..parent_end], true);
                }
            }
            ctx.fs
                .write_file(&resolved, &response.body)
                .map_err(Error::Fs)?;
        } else if opts.remote_name {
            let filename = extract_remote_filename(&opts.url);
            let resolved = crate::fs::path::resolve(ctx.cwd, &filename);
            ctx.fs
                .write_file(&resolved, &response.body)
                .map_err(Error::Fs)?;
        } else {
            stdout.push_str(&body_str);
        }
    }

    if let Some(ref jar_path) = opts.cookie_jar {
        let resolved = crate::fs::path::resolve(ctx.cwd, jar_path);
        let mut jar_content = String::new();
        for (k, v) in &response.headers {
            if k.eq_ignore_ascii_case("set-cookie") {
                let _ = writeln!(jar_content, "{v}");
            }
        }
        if !jar_content.is_empty() {
            if let Some(parent_end) = resolved.rfind('/') {
                if parent_end > 0 {
                    let _ = ctx.fs.mkdir(&resolved[..parent_end], true);
                }
            }
            let _ = ctx.fs.write_file(&resolved, jar_content.as_bytes());
        }
    }

    if let Some(ref format) = opts.write_out {
        let formatted = format_write_out(format, &response);
        stdout.push_str(&formatted);
    }

    Ok(ExecResult {
        stdout,
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
    })
}

fn determine_display_method(opts: &parse::CurlOptions) -> &str {
    if let Some(ref m) = opts.method {
        return m;
    }
    if opts.head {
        return "HEAD";
    }
    if opts.upload_file.is_some() {
        return "PUT";
    }
    if opts.data.is_some()
        || opts.data_raw.is_some()
        || opts.data_binary.is_some()
        || !opts.form_fields.is_empty()
    {
        return "POST";
    }
    "GET"
}

fn extract_host(url: &str) -> &str {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    without_scheme.split('/').next().unwrap_or(without_scheme)
}

fn extract_remote_filename(url: &str) -> String {
    let without_query = url.split('?').next().unwrap_or(url);
    let without_fragment = without_query.split('#').next().unwrap_or(without_query);
    let path_part = without_fragment
        .strip_prefix("https://")
        .or_else(|| without_fragment.strip_prefix("http://"))
        .unwrap_or(without_fragment);
    let after_host = path_part.find('/').map_or("", |i| &path_part[i..]);
    let basename = after_host.rsplit('/').next().unwrap_or("");
    if basename.is_empty() {
        "index.html".to_string()
    } else {
        basename.to_string()
    }
}

fn format_write_out(format: &str, response: &request::CurlResponse) -> String {
    let mut result = format.to_string();
    result = result.replace(
        "%{http_code}",
        &response.status.to_string(),
    );
    result = result.replace(
        "%{response_code}",
        &response.status.to_string(),
    );
    result = result.replace("%{url_effective}", &response.url);
    result = result.replace(
        "%{size_download}",
        &response.body.len().to_string(),
    );
    let ct = response
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map_or("", |(_, v)| v.as_str());
    result = result.replace("%{content_type}", ct);
    result = result.replace(
        "%{num_headers}",
        &response.headers.len().to_string(),
    );
    result = result.replace("\\n", "\n");
    result = result.replace("\\t", "\t");
    result
}
