use crate::error::ParseError;

use super::{is_redirect_start, is_valid_var_name, reserved_word, Lexer, Token, TokenKind};

impl Lexer<'_> {
    pub(super) fn try_operator(&mut self) -> Result<Option<Token>, ParseError> {
        let c0 = self.peek_char();
        let c1 = self.peek_at(1);
        let c2 = self.peek_at(2);
        let span = self.span();

        if c0 == b'<' && c1 == b'<' && c2 == b'<' {
            self.advance_n(3);
            return Ok(Some(Token::new(TokenKind::TLess, span)));
        }
        if c0 == b'<' && c1 == b'<' && c2 == b'-' {
            self.register_heredoc(true)?;
            self.advance_n(3);
            return Ok(Some(Token::new(TokenKind::DLessDash, span)));
        }
        if c0 == b';' && c1 == b';' && c2 == b'&' && self.dparen_depth == 0 {
            self.advance_n(3);
            return Ok(Some(Token::new(TokenKind::SemiSemiAnd, span)));
        }
        if c0 == b'&' && c1 == b'>' && c2 == b'>' {
            self.advance_n(3);
            return Ok(Some(Token::new(TokenKind::AndDGreat, span)));
        }

        if c0 == b'<' && c1 == b'<' {
            self.register_heredoc(false)?;
            self.advance_n(2);
            return Ok(Some(Token::new(TokenKind::DLess, span)));
        }
        if c0 == b'>' && c1 == b'>' {
            self.advance_n(2);
            return Ok(Some(Token::new(TokenKind::DGreat, span)));
        }
        if c0 == b'<' && c1 == b'&' {
            self.advance_n(2);
            return Ok(Some(Token::new(TokenKind::LessAnd, span)));
        }
        if c0 == b'>' && c1 == b'&' {
            self.advance_n(2);
            return Ok(Some(Token::new(TokenKind::GreatAnd, span)));
        }
        if c0 == b'<' && c1 == b'>' {
            self.advance_n(2);
            return Ok(Some(Token::new(TokenKind::LessGreat, span)));
        }
        if c0 == b'>' && c1 == b'|' {
            self.advance_n(2);
            return Ok(Some(Token::new(TokenKind::Clobber, span)));
        }
        if c0 == b'&' && c1 == b'>' {
            self.advance_n(2);
            return Ok(Some(Token::new(TokenKind::AndGreat, span)));
        }
        if c0 == b'&' && c1 == b'&' {
            self.advance_n(2);
            return Ok(Some(Token::new(TokenKind::And, span)));
        }
        if c0 == b'|' && c1 == b'|' {
            self.advance_n(2);
            return Ok(Some(Token::new(TokenKind::Or, span)));
        }
        if c0 == b'|' && c1 == b'&' {
            self.advance_n(2);
            return Ok(Some(Token::new(TokenKind::PipeAmp, span)));
        }
        if c0 == b';' && c1 == b';' && self.dparen_depth == 0 {
            self.advance_n(2);
            return Ok(Some(Token::new(TokenKind::DSemi, span)));
        }
        if c0 == b';' && c1 == b'&' && self.dparen_depth == 0 {
            self.advance_n(2);
            return Ok(Some(Token::new(TokenKind::SemiAnd, span)));
        }

        // Ambiguous: could be arithmetic (( )) or nested subshells ( ( ) ).
        // We default to arithmetic command for now.
        if c0 == b'(' && c1 == b'(' {
            self.advance_n(2);
            self.dparen_depth = 1;
            return Ok(Some(Token::new(TokenKind::DParen, span)));
        }
        if c0 == b')' && c1 == b')' && self.dparen_depth > 0 {
            self.advance_n(2);
            self.dparen_depth = 0;
            return Ok(Some(Token::new(TokenKind::DParenClose, span)));
        }

        if c0 == b'[' && c1 == b'[' && self.is_word_boundary_at(self.pos + 2) {
            self.advance_n(2);
            return Ok(Some(Token::new(TokenKind::DBrack, span)));
        }
        if c0 == b']' && c1 == b']' {
            self.advance_n(2);
            return Ok(Some(Token::new(TokenKind::DBrackClose, span)));
        }

        if (c0 == b'<' || c0 == b'>') && c1 == b'(' {
            return Ok(None);
        }

        let single = match c0 {
            b'|' => Some(TokenKind::Pipe),
            b'&' => Some(TokenKind::Amp),
            b';' => Some(TokenKind::Semi),
            b'(' => {
                if self.dparen_depth > 0 {
                    self.dparen_depth += 1;
                }
                Some(TokenKind::LParen)
            }
            b')' => {
                if self.dparen_depth > 1 {
                    self.dparen_depth -= 1;
                }
                Some(TokenKind::RParen)
            }
            b'<' => Some(TokenKind::Less),
            b'>' => Some(TokenKind::Great),
            b'{' => {
                if self.is_brace_expansion() {
                    None
                } else {
                    Some(TokenKind::LBrace)
                }
            }
            b'}' => Some(TokenKind::RBrace),
            b'!' => {
                // Don't emit Bang if followed by '(' (extglob pattern !(...)  )
                if c1 == b'(' {
                    None
                } else {
                    Some(TokenKind::Bang)
                }
            }
            _ => None,
        };

        if let Some(kind) = single {
            self.advance();
            return Ok(Some(Token::new(kind, span)));
        }

        Ok(None)
    }

    pub(super) fn classify_word(&self, value: String) -> TokenKind {
        if !value.starts_with('\'') && !value.starts_with('"') {
            if let Some(kind) = Self::try_assignment(&value) {
                return kind;
            }
        }

        // Reserved words are only recognized in command-start position:
        // at the beginning of input, or after a separator/operator token.
        // They are NOT recognized as arguments to commands (e.g. `echo done`).
        if !value.starts_with('\'') && !value.starts_with('"') && !value.contains('\\') {
            if self.is_reserved_word_position() {
                if let Some(kind) = reserved_word(&value) {
                    return kind;
                }
            } else if matches!(value.as_str(), "in" | "do") {
                // `in` and `do` are special: they are recognized after a
                // regular word for `case WORD in`, `for VAR in`, `for VAR do`.
                if let Some(kind) = reserved_word(&value) {
                    return kind;
                }
            }
        }

        // Pure digit token immediately followed by `<` or `>` is an I/O number.
        // The digit must be directly adjacent (no space): `2>` is fd redirect,
        // but `2 >` is the word "2" followed by a redirect of stdout.
        if value.bytes().all(|b| b.is_ascii_digit())
            && !value.is_empty()
            && self.pos < self.input.len()
            && is_redirect_start(self.input[self.pos])
        {
            return TokenKind::IoNumber(value);
        }

        TokenKind::Word(value)
    }

    /// Returns true if the current position can accept a reserved word.
    /// Reserved words are recognized only at the start of a command:
    /// after separators, operators, or other reserved words that introduce
    /// a new command context.  The special case for `in` and `do` after a
    /// word is handled via `classify_word_after_word`.
    fn is_reserved_word_position(&self) -> bool {
        let last = self.tokens.last().map(|t| &t.kind);
        match last {
            // Start of input
            None => true,
            // After a regular word, only `in` and `do` are recognized
            // (needed for `case WORD in`, `for VAR in`, `for VAR do`).
            // Other reserved words like `done`, `fi`, `then` are NOT
            // recognized as arguments (e.g. `echo done` stays a word).
            Some(TokenKind::Word(_) | TokenKind::AssignmentWord(_)) => false,
            Some(kind) => matches!(
                kind,
                // Separators
                TokenKind::Newline
                    | TokenKind::Semi
                    | TokenKind::Amp
                    // Pipe operators (next command)
                    | TokenKind::Pipe
                    | TokenKind::PipeAmp
                    // Boolean operators (next command)
                    | TokenKind::And
                    | TokenKind::Or
                    // Grouping/compound tokens
                    | TokenKind::LParen
                    | TokenKind::RParen
                    | TokenKind::LBrace
                    | TokenKind::RBrace
                    | TokenKind::DParen
                    | TokenKind::DParenClose
                    | TokenKind::DBrack
                    | TokenKind::DBrackClose
                    | TokenKind::Bang
                    // After reserved words that introduce sub-commands
                    | TokenKind::If
                    | TokenKind::Then
                    | TokenKind::Else
                    | TokenKind::Elif
                    | TokenKind::Fi
                    | TokenKind::While
                    | TokenKind::Until
                    | TokenKind::Do
                    | TokenKind::Done
                    | TokenKind::For
                    | TokenKind::Case
                    | TokenKind::Esac
                    | TokenKind::In
                    | TokenKind::Function
                    | TokenKind::Select
                    | TokenKind::Time
                    | TokenKind::Coproc
                    // Case terminators
                    | TokenKind::DSemi
                    | TokenKind::SemiAnd
                    | TokenKind::SemiSemiAnd
            ),
        }
    }

    pub(super) fn try_assignment(value: &str) -> Option<TokenKind> {
        let mut bracket_depth = 0u32;
        let bytes = value.as_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            match b {
                b'[' => bracket_depth += 1,
                b']' => bracket_depth = bracket_depth.saturating_sub(1),
                b'=' if bracket_depth == 0 && i > 0 => {
                    let lhs = &value[..i];
                    let lhs = lhs.strip_suffix('+').unwrap_or(lhs);
                    if is_valid_var_name(lhs) {
                        return Some(TokenKind::AssignmentWord(value.to_string()));
                    }
                    return None;
                }
                _ => {}
            }
        }
        None
    }
}
