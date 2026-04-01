pub(super) struct FormField {
    pub name: String,
    pub value: String,
    pub is_file: bool,
    pub is_file_content: bool,
    pub content_type: Option<String>,
    pub filename: Option<String>,
}

#[allow(clippy::struct_excessive_bools)]
pub(super) struct CurlOptions {
    pub url: String,
    pub method: Option<String>,
    pub headers: Vec<(String, String)>,
    pub data: Option<String>,
    pub data_raw: Option<String>,
    pub data_binary: Option<String>,
    pub form_fields: Vec<FormField>,
    pub user: Option<String>,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
    pub cookie: Option<String>,
    pub cookie_jar: Option<String>,
    pub output: Option<String>,
    pub remote_name: bool,
    pub upload_file: Option<String>,
    pub include: bool,
    pub head: bool,
    pub silent: bool,
    pub show_error: bool,
    pub fail: bool,
    pub location: bool,
    pub insecure: bool,
    pub verbose: bool,
    pub max_time: Option<u64>,
    pub write_out: Option<String>,
    pub max_redirects: Option<u32>,
    pub connect_timeout: Option<u64>,
}

impl CurlOptions {
    fn new() -> Self {
        Self {
            url: String::new(),
            method: None,
            headers: Vec::new(),
            data: None,
            data_raw: None,
            data_binary: None,
            form_fields: Vec::new(),
            user: None,
            user_agent: None,
            referer: None,
            cookie: None,
            cookie_jar: None,
            output: None,
            remote_name: false,
            upload_file: None,
            include: false,
            head: false,
            silent: false,
            show_error: false,
            fail: false,
            location: false,
            insecure: false,
            verbose: false,
            max_time: None,
            write_out: None,
            max_redirects: None,
            connect_timeout: None,
        }
    }
}

const NO_ARG_FLAGS: &[u8] = b"sSfLIivOk";

const ARG_FLAGS: &[(u8, &str)] = &[
    (b'X', "method"),
    (b'H', "header"),
    (b'd', "data"),
    (b'u', "user"),
    (b'A', "user-agent"),
    (b'e', "referer"),
    (b'b', "cookie"),
    (b'c', "cookie-jar"),
    (b'o', "output"),
    (b'T', "upload-file"),
    (b'F', "form"),
    (b'm', "max-time"),
    (b'w', "write-out"),
];

fn arg_flag_name(ch: u8) -> Option<&'static str> {
    for &(flag, name) in ARG_FLAGS {
        if flag == ch {
            return Some(name);
        }
    }
    None
}

fn apply_no_arg_flag(opts: &mut CurlOptions, ch: u8) {
    match ch {
        b's' => opts.silent = true,
        b'S' => opts.show_error = true,
        b'f' => opts.fail = true,
        b'L' => opts.location = true,
        b'I' => opts.head = true,
        b'i' => opts.include = true,
        b'v' => opts.verbose = true,
        b'O' => opts.remote_name = true,
        b'k' => opts.insecure = true,
        _ => {}
    }
}

fn apply_valued_flag(opts: &mut CurlOptions, name: &str, value: String) -> Result<(), String> {
    match name {
        "method" => opts.method = Some(value),
        "header" => {
            if let Some(pos) = value.find(':') {
                let key = value[..pos].trim().to_string();
                let val = value[pos + 1..].trim().to_string();
                opts.headers.push((key, val));
            } else {
                return Err(format!("curl: invalid header '{value}'"));
            }
        }
        "data" => {
            if let Some(existing) = opts.data.take() {
                opts.data = Some(format!("{existing}&{value}"));
            } else {
                opts.data = Some(value);
            }
        }
        "data-raw" => opts.data_raw = Some(value),
        "data-binary" => opts.data_binary = Some(value),
        "user" => opts.user = Some(value),
        "user-agent" => opts.user_agent = Some(value),
        "referer" => opts.referer = Some(value),
        "cookie" => opts.cookie = Some(value),
        "cookie-jar" => opts.cookie_jar = Some(value),
        "output" => opts.output = Some(value),
        "upload-file" => opts.upload_file = Some(value),
        "form" => opts.form_fields.push(parse_form_field(&value)?),
        "max-time" => {
            opts.max_time = Some(
                value
                    .parse::<u64>()
                    .map_err(|_| format!("curl: invalid timeout '{value}'"))?,
            );
        }
        "write-out" => opts.write_out = Some(value),
        "max-redirs" => {
            opts.max_redirects = Some(
                value
                    .parse::<u32>()
                    .map_err(|_| format!("curl: invalid max-redirs '{value}'"))?,
            );
        }
        "connect-timeout" => {
            opts.connect_timeout = Some(
                value
                    .parse::<u64>()
                    .map_err(|_| format!("curl: invalid connect-timeout '{value}'"))?,
            );
        }
        _ => return Err(format!("curl: unknown option '{name}'")),
    }
    Ok(())
}

fn parse_form_field(input: &str) -> Result<FormField, String> {
    let eq_pos = input
        .find('=')
        .ok_or_else(|| format!("curl: malformed form field '{input}'"))?;
    let name = input[..eq_pos].to_string();
    let mut raw_value = &input[eq_pos + 1..];

    let mut is_file = false;
    let mut is_file_content = false;

    if let Some(rest) = raw_value.strip_prefix('@') {
        is_file = true;
        raw_value = rest;
    } else if let Some(rest) = raw_value.strip_prefix('<') {
        is_file_content = true;
        raw_value = rest;
    }

    let mut content_type = None;
    let mut filename = None;
    let mut value = raw_value.to_string();

    if is_file || is_file_content {
        if let Some(semi) = value.find(';') {
            let suffixes = value[semi + 1..].to_string();
            value.truncate(semi);
            for part in suffixes.split(';') {
                let part = part.trim();
                if let Some(ct) = part.strip_prefix("type=") {
                    content_type = Some(ct.to_string());
                } else if let Some(fn_) = part.strip_prefix("filename=") {
                    filename = Some(fn_.to_string());
                }
            }
        }
    }

    Ok(FormField {
        name,
        value,
        is_file,
        is_file_content,
        content_type,
        filename,
    })
}

fn consume_long_option(
    opts: &mut CurlOptions,
    arg: &str,
    args: &[&str],
    idx: &mut usize,
) -> Result<(), String> {
    let name_value = &arg[2..];
    let (name, inline_val) = match name_value.find('=') {
        Some(pos) => (&name_value[..pos], Some(name_value[pos + 1..].to_string())),
        None => (name_value, None),
    };

    match name {
        "silent" => opts.silent = true,
        "show-error" => opts.show_error = true,
        "fail" => opts.fail = true,
        "location" => opts.location = true,
        "head" => opts.head = true,
        "include" => opts.include = true,
        "verbose" => opts.verbose = true,
        "remote-name" => opts.remote_name = true,
        "insecure" => opts.insecure = true,
        "compressed" => {}
        "request" => {
            let v = take_value(inline_val, args, idx, name)?;
            apply_valued_flag(opts, "method", v)?;
        }
        "header" => {
            let v = take_value(inline_val, args, idx, name)?;
            apply_valued_flag(opts, "header", v)?;
        }
        "data" | "data-ascii" => {
            let v = take_value(inline_val, args, idx, name)?;
            apply_valued_flag(opts, "data", v)?;
        }
        "data-raw" => {
            let v = take_value(inline_val, args, idx, name)?;
            apply_valued_flag(opts, "data-raw", v)?;
        }
        "data-binary" => {
            let v = take_value(inline_val, args, idx, name)?;
            apply_valued_flag(opts, "data-binary", v)?;
        }
        "data-urlencode" => {
            let v = take_value(inline_val, args, idx, name)?;
            let encoded = url_encode_value(&v);
            apply_valued_flag(opts, "data", encoded)?;
        }
        "user" => {
            let v = take_value(inline_val, args, idx, name)?;
            apply_valued_flag(opts, "user", v)?;
        }
        "user-agent" => {
            let v = take_value(inline_val, args, idx, name)?;
            apply_valued_flag(opts, "user-agent", v)?;
        }
        "referer" => {
            let v = take_value(inline_val, args, idx, name)?;
            apply_valued_flag(opts, "referer", v)?;
        }
        "cookie" => {
            let v = take_value(inline_val, args, idx, name)?;
            apply_valued_flag(opts, "cookie", v)?;
        }
        "cookie-jar" => {
            let v = take_value(inline_val, args, idx, name)?;
            apply_valued_flag(opts, "cookie-jar", v)?;
        }
        "output" => {
            let v = take_value(inline_val, args, idx, name)?;
            apply_valued_flag(opts, "output", v)?;
        }
        "upload-file" => {
            let v = take_value(inline_val, args, idx, name)?;
            apply_valued_flag(opts, "upload-file", v)?;
        }
        "form" => {
            let v = take_value(inline_val, args, idx, name)?;
            apply_valued_flag(opts, "form", v)?;
        }
        "max-time" => {
            let v = take_value(inline_val, args, idx, name)?;
            apply_valued_flag(opts, "max-time", v)?;
        }
        "connect-timeout" => {
            let v = take_value(inline_val, args, idx, name)?;
            apply_valued_flag(opts, "connect-timeout", v)?;
        }
        "write-out" => {
            let v = take_value(inline_val, args, idx, name)?;
            apply_valued_flag(opts, "write-out", v)?;
        }
        "max-redirs" => {
            let v = take_value(inline_val, args, idx, name)?;
            apply_valued_flag(opts, "max-redirs", v)?;
        }
        "url" => {
            let v = take_value(inline_val, args, idx, name)?;
            opts.url = v;
        }
        _ => return Err(format!("curl: unknown option '--{name}'")),
    }
    Ok(())
}

fn take_value(
    inline: Option<String>,
    args: &[&str],
    idx: &mut usize,
    name: &str,
) -> Result<String, String> {
    if let Some(v) = inline {
        return Ok(v);
    }
    *idx += 1;
    if *idx < args.len() {
        Ok(args[*idx].to_string())
    } else {
        Err(format!("curl: option '--{name}' requires a value"))
    }
}

fn url_encode_value(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len() * 3);
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push('%');
                encoded.push(char::from(b"0123456789ABCDEF"[(byte >> 4) as usize]));
                encoded.push(char::from(b"0123456789ABCDEF"[(byte & 0x0F) as usize]));
            }
        }
    }
    encoded
}

pub(super) fn parse_curl_args(args: &[&str]) -> Result<CurlOptions, String> {
    let mut opts = CurlOptions::new();
    let mut idx = 0;

    while idx < args.len() {
        let arg = args[idx];

        if arg == "--" {
            idx += 1;
            if idx < args.len() && opts.url.is_empty() {
                opts.url = args[idx].to_string();
            }
            break;
        }

        if arg.starts_with("--") {
            consume_long_option(&mut opts, arg, args, &mut idx)?;
            idx += 1;
            continue;
        }

        if arg.starts_with('-') && arg.len() > 1 {
            let bytes = arg.as_bytes();
            let mut ci = 1;
            while ci < bytes.len() {
                let ch = bytes[ci];
                if NO_ARG_FLAGS.contains(&ch) {
                    apply_no_arg_flag(&mut opts, ch);
                    ci += 1;
                } else if arg_flag_name(ch).is_some() {
                    let name = arg_flag_name(ch).unwrap_or("");
                    let value = if ci + 1 < bytes.len() {
                        String::from_utf8_lossy(&bytes[ci + 1..]).into_owned()
                    } else {
                        idx += 1;
                        if idx >= args.len() {
                            return Err(format!(
                                "curl: option '-{}' requires a value",
                                ch as char
                            ));
                        }
                        args[idx].to_string()
                    };
                    apply_valued_flag(&mut opts, name, value)?;
                    break;
                } else {
                    return Err(format!("curl: unknown option '-{}'", ch as char));
                }
            }
            idx += 1;
            continue;
        }

        if opts.url.is_empty() {
            opts.url = arg.to_string();
        }
        idx += 1;
    }

    if opts.url.is_empty() {
        return Err("curl: no URL specified".to_string());
    }

    Ok(opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_get() {
        let opts = parse_curl_args(&["https://example.com"]).unwrap();
        assert_eq!(opts.url, "https://example.com");
        assert!(opts.method.is_none());
    }

    #[test]
    fn parse_post_with_data() {
        let opts = parse_curl_args(&["-X", "POST", "-d", "{}", "https://api.com"]).unwrap();
        assert_eq!(opts.method.as_deref(), Some("POST"));
        assert_eq!(opts.data.as_deref(), Some("{}"));
    }

    #[test]
    fn parse_combined_flags() {
        let opts = parse_curl_args(&["-sSfL", "https://example.com"]).unwrap();
        assert!(opts.silent);
        assert!(opts.show_error);
        assert!(opts.fail);
        assert!(opts.location);
    }

    #[test]
    fn parse_headers() {
        let opts = parse_curl_args(&[
            "-H",
            "Content-Type: application/json",
            "-H",
            "Accept: */*",
            "https://api.com",
        ])
        .unwrap();
        assert_eq!(opts.headers.len(), 2);
        assert_eq!(opts.headers[0].0, "Content-Type");
        assert_eq!(opts.headers[0].1, "application/json");
    }

    #[test]
    fn parse_auth() {
        let opts = parse_curl_args(&["-u", "user:pass", "https://api.com"]).unwrap();
        assert_eq!(opts.user.as_deref(), Some("user:pass"));
    }

    #[test]
    fn parse_output_file() {
        let opts = parse_curl_args(&["-o", "output.json", "https://api.com"]).unwrap();
        assert_eq!(opts.output.as_deref(), Some("output.json"));
    }

    #[test]
    fn parse_form_field() {
        let opts = parse_curl_args(&[
            "-F",
            "name=value",
            "-F",
            "file=@/path/to/file",
            "https://api.com",
        ])
        .unwrap();
        assert_eq!(opts.form_fields.len(), 2);
        assert!(!opts.form_fields[0].is_file);
        assert!(opts.form_fields[1].is_file);
        assert_eq!(opts.form_fields[1].value, "/path/to/file");
    }

    #[test]
    fn parse_long_form_options() {
        let opts = parse_curl_args(&[
            "--request",
            "PUT",
            "--header",
            "X-Custom: val",
            "--silent",
            "--fail",
            "https://api.com",
        ])
        .unwrap();
        assert_eq!(opts.method.as_deref(), Some("PUT"));
        assert_eq!(opts.headers.len(), 1);
        assert!(opts.silent);
        assert!(opts.fail);
    }

    #[test]
    fn parse_long_form_with_equals() {
        let opts =
            parse_curl_args(&["--request=DELETE", "--max-time=10", "https://api.com"]).unwrap();
        assert_eq!(opts.method.as_deref(), Some("DELETE"));
        assert_eq!(opts.max_time, Some(10));
    }

    #[test]
    fn parse_combined_short_with_value() {
        let opts = parse_curl_args(&["-sXPOST", "-d", "body", "https://api.com"]).unwrap();
        assert!(opts.silent);
        assert_eq!(opts.method.as_deref(), Some("POST"));
        assert_eq!(opts.data.as_deref(), Some("body"));
    }

    #[test]
    fn parse_no_url_error() {
        let result = parse_curl_args(&["-s", "-f"]);
        assert!(result.is_err());
    }

    #[test]
    fn parse_multiple_data_concatenates() {
        let opts =
            parse_curl_args(&["-d", "a=1", "-d", "b=2", "https://api.com"]).unwrap();
        assert_eq!(opts.data.as_deref(), Some("a=1&b=2"));
    }

    #[test]
    fn parse_form_field_with_type() {
        let opts =
            parse_curl_args(&["-F", "pic=@photo.jpg;type=image/jpeg", "https://api.com"])
                .unwrap();
        assert_eq!(opts.form_fields.len(), 1);
        assert!(opts.form_fields[0].is_file);
        assert_eq!(opts.form_fields[0].value, "photo.jpg");
        assert_eq!(
            opts.form_fields[0].content_type.as_deref(),
            Some("image/jpeg")
        );
    }

    #[test]
    fn parse_form_field_with_filename() {
        let opts = parse_curl_args(&[
            "-F",
            "file=@data.bin;filename=upload.bin;type=application/octet-stream",
            "https://api.com",
        ])
        .unwrap();
        assert_eq!(opts.form_fields[0].filename.as_deref(), Some("upload.bin"));
        assert_eq!(
            opts.form_fields[0].content_type.as_deref(),
            Some("application/octet-stream")
        );
    }

    #[test]
    fn parse_file_content_field() {
        let opts =
            parse_curl_args(&["-F", "data=</tmp/content.txt", "https://api.com"]).unwrap();
        assert!(opts.form_fields[0].is_file_content);
        assert_eq!(opts.form_fields[0].value, "/tmp/content.txt");
    }

    #[test]
    fn url_encode_basic() {
        assert_eq!(url_encode_value("hello world"), "hello%20world");
        assert_eq!(url_encode_value("a&b=c"), "a%26b%3Dc");
        assert_eq!(url_encode_value("safe-_.~"), "safe-_.~");
    }
}
