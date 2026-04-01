//! Parses a token stream into an AST.

mod compound;
mod word;

pub use crate::ast::Script;

use crate::ast::{
    Assignment, Command, CompoundCommand, HereDoc, ListOp, Pipeline,
    Redirection, RedirectOp, RedirectTarget, SimpleCommand, Statement,
};
use crate::ast::word::{Word, WordPart};
use crate::error::ParseError;
use crate::lexer::{Lexer, Token, TokenKind};

const MAX_PARSER_DEPTH: u32 = 200;
const MAX_PARSE_ITERATIONS: u32 = 1_000_000;

/// Parse a bash script string into an AST.
///
/// # Errors
/// Returns `ParseError` on syntax errors, exceeded nesting depth, or runaway input.
pub fn parse(input: &str) -> Result<Script, ParseError> {
    let tokens = Lexer::new(input).tokenize()?;
    let mut parser = Parser::new(tokens);
    parser.parse_script()
}

struct Parser {
    pub(super) tokens: Vec<Token>,
    pub(super) pos: usize,
    pub(super) depth: u32,
    pub(super) iterations: u32,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            depth: 0,
            iterations: 0,
        }
    }

    fn parse_script(&mut self) -> Result<Script, ParseError> {
        self.skip_newlines();
        let mut statements = Vec::new();
        while !self.check(&TokenKind::Eof) {
            self.tick()?;
            let stmt = self.parse_statement()?;
            statements.push(stmt);
            self.skip_separators();
        }
        Ok(Script { statements })
    }

    /// Parse a statement: one or more pipelines joined by `&&` or `||`,
    /// optionally ending with `&` for background execution.
    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        self.enter_depth()?;
        let result = self.parse_statement_inner();
        self.depth -= 1;
        result
    }

    fn parse_statement_inner(&mut self) -> Result<Statement, ParseError> {
        let first = self.parse_pipeline()?;
        let mut pipelines = vec![first];
        let mut operators = Vec::new();

        loop {
            self.tick()?;
            match self.current_kind() {
                TokenKind::And => {
                    self.advance();
                    self.skip_newlines();
                    operators.push(ListOp::And);
                    pipelines.push(self.parse_pipeline()?);
                }
                TokenKind::Or => {
                    self.advance();
                    self.skip_newlines();
                    operators.push(ListOp::Or);
                    pipelines.push(self.parse_pipeline()?);
                }
                _ => break,
            }
        }

        let background = self.check(&TokenKind::Amp);
        if background {
            self.advance();
        }

        Ok(Statement {
            pipelines,
            operators,
            background,
        })
    }

    /// Parse a pipeline: commands connected by `|` or `|&`, with optional `time` and `!` prefixes.
    fn parse_pipeline(&mut self) -> Result<Pipeline, ParseError> {
        let timed = self.check(&TokenKind::Time);
        if timed {
            self.advance();
        }

        let negated = self.check(&TokenKind::Bang);
        if negated {
            self.advance();
        }

        let first = self.parse_command()?;
        let mut commands = vec![first];
        let mut pipe_stderr = Vec::new();

        loop {
            self.tick()?;
            match self.current_kind() {
                TokenKind::Pipe => {
                    self.advance();
                    self.skip_newlines();
                    pipe_stderr.push(false);
                    commands.push(self.parse_command()?);
                }
                TokenKind::PipeAmp => {
                    self.advance();
                    self.skip_newlines();
                    pipe_stderr.push(true);
                    commands.push(self.parse_command()?);
                }
                _ => break,
            }
        }

        Ok(Pipeline {
            commands,
            negated,
            pipe_stderr,
            timed,
        })
    }

    /// Parse a single command: simple command, compound command, or function def.
    fn parse_command(&mut self) -> Result<Command, ParseError> {
        self.enter_depth()?;
        let result = self.parse_command_inner();
        self.depth -= 1;
        result
    }

    fn parse_command_inner(&mut self) -> Result<Command, ParseError> {
        match self.current_kind() {
            TokenKind::If => return self.parse_if().map(|c| Command::Compound(CompoundCommand::If(c))),
            TokenKind::For => return self.parse_for().map(Command::Compound),
            TokenKind::While => return self.parse_while().map(|c| Command::Compound(CompoundCommand::While(c))),
            TokenKind::Until => return self.parse_until().map(|c| Command::Compound(CompoundCommand::Until(c))),
            TokenKind::Case => return self.parse_case().map(|c| Command::Compound(CompoundCommand::Case(c))),
            TokenKind::LParen => return self.parse_subshell().map(|c| Command::Compound(CompoundCommand::Subshell(c))),
            TokenKind::LBrace => return self.parse_group().map(|c| Command::Compound(CompoundCommand::Group(c))),
            TokenKind::DParen => return self.parse_arithmetic_cmd().map(|c| Command::Compound(CompoundCommand::Arithmetic(c))),
            TokenKind::DBrack => return self.parse_conditional_cmd().map(|c| Command::Compound(CompoundCommand::Conditional(c))),
            TokenKind::Function => return self.parse_function_keyword(),
            _ => {}
        }

        if self.is_function_def() {
            return self.parse_function_def();
        }

        self.parse_simple_command()
    }

    fn parse_simple_command(&mut self) -> Result<Command, ParseError> {
        let mut assignments = Vec::new();
        let mut redirections = Vec::new();

        loop {
            self.tick()?;
            if let Some(redir) = self.try_parse_redirection()? {
                redirections.push(redir);
            } else if let Some(assign) = self.try_parse_assignment() {
                assignments.push(assign);
            } else {
                break;
            }
        }

        let name = if self.is_word() {
            Some(self.parse_word()?)
        } else {
            None
        };

        let mut args = Vec::new();
        if name.is_some() {
            loop {
                self.tick()?;
                if let Some(redir) = self.try_parse_redirection()? {
                    redirections.push(redir);
                } else if self.is_word() {
                    args.push(self.parse_word()?);
                } else {
                    break;
                }
            }
        }

        Ok(Command::Simple(SimpleCommand {
            assignments,
            name,
            args,
            redirections,
        }))
    }

    fn try_parse_assignment(&mut self) -> Option<Assignment> {
        if let TokenKind::AssignmentWord(s) = self.current_kind().clone() {
            self.advance();
            Some(parse_assignment_word(&s))
        } else {
            None
        }
    }

    fn parse_word(&mut self) -> Result<Word, ParseError> {
        let text = self.expect_word_value()?;
        Ok(parse_word_string(&text))
    }

    fn is_word(&self) -> bool {
        matches!(
            self.current_kind(),
            TokenKind::Word(_) | TokenKind::AssignmentWord(_)
        )
    }



    fn try_parse_redirection(&mut self) -> Result<Option<Redirection>, ParseError> {
        let fd = if let TokenKind::IoNumber(n) = self.current_kind() {
            let fd: u32 = n.parse().map_err(|_| self.error("invalid fd number"))?;
            self.advance();
            Some(fd)
        } else {
            None
        };

        let operator = match self.current_kind() {
            TokenKind::Less => RedirectOp::Input,
            TokenKind::Great => RedirectOp::Output,
            TokenKind::DGreat => RedirectOp::Append,
            TokenKind::LessAnd => RedirectOp::DupInput,
            TokenKind::GreatAnd => RedirectOp::DupOutput,
            TokenKind::LessGreat => RedirectOp::ReadWrite,
            TokenKind::Clobber => RedirectOp::Clobber,
            TokenKind::AndGreat => RedirectOp::OutputAll,
            TokenKind::AndDGreat => RedirectOp::AppendAll,
            TokenKind::TLess => RedirectOp::HereString,
            TokenKind::DLess => RedirectOp::HereDoc,
            TokenKind::DLessDash => RedirectOp::HereDocStrip,
            _ => {
                if fd.is_some() {
                    return Err(self.error("expected redirection operator after fd number"));
                }
                return Ok(None);
            }
        };

        self.advance();

        let target = match operator {
            RedirectOp::HereDoc | RedirectOp::HereDocStrip => {
                let delim_word = self.parse_word()?;
                let quoted = delim_word.parts.iter().any(|p| matches!(
                    p,
                    WordPart::SingleQuoted(_) | WordPart::DoubleQuoted(_) | WordPart::Escaped(_)
                ));
                let content = self.consume_heredoc_content();
                let content_word = if quoted {
                    Word { parts: vec![WordPart::Literal(content)] }
                } else {
                    parse_word_string(&content)
                };
                RedirectTarget::HereDoc(HereDoc {
                    delimiter: word_to_string(&delim_word),
                    content: content_word,
                    strip_tabs: operator == RedirectOp::HereDocStrip,
                    quoted,
                })
            }
            _ => {
                let target_word = self.parse_word()?;
                RedirectTarget::Word(target_word)
            }
        };

        Ok(Some(Redirection {
            fd,
            fd_variable: None,
            operator,
            target,
        }))
    }

    fn consume_heredoc_content(&mut self) -> String {
        for i in self.pos..self.tokens.len() {
            if let TokenKind::HeredocContent(ref content) = self.tokens[i].kind {
                let result = content.clone();
                self.tokens.remove(i);
                return result;
            }
        }
        String::new()
    }

    fn parse_optional_redirections(&mut self) -> Result<Vec<Redirection>, ParseError> {
        let mut redirections = Vec::new();
        while let Some(redir) = self.try_parse_redirection()? {
            redirections.push(redir);
        }
        Ok(redirections)
    }

    fn parse_compound_list(&mut self) -> Result<Vec<Statement>, ParseError> {
        self.skip_newlines();
        let mut stmts = Vec::new();
        while self.is_command_start() {
            self.tick()?;
            stmts.push(self.parse_statement()?);
            self.skip_separators();
        }
        Ok(stmts)
    }

    fn is_command_start(&self) -> bool {
        !matches!(
            self.current_kind(),
            TokenKind::Eof
                | TokenKind::Then
                | TokenKind::Else
                | TokenKind::Elif
                | TokenKind::Fi
                | TokenKind::Do
                | TokenKind::Done
                | TokenKind::Esac
                | TokenKind::RBrace
                | TokenKind::RParen
                | TokenKind::DSemi
                | TokenKind::SemiAnd
                | TokenKind::SemiSemiAnd
                | TokenKind::DParenClose
                | TokenKind::DBrackClose
        )
    }

    fn current_kind(&self) -> &TokenKind {
        if self.pos < self.tokens.len() {
            &self.tokens[self.pos].kind
        } else {
            &TokenKind::Eof
        }
    }

    fn current_span(&self) -> crate::lexer::Span {
        if self.pos < self.tokens.len() {
            self.tokens[self.pos].span
        } else {
            crate::lexer::Span::default()
        }
    }

    fn current_token_text(&self) -> String {
        match self.current_kind() {
            TokenKind::Word(s) | TokenKind::AssignmentWord(s) | TokenKind::IoNumber(s) => s.clone(),
            TokenKind::Pipe => "|".to_string(),
            TokenKind::And => "&&".to_string(),
            TokenKind::Or => "||".to_string(),
            TokenKind::Semi => ";".to_string(),
            TokenKind::Amp => "&".to_string(),
            TokenKind::Bang => "!".to_string(),
            TokenKind::Less => "<".to_string(),
            TokenKind::Great => ">".to_string(),
            TokenKind::LParen => "(".to_string(),
            TokenKind::RParen => ")".to_string(),
            other => format!("{other:?}"),
        }
    }

    fn peek_kind(&self, offset: usize) -> &TokenKind {
        let idx = self.pos + offset;
        if idx < self.tokens.len() {
            &self.tokens[idx].kind
        } else {
            &TokenKind::Eof
        }
    }

    fn check(&self, kind: &TokenKind) -> bool {
        self.current_kind() == kind
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    fn expect(&mut self, kind: &TokenKind) -> Result<(), ParseError> {
        if self.current_kind() == kind {
            self.advance();
            Ok(())
        } else {
            Err(self.error(format!(
                "expected {kind:?}, got {:?}",
                self.current_kind()
            )))
        }
    }

    fn expect_word_value(&mut self) -> Result<String, ParseError> {
        match self.current_kind().clone() {
            TokenKind::Word(s) | TokenKind::AssignmentWord(s) => {
                self.advance();
                Ok(s)
            }
            other => Err(self.error(format!("expected word, got {other:?}"))),
        }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.tokens.len() || *self.current_kind() == TokenKind::Eof
    }

    fn skip_newlines(&mut self) {
        while self.check(&TokenKind::Newline) {
            self.advance();
        }
    }

    fn skip_separators(&mut self) {
        while matches!(
            self.current_kind(),
            TokenKind::Newline | TokenKind::Semi
        ) {
            self.advance();
        }
    }

    fn error(&self, message: impl Into<String>) -> ParseError {
        let span = self.current_span();
        ParseError {
            message: message.into(),
            line: span.line,
            column: span.column,
        }
    }

    fn tick(&mut self) -> Result<(), ParseError> {
        self.iterations += 1;
        if self.iterations > MAX_PARSE_ITERATIONS {
            return Err(self.error("maximum parse iterations exceeded"));
        }
        Ok(())
    }

    fn enter_depth(&mut self) -> Result<(), ParseError> {
        self.depth += 1;
        if self.depth > MAX_PARSER_DEPTH {
            return Err(self.error("maximum nesting depth exceeded"));
        }
        Ok(())
    }

}

// Re-export word parsing functions used by this module and by tests.
use word::{parse_word_string, parse_assignment_word, word_to_string};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::conditional::{self, CondExpr};
    use crate::ast::word::{GlobPart, ParamOp, PatternSide, WordPart};

    fn p(input: &str) -> Script {
        parse(input).unwrap()
    }

    fn first_cmd(input: &str) -> Command {
        let script = p(input);
        script.statements[0].pipelines[0].commands[0].clone()
    }

    #[test]
    fn parse_simple_command() {
        let cmd = first_cmd("echo hello world");
        if let Command::Simple(sc) = cmd {
            assert!(sc.name.is_some());
            assert_eq!(sc.args.len(), 2);
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn parse_pipe() {
        let script = p("ls | grep foo | wc -l");
        let pipeline = &script.statements[0].pipelines[0];
        assert_eq!(pipeline.commands.len(), 3);
    }

    #[test]
    fn parse_and_or() {
        let script = p("a && b || c");
        assert_eq!(script.statements[0].pipelines.len(), 3);
        assert_eq!(script.statements[0].operators, vec![ListOp::And, ListOp::Or]);
    }

    #[test]
    fn parse_if() {
        let cmd = first_cmd("if true; then echo yes; fi");
        assert!(matches!(cmd, Command::Compound(CompoundCommand::If(_))));
    }

    #[test]
    fn parse_if_else() {
        let cmd = first_cmd("if true; then echo yes; else echo no; fi");
        if let Command::Compound(CompoundCommand::If(if_cmd)) = cmd {
            assert!(if_cmd.else_body.is_some());
        } else {
            panic!("expected if");
        }
    }

    #[test]
    fn parse_for_loop() {
        let cmd = first_cmd("for x in a b c; do echo $x; done");
        if let Command::Compound(CompoundCommand::For(for_cmd)) = cmd {
            assert_eq!(for_cmd.variable, "x");
            assert_eq!(for_cmd.words.as_ref().unwrap().len(), 3);
        } else {
            panic!("expected for");
        }
    }

    #[test]
    fn parse_while_loop() {
        let cmd = first_cmd("while true; do echo loop; done");
        assert!(matches!(cmd, Command::Compound(CompoundCommand::While(_))));
    }

    #[test]
    fn parse_case() {
        let cmd = first_cmd("case $x in\na) echo a;;\nb) echo b;;\nesac");
        if let Command::Compound(CompoundCommand::Case(case_cmd)) = cmd {
            assert_eq!(case_cmd.items.len(), 2);
        } else {
            panic!("expected case");
        }
    }

    #[test]
    fn parse_subshell() {
        let cmd = first_cmd("(echo hello)");
        assert!(matches!(cmd, Command::Compound(CompoundCommand::Subshell(_))));
    }

    #[test]
    fn parse_group() {
        let cmd = first_cmd("{ echo hello; }");
        assert!(matches!(cmd, Command::Compound(CompoundCommand::Group(_))));
    }

    #[test]
    fn parse_function_def() {
        let cmd = first_cmd("foo() { echo hi; }");
        if let Command::FunctionDef(fd) = cmd {
            assert_eq!(fd.name, "foo");
        } else {
            panic!("expected function def");
        }
    }

    #[test]
    fn parse_function_keyword() {
        let cmd = first_cmd("function bar { echo hi; }");
        if let Command::FunctionDef(fd) = cmd {
            assert_eq!(fd.name, "bar");
        } else {
            panic!("expected function def");
        }
    }

    #[test]
    fn parse_assignment() {
        let cmd = first_cmd("FOO=bar");
        if let Command::Simple(sc) = cmd {
            assert_eq!(sc.assignments.len(), 1);
            assert_eq!(sc.assignments[0].name, "FOO");
            assert!(sc.name.is_none());
        } else {
            panic!("expected simple command with assignment");
        }
    }

    #[test]
    fn parse_assignment_before_command() {
        let cmd = first_cmd("FOO=bar echo hello");
        if let Command::Simple(sc) = cmd {
            assert_eq!(sc.assignments.len(), 1);
            assert!(sc.name.is_some());
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn parse_redirect_output() {
        let cmd = first_cmd("echo hello > file.txt");
        if let Command::Simple(sc) = cmd {
            assert_eq!(sc.redirections.len(), 1);
            assert_eq!(sc.redirections[0].operator, RedirectOp::Output);
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn parse_redirect_append() {
        let cmd = first_cmd("echo hello >> file.txt");
        if let Command::Simple(sc) = cmd {
            assert_eq!(sc.redirections[0].operator, RedirectOp::Append);
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn parse_redirect_input() {
        let cmd = first_cmd("cat < file.txt");
        if let Command::Simple(sc) = cmd {
            assert_eq!(sc.redirections[0].operator, RedirectOp::Input);
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn parse_fd_redirect() {
        let cmd = first_cmd("cmd 2> /dev/null");
        if let Command::Simple(sc) = cmd {
            assert_eq!(sc.redirections[0].fd, Some(2));
            assert_eq!(sc.redirections[0].operator, RedirectOp::Output);
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn parse_background() {
        let script = p("sleep 10 &");
        assert!(script.statements[0].background);
    }

    #[test]
    fn parse_negated_pipeline() {
        let script = p("! true");
        assert!(script.statements[0].pipelines[0].negated);
    }

    #[test]
    fn parse_conditional_file_test() {
        let cmd = first_cmd("[[ -f file.txt ]]");
        if let Command::Compound(CompoundCommand::Conditional(cond)) = cmd {
            assert!(matches!(cond.expression, CondExpr::Unary { .. }));
        } else {
            panic!("expected conditional");
        }
    }

    #[test]
    fn parse_conditional_string_eq() {
        let cmd = first_cmd("[[ $a == $b ]]");
        if let Command::Compound(CompoundCommand::Conditional(cond)) = cmd {
            if let CondExpr::Binary { op, .. } = cond.expression {
                assert_eq!(op, conditional::CondBinaryOp::Eq);
            } else {
                panic!("expected binary");
            }
        } else {
            panic!("expected conditional");
        }
    }

    #[test]
    fn parse_multiple_statements() {
        let script = p("echo a; echo b; echo c");
        assert_eq!(script.statements.len(), 3);
    }

    #[test]
    fn parse_empty() {
        let script = p("");
        assert!(script.statements.is_empty());
    }

    #[test]
    fn parse_heredoc() {
        let cmd = first_cmd("cat <<EOF\nhello world\nEOF\n");
        if let Command::Simple(sc) = cmd {
            assert_eq!(sc.redirections.len(), 1);
            assert!(matches!(
                sc.redirections[0].target,
                RedirectTarget::HereDoc(_)
            ));
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn word_parsing_single_quoted() {
        let w = parse_word_string("'hello world'");
        assert_eq!(w.parts.len(), 1);
        assert!(matches!(&w.parts[0], WordPart::SingleQuoted(s) if s == "hello world"));
    }

    #[test]
    fn word_parsing_double_quoted() {
        let w = parse_word_string("\"hello $USER\"");
        assert_eq!(w.parts.len(), 1);
        assert!(matches!(&w.parts[0], WordPart::DoubleQuoted(_)));
    }

    #[test]
    fn word_parsing_variable() {
        let w = parse_word_string("$HOME");
        assert!(matches!(&w.parts[0], WordPart::Parameter(p) if p.name == "HOME"));
    }

    #[test]
    fn word_parsing_param_default() {
        let w = parse_word_string("${VAR:-default}");
        if let WordPart::Parameter(p) = &w.parts[0] {
            assert_eq!(p.name, "VAR");
            assert!(matches!(&p.operation, Some(ParamOp::Default { colon: true, .. })));
        } else {
            panic!("expected parameter");
        }
    }

    #[test]
    fn word_parsing_glob_star() {
        let w = parse_word_string("*.txt");
        assert!(matches!(&w.parts[0], WordPart::Glob(GlobPart::Star)));
    }

    #[test]
    fn word_parsing_tilde() {
        let w = parse_word_string("~/docs");
        assert!(matches!(&w.parts[0], WordPart::TildeExpansion(u) if u.is_empty()));
    }

    #[test]
    fn word_parsing_command_subst() {
        let w = parse_word_string("$(date)");
        assert!(matches!(&w.parts[0], WordPart::CommandSubstitution(s) if s == "date"));
    }

    #[test]
    fn word_parsing_backtick() {
        let w = parse_word_string("`date`");
        assert!(matches!(&w.parts[0], WordPart::CommandSubstitution(s) if s == "date"));
    }

    #[test]
    fn word_parsing_length() {
        let w = parse_word_string("${#VAR}");
        if let WordPart::Parameter(p) = &w.parts[0] {
            assert_eq!(p.name, "VAR");
            assert!(matches!(p.operation, Some(ParamOp::Length)));
        } else {
            panic!("expected parameter");
        }
    }

    #[test]
    fn word_parsing_pattern_removal() {
        let w = parse_word_string("${VAR##*/}");
        if let WordPart::Parameter(p) = &w.parts[0] {
            assert_eq!(p.name, "VAR");
            assert!(matches!(
                p.operation,
                Some(ParamOp::PatternRemoval {
                    side: PatternSide::Prefix,
                    greedy: true,
                    ..
                })
            ));
        } else {
            panic!("expected parameter");
        }
    }
}
