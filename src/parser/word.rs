use crate::ast::{ArithExpr, Assignment};
use crate::ast::word::{
    BraceExpansion, CaseDirection, GlobPart, ParamOp, ParameterExpansion, PatternAnchor,
    PatternSide, ProcessDirection, TransformOp, Word, WordPart,
};

/// Parse a raw word string into a `Word` with proper parts.
pub(super) fn parse_word_string(s: &str) -> Word {
    let mut parts = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut literal = String::new();

    if bytes.len() >= 3 && (bytes[0] == b'<' || bytes[0] == b'>') && bytes[1] == b'(' && bytes[bytes.len() - 1] == b')' {
        let direction = if bytes[0] == b'<' { ProcessDirection::In } else { ProcessDirection::Out };
        let command: String = bytes[2..bytes.len() - 1].iter().map(|&b| b as char).collect();
        return Word {
            parts: vec![WordPart::ProcessSubstitution { command, direction }],
        };
    }

    while i < bytes.len() {
        match bytes[i] {
            b'\'' => {
                if !literal.is_empty() {
                    parts.push(WordPart::Literal(std::mem::take(&mut literal)));
                }
                i += 1;
                let start = i;
                while i < bytes.len() && bytes[i] != b'\'' {
                    i += 1;
                }
                let content: String = bytes[start..i]
                    .iter()
                    .map(|&b| b as char)
                    .collect();
                parts.push(WordPart::SingleQuoted(content));
                if i < bytes.len() {
                    i += 1;
                }
            }
            b'"' => {
                if !literal.is_empty() {
                    parts.push(WordPart::Literal(std::mem::take(&mut literal)));
                }
                i += 1;
                let mut inner_parts = Vec::new();
                let mut inner_lit = String::new();
                while i < bytes.len() && bytes[i] != b'"' {
                    if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        inner_lit.push(bytes[i + 1] as char);
                        i += 2;
                    } else if bytes[i] == b'$' {
                        if !inner_lit.is_empty() {
                            inner_parts.push(WordPart::Literal(std::mem::take(&mut inner_lit)));
                        }
                        let (part, consumed) = parse_dollar(&bytes[i..]);
                        inner_parts.push(part);
                        i += consumed;
                    } else {
                        inner_lit.push(bytes[i] as char);
                        i += 1;
                    }
                }
                if !inner_lit.is_empty() {
                    inner_parts.push(WordPart::Literal(inner_lit));
                }
                if i < bytes.len() {
                    i += 1;
                }
                parts.push(WordPart::DoubleQuoted(inner_parts));
            }
            b'\\' if i + 1 < bytes.len() => {
                if !literal.is_empty() {
                    parts.push(WordPart::Literal(std::mem::take(&mut literal)));
                }
                parts.push(WordPart::Escaped(bytes[i + 1] as char));
                i += 2;
            }
            b'$' => {
                if !literal.is_empty() {
                    parts.push(WordPart::Literal(std::mem::take(&mut literal)));
                }
                // Check for $'...' (ANSI-C quoting)
                if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    i += 2;
                    let start = i;
                    while i < bytes.len() && bytes[i] != b'\'' {
                        if bytes[i] == b'\\' && i + 1 < bytes.len() {
                            i += 2;
                        } else {
                            i += 1;
                        }
                    }
                    let content: String = bytes[start..i]
                        .iter()
                        .map(|&b| b as char)
                        .collect();
                    parts.push(WordPart::AnsiCQuoted(content));
                    if i < bytes.len() {
                        i += 1;
                    }
                } else {
                    let (part, consumed) = parse_dollar(&bytes[i..]);
                    parts.push(part);
                    i += consumed;
                }
            }
            b'`' => {
                if !literal.is_empty() {
                    parts.push(WordPart::Literal(std::mem::take(&mut literal)));
                }
                i += 1;
                let start = i;
                while i < bytes.len() && bytes[i] != b'`' {
                    if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                let cmd: String = bytes[start..i].iter().map(|&b| b as char).collect();
                parts.push(WordPart::CommandSubstitution(cmd));
                if i < bytes.len() {
                    i += 1;
                }
            }
            b'*' => {
                if !literal.is_empty() {
                    parts.push(WordPart::Literal(std::mem::take(&mut literal)));
                }
                if i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                    parts.push(WordPart::Glob(GlobPart::GlobStar));
                    i += 2;
                } else {
                    parts.push(WordPart::Glob(GlobPart::Star));
                    i += 1;
                }
            }
            b'?' => {
                if !literal.is_empty() {
                    parts.push(WordPart::Literal(std::mem::take(&mut literal)));
                }
                parts.push(WordPart::Glob(GlobPart::Question));
                i += 1;
            }
            b'[' => {
                // Only treat as glob char class if there's a matching ']' in the word
                if let Some(close) = bytes[i + 1..].iter().position(|&b| b == b']') {
                    if !literal.is_empty() {
                        parts.push(WordPart::Literal(std::mem::take(&mut literal)));
                    }
                    i += 1;
                    let negated = i < bytes.len() && (bytes[i] == b'!' || bytes[i] == b'^');
                    if negated {
                        i += 1;
                    }
                    let start = i;
                    let end = start + close - usize::from(negated);
                    let content: String = bytes[start..end].iter().map(|&b| b as char).collect();
                    i = end + 1;
                    parts.push(WordPart::Glob(GlobPart::CharClass { negated, content }));
                } else {
                    // No matching ] - treat as literal
                    literal.push('[');
                    i += 1;
                }
            }
            b'~' if i == 0 => {
                // Tilde expansion only at start of word
                i += 1;
                let start = i;
                while i < bytes.len() && bytes[i] != b'/' && !is_expansion_char(bytes[i]) {
                    i += 1;
                }
                let user: String = bytes[start..i].iter().map(|&b| b as char).collect();
                parts.push(WordPart::TildeExpansion(user));
            }
            b'{' => {
                if let Some((be, consumed)) = try_parse_brace_expansion(bytes, i) {
                    if !literal.is_empty() {
                        parts.push(WordPart::Literal(std::mem::take(&mut literal)));
                    }
                    parts.push(WordPart::BraceExpansion(be));
                    i += consumed;
                } else {
                    literal.push('{');
                    i += 1;
                }
            }
            _ => {
                literal.push(bytes[i] as char);
                i += 1;
            }
        }
    }

    if !literal.is_empty() {
        parts.push(WordPart::Literal(literal));
    }

    // If empty word (e.g. from empty quotes), produce at least one empty part
    if parts.is_empty() {
        parts.push(WordPart::Literal(String::new()));
    }

    Word { parts }
}

pub(super) fn is_expansion_char(b: u8) -> bool {
    matches!(b, b'$' | b'`' | b'\'' | b'"' | b'\\')
}

/// Parse a `$...` expression at the start of `bytes`. Returns the word part and
/// number of bytes consumed.
pub(super) fn parse_dollar(bytes: &[u8]) -> (WordPart, usize) {
    if bytes.len() < 2 {
        return (WordPart::Literal("$".to_string()), 1);
    }

    match bytes[1] {
        b'(' => {
            if bytes.len() > 2 && bytes[2] == b'(' {
                // Start at first '(' to track depth 2, then strip both layers of parens
                let (content, len) = extract_balanced(bytes, 1, b'(', b')');
                let inner = content
                    .strip_prefix("((").unwrap_or(&content)
                    .strip_suffix("))").unwrap_or(&content)
                    .to_string();
                let expr = if let Ok(n) = inner.trim().parse::<i64>() {
                    ArithExpr::Number(n)
                } else {
                    ArithExpr::Variable(inner)
                };
                (WordPart::ArithmeticExpansion(expr), len + 1)
            } else {
                let (content, len) = extract_balanced(bytes, 1, b'(', b')');
                let inner = &content[1..content.len().saturating_sub(1)];
                (WordPart::CommandSubstitution(inner.to_string()), len + 1)
            }
        }
        b'{' => {
            let (content, len) = extract_balanced(bytes, 1, b'{', b'}');
            let inner = &content[1..content.len().saturating_sub(1)];
            let expansion = parse_param_expansion(inner);
            (WordPart::Parameter(expansion), len + 1)
        }
        b if b.is_ascii_alphabetic() || b == b'_' => {
            let mut end = 2;
            while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
                end += 1;
            }
            let name: String = bytes[1..end].iter().map(|&b| b as char).collect();
            (
                WordPart::Parameter(ParameterExpansion {
                    name,
                    subscript: None,
                    operation: None,
                }),
                end,
            )
        }
        b'?' | b'$' | b'!' | b'#' | b'@' | b'*' | b'-' | b'_' => {
            let name = String::from(bytes[1] as char);
            (
                WordPart::Parameter(ParameterExpansion {
                    name,
                    subscript: None,
                    operation: None,
                }),
                2,
            )
        }
        b if b.is_ascii_digit() => {
            let name = String::from(bytes[1] as char);
            (
                WordPart::Parameter(ParameterExpansion {
                    name,
                    subscript: None,
                    operation: None,
                }),
                2,
            )
        }
        _ => (WordPart::Literal("$".to_string()), 1),
    }
}

/// Extract balanced content starting from `start` in `bytes`, tracking `open`/`close` delimiters.
/// Returns the content (including delimiters) and total bytes consumed from `start`.
pub(super) fn extract_balanced(bytes: &[u8], start: usize, open: u8, close: u8) -> (String, usize) {
    let mut depth = 0u32;
    let mut i = start;
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    while i < bytes.len() {
        let b = bytes[i];
        if in_single_quote {
            if b == b'\'' {
                in_single_quote = false;
            }
            i += 1;
            continue;
        }
        if in_double_quote {
            if b == b'"' {
                in_double_quote = false;
            } else if b == b'\\' && i + 1 < bytes.len() {
                i += 1;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' => in_single_quote = true,
            b'"' => in_double_quote = true,
            b'\\' if i + 1 < bytes.len() => {
                i += 2;
                continue;
            }
            _ if b == open => depth += 1,
            _ if b == close => {
                depth -= 1;
                if depth == 0 {
                    let content: String = bytes[start..=i].iter().map(|&b| b as char).collect();
                    return (content, i - start + 1);
                }
            }
            _ => {}
        }
        i += 1;
    }

    // Unterminated - return what we have
    let content: String = bytes[start..i].iter().map(|&b| b as char).collect();
    (content, i - start)
}

/// Parse the inner content of `${...}` into a `ParameterExpansion`.
pub(super) fn parse_param_expansion(inner: &str) -> ParameterExpansion {
    let bytes = inner.as_bytes();
    if bytes.is_empty() {
        return ParameterExpansion {
            name: String::new(),
            subscript: None,
            operation: None,
        };
    }

    if bytes[0] == b'#' && bytes.len() > 1 && !bytes[1..].contains(&b':') {
        let rest_str: String = bytes[1..].iter().map(|&b| b as char).collect();
        let (name, subscript) = extract_subscript(&rest_str);
        return ParameterExpansion {
            name,
            subscript,
            operation: Some(ParamOp::Length),
        };
    }

    if bytes[0] == b'!' && bytes.len() > 1 {
        let rest: String = bytes[1..].iter().map(|&b| b as char).collect();
        if rest.contains('[') {
            let (name, _) = extract_subscript(&rest);
            if rest.ends_with("[@]") || rest.ends_with("[*]") {
                let star = rest.ends_with("[*]");
                return ParameterExpansion {
                    name,
                    subscript: None,
                    operation: Some(ParamOp::ArrayKeys { star }),
                };
            }
        }
        if rest.ends_with('*') || rest.ends_with('@') {
            let star = rest.ends_with('*');
            return ParameterExpansion {
                name: rest[..rest.len() - 1].to_string(),
                subscript: None,
                operation: Some(ParamOp::VarNamePrefix { star }),
            };
        }
        return ParameterExpansion {
            name: rest,
            subscript: None,
            operation: Some(ParamOp::Indirection),
        };
    }

    let mut name_end = 0;
    while name_end < bytes.len()
        && (bytes[name_end].is_ascii_alphanumeric()
            || bytes[name_end] == b'_'
            || (name_end == 0
                && (bytes[name_end] == b'?' || bytes[name_end] == b'#'
                    || bytes[name_end] == b'$' || bytes[name_end] == b'!'
                    || bytes[name_end] == b'@' || bytes[name_end] == b'*'
                    || bytes[name_end] == b'-' || bytes[name_end].is_ascii_digit())))
    {
        name_end += 1;
        // Special single-char variables: only take one char
        if name_end == 1
            && matches!(
                bytes[0],
                b'?' | b'#' | b'$' | b'!' | b'@' | b'*' | b'-'
            )
        {
            break;
        }
    }

    let name: String = bytes[..name_end].iter().map(|&b| b as char).collect();
    let rest = &inner[name_end..];

    let (subscript, rest) = if rest.starts_with('[') {
        if let Some(close) = rest.find(']') {
            let sub_str = &rest[1..close];
            let sub_word = parse_word_string(sub_str);
            (Some(Box::new(sub_word)), &rest[close + 1..])
        } else {
            (None, rest)
        }
    } else {
        (None, rest)
    };

    if rest.is_empty() {
        return ParameterExpansion {
            name,
            subscript,
            operation: None,
        };
    }

    let operation = parse_param_op(rest);

    ParameterExpansion {
        name,
        subscript,
        operation,
    }
}

pub(super) fn parse_param_op(rest: &str) -> Option<ParamOp> {
    let bytes = rest.as_bytes();
    if bytes.is_empty() {
        return None;
    }

    match bytes[0] {
        b':' if bytes.len() > 1 => match bytes[1] {
            b'-' => Some(ParamOp::Default {
                word: parse_word_string(&rest[2..]),
                colon: true,
            }),
            b'=' => Some(ParamOp::AssignDefault {
                word: parse_word_string(&rest[2..]),
                colon: true,
            }),
            b'?' => Some(ParamOp::Error {
                word: parse_word_string(&rest[2..]),
                colon: true,
            }),
            b'+' => Some(ParamOp::Alternative {
                word: parse_word_string(&rest[2..]),
                colon: true,
            }),
            _ => {
                // ${var:offset} or ${var:offset:length}
                let sub = &rest[1..];
                if let Some(colon_pos) = sub[1..].find(':').map(|p| p + 1) {
                    Some(ParamOp::Substring {
                        offset: parse_word_string(&sub[..colon_pos]),
                        length: Some(parse_word_string(&sub[colon_pos + 1..])),
                    })
                } else {
                    Some(ParamOp::Substring {
                        offset: parse_word_string(sub),
                        length: None,
                    })
                }
            }
        },
        b'-' => Some(ParamOp::Default {
            word: parse_word_string(&rest[1..]),
            colon: false,
        }),
        b'=' => Some(ParamOp::AssignDefault {
            word: parse_word_string(&rest[1..]),
            colon: false,
        }),
        b'?' => Some(ParamOp::Error {
            word: parse_word_string(&rest[1..]),
            colon: false,
        }),
        b'+' => Some(ParamOp::Alternative {
            word: parse_word_string(&rest[1..]),
            colon: false,
        }),
        b'#' => {
            if bytes.len() > 1 && bytes[1] == b'#' {
                Some(ParamOp::PatternRemoval {
                    pattern: parse_word_string(&rest[2..]),
                    side: PatternSide::Prefix,
                    greedy: true,
                })
            } else {
                Some(ParamOp::PatternRemoval {
                    pattern: parse_word_string(&rest[1..]),
                    side: PatternSide::Prefix,
                    greedy: false,
                })
            }
        }
        b'%' => {
            if bytes.len() > 1 && bytes[1] == b'%' {
                Some(ParamOp::PatternRemoval {
                    pattern: parse_word_string(&rest[2..]),
                    side: PatternSide::Suffix,
                    greedy: true,
                })
            } else {
                Some(ParamOp::PatternRemoval {
                    pattern: parse_word_string(&rest[1..]),
                    side: PatternSide::Suffix,
                    greedy: false,
                })
            }
        }
        b'/' => {
            let all = bytes.len() > 1 && bytes[1] == b'/';
            let pattern_start = if all { 2 } else { 1 };
            let pattern_rest = &rest[pattern_start..];
            let (pattern, replacement) = if let Some(slash_pos) = pattern_rest.find('/') {
                (
                    &pattern_rest[..slash_pos],
                    Some(parse_word_string(&pattern_rest[slash_pos + 1..])),
                )
            } else {
                (pattern_rest, None)
            };
            let anchor = if pattern.starts_with('#') {
                Some(PatternAnchor::Start)
            } else if pattern.starts_with('%') {
                Some(PatternAnchor::End)
            } else {
                None
            };
            let pattern_trimmed = if anchor.is_some() {
                &pattern[1..]
            } else {
                pattern
            };
            Some(ParamOp::PatternReplace {
                pattern: parse_word_string(pattern_trimmed),
                replacement,
                all,
                anchor,
            })
        }
        b'^' => {
            let all = bytes.len() > 1 && bytes[1] == b'^';
            Some(ParamOp::CaseModify {
                direction: CaseDirection::Upper,
                all,
            })
        }
        b',' => {
            let all = bytes.len() > 1 && bytes[1] == b',';
            Some(ParamOp::CaseModify {
                direction: CaseDirection::Lower,
                all,
            })
        }
        b'@' if bytes.len() > 1 => {
            let op = match bytes[1] {
                b'Q' => TransformOp::Quote,
                b'E' => TransformOp::Escape,
                b'P' => TransformOp::Prompt,
                b'A' => TransformOp::Assignment,
                b'a' => TransformOp::Attributes,
                b'K' => TransformOp::KeyValue,
                b'k' => TransformOp::KeyValueUnquoted,
                b'u' => TransformOp::UpperFirst,
                b'U' => TransformOp::UpperAll,
                b'L' => TransformOp::LowerAll,
                _ => return None,
            };
            Some(ParamOp::Transform(op))
        }
        _ => None,
    }
}

pub(super) fn parse_assignment_word(s: &str) -> Assignment {
    let (name, rest, append) = if let Some(pos) = s.find("+=") {
        (&s[..pos], &s[pos + 2..], true)
    } else if let Some(pos) = s.find('=') {
        (&s[..pos], &s[pos + 1..], false)
    } else {
        return Assignment {
            name: s.to_string(),
            value: None,
            append: false,
            array: None,
        };
    };

    if rest.starts_with('(') && rest.ends_with(')') {
        let inner = &rest[1..rest.len() - 1];
        let elements: Vec<Word> = inner
            .split_whitespace()
            .map(parse_word_string)
            .collect();
        return Assignment {
            name: name.to_string(),
            value: None,
            append,
            array: Some(elements),
        };
    }

    Assignment {
        name: name.to_string(),
        value: if rest.is_empty() {
            Some(Word::literal(""))
        } else {
            Some(parse_word_string(rest))
        },
        append,
        array: None,
    }
}

pub(super) fn word_to_string(w: &Word) -> String {
    let mut s = String::new();
    for part in &w.parts {
        match part {
            WordPart::Literal(lit) => s.push_str(lit),
            WordPart::SingleQuoted(sq) => s.push_str(sq),
            WordPart::Escaped(c) => s.push(*c),
            _ => {} // Other parts are expanded at runtime
        }
    }
    s
}

fn try_parse_brace_expansion(bytes: &[u8], start: usize) -> Option<(BraceExpansion, usize)> {
    let mut depth = 0u32;
    let mut i = start;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    let inner: String = bytes[start + 1..i].iter().map(|&b| b as char).collect();
                    let consumed = i - start + 1;

                    if let Some(be) = parse_brace_inner(&inner) {
                        return Some((be, consumed));
                    }
                    return None;
                }
            }
            b'\\' if i + 1 < bytes.len() => {
                i += 2;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn parse_brace_inner(inner: &str) -> Option<BraceExpansion> {
    if let Some(range) = try_parse_range(inner) {
        return Some(range);
    }

    if inner.contains(',') {
        let items: Vec<Word> = split_brace_list(inner)
            .into_iter()
            .map(|s| parse_word_string(&s))
            .collect();
        if items.len() >= 2 {
            return Some(BraceExpansion::List(items));
        }
    }

    None
}

fn try_parse_range(inner: &str) -> Option<BraceExpansion> {
    let parts: Vec<&str> = inner.split("..").collect();
    if parts.len() < 2 || parts.len() > 3 {
        return None;
    }

    let start = parts[0];
    let end = parts[1];
    let step = if parts.len() == 3 { Some(parts[2].to_string()) } else { None };

    let is_numeric = start.parse::<i64>().is_ok() && end.parse::<i64>().is_ok();
    let is_char = start.len() == 1
        && end.len() == 1
        && start.as_bytes()[0].is_ascii_alphabetic()
        && end.as_bytes()[0].is_ascii_alphabetic();

    if is_numeric || is_char {
        Some(BraceExpansion::Range {
            start: start.to_string(),
            end: end.to_string(),
            step,
        })
    } else {
        None
    }
}

fn extract_subscript(s: &str) -> (String, Option<Box<Word>>) {
    if let Some(open) = s.find('[') {
        if let Some(close) = s[open..].find(']') {
            let name = s[..open].to_string();
            let sub_str = &s[open + 1..open + close];
            let sub_word = parse_word_string(sub_str);
            return (name, Some(Box::new(sub_word)));
        }
    }
    (s.to_string(), None)
}

fn split_brace_list(s: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut depth = 0u32;
    for c in s.chars() {
        match c {
            '{' => {
                depth += 1;
                current.push(c);
            }
            '}' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                items.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    items.push(current);
    items
}
