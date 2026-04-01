//! Tokenizes bash script strings into a token stream.

pub mod token;
mod heredoc;
mod operators;
mod words;

pub use token::{Span, Token, TokenKind};

use crate::error::ParseError;

const MAX_INPUT_SIZE: usize = 1_000_000;
const MAX_TOKENS: usize = 100_000;

/// Pending heredoc waiting for content collection after the newline.
pub(super) struct PendingHeredoc {
    delimiter: String,
    strip_tabs: bool,
}

/// Bash lexer that tokenizes input into a stream of [`Token`]s.
pub struct Lexer<'a> {
    pub(super) input: &'a [u8],
    pub(super) pos: usize,
    pub(super) line: usize,
    pub(super) col: usize,
    /// Tracks `(( ... ))` nesting so `#` isn't treated as a comment inside arithmetic.
    pub(super) dparen_depth: u32,
    pub(super) pending_heredocs: Vec<PendingHeredoc>,
    pub(super) tokens: Vec<Token>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            pos: 0,
            line: 1,
            col: 1,
            dparen_depth: 0,
            pending_heredocs: Vec::new(),
            tokens: Vec::new(),
        }
    }

    /// Tokenize the entire input into a token stream.
    pub fn tokenize(mut self) -> Result<Vec<Token>, ParseError> {
        if self.input.len() > MAX_INPUT_SIZE {
            return Err(ParseError {
                message: format!("input exceeds maximum size of {MAX_INPUT_SIZE} bytes"),
                line: 1,
                column: 1,
            });
        }

        loop {
            self.skip_whitespace();

            if self.at_end() {
                self.collect_pending_heredocs();
                self.push_token(TokenKind::Eof);
                break;
            }

            if self.tokens.len() >= MAX_TOKENS {
                return Err(self.error(format!(
                    "token count exceeds maximum of {MAX_TOKENS}"
                )));
            }

            let token = self.next_token()?;
            let is_newline = token.kind == TokenKind::Newline;
            self.tokens.push(token);

            if is_newline && !self.pending_heredocs.is_empty() {
                self.collect_pending_heredocs();
            }
        }

        Ok(self.tokens)
    }

    fn next_token(&mut self) -> Result<Token, ParseError> {
        let ch = self.peek_char();
        let span = self.span();

        if ch == b'\n' {
            self.advance();
            return Ok(Token::new(TokenKind::Newline, span));
        }

        // `#` is a comment only outside arithmetic context
        if ch == b'#' && self.dparen_depth == 0 {
            self.skip_comment();
            if self.peek_char() == b'\n' {
                let span = self.span();
                self.advance();
                return Ok(Token::new(TokenKind::Newline, span));
            }
            return Ok(Token::new(TokenKind::Eof, self.span()));
        }

        if let Some(tok) = self.try_operator()? {
            return Ok(tok);
        }

        self.read_word()
    }

    pub(super) fn skip_whitespace(&mut self) {
        while !self.at_end() {
            let ch = self.peek_char();
            match ch {
                b' ' | b'\t' => self.advance(),
                b'\\' if self.peek_at(1) == b'\n' => {
                    self.advance();
                    self.advance();
                }
                _ => break,
            }
        }
    }

    pub(super) fn skip_comment(&mut self) {
        while !self.at_end() && self.peek_char() != b'\n' {
            self.advance();
        }
    }

    pub(super) fn peek_char(&self) -> u8 {
        if self.pos < self.input.len() {
            self.input[self.pos]
        } else {
            0
        }
    }

    pub(super) fn peek_at(&self, offset: usize) -> u8 {
        let idx = self.pos + offset;
        if idx < self.input.len() {
            self.input[idx]
        } else {
            0
        }
    }

    pub(super) fn advance(&mut self) {
        if self.pos < self.input.len() {
            if self.input[self.pos] == b'\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
            self.pos += 1;
        }
    }

    pub(super) fn advance_n(&mut self, n: usize) {
        for _ in 0..n {
            self.advance();
        }
    }

    pub(super) fn at_end(&self) -> bool {
        self.pos >= self.input.len()
    }

    pub(super) fn span(&self) -> Span {
        Span {
            line: self.line,
            column: self.col,
        }
    }

    pub(super) fn push_token(&mut self, kind: TokenKind) {
        self.tokens.push(Token::new(kind, self.span()));
    }

    pub(super) fn error(&self, message: String) -> ParseError {
        ParseError {
            message,
            line: self.line,
            column: self.col,
        }
    }

    pub(super) fn is_word_boundary_at(&self, pos: usize) -> bool {
        if pos >= self.input.len() {
            return true;
        }
        is_word_boundary(self.input[pos])
    }

    pub(super) fn is_brace_expansion(&self) -> bool {
        let mut depth = 0u32;
        let mut scan = self.pos;
        let mut has_comma = false;
        let mut has_dotdot = false;
        while scan < self.input.len() {
            match self.input[scan] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return has_comma || has_dotdot;
                    }
                }
                b',' if depth == 1 => has_comma = true,
                b'.' if depth == 1
                    && scan + 1 < self.input.len()
                    && self.input[scan + 1] == b'.' =>
                {
                    has_dotdot = true;
                }
                b' ' | b'\t' | b'\n' | b';' if depth == 1 => return false,
                _ => {}
            }
            scan += 1;
        }
        false
    }
}

fn is_word_boundary(ch: u8) -> bool {
    matches!(ch, b' ' | b'\t' | b'\n' | b';' | b'&' | b'|' | b'(' | b')' | b'<' | b'>' | b'{' | b'}')
}

fn is_redirect_start(ch: u8) -> bool {
    matches!(ch, b'<' | b'>')
}

fn is_valid_var_name(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    if !bytes[0].is_ascii_alphabetic() && bytes[0] != b'_' {
        return false;
    }
    let mut i = 1;
    while i < bytes.len() {
        if bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' {
            i += 1;
        } else if bytes[i] == b'[' {
            let mut depth = 1u32;
            i += 1;
            while i < bytes.len() && depth > 0 {
                if bytes[i] == b'[' {
                    depth += 1;
                } else if bytes[i] == b']' {
                    depth -= 1;
                }
                i += 1;
            }
            break;
        } else {
            return false;
        }
    }
    i == bytes.len()
}

fn reserved_word(word: &str) -> Option<TokenKind> {
    match word {
        "if" => Some(TokenKind::If),
        "then" => Some(TokenKind::Then),
        "else" => Some(TokenKind::Else),
        "elif" => Some(TokenKind::Elif),
        "fi" => Some(TokenKind::Fi),
        "for" => Some(TokenKind::For),
        "while" => Some(TokenKind::While),
        "until" => Some(TokenKind::Until),
        "do" => Some(TokenKind::Do),
        "done" => Some(TokenKind::Done),
        "case" => Some(TokenKind::Case),
        "esac" => Some(TokenKind::Esac),
        "in" => Some(TokenKind::In),
        "function" => Some(TokenKind::Function),
        "select" => Some(TokenKind::Select),
        "time" => Some(TokenKind::Time),
        "coproc" => Some(TokenKind::Coproc),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(input: &str) -> Vec<TokenKind> {
        Lexer::new(input)
            .tokenize()
            .unwrap()
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    fn words(input: &str) -> Vec<String> {
        lex(input)
            .into_iter()
            .filter_map(|k| match k {
                TokenKind::Word(s) | TokenKind::AssignmentWord(s) => Some(s),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn simple_command() {
        let tokens = lex("echo hello world");
        assert_eq!(tokens, vec![
            TokenKind::Word("echo".into()),
            TokenKind::Word("hello".into()),
            TokenKind::Word("world".into()),
            TokenKind::Eof,
        ]);
    }

    #[test]
    fn pipe() {
        let tokens = lex("ls | grep foo");
        assert_eq!(tokens, vec![
            TokenKind::Word("ls".into()),
            TokenKind::Pipe,
            TokenKind::Word("grep".into()),
            TokenKind::Word("foo".into()),
            TokenKind::Eof,
        ]);
    }

    #[test]
    fn and_or() {
        let tokens = lex("a && b || c");
        assert_eq!(tokens, vec![
            TokenKind::Word("a".into()),
            TokenKind::And,
            TokenKind::Word("b".into()),
            TokenKind::Or,
            TokenKind::Word("c".into()),
            TokenKind::Eof,
        ]);
    }

    #[test]
    fn redirections() {
        let tokens = lex("echo hello > out.txt 2>&1");
        assert!(tokens.contains(&TokenKind::Great));
        assert!(tokens.contains(&TokenKind::GreatAnd));
    }

    #[test]
    fn heredoc() {
        let tokens = lex("cat <<EOF\nhello world\nEOF\n");
        assert!(tokens.iter().any(|t| matches!(&t, TokenKind::HeredocContent(s) if s == "hello world\n")));
    }

    #[test]
    fn heredoc_strip_tabs() {
        let tokens = lex("cat <<-EOF\n\thello\n\tEOF\n");
        assert!(tokens.iter().any(|t| matches!(&t, TokenKind::HeredocContent(s) if s == "\thello\n")));
    }

    #[test]
    fn single_quoted() {
        let w = words("echo 'hello world'");
        assert_eq!(w, vec!["echo", "'hello world'"]);
    }

    #[test]
    fn double_quoted() {
        let w = words("echo \"hello $USER\"");
        assert_eq!(w, vec!["echo", "\"hello $USER\""]);
    }

    #[test]
    fn escaped_char() {
        let w = words("echo hello\\ world");
        assert_eq!(w, vec!["echo", "hello\\ world"]);
    }

    #[test]
    fn command_substitution() {
        let w = words("echo $(date)");
        assert_eq!(w, vec!["echo", "$(date)"]);
    }

    #[test]
    fn arithmetic_expansion() {
        let w = words("echo $((1 + 2))");
        assert_eq!(w, vec!["echo", "$((1 + 2))"]);
    }

    #[test]
    fn parameter_expansion() {
        let w = words("echo ${HOME}");
        assert_eq!(w, vec!["echo", "${HOME}"]);
    }

    #[test]
    fn assignment_word() {
        let tokens = lex("FOO=bar");
        assert!(matches!(&tokens[0], TokenKind::AssignmentWord(s) if s == "FOO=bar"));
    }

    #[test]
    fn assignment_append() {
        let tokens = lex("FOO+=bar");
        assert!(matches!(&tokens[0], TokenKind::AssignmentWord(s) if s == "FOO+=bar"));
    }

    #[test]
    fn reserved_words() {
        let tokens = lex("if true; then echo yes; fi");
        assert_eq!(tokens[0], TokenKind::If);
        assert_eq!(tokens[2], TokenKind::Semi);
        assert_eq!(tokens[3], TokenKind::Then);
        assert!(matches!(&tokens[6], TokenKind::Semi));
        assert_eq!(tokens[7], TokenKind::Fi);
    }

    #[test]
    fn for_loop() {
        let tokens = lex("for x in a b c; do echo $x; done");
        assert_eq!(tokens[0], TokenKind::For);
        assert_eq!(tokens[2], TokenKind::In);
        assert_eq!(tokens[7], TokenKind::Do);
        assert_eq!(tokens[11], TokenKind::Done);
    }

    #[test]
    fn io_number() {
        let tokens = lex("cmd 2> /dev/null");
        assert!(matches!(&tokens[1], TokenKind::IoNumber(n) if n == "2"));
        assert_eq!(tokens[2], TokenKind::Great);
    }

    #[test]
    fn semicolon_separators() {
        let tokens = lex("a; b; c");
        assert_eq!(tokens[1], TokenKind::Semi);
        assert_eq!(tokens[3], TokenKind::Semi);
    }

    #[test]
    fn background() {
        let tokens = lex("sleep 10 &");
        assert!(tokens.contains(&TokenKind::Amp));
    }

    #[test]
    fn case_terminators() {
        let tokens = lex(";;");
        assert_eq!(tokens[0], TokenKind::DSemi);
    }

    #[test]
    fn newlines() {
        let tokens = lex("a\nb\n");
        assert_eq!(tokens, vec![
            TokenKind::Word("a".into()),
            TokenKind::Newline,
            TokenKind::Word("b".into()),
            TokenKind::Newline,
            TokenKind::Eof,
        ]);
    }

    #[test]
    fn comment_skipped() {
        let tokens = lex("echo hello # comment\necho bye");
        assert!(tokens.iter().all(|t| {
            if let TokenKind::Word(s) = t {
                s != "comment" && s != "#"
            } else {
                true
            }
        }));
    }

    #[test]
    fn line_continuation() {
        let w = words("echo hel\\\nlo");
        assert_eq!(w, vec!["echo", "hello"]);
    }

    #[test]
    fn here_string() {
        let tokens = lex("cat <<< 'hello'");
        assert!(tokens.contains(&TokenKind::TLess));
    }

    #[test]
    fn pipe_amp() {
        let tokens = lex("cmd1 |& cmd2");
        assert!(tokens.contains(&TokenKind::PipeAmp));
    }

    #[test]
    fn double_bracket() {
        let tokens = lex("[[ -f file ]]");
        assert_eq!(tokens[0], TokenKind::DBrack);
        assert!(tokens.contains(&TokenKind::DBrackClose));
    }

    #[test]
    fn arithmetic_command() {
        let tokens = lex("(( x + 1 ))");
        assert_eq!(tokens[0], TokenKind::DParen);
        assert!(tokens.contains(&TokenKind::DParenClose));
    }

    #[test]
    fn subshell() {
        let tokens = lex("(echo hello)");
        assert_eq!(tokens[0], TokenKind::LParen);
        assert!(tokens.contains(&TokenKind::RParen));
    }

    #[test]
    fn group() {
        let tokens = lex("{ echo hello; }");
        assert_eq!(tokens[0], TokenKind::LBrace);
        assert!(tokens.contains(&TokenKind::RBrace));
    }

    #[test]
    fn function_keyword() {
        let tokens = lex("function foo { echo hi; }");
        assert_eq!(tokens[0], TokenKind::Function);
    }

    #[test]
    fn nested_command_substitution() {
        let w = words("echo $(echo $(date))");
        assert_eq!(w, vec!["echo", "$(echo $(date))"]);
    }

    #[test]
    fn backtick_substitution() {
        let w = words("echo `date`");
        assert_eq!(w, vec!["echo", "`date`"]);
    }

    #[test]
    fn ansi_c_quoting() {
        let w = words("echo $'hello\\nworld'");
        assert_eq!(w, vec!["echo", "$'hello\\nworld'"]);
    }

    #[test]
    fn empty_input() {
        let tokens = lex("");
        assert_eq!(tokens, vec![TokenKind::Eof]);
    }

    #[test]
    fn only_whitespace() {
        let tokens = lex("   \t  ");
        assert_eq!(tokens, vec![TokenKind::Eof]);
    }

    #[test]
    fn multiple_heredocs() {
        let input = "cat <<A <<B\nfirst\nA\nsecond\nB\n";
        let tokens = lex(input);
        let heredocs: Vec<_> = tokens.iter().filter(|t| matches!(t, TokenKind::HeredocContent(_))).collect();
        assert_eq!(heredocs.len(), 2);
    }

    #[test]
    fn redirect_all_output() {
        let tokens = lex("cmd &> file");
        assert!(tokens.contains(&TokenKind::AndGreat));
    }

    #[test]
    fn append_all_output() {
        let tokens = lex("cmd &>> file");
        assert!(tokens.contains(&TokenKind::AndDGreat));
    }

    #[test]
    fn clobber() {
        let tokens = lex("echo hi >| file");
        assert!(tokens.contains(&TokenKind::Clobber));
    }

    #[test]
    fn bang_negation() {
        let tokens = lex("! true");
        assert_eq!(tokens[0], TokenKind::Bang);
    }

    #[test]
    fn quoted_reserved_word_is_word() {
        let tokens = lex("'if'");
        assert!(matches!(&tokens[0], TokenKind::Word(s) if s == "'if'"));
    }
}
