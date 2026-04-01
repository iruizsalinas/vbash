use crate::error::ParseError;

use super::{Lexer, PendingHeredoc, Token, TokenKind};

impl Lexer<'_> {
    /// Lookahead to find and register the heredoc delimiter without advancing the main cursor.
    pub(super) fn register_heredoc(&mut self, strip_tabs: bool) -> Result<(), ParseError> {
        let save_pos = self.pos;
        let save_line = self.line;
        let save_col = self.col;

        let op_len = if strip_tabs { 3 } else { 2 };
        let mut look = self.pos + op_len;

        while look < self.input.len() && (self.input[look] == b' ' || self.input[look] == b'\t') {
            look += 1;
        }

        let mut delimiter = String::new();

        while look < self.input.len() {
            let ch = self.input[look];
            match ch {
                b'\'' => {
                    look += 1;
                    while look < self.input.len() && self.input[look] != b'\'' {
                        delimiter.push(self.input[look] as char);
                        look += 1;
                    }
                    if look < self.input.len() {
                        look += 1;
                    }
                }
                b'"' => {
                    look += 1;
                    while look < self.input.len() && self.input[look] != b'"' {
                        if self.input[look] == b'\\' && look + 1 < self.input.len() {
                            look += 1;
                        }
                        delimiter.push(self.input[look] as char);
                        look += 1;
                    }
                    if look < self.input.len() {
                        look += 1;
                    }
                }
                b'\\' => {
                    look += 1;
                    if look < self.input.len() {
                        delimiter.push(self.input[look] as char);
                        look += 1;
                    }
                }
                b' ' | b'\t' | b'\n' | b';' | b'<' | b'>' | b'&' | b'|' | b'(' | b')' => {
                    break;
                }
                _ => {
                    delimiter.push(ch as char);
                    look += 1;
                }
            }
        }

        self.pos = save_pos;
        self.line = save_line;
        self.col = save_col;

        if delimiter.is_empty() {
            return Err(self.error("missing heredoc delimiter".to_string()));
        }

        self.pending_heredocs.push(PendingHeredoc {
            delimiter,
            strip_tabs,
        });

        Ok(())
    }

    pub(super) fn collect_pending_heredocs(&mut self) {
        while !self.pending_heredocs.is_empty() {
            let heredoc = self.pending_heredocs.remove(0);
            let content = self.read_heredoc_content(&heredoc);
            self.tokens.push(Token::new(
                TokenKind::HeredocContent(content),
                self.span(),
            ));
        }
    }

    pub(super) fn read_heredoc_content(&mut self, heredoc: &PendingHeredoc) -> String {
        let mut content = String::new();

        loop {
            if self.at_end() {
                break;
            }

            let mut line = String::new();
            while !self.at_end() && self.peek_char() != b'\n' {
                line.push(self.peek_char() as char);
                self.advance();
            }

            if !self.at_end() {
                self.advance();
            }

            let trimmed = if heredoc.strip_tabs {
                line.trim_start_matches('\t')
            } else {
                &line
            };

            if trimmed == heredoc.delimiter {
                break;
            }

            content.push_str(&line);
            content.push('\n');

            if content.len() > super::MAX_INPUT_SIZE {
                break;
            }
        }

        content
    }
}
