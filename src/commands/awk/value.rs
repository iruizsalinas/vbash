use super::builtins::format_g;

#[derive(Debug, Clone)]
pub(super) enum AwkValue {
    Str(String),
    Num(f64),
    Uninit,
}

impl AwkValue {
    pub fn to_num(&self) -> f64 {
        match self {
            Self::Num(n) => *n,
            Self::Str(s) => parse_awk_number(s),
            Self::Uninit => 0.0,
        }
    }

    pub fn to_str(&self) -> String {
        match self {
            Self::Str(s) => s.clone(),
            Self::Num(n) => format_number(*n),
            Self::Uninit => String::new(),
        }
    }

    pub fn is_true(&self) -> bool {
        match self {
            Self::Num(n) => *n != 0.0,
            Self::Str(s) => !s.is_empty(),
            Self::Uninit => false,
        }
    }
}

pub(super) fn parse_awk_number(s: &str) -> f64 {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return 0.0;
    }
    let mut end = 0;
    let bytes = trimmed.as_bytes();
    if end < bytes.len() && (bytes[end] == b'+' || bytes[end] == b'-') {
        end += 1;
    }
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end < bytes.len() && bytes[end] == b'.' {
        end += 1;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
    }
    if end < bytes.len() && (bytes[end] == b'e' || bytes[end] == b'E') {
        end += 1;
        if end < bytes.len() && (bytes[end] == b'+' || bytes[end] == b'-') {
            end += 1;
        }
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
    }
    trimmed[..end].parse().unwrap_or(0.0)
}

pub(super) fn format_number(n: f64) -> String {
    if n.is_nan() {
        return "nan".to_string();
    }
    if n.is_infinite() {
        return if n > 0.0 { "inf".to_string() } else { "-inf".to_string() };
    }
    #[allow(clippy::float_cmp)]
    if n == n.trunc() && n.abs() < 1e16 {
        format!("{}", n as i64)
    } else {
        format_g(n, 6, false)
    }
}
