#[derive(Debug, Clone, PartialEq)]
pub(super) enum Token {
    Number(f64),
    Str(String),
    Regex(String),
    Ident(String),
    Begin,
    End,
    If,
    Else,
    While,
    For,
    Do,
    In,
    Break,
    Continue,
    Next,
    Exit,
    Return,
    Delete,
    Function,
    Getline,
    Print,
    Printf,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Semi,
    Comma,
    Newline,
    Dollar,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    Assign,
    PlusAssign,
    MinusAssign,
    StarAssign,
    SlashAssign,
    PercentAssign,
    CaretAssign,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Not,
    Match,
    NotMatch,
    Append,
    Pipe,
    PlusPlus,
    MinusMinus,
    Question,
    Colon,
    Eof,
}

fn is_keyword(s: &str) -> Option<Token> {
    match s {
        "BEGIN" => Some(Token::Begin),
        "END" => Some(Token::End),
        "if" => Some(Token::If),
        "else" => Some(Token::Else),
        "while" => Some(Token::While),
        "for" => Some(Token::For),
        "do" => Some(Token::Do),
        "in" => Some(Token::In),
        "break" => Some(Token::Break),
        "continue" => Some(Token::Continue),
        "next" => Some(Token::Next),
        "exit" => Some(Token::Exit),
        "return" => Some(Token::Return),
        "delete" => Some(Token::Delete),
        "function" => Some(Token::Function),
        "getline" => Some(Token::Getline),
        "print" => Some(Token::Print),
        "printf" => Some(Token::Printf),
        _ => None,
    }
}

fn can_precede_regex(prev: &Token) -> bool {
    matches!(
        prev,
        Token::Newline
            | Token::Semi
            | Token::LParen
            | Token::LBrace
            | Token::RBrace
            | Token::Comma
            | Token::Not
            | Token::And
            | Token::Or
            | Token::Match
            | Token::NotMatch
            | Token::Return
            | Token::Print
            | Token::Printf
    )
}

#[allow(clippy::unnecessary_wraps)]
pub(super) fn lex(src: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let bytes = src.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        match bytes[i] {
            b' ' | b'\t' | b'\r' => {
                i += 1;
            }
            b'#' => {
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'\\' if i + 1 < len && bytes[i + 1] == b'\n' => {
                i += 2;
            }
            b'\n' => {
                if let Some(last) = tokens.last() {
                    if !matches!(
                        last,
                        Token::Newline
                            | Token::Semi
                            | Token::LBrace
                            | Token::Comma
                            | Token::And
                            | Token::Or
                            | Token::Do
                    ) {
                        tokens.push(Token::Newline);
                    }
                }
                i += 1;
            }
            b'"' => {
                i += 1;
                let mut s = String::new();
                while i < len && bytes[i] != b'"' {
                    if bytes[i] == b'\\' && i + 1 < len {
                        i += 1;
                        match bytes[i] {
                            b'n' => s.push('\n'),
                            b't' => s.push('\t'),
                            b'r' => s.push('\r'),
                            b'\\' => s.push('\\'),
                            b'"' => s.push('"'),
                            b'a' => s.push('\x07'),
                            b'b' => s.push('\x08'),
                            b'f' => s.push('\x0c'),
                            b'v' => s.push('\x0b'),
                            b'/' => s.push('/'),
                            other => {
                                s.push('\\');
                                s.push(other as char);
                            }
                        }
                    } else {
                        s.push(bytes[i] as char);
                    }
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
                tokens.push(Token::Str(s));
            }
            b'/' => {
                let prev_is_regex = tokens.is_empty()
                    || tokens.last().is_none_or(can_precede_regex);
                if prev_is_regex {
                    i += 1;
                    let mut pat = String::new();
                    while i < len && bytes[i] != b'/' {
                        if bytes[i] == b'\\' && i + 1 < len {
                            pat.push(bytes[i] as char);
                            i += 1;
                            pat.push(bytes[i] as char);
                        } else {
                            pat.push(bytes[i] as char);
                        }
                        i += 1;
                    }
                    if i < len {
                        i += 1;
                    }
                    tokens.push(Token::Regex(pat));
                } else if i + 1 < len && bytes[i + 1] == b'=' {
                    tokens.push(Token::SlashAssign);
                    i += 2;
                } else {
                    tokens.push(Token::Slash);
                    i += 1;
                }
            }
            b'0'..=b'9' | b'.' if (bytes[i] != b'.' || (i + 1 < len && bytes[i + 1].is_ascii_digit())) => {
                let start = i;
                if i + 1 < len && bytes[i] == b'0' && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X') {
                    i += 2;
                    while i < len && bytes[i].is_ascii_hexdigit() {
                        i += 1;
                    }
                    let hex_str: String = bytes[start + 2..i].iter().map(|&b| b as char).collect();
                    let val = i64::from_str_radix(&hex_str, 16).unwrap_or(0);
                    tokens.push(Token::Number(val as f64));
                } else {
                    while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                        i += 1;
                    }
                    if i < len && (bytes[i] == b'e' || bytes[i] == b'E') {
                        i += 1;
                        if i < len && (bytes[i] == b'+' || bytes[i] == b'-') {
                            i += 1;
                        }
                        while i < len && bytes[i].is_ascii_digit() {
                            i += 1;
                        }
                    }
                    let num_str: String = bytes[start..i].iter().map(|&b| b as char).collect();
                    let val: f64 = num_str.parse().unwrap_or(0.0);
                    tokens.push(Token::Number(val));
                }
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                let start = i;
                while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let word: String = bytes[start..i].iter().map(|&b| b as char).collect();
                if let Some(kw) = is_keyword(&word) {
                    tokens.push(kw);
                } else {
                    tokens.push(Token::Ident(word));
                }
            }
            b'(' => { tokens.push(Token::LParen); i += 1; }
            b')' => { tokens.push(Token::RParen); i += 1; }
            b'{' => { tokens.push(Token::LBrace); i += 1; }
            b'}' => { tokens.push(Token::RBrace); i += 1; }
            b'[' => { tokens.push(Token::LBracket); i += 1; }
            b']' => { tokens.push(Token::RBracket); i += 1; }
            b';' => { tokens.push(Token::Semi); i += 1; }
            b',' => { tokens.push(Token::Comma); i += 1; }
            b'$' => { tokens.push(Token::Dollar); i += 1; }
            b'?' => { tokens.push(Token::Question); i += 1; }
            b':' => { tokens.push(Token::Colon); i += 1; }
            b'+' => {
                if i + 1 < len && bytes[i + 1] == b'+' {
                    tokens.push(Token::PlusPlus); i += 2;
                } else if i + 1 < len && bytes[i + 1] == b'=' {
                    tokens.push(Token::PlusAssign); i += 2;
                } else {
                    tokens.push(Token::Plus); i += 1;
                }
            }
            b'-' => {
                if i + 1 < len && bytes[i + 1] == b'-' {
                    tokens.push(Token::MinusMinus); i += 2;
                } else if i + 1 < len && bytes[i + 1] == b'=' {
                    tokens.push(Token::MinusAssign); i += 2;
                } else {
                    tokens.push(Token::Minus); i += 1;
                }
            }
            b'*' => {
                if i + 1 < len && bytes[i + 1] == b'=' {
                    tokens.push(Token::StarAssign); i += 2;
                } else {
                    tokens.push(Token::Star); i += 1;
                }
            }
            b'%' => {
                if i + 1 < len && bytes[i + 1] == b'=' {
                    tokens.push(Token::PercentAssign); i += 2;
                } else {
                    tokens.push(Token::Percent); i += 1;
                }
            }
            b'^' => {
                if i + 1 < len && bytes[i + 1] == b'=' {
                    tokens.push(Token::CaretAssign); i += 2;
                } else {
                    tokens.push(Token::Caret); i += 1;
                }
            }
            b'=' => {
                if i + 1 < len && bytes[i + 1] == b'=' {
                    tokens.push(Token::Eq); i += 2;
                } else {
                    tokens.push(Token::Assign); i += 1;
                }
            }
            b'!' => {
                if i + 1 < len && bytes[i + 1] == b'=' {
                    tokens.push(Token::Ne); i += 2;
                } else if i + 1 < len && bytes[i + 1] == b'~' {
                    tokens.push(Token::NotMatch); i += 2;
                } else {
                    tokens.push(Token::Not); i += 1;
                }
            }
            b'<' => {
                if i + 1 < len && bytes[i + 1] == b'=' {
                    tokens.push(Token::Le); i += 2;
                } else {
                    tokens.push(Token::Lt); i += 1;
                }
            }
            b'>' => {
                if i + 1 < len && bytes[i + 1] == b'>' {
                    tokens.push(Token::Append); i += 2;
                } else if i + 1 < len && bytes[i + 1] == b'=' {
                    tokens.push(Token::Ge); i += 2;
                } else {
                    tokens.push(Token::Gt); i += 1;
                }
            }
            b'&' => {
                if i + 1 < len && bytes[i + 1] == b'&' {
                    tokens.push(Token::And); i += 2;
                } else {
                    i += 1;
                }
            }
            b'|' => {
                if i + 1 < len && bytes[i + 1] == b'|' {
                    tokens.push(Token::Or); i += 2;
                } else {
                    tokens.push(Token::Pipe); i += 1;
                }
            }
            b'~' => { tokens.push(Token::Match); i += 1; }
            _ => {
                i += 1;
            }
        }
    }

    tokens.push(Token::Eof);
    Ok(tokens)
}
