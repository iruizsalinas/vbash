use super::ast::StringPart;
use super::parser::parse_tokens;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Token {
    Dot,
    Pipe,
    Comma,
    Colon,
    Semicolon,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Question,
    DotDot,
    SlashSlash,
    Eq,
    PipeEq,
    PlusEq,
    MinusEq,
    MulEq,
    DivEq,
    ModEq,
    EqEq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
    Plus,
    Minus,
    Mul,
    Div,
    Mod,
    Not,
    And,
    Or,
    If,
    Then,
    Elif,
    Else,
    End,
    Try,
    Catch,
    As,
    Def,
    Reduce,
    Foreach,
    Label,
    Break,
    Ident(String),
    Var(String),
    Str(String),
    Number(f64),
    True,
    False,
    Null,
    Format(String),
}

thread_local! {
    pub(super) static INTERP_PARTS: std::cell::RefCell<Vec<Vec<StringPart>>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub(super) fn lex(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            ' ' | '\t' | '\n' | '\r' => { i += 1; }
            '#' => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '.' => {
                if i + 1 < chars.len() && chars[i + 1] == '.' {
                    tokens.push(Token::DotDot);
                    i += 2;
                } else if i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
                    let start = i;
                    i += 1;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                    if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                        i += 1;
                        if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
                            i += 1;
                        }
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                    }
                    let s: String = chars[start..i].iter().collect();
                    let n = s.parse::<f64>().map_err(|e| format!("bad number: {e}"))?;
                    tokens.push(Token::Number(n));
                } else {
                    tokens.push(Token::Dot);
                    i += 1;
                }
            }
            '|' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::PipeEq);
                    i += 2;
                } else {
                    tokens.push(Token::Pipe);
                    i += 1;
                }
            }
            ',' => { tokens.push(Token::Comma); i += 1; }
            ':' => { tokens.push(Token::Colon); i += 1; }
            ';' => { tokens.push(Token::Semicolon); i += 1; }
            '(' => { tokens.push(Token::LParen); i += 1; }
            ')' => { tokens.push(Token::RParen); i += 1; }
            '[' => { tokens.push(Token::LBracket); i += 1; }
            ']' => { tokens.push(Token::RBracket); i += 1; }
            '{' => { tokens.push(Token::LBrace); i += 1; }
            '}' => { tokens.push(Token::RBrace); i += 1; }
            '?' => {
                if i + 1 < chars.len() && chars[i + 1] == '/' && i + 2 < chars.len() && chars[i + 2] == '/' {
                    tokens.push(Token::Question);
                    tokens.push(Token::SlashSlash);
                    i += 3;
                } else {
                    tokens.push(Token::Question);
                    i += 1;
                }
            }
            '/' => {
                if i + 1 < chars.len() && chars[i + 1] == '/' {
                    tokens.push(Token::SlashSlash);
                    i += 2;
                } else if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::DivEq);
                    i += 2;
                } else {
                    tokens.push(Token::Div);
                    i += 1;
                }
            }
            '=' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::EqEq);
                    i += 2;
                } else {
                    tokens.push(Token::Eq);
                    i += 1;
                }
            }
            '!' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::NotEq);
                    i += 2;
                } else {
                    tokens.push(Token::Not);
                    i += 1;
                }
            }
            '<' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::Le);
                    i += 2;
                } else {
                    tokens.push(Token::Lt);
                    i += 1;
                }
            }
            '>' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::Ge);
                    i += 2;
                } else {
                    tokens.push(Token::Gt);
                    i += 1;
                }
            }
            '+' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::PlusEq);
                    i += 2;
                } else {
                    tokens.push(Token::Plus);
                    i += 1;
                }
            }
            '-' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::MinusEq);
                    i += 2;
                } else if i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
                    let prev_is_value = tokens.last().is_some_and(|t| matches!(t,
                        Token::Number(_) | Token::Str(_) | Token::True | Token::False | Token::Null |
                        Token::RParen | Token::RBracket | Token::RBrace | Token::Ident(_) | Token::Var(_) | Token::Dot
                    ));
                    if prev_is_value {
                        tokens.push(Token::Minus);
                        i += 1;
                    } else {
                        let start = i;
                        i += 1;
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                        if i < chars.len() && chars[i] == '.' {
                            i += 1;
                            while i < chars.len() && chars[i].is_ascii_digit() {
                                i += 1;
                            }
                        }
                        if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                            i += 1;
                            if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
                                i += 1;
                            }
                            while i < chars.len() && chars[i].is_ascii_digit() {
                                i += 1;
                            }
                        }
                        let s: String = chars[start..i].iter().collect();
                        let n = s.parse::<f64>().map_err(|e| format!("bad number: {e}"))?;
                        tokens.push(Token::Number(n));
                    }
                } else {
                    tokens.push(Token::Minus);
                    i += 1;
                }
            }
            '*' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::MulEq);
                    i += 2;
                } else {
                    tokens.push(Token::Mul);
                    i += 1;
                }
            }
            '%' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::ModEq);
                    i += 2;
                } else {
                    tokens.push(Token::Mod);
                    i += 1;
                }
            }
            '@' => {
                i += 1;
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let name: String = chars[start..i].iter().collect();
                tokens.push(Token::Format(name));
            }
            '$' => {
                i += 1;
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let name: String = chars[start..i].iter().collect();
                if name.is_empty() {
                    return Err("expected variable name after $".to_string());
                }
                tokens.push(Token::Var(name));
            }
            '"' => {
                i += 1;
                let mut s = String::new();
                let mut parts: Vec<StringPart> = Vec::new();
                let mut has_interp = false;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1;
                        match chars[i] {
                            'n' => s.push('\n'),
                            't' => s.push('\t'),
                            'r' => s.push('\r'),
                            '\\' => s.push('\\'),
                            '"' => s.push('"'),
                            '/' => s.push('/'),
                            '(' => {
                                has_interp = true;
                                if !s.is_empty() {
                                    parts.push(StringPart::Literal(std::mem::take(&mut s)));
                                }
                                i += 1;
                                let mut depth = 1u32;
                                let expr_start = i;
                                while i < chars.len() && depth > 0 {
                                    if chars[i] == '(' {
                                        depth += 1;
                                    } else if chars[i] == ')' {
                                        depth -= 1;
                                    }
                                    if depth > 0 {
                                        i += 1;
                                    }
                                }
                                let expr_str: String = chars[expr_start..i].iter().collect();
                                let inner_tokens = lex(&expr_str)?;
                                let inner_expr = parse_tokens(&inner_tokens)?;
                                parts.push(StringPart::Expr(inner_expr));
                                i += 1;
                                continue;
                            }
                            'u' => {
                                i += 1;
                                let mut hex = String::new();
                                while i < chars.len() && hex.len() < 4 && chars[i].is_ascii_hexdigit() {
                                    hex.push(chars[i]);
                                    i += 1;
                                }
                                if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                                    if let Some(ch) = char::from_u32(cp) {
                                        s.push(ch);
                                    }
                                }
                                continue;
                            }
                            other => { s.push('\\'); s.push(other); }
                        }
                        i += 1;
                    } else {
                        s.push(chars[i]);
                        i += 1;
                    }
                }
                if i < chars.len() {
                    i += 1;
                }
                if has_interp {
                    if !s.is_empty() {
                        parts.push(StringPart::Literal(s));
                    }
                    tokens.push(Token::Str(String::new()));
                    tokens.pop();
                    tokens.push(Token::Str(format!("\x00INTERP:{}", parts.len())));
                    INTERP_PARTS.with(|cell| {
                        cell.borrow_mut().push(parts);
                    });
                } else {
                    tokens.push(Token::Str(s));
                }
            }
            c if c.is_ascii_digit() => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                if i < chars.len() && chars[i] == '.' {
                    i += 1;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                    i += 1;
                    if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
                        i += 1;
                    }
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                let s: String = chars[start..i].iter().collect();
                let n = s.parse::<f64>().map_err(|e| format!("bad number: {e}"))?;
                tokens.push(Token::Number(n));
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                match word.as_str() {
                    "not" => tokens.push(Token::Not),
                    "and" => tokens.push(Token::And),
                    "or" => tokens.push(Token::Or),
                    "if" => tokens.push(Token::If),
                    "then" => tokens.push(Token::Then),
                    "elif" => tokens.push(Token::Elif),
                    "else" => tokens.push(Token::Else),
                    "end" => tokens.push(Token::End),
                    "try" => tokens.push(Token::Try),
                    "catch" => tokens.push(Token::Catch),
                    "as" => tokens.push(Token::As),
                    "def" => tokens.push(Token::Def),
                    "reduce" => tokens.push(Token::Reduce),
                    "foreach" => tokens.push(Token::Foreach),
                    "label" => tokens.push(Token::Label),
                    "break" => tokens.push(Token::Break),
                    "true" => tokens.push(Token::True),
                    "false" => tokens.push(Token::False),
                    "null" => tokens.push(Token::Null),
                    _ => tokens.push(Token::Ident(word)),
                }
            }
            other => {
                return Err(format!("unexpected character: '{other}'"));
            }
        }
    }
    Ok(tokens)
}
