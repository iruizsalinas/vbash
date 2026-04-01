use std::io::Read;
use std::time::Duration;

use crate::fs::VirtualFs;
#[cfg(feature = "network")]
use crate::NetworkPolicy;

use super::parse::{CurlOptions, FormField};
use super::security::{validate_url, validate_method, resolve_redirect_url};

pub(super) struct CurlResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub url: String,
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };
        let n = u32::from(b0) << 16 | u32::from(b1) << 8 | u32::from(b2);
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((n >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(n & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

pub(super) fn validate_header_value(value: &str) -> Result<(), String> {
    if value.contains('\r') || value.contains('\n') {
        return Err("curl: header value contains invalid characters".to_string());
    }
    Ok(())
}

fn determine_method(opts: &CurlOptions) -> &str {
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

fn has_body(method: &str) -> bool {
    matches!(method.to_ascii_uppercase().as_str(), "POST" | "PUT" | "PATCH")
}

fn build_multipart_body(
    fields: &[FormField],
    boundary: &str,
    fs: &dyn VirtualFs,
    cwd: &str,
) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    for field in fields {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        if field.is_file || field.is_file_content {
            let path = crate::fs::path::resolve(cwd, &field.value);
            let file_data = fs
                .read_file(&path)
                .map_err(|e| format!("curl: {e}"))?;
            let fname = field
                .filename
                .as_deref()
                .unwrap_or_else(|| crate::fs::path::basename(&field.value));
            let ct = field
                .content_type
                .as_deref()
                .unwrap_or("application/octet-stream");
            if field.is_file_content {
                body.extend_from_slice(
                    format!(
                        "Content-Disposition: form-data; name=\"{}\"\r\n",
                        field.name
                    )
                    .as_bytes(),
                );
                body.extend_from_slice(format!("Content-Type: {ct}\r\n\r\n").as_bytes());
            } else {
                body.extend_from_slice(
                    format!(
                        "Content-Disposition: form-data; name=\"{}\"; filename=\"{fname}\"\r\n",
                        field.name
                    )
                    .as_bytes(),
                );
                body.extend_from_slice(format!("Content-Type: {ct}\r\n\r\n").as_bytes());
            }
            body.extend_from_slice(&file_data);
        } else {
            body.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"{}\"\r\n\r\n",
                    field.name
                )
                .as_bytes(),
            );
            body.extend_from_slice(field.value.as_bytes());
        }
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    Ok(body)
}

fn read_body_chunked(
    body: &mut ureq::Body,
    max_size: usize,
) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let mut reader = body.as_reader();
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                if buf.len() + n > max_size {
                    return Err(format!(
                        "curl: response body exceeds maximum size ({max_size} bytes)"
                    ));
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {},
            Err(e) => return Err(format!("curl: error reading response: {e}")),
        }
    }
    Ok(buf)
}

fn extract_headers(headers: &ureq::http::HeaderMap) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for (name, value) in headers {
        let val = value.to_str().unwrap_or("");
        result.push((name.to_string(), val.to_string()));
    }
    result
}

fn status_text_for(status: u16) -> &'static str {
    match status {
        100 => "Continue",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        409 => "Conflict",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Unknown",
    }
}

#[allow(clippy::type_complexity)]
fn get_body_data(
    opts: &CurlOptions,
    fs: &dyn VirtualFs,
    cwd: &str,
) -> Result<Option<(Vec<u8>, Option<String>)>, (String, i32)> {
    if !opts.form_fields.is_empty() {
        let boundary = "----vbash-form-boundary";
        let body =
            build_multipart_body(&opts.form_fields, boundary, fs, cwd).map_err(|e| (e, 26))?;
        let ct = format!("multipart/form-data; boundary={boundary}");
        return Ok(Some((body, Some(ct))));
    }
    if let Some(ref data) = opts.data {
        let has_ct = opts
            .headers
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("content-type"));
        let ct = if has_ct {
            None
        } else {
            Some("application/x-www-form-urlencoded".to_string())
        };
        return Ok(Some((data.as_bytes().to_vec(), ct)));
    }
    if let Some(ref data) = opts.data_raw {
        let has_ct = opts
            .headers
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("content-type"));
        let ct = if has_ct {
            None
        } else {
            Some("application/x-www-form-urlencoded".to_string())
        };
        return Ok(Some((data.as_bytes().to_vec(), ct)));
    }
    if let Some(ref data) = opts.data_binary {
        return Ok(Some((data.as_bytes().to_vec(), None)));
    }
    if let Some(ref upload_path) = opts.upload_file {
        let resolved = crate::fs::path::resolve(cwd, upload_path);
        let file_data = fs
            .read_file(&resolved)
            .map_err(|e| (format!("curl: {e}"), 26))?;
        return Ok(Some((file_data, None)));
    }
    Ok(None)
}

/// Send a single HTTP request (no redirect following).
fn send_one_request(
    agent: &ureq::Agent,
    method: &str,
    url: &str,
    opts: &CurlOptions,
    body_data: Option<&(Vec<u8>, Option<String>)>,
    max_response_size: usize,
) -> Result<CurlResponse, (String, i32)> {
    let ua = opts
        .user_agent
        .as_deref()
        .unwrap_or("vbash-curl/0.1");

    let send_with_body = has_body(method) || body_data.is_some();

    let mut builder = ureq::http::Request::builder()
        .method(method)
        .uri(url)
        .header("User-Agent", ua);

    for (key, value) in &opts.headers {
        builder = builder.header(key.as_str(), value.as_str());
    }
    if let Some(ref referer) = opts.referer {
        builder = builder.header("Referer", referer.as_str());
    }
    if let Some(ref cookie) = opts.cookie {
        builder = builder.header("Cookie", cookie.as_str());
    }
    if let Some(ref user) = opts.user {
        let encoded = base64_encode(user.as_bytes());
        let auth_val = format!("Basic {encoded}");
        builder = builder.header("Authorization", auth_val);
    }

    let send_result = if send_with_body {
        if let Some((data, ct)) = body_data {
            if let Some(ct_val) = ct {
                builder = builder.header("Content-Type", ct_val.as_str());
            }
            let request = builder
                .body(data.clone())
                .map_err(|e| (format!("curl: {e}"), 3))?;
            agent.run(request)
        } else {
            let request = builder
                .body(vec![])
                .map_err(|e| (format!("curl: {e}"), 3))?;
            agent.run(request)
        }
    } else {
        let request = builder
            .body(())
            .map_err(|e| (format!("curl: {e}"), 3))?;
        agent.run(request)
    };

    match send_result {
        Ok(mut response) => {
            let status = response.status().as_u16();
            let stext = status_text_for(status).to_string();
            let hdrs = extract_headers(response.headers());
            let body =
                read_body_chunked(response.body_mut(), max_response_size).map_err(|e| (e, 23))?;
            Ok(CurlResponse {
                status,
                status_text: stext,
                headers: hdrs,
                body,
                url: url.to_string(),
            })
        }
        Err(ureq::Error::Timeout(_)) => Err(("curl: operation timed out".to_string(), 28)),
        Err(e) => Err((format!("curl: failed to connect: {e}"), 7)),
    }
}

pub(super) fn execute_request(
    opts: &CurlOptions,
    fs: &dyn VirtualFs,
    cwd: &str,
    network_policy: Option<&NetworkPolicy>,
) -> Result<CurlResponse, (String, i32)> {
    validate_url(&opts.url, network_policy).map_err(|e| (e, 7))?;

    if opts.insecure {
        return Err((
            "curl: TLS verification cannot be disabled in sandbox mode".to_string(),
            7,
        ));
    }

    let method = determine_method(opts);

    validate_method(method, network_policy).map_err(|e| (e, 7))?;

    let timeout_secs = match (opts.max_time, opts.connect_timeout) {
        (Some(m), Some(c)) => m.min(c),
        (Some(m), None) => m,
        (None, Some(c)) => c,
        (None, None) => 30,
    };

    let max_response_size = network_policy
        .map_or(10 * 1024 * 1024, |p| p.max_response_size);
    let max_redirects = network_policy
        .map_or(20, |p| p.max_redirects);

    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(timeout_secs)))
        .http_status_as_error(false)
        .build()
        .into();
    let agent: ureq::Agent = config;

    for (_, value) in &opts.headers {
        validate_header_value(value).map_err(|e| (e, 3))?;
    }

    let body_data = get_body_data(opts, fs, cwd)?;

    let mut current_url = opts.url.clone();
    let mut redirect_count: u32 = 0;

    loop {
        let response = send_one_request(
            &agent,
            method,
            &current_url,
            opts,
            body_data.as_ref(),
            max_response_size,
        )?;

        let is_redirect = matches!(response.status, 301 | 302 | 303 | 307 | 308);
        if is_redirect && opts.location {
            redirect_count += 1;
            let effective_max = opts.max_redirects.unwrap_or(max_redirects);
            if redirect_count > effective_max {
                return Err((
                    format!("curl: maximum redirects ({effective_max}) exceeded"),
                    47,
                ));
            }

            let location = response
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("location"))
                .map(|(_, v)| v.clone());

            if let Some(loc) = location {
                let next_url = resolve_redirect_url(&current_url, &loc);
                validate_url(&next_url, network_policy).map_err(|e| (e, 7))?;

                current_url = next_url;
                continue;
            }
        }

        return Ok(response);
    }
}
