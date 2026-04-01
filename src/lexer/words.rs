use crate::error::ParseError;

use super::{is_word_boundary, Lexer, Token};

fn is_valid_array_name(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    if !bytes[0].is_ascii_alphabetic() && bytes[0] != b'_' {
        return false;
    }
    bytes[1..].iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_')
}

impl Lexer<'_> {
    pub(super) fn read_word(&mut self) -> Result<Token, ParseError> {
        let span = self.span();
        let mut value = String::new();
        let mut is_quoted = false;

        while !self.at_end() {
            let ch = self.peek_char();

            if is_word_boundary(ch) && !is_quoted {
                if ch == b'{' && self.is_brace_expansion() {
                    // Fall through to brace expansion handling below
                } else if (ch == b'<' || ch == b'>') && self.peek_at(1) == b'(' && value.is_empty() {
                    // Fall through to process substitution handling below
                } else if ch == b'(' && !value.is_empty() && matches!(value.as_bytes()[value.len() - 1], b'@' | b'*' | b'+' | b'?' | b'!') {
                    // Extglob pattern: @(...), *(...), +(...), ?(...), !(...)
                    // Fall through to extglob handling below
                } else {
                    break;
                }
            }

            match ch {
                b'\'' if !is_quoted => {
                    value.push('\'');
                    self.advance();
                    loop {
                        if self.at_end() {
                            return Err(self.error("unterminated single quote".to_string()));
                        }
                        let c = self.peek_char();
                        self.advance();
                        value.push(c as char);
                        if c == b'\'' {
                            break;
                        }
                    }
                }
                b'"' => {
                    value.push('"');
                    self.advance();
                    is_quoted = !is_quoted;
                    if is_quoted {
                        loop {
                            if self.at_end() {
                                return Err(self.error("unterminated double quote".to_string()));
                            }
                            let c = self.peek_char();
                            if c == b'"' {
                                value.push('"');
                                self.advance();
                                is_quoted = false;
                                break;
                            }
                            if c == b'\\' && self.pos + 1 < self.input.len() {
                                let next = self.input[self.pos + 1];
                                if next == b'\n' {
                                    self.advance();
                                    self.advance();
                                    continue;
                                }
                                value.push('\\');
                                self.advance();
                                value.push(self.peek_char() as char);
                                self.advance();
                                continue;
                            }
                            value.push(c as char);
                            self.advance();
                        }
                    }
                }
                b'\\' => {
                    if self.pos + 1 < self.input.len() {
                        let next = self.input[self.pos + 1];
                        if next == b'\n' {
                            self.advance();
                            self.advance();
                        } else {
                            value.push('\\');
                            self.advance();
                            value.push(self.peek_char() as char);
                            self.advance();
                        }
                    } else {
                        value.push('\\');
                        self.advance();
                    }
                }
                b'$' if self.peek_at(1) == b'\'' => {
                    value.push('$');
                    value.push('\'');
                    self.advance_n(2);
                    loop {
                        if self.at_end() {
                            return Err(self.error("unterminated $'...' quote".to_string()));
                        }
                        let c = self.peek_char();
                        value.push(c as char);
                        self.advance();
                        if c == b'\'' {
                            break;
                        }
                        if c == b'\\' && !self.at_end() {
                            value.push(self.peek_char() as char);
                            self.advance();
                        }
                    }
                }
                b'`' => {
                    value.push('`');
                    self.advance();
                    loop {
                        if self.at_end() {
                            return Err(self.error("unterminated backtick".to_string()));
                        }
                        let c = self.peek_char();
                        value.push(c as char);
                        self.advance();
                        if c == b'`' {
                            break;
                        }
                        if c == b'\\' && !self.at_end() {
                            value.push(self.peek_char() as char);
                            self.advance();
                        }
                    }
                }
                b'<' | b'>' if self.peek_at(1) == b'(' && value.is_empty() => {
                    self.read_process_substitution(&mut value)?;
                }
                b'$' if self.peek_at(1) == b'(' => {
                    self.read_dollar_paren(&mut value)?;
                }
                b'$' if self.peek_at(1) == b'{' => {
                    self.read_dollar_brace(&mut value)?;
                }
                b'{' if !is_quoted => {
                    if let Some(brace_str) = self.try_read_brace_expansion() {
                        value.push_str(&brace_str);
                    } else {
                        break;
                    }
                }
                b'(' if !value.is_empty() && matches!(value.as_bytes()[value.len() - 1], b'@' | b'*' | b'+' | b'?' | b'!') => {
                    // Read extglob pattern: @(...), *(...), +(...), ?(...), !(...)
                    value.push('(');
                    self.advance();
                    let mut depth = 1u32;
                    while !self.at_end() && depth > 0 {
                        let c = self.peek_char();
                        value.push(c as char);
                        self.advance();
                        match c {
                            b'(' => depth += 1,
                            b')' => depth -= 1,
                            _ => {}
                        }
                    }
                }
                _ => {
                    value.push(ch as char);
                    self.advance();
                }
            }
        }

        if value.is_empty() {
            return Err(self.error("unexpected character".to_string()));
        }

        if (value.ends_with('=') || value.ends_with("+="))
            && !self.at_end()
            && self.peek_char() == b'('
        {
            let eq_pos = if value.ends_with("+=") {
                value.len() - 2
            } else {
                value.len() - 1
            };
            let lhs = &value[..eq_pos];
            let lhs = lhs.strip_suffix('+').unwrap_or(lhs);
            if is_valid_array_name(lhs) {
                self.advance(); // consume '('
                value.push('(');
                let mut depth = 1u32;
                while !self.at_end() && depth > 0 {
                    let c = self.peek_char();
                    value.push(c as char);
                    self.advance();
                    match c {
                        b'(' => depth += 1,
                        b')' => depth -= 1,
                        _ => {}
                    }
                }
            }
        }

        let kind = self.classify_word(value);
        Ok(Token::new(kind, span))
    }

    /// Read `$(...)` or `$((...))` into the value buffer.
    pub(super) fn read_dollar_paren(&mut self, value: &mut String) -> Result<(), ParseError> {
        value.push('$');
        value.push('(');
        self.advance_n(2);

        if self.peek_char() == b'(' {
            value.push('(');
            self.advance();
            let mut depth = 2u32;
            loop {
                if self.at_end() {
                    return Err(self.error("unterminated $((...))".to_string()));
                }
                let c = self.peek_char();
                value.push(c as char);
                self.advance();
                match c {
                    b'(' => depth += 1,
                    b')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    b'\'' => self.read_single_quoted_into(value)?,
                    b'"' => self.read_double_quoted_into(value)?,
                    _ => {}
                }
            }
            return Ok(());
        }

        let mut depth = 1u32;
        loop {
            if self.at_end() {
                return Err(self.error("unterminated $(...)".to_string()));
            }
            let c = self.peek_char();
            value.push(c as char);
            self.advance();
            match c {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                b'\'' => self.read_single_quoted_into(value)?,
                b'"' => self.read_double_quoted_into(value)?,
                b'\\' if !self.at_end() => {
                    value.push(self.peek_char() as char);
                    self.advance();
                }
                b'`' => self.read_backtick_into(value)?,
                _ => {}
            }
        }
        Ok(())
    }

    /// Read `${...}` into the value buffer.
    pub(super) fn read_dollar_brace(&mut self, value: &mut String) -> Result<(), ParseError> {
        value.push('$');
        value.push('{');
        self.advance_n(2);
        let mut depth = 1u32;
        loop {
            if self.at_end() {
                return Err(self.error("unterminated ${...}".to_string()));
            }
            let c = self.peek_char();
            value.push(c as char);
            self.advance();
            match c {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                b'\'' => self.read_single_quoted_into(value)?,
                b'"' => self.read_double_quoted_into(value)?,
                b'\\' if !self.at_end() => {
                    value.push(self.peek_char() as char);
                    self.advance();
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub(super) fn try_read_brace_expansion(&mut self) -> Option<String> {
        let start = self.pos;
        let mut depth = 0u32;
        let mut has_comma = false;
        let mut has_dotdot = false;
        let mut scan = start;

        while scan < self.input.len() {
            match self.input[scan] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        if has_comma || has_dotdot {
                            let content: String = self.input[start..=scan]
                                .iter()
                                .map(|&b| b as char)
                                .collect();
                            self.pos = scan + 1;
                            self.col += scan + 1 - start;
                            return Some(content);
                        }
                        return None;
                    }
                }
                b',' if depth == 1 => has_comma = true,
                b'.' if depth == 1
                    && scan + 1 < self.input.len()
                    && self.input[scan + 1] == b'.' =>
                {
                    has_dotdot = true;
                }
                _ => {}
            }
            scan += 1;
        }
        None
    }

    pub(super) fn read_process_substitution(&mut self, value: &mut String) -> Result<(), ParseError> {
        let dir_char = self.peek_char();
        value.push(dir_char as char);
        self.advance();
        value.push('(');
        self.advance();
        let mut depth = 1u32;
        loop {
            if self.at_end() {
                return Err(self.error("unterminated process substitution".to_string()));
            }
            let c = self.peek_char();
            value.push(c as char);
            self.advance();
            match c {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                b'\'' => self.read_single_quoted_into(value)?,
                b'"' => self.read_double_quoted_into(value)?,
                b'\\' if !self.at_end() => {
                    value.push(self.peek_char() as char);
                    self.advance();
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub(super) fn read_single_quoted_into(&mut self, value: &mut String) -> Result<(), ParseError> {
        loop {
            if self.at_end() {
                return Err(self.error("unterminated single quote".to_string()));
            }
            let c = self.peek_char();
            value.push(c as char);
            self.advance();
            if c == b'\'' {
                return Ok(());
            }
        }
    }

    pub(super) fn read_double_quoted_into(&mut self, value: &mut String) -> Result<(), ParseError> {
        loop {
            if self.at_end() {
                return Err(self.error("unterminated double quote".to_string()));
            }
            let c = self.peek_char();
            value.push(c as char);
            self.advance();
            match c {
                b'"' => return Ok(()),
                b'\\' if !self.at_end() => {
                    value.push(self.peek_char() as char);
                    self.advance();
                }
                _ => {}
            }
        }
    }

    pub(super) fn read_backtick_into(&mut self, value: &mut String) -> Result<(), ParseError> {
        loop {
            if self.at_end() {
                return Err(self.error("unterminated backtick".to_string()));
            }
            let c = self.peek_char();
            value.push(c as char);
            self.advance();
            match c {
                b'`' => return Ok(()),
                b'\\' if !self.at_end() => {
                    value.push(self.peek_char() as char);
                    self.advance();
                }
                _ => {}
            }
        }
    }
}
