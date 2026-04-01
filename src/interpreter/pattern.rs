use crate::ast::word::WordPart;

pub(super) fn ifs_split(s: &str, ifs: Option<&str>) -> Vec<String> {
    let ifs = ifs.unwrap_or(" \t\n");
    if ifs.is_empty() {
        return vec![s.to_string()];
    }

    let mut fields = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if ifs.contains(c) {
            if !current.is_empty() {
                fields.push(std::mem::take(&mut current));
            }
            i += 1;
            while i < chars.len() && ifs.contains(chars[i]) && chars[i].is_ascii_whitespace() {
                i += 1;
            }
        } else {
            current.push(c);
            i += 1;
        }
    }
    if !current.is_empty() {
        fields.push(current);
    }
    fields
}

pub(super) fn glob_match(pattern: &str, text: &str, extglob: bool, nocase: bool) -> bool {
    if extglob {
        if let Some(result) = extglob_match(pattern, text, nocase) {
            return result;
        }
    }
    glob_match_inner(pattern.as_bytes(), text.as_bytes(), nocase)
}

/// Handle extglob matching. Returns Some(bool) if the pattern contains extglob,
/// None if it should fall back to normal glob.
fn extglob_match(pattern: &str, text: &str, nocase: bool) -> Option<bool> {
    if !has_extglob_pattern(pattern) {
        return None;
    }
    // Check for negation extglob: !(pattern) possibly with prefix/suffix
    if let Some(neg) = try_extglob_negation(pattern, text, nocase) {
        return Some(neg);
    }
    // For non-negation extglob, use regex
    if let Some(regex_pat) = extglob_to_regex(pattern) {
        let prefix = if nocase { "(?i)^" } else { "^" };
        if let Ok(re) = regex::Regex::new(&format!("{prefix}{regex_pat}$")) {
            return Some(re.is_match(text));
        }
    }
    None
}

/// Handle !(pattern) extglob by checking if text does NOT match the inner pattern.
fn try_extglob_negation(pattern: &str, text: &str, nocase: bool) -> Option<bool> {
    let chars: Vec<char> = pattern.chars().collect();
    // Find the !(...)  construct
    let mut i = 0;
    let mut prefix = String::new();
    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '!' && chars[i + 1] == '(' {
            // Found !( at position i
            i += 2;
            let mut depth = 1u32;
            let mut inner = String::new();
            while i < chars.len() && depth > 0 {
                if chars[i] == '(' {
                    depth += 1;
                    inner.push('(');
                } else if chars[i] == ')' {
                    depth -= 1;
                    if depth > 0 {
                        inner.push(')');
                    }
                } else {
                    inner.push(chars[i]);
                }
                i += 1;
            }
            let suffix: String = chars[i..].iter().collect();
            // Split inner on | to get alternatives
            let alternatives: Vec<&str> = inner.split('|').collect();
            // For !(alt1|alt2)suffix to match text:
            // text must match *suffix, and the part before suffix must NOT match any alternative
            // Simple case: no prefix, no suffix
            if prefix.is_empty() && suffix.is_empty() {
                // !(alt1|alt2) matches text if text != alt1 and text != alt2
                let matches_any = alternatives.iter().any(|alt| glob_match_inner(alt.as_bytes(), text.as_bytes(), nocase));
                return Some(!matches_any);
            }
            // With prefix/suffix: text must match prefix*suffix but the * part must not match alternatives
            if text.starts_with(&prefix) && text.ends_with(&suffix) {
                let mid_start = prefix.len();
                let mid_end = text.len().saturating_sub(suffix.len());
                if mid_start <= mid_end {
                    let mid = &text[mid_start..mid_end];
                    let matches_any = alternatives.iter().any(|alt| glob_match_inner(alt.as_bytes(), mid.as_bytes(), nocase));
                    return Some(!matches_any);
                }
            }
            return Some(false);
        }
        prefix.push(chars[i]);
        i += 1;
    }
    None // No !( found
}

pub(super) fn glob_match_inner(pattern: &[u8], text: &[u8], nocase: bool) -> bool {
    let mut pat_idx = 0;
    let mut txt_idx = 0;
    let mut saved_pat = usize::MAX;
    let mut saved_txt = 0;

    while txt_idx < text.len() {
        if pat_idx < pattern.len() && pattern[pat_idx] == b'[' {
            // Bracket character class
            let mut pi = pat_idx + 1;
            if pi >= pattern.len() {
                return false;
            }

            let negated = pattern[pi] == b'!' || pattern[pi] == b'^';
            if negated {
                pi += 1;
            }

            let mut matched = false;
            let tc = if nocase { text[txt_idx].to_ascii_lowercase() } else { text[txt_idx] };

            // Handle ] as first char in class (literal ])
            if pi < pattern.len() && pattern[pi] == b']' {
                if tc == b']' {
                    matched = true;
                }
                pi += 1;
            }

            while pi < pattern.len() && pattern[pi] != b']' {
                if pi + 2 < pattern.len() && pattern[pi + 1] == b'-' && pattern[pi + 2] != b']' {
                    // Range: a-z
                    let lo = if nocase { pattern[pi].to_ascii_lowercase() } else { pattern[pi] };
                    let hi = if nocase { pattern[pi + 2].to_ascii_lowercase() } else { pattern[pi + 2] };
                    if tc >= lo.min(hi) && tc <= lo.max(hi) {
                        matched = true;
                    }
                    pi += 3;
                } else {
                    let pc = if nocase { pattern[pi].to_ascii_lowercase() } else { pattern[pi] };
                    if tc == pc {
                        matched = true;
                    }
                    pi += 1;
                }
            }

            if pi < pattern.len() {
                pi += 1; // skip ]
            }

            if matched == negated {
                // negated XOR matched: no match on this path
                if saved_pat == usize::MAX {
                    return false;
                }
                pat_idx = saved_pat + 1;
                saved_txt += 1;
                txt_idx = saved_txt;
            } else {
                pat_idx = pi;
                txt_idx += 1;
            }
        } else if pat_idx < pattern.len()
            && (pattern[pat_idx] == b'?'
                || pattern[pat_idx] == text[txt_idx]
                || (nocase && pattern[pat_idx].eq_ignore_ascii_case(&text[txt_idx])))
        {
            pat_idx += 1;
            txt_idx += 1;
        } else if pat_idx < pattern.len() && pattern[pat_idx] == b'*' {
            saved_pat = pat_idx;
            saved_txt = txt_idx;
            pat_idx += 1;
        } else if saved_pat != usize::MAX {
            pat_idx = saved_pat + 1;
            saved_txt += 1;
            txt_idx = saved_txt;
        } else {
            return false;
        }
    }

    while pat_idx < pattern.len() && pattern[pat_idx] == b'*' {
        pat_idx += 1;
    }

    pat_idx == pattern.len()
}

fn has_extglob_pattern(s: &str) -> bool {
    let bytes = s.as_bytes();
    for i in 0..bytes.len().saturating_sub(1) {
        if matches!(bytes[i], b'@' | b'*' | b'+' | b'?' | b'!') && bytes[i + 1] == b'(' {
            return true;
        }
    }
    false
}

fn extglob_to_regex(pattern: &str) -> Option<String> {
    if !has_extglob_pattern(pattern) {
        return None;
    }
    let mut result = String::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len()
            && chars[i + 1] == '('
            && matches!(chars[i], '@' | '*' | '+' | '?' | '!')
        {
            let op = chars[i];
            i += 2;
            let mut depth = 1u32;
            let mut inner = String::new();
            while i < chars.len() && depth > 0 {
                if chars[i] == '(' {
                    depth += 1;
                    inner.push('(');
                } else if chars[i] == ')' {
                    depth -= 1;
                    if depth > 0 {
                        inner.push(')');
                    }
                } else if chars[i] == '|' && depth == 1 {
                    inner.push('|');
                } else {
                    let c = chars[i];
                    if regex_needs_escape(c) {
                        inner.push('\\');
                    }
                    inner.push(c);
                }
                i += 1;
            }
            match op {
                '@' => {
                    result.push('(');
                    result.push_str(&inner);
                    result.push(')');
                }
                '*' => {
                    result.push('(');
                    result.push_str(&inner);
                    result.push_str(")*");
                }
                '+' => {
                    result.push('(');
                    result.push_str(&inner);
                    result.push_str(")+");
                }
                '?' => {
                    result.push('(');
                    result.push_str(&inner);
                    result.push_str(")?");
                }
                '!' => {
                    result.push_str("(?!(?:");
                    result.push_str(&inner);
                    result.push_str(")$).*");
                }
                _ => {}
            }
        } else {
            match chars[i] {
                '*' => result.push_str(".*"),
                '?' => result.push('.'),
                c => {
                    if regex_needs_escape(c) {
                        result.push('\\');
                    }
                    result.push(c);
                }
            }
            i += 1;
        }
    }
    Some(result)
}

fn regex_needs_escape(c: char) -> bool {
    matches!(c, '.' | '^' | '$' | '{' | '}' | '[' | ']' | '+' | '\\')
}

pub(super) fn glob_part_to_string(part: &WordPart) -> String {
    match part {
        WordPart::Glob(crate::ast::word::GlobPart::Star) => "*".to_string(),
        WordPart::Glob(crate::ast::word::GlobPart::Question) => "?".to_string(),
        WordPart::Glob(crate::ast::word::GlobPart::GlobStar) => "**".to_string(),
        WordPart::Glob(crate::ast::word::GlobPart::CharClass { negated, content }) => {
            if *negated {
                format!("[!{content}]")
            } else {
                format!("[{content}]")
            }
        }
        _ => String::new(),
    }
}

pub(super) fn expand_ansi_c(s: &str) -> String {
    let mut result = String::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 1;
            match bytes[i] {
                b'n' => result.push('\n'),
                b't' => result.push('\t'),
                b'r' => result.push('\r'),
                b'a' => result.push('\x07'),
                b'b' => result.push('\x08'),
                b'f' => result.push('\x0c'),
                b'v' => result.push('\x0b'),
                b'e' | b'E' => result.push('\x1b'),
                b'\\' => result.push('\\'),
                b'\'' => result.push('\''),
                b'"' => result.push('"'),
                b'0' => {
                    let start = i + 1;
                    let mut end = start;
                    while end < bytes.len() && end < start + 3 && bytes[end].is_ascii_digit() {
                        end += 1;
                    }
                    let oct_str: String = bytes[start..end].iter().map(|&b| b as char).collect();
                    if let Ok(n) = u8::from_str_radix(&oct_str, 8) {
                        result.push(n as char);
                    }
                    i = end - 1;
                }
                b'x' => {
                    let start = i + 1;
                    let mut end = start;
                    while end < bytes.len() && end < start + 2 && bytes[end].is_ascii_hexdigit() {
                        end += 1;
                    }
                    let hex_str: String = bytes[start..end].iter().map(|&b| b as char).collect();
                    if let Ok(n) = u8::from_str_radix(&hex_str, 16) {
                        result.push(n as char);
                    }
                    i = end - 1;
                }
                b'u' => {
                    let start = i + 1;
                    let mut end = start;
                    while end < bytes.len() && end < start + 4 && bytes[end].is_ascii_hexdigit() {
                        end += 1;
                    }
                    if end > start {
                        let hex_str: String = bytes[start..end].iter().map(|&b| b as char).collect();
                        if let Ok(n) = u32::from_str_radix(&hex_str, 16) {
                            if let Some(c) = char::from_u32(n) {
                                result.push(c);
                            }
                        }
                        i = end - 1;
                    } else {
                        result.push('\\');
                        result.push('u');
                    }
                }
                b'U' => {
                    let start = i + 1;
                    let mut end = start;
                    while end < bytes.len() && end < start + 8 && bytes[end].is_ascii_hexdigit() {
                        end += 1;
                    }
                    if end > start {
                        let hex_str: String = bytes[start..end].iter().map(|&b| b as char).collect();
                        if let Ok(n) = u32::from_str_radix(&hex_str, 16) {
                            if let Some(c) = char::from_u32(n) {
                                result.push(c);
                            }
                        }
                        i = end - 1;
                    } else {
                        result.push('\\');
                        result.push('U');
                    }
                }
                other => {
                    result.push('\\');
                    result.push(other as char);
                }
            }
        } else {
            result.push(bytes[i] as char);
        }
        i += 1;
    }
    result
}

pub(super) fn remove_pattern(value: &str, pattern: &str, side: crate::ast::word::PatternSide, greedy: bool) -> String {
    match side {
        crate::ast::word::PatternSide::Prefix => {
            if greedy {
                for i in (0..=value.len()).rev() {
                    if glob_match(pattern, &value[..i], false, false) {
                        return value[i..].to_string();
                    }
                }
            } else {
                for i in 0..=value.len() {
                    if glob_match(pattern, &value[..i], false, false) {
                        return value[i..].to_string();
                    }
                }
            }
            value.to_string()
        }
        crate::ast::word::PatternSide::Suffix => {
            if greedy {
                for i in 0..=value.len() {
                    if glob_match(pattern, &value[i..], false, false) {
                        return value[..i].to_string();
                    }
                }
            } else {
                for i in (0..=value.len()).rev() {
                    if glob_match(pattern, &value[i..], false, false) {
                        return value[..i].to_string();
                    }
                }
            }
            value.to_string()
        }
    }
}

pub(super) fn replace_pattern(
    value: &str,
    pattern: &str,
    replacement: &str,
    all: bool,
    anchor: Option<crate::ast::word::PatternAnchor>,
) -> String {
    use crate::ast::word::PatternAnchor;

    if pattern.is_empty() {
        return value.to_string();
    }

    let chars: Vec<char> = value.chars().collect();

    if anchor == Some(PatternAnchor::Start) {
        for end in (1..=chars.len()).rev() {
            let substr: String = chars[..end].iter().collect();
            if glob_match(pattern, &substr, false, false) {
                let rest: String = chars[end..].iter().collect();
                return format!("{replacement}{rest}");
            }
        }
        return value.to_string();
    }

    if anchor == Some(PatternAnchor::End) {
        for start in 0..chars.len() {
            let substr: String = chars[start..].iter().collect();
            if glob_match(pattern, &substr, false, false) {
                let prefix: String = chars[..start].iter().collect();
                return format!("{prefix}{replacement}");
            }
        }
        return value.to_string();
    }

    let mut result = String::new();
    let mut i = 0;

    while i < chars.len() {
        let mut matched = false;
        for end in (i + 1..=chars.len()).rev() {
            let substr: String = chars[i..end].iter().collect();
            if glob_match(pattern, &substr, false, false) {
                result.push_str(replacement);
                i = end;
                matched = true;
                if !all {
                    let rest: String = chars[i..].iter().collect();
                    result.push_str(&rest);
                    return result;
                }
                break;
            }
        }
        if !matched {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

pub(super) fn modify_case(value: &str, direction: crate::ast::word::CaseDirection, all: bool) -> String {
    match direction {
        crate::ast::word::CaseDirection::Upper => {
            if all {
                value.to_uppercase()
            } else {
                let mut chars = value.chars();
                match chars.next() {
                    Some(c) => {
                        let mut s = c.to_uppercase().to_string();
                        s.extend(chars);
                        s
                    }
                    None => String::new(),
                }
            }
        }
        crate::ast::word::CaseDirection::Lower => {
            if all {
                value.to_lowercase()
            } else {
                let mut chars = value.chars();
                match chars.next() {
                    Some(c) => {
                        let mut s = c.to_lowercase().to_string();
                        s.extend(chars);
                        s
                    }
                    None => String::new(),
                }
            }
        }
    }
}
