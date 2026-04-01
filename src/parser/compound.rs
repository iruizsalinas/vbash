//! Compound command parsing.

use crate::ast::{
    ArithExpr, ArithmeticCmd, CaseCmd, CaseItem, CaseTerminator, Command,
    CompoundCommand, ConditionalCmd, CStyleForCmd, ForCmd, FunctionDef,
    GroupCmd, IfClause, IfCmd, SubshellCmd, UntilCmd, WhileCmd,
};
use crate::ast::conditional::{self, CondExpr};
use crate::ast::word::WordPart;
use crate::ast::Word;
use crate::error::ParseError;
use crate::lexer::TokenKind;

impl super::Parser {
    pub(super) fn parse_if(&mut self) -> Result<IfCmd, ParseError> {
        self.expect(&TokenKind::If)?;
        let mut clauses = Vec::new();

        let condition = self.parse_compound_list()?;
        self.expect(&TokenKind::Then)?;
        let body = self.parse_compound_list()?;
        clauses.push(IfClause { condition, body });

        while self.check(&TokenKind::Elif) {
            self.advance();
            let condition = self.parse_compound_list()?;
            self.expect(&TokenKind::Then)?;
            let body = self.parse_compound_list()?;
            clauses.push(IfClause { condition, body });
        }

        let else_body = if self.check(&TokenKind::Else) {
            self.advance();
            Some(self.parse_compound_list()?)
        } else {
            None
        };

        self.expect(&TokenKind::Fi)?;
        let redirections = self.parse_optional_redirections()?;

        Ok(IfCmd {
            clauses,
            else_body,
            redirections,
        })
    }

    pub(super) fn parse_for(&mut self) -> Result<CompoundCommand, ParseError> {
        self.expect(&TokenKind::For)?;

        if self.check(&TokenKind::DParen) {
            return self.parse_c_style_for();
        }

        let variable = self.expect_word_value()?;

        let words = if self.check(&TokenKind::In) {
            self.advance();
            let mut words = Vec::new();
            while self.is_word() {
                words.push(self.parse_word()?);
            }
            Some(words)
        } else {
            None
        };

        self.skip_separators();
        self.expect(&TokenKind::Do)?;
        let body = self.parse_compound_list()?;
        self.expect(&TokenKind::Done)?;
        let redirections = self.parse_optional_redirections()?;

        Ok(CompoundCommand::For(ForCmd {
            variable,
            words,
            body,
            redirections,
        }))
    }

    pub(super) fn parse_c_style_for(&mut self) -> Result<CompoundCommand, ParseError> {
        self.expect(&TokenKind::DParen)?;

        let init = self.parse_arith_expr_simple();
        self.skip_semicolons_and_words(b';');
        let condition = self.parse_arith_expr_simple();
        self.skip_semicolons_and_words(b';');
        let update = self.parse_arith_expr_simple();

        self.expect(&TokenKind::DParenClose)?;
        self.skip_separators();
        self.expect(&TokenKind::Do)?;
        let body = self.parse_compound_list()?;
        self.expect(&TokenKind::Done)?;
        let redirections = self.parse_optional_redirections()?;

        Ok(CompoundCommand::CStyleFor(CStyleForCmd {
            init,
            condition,
            update,
            body,
            redirections,
        }))
    }

    pub(super) fn parse_arith_expr_simple(&mut self) -> Option<ArithExpr> {
        let mut text = String::new();
        while !self.at_end()
            && !self.check(&TokenKind::Semi)
            && !self.check(&TokenKind::DParenClose)
        {
            let fragment = match self.current_kind() {
                TokenKind::Word(s) | TokenKind::AssignmentWord(s) => Some(s.clone()),
                TokenKind::Less => Some("<".to_string()),
                TokenKind::Great => Some(">".to_string()),
                TokenKind::Bang => Some("!".to_string()),
                TokenKind::Pipe => Some("|".to_string()),
                TokenKind::Amp => Some("&".to_string()),
                TokenKind::LParen => Some("(".to_string()),
                TokenKind::RParen => Some(")".to_string()),
                _ => None,
            };
            if let Some(s) = fragment {
                text.push_str(&s);
                self.advance();
            } else {
                break;
            }
        }
        if text.is_empty() {
            None
        } else if let Ok(n) = text.parse::<i64>() {
            Some(ArithExpr::Number(n))
        } else {
            Some(ArithExpr::Variable(text))
        }
    }

    pub(super) fn skip_semicolons_and_words(&mut self, _ch: u8) {
        if self.check(&TokenKind::Semi) {
            self.advance();
        }
    }

    pub(super) fn parse_while(&mut self) -> Result<WhileCmd, ParseError> {
        self.expect(&TokenKind::While)?;
        let condition = self.parse_compound_list()?;
        self.expect(&TokenKind::Do)?;
        let body = self.parse_compound_list()?;
        self.expect(&TokenKind::Done)?;
        let redirections = self.parse_optional_redirections()?;

        Ok(WhileCmd {
            condition,
            body,
            redirections,
        })
    }

    pub(super) fn parse_until(&mut self) -> Result<UntilCmd, ParseError> {
        self.expect(&TokenKind::Until)?;
        let condition = self.parse_compound_list()?;
        self.expect(&TokenKind::Do)?;
        let body = self.parse_compound_list()?;
        self.expect(&TokenKind::Done)?;
        let redirections = self.parse_optional_redirections()?;

        Ok(UntilCmd {
            condition,
            body,
            redirections,
        })
    }

    pub(super) fn parse_case(&mut self) -> Result<CaseCmd, ParseError> {
        self.expect(&TokenKind::Case)?;
        let word = self.parse_word()?;
        self.skip_newlines();
        self.expect(&TokenKind::In)?;
        self.skip_newlines();

        let mut items = Vec::new();
        while !self.check(&TokenKind::Esac) && !self.at_end() {
            self.tick()?;
            if self.check(&TokenKind::LParen) {
                self.advance();
            }

            let mut patterns = vec![self.parse_word()?];
            while self.check(&TokenKind::Pipe) {
                self.advance();
                patterns.push(self.parse_word()?);
            }

            self.expect(&TokenKind::RParen)?;
            self.skip_newlines();

            let body = self.parse_case_body()?;

            let terminator = if self.check(&TokenKind::DSemi) {
                self.advance();
                CaseTerminator::Break
            } else if self.check(&TokenKind::SemiAnd) {
                self.advance();
                CaseTerminator::FallThrough
            } else if self.check(&TokenKind::SemiSemiAnd) {
                self.advance();
                CaseTerminator::Continue
            } else {
                CaseTerminator::Break
            };

            self.skip_newlines();
            items.push(CaseItem {
                patterns,
                body,
                terminator,
            });
        }

        self.expect(&TokenKind::Esac)?;
        let redirections = self.parse_optional_redirections()?;

        Ok(CaseCmd {
            word,
            items,
            redirections,
        })
    }

    pub(super) fn parse_case_body(&mut self) -> Result<Vec<crate::ast::Statement>, ParseError> {
        let mut stmts = Vec::new();
        while !self.check(&TokenKind::DSemi)
            && !self.check(&TokenKind::SemiAnd)
            && !self.check(&TokenKind::SemiSemiAnd)
            && !self.check(&TokenKind::Esac)
            && !self.at_end()
        {
            self.tick()?;
            if self.check(&TokenKind::Newline) || self.check(&TokenKind::Semi) {
                self.advance();
                continue;
            }
            if !self.is_command_start() {
                break;
            }
            stmts.push(self.parse_statement()?);
            self.skip_newlines();
        }
        Ok(stmts)
    }

    pub(super) fn parse_subshell(&mut self) -> Result<SubshellCmd, ParseError> {
        self.expect(&TokenKind::LParen)?;
        let body = self.parse_compound_list()?;
        self.expect(&TokenKind::RParen)?;
        let redirections = self.parse_optional_redirections()?;
        Ok(SubshellCmd {
            body,
            redirections,
        })
    }

    pub(super) fn parse_group(&mut self) -> Result<GroupCmd, ParseError> {
        self.expect(&TokenKind::LBrace)?;
        let body = self.parse_compound_list()?;
        self.expect(&TokenKind::RBrace)?;
        let redirections = self.parse_optional_redirections()?;
        Ok(GroupCmd {
            body,
            redirections,
        })
    }

    pub(super) fn parse_arithmetic_cmd(&mut self) -> Result<ArithmeticCmd, ParseError> {
        self.expect(&TokenKind::DParen)?;
        let mut expr_text = String::new();
        while !self.check(&TokenKind::DParenClose) && !self.at_end() {
            if let TokenKind::Word(s) = self.current_kind() {
                if !expr_text.is_empty() {
                    expr_text.push(' ');
                }
                expr_text.push_str(s);
                self.advance();
            } else {
                let tok_text = self.current_token_text();
                if !expr_text.is_empty() {
                    expr_text.push(' ');
                }
                expr_text.push_str(&tok_text);
                self.advance();
            }
        }
        self.expect(&TokenKind::DParenClose)?;
        let redirections = self.parse_optional_redirections()?;

        let expression = if let Ok(n) = expr_text.trim().parse::<i64>() {
            ArithExpr::Number(n)
        } else {
            ArithExpr::Variable(expr_text)
        };

        Ok(ArithmeticCmd {
            expression,
            redirections,
        })
    }

    pub(super) fn parse_conditional_cmd(&mut self) -> Result<ConditionalCmd, ParseError> {
        self.expect(&TokenKind::DBrack)?;
        let expression = self.parse_cond_expr()?;
        self.expect(&TokenKind::DBrackClose)?;
        let redirections = self.parse_optional_redirections()?;
        Ok(ConditionalCmd {
            expression,
            redirections,
        })
    }

    pub(super) fn parse_cond_expr(&mut self) -> Result<CondExpr, ParseError> {
        self.parse_cond_or()
    }

    pub(super) fn parse_cond_or(&mut self) -> Result<CondExpr, ParseError> {
        let mut left = self.parse_cond_and()?;
        while self.check(&TokenKind::Or) {
            self.advance();
            let right = self.parse_cond_and()?;
            left = CondExpr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    pub(super) fn parse_cond_and(&mut self) -> Result<CondExpr, ParseError> {
        let mut left = self.parse_cond_not()?;
        while self.check(&TokenKind::And) {
            self.advance();
            let right = self.parse_cond_not()?;
            left = CondExpr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    pub(super) fn parse_cond_not(&mut self) -> Result<CondExpr, ParseError> {
        if self.check(&TokenKind::Bang) {
            self.advance();
            let expr = self.parse_cond_primary()?;
            return Ok(CondExpr::Not(Box::new(expr)));
        }
        self.parse_cond_primary()
    }

    pub(super) fn parse_cond_primary(&mut self) -> Result<CondExpr, ParseError> {
        if self.check(&TokenKind::LParen) {
            self.advance();
            let expr = self.parse_cond_expr()?;
            self.expect(&TokenKind::RParen)?;
            return Ok(CondExpr::Group(Box::new(expr)));
        }

        if let Some(op) = self.try_cond_unary_op() {
            self.advance();
            let operand = self.parse_word()?;
            return Ok(CondExpr::Unary { op, operand });
        }

        let left = self.parse_word()?;

        if let Some(op) = self.try_cond_binary_op() {
            let is_regex = matches!(op, conditional::CondBinaryOp::RegexMatch);
            self.advance();
            let right = if is_regex {
                self.parse_cond_regex_word()?
            } else {
                self.parse_word()?
            };
            return Ok(CondExpr::Binary { op, left, right });
        }

        // Standalone word -- treated as -n test (non-empty string)
        Ok(CondExpr::Unary {
            op: conditional::CondUnaryOp::StringNonEmpty,
            operand: left,
        })
    }

    fn parse_cond_regex_word(&mut self) -> Result<Word, ParseError> {
        let mut raw = String::new();
        loop {
            match self.current_kind() {
                TokenKind::DBrackClose | TokenKind::And | TokenKind::Or | TokenKind::Eof => break,
                TokenKind::RParen => {
                    raw.push(')');
                    self.advance();
                }
                TokenKind::LParen => {
                    raw.push('(');
                    self.advance();
                }
                TokenKind::Word(s) => {
                    raw.push_str(s.as_str());
                    self.advance();
                }
                _ => {
                    let tok_str = format!("{:?}", self.current_kind());
                    raw.push_str(&tok_str);
                    self.advance();
                }
            }
        }
        let raw = raw.trim().to_string();
        if raw.is_empty() {
            return Err(self.error("expected regex pattern"));
        }
        Ok(Word { parts: vec![WordPart::Literal(raw)] })
    }

    pub(super) fn try_cond_unary_op(&self) -> Option<conditional::CondUnaryOp> {
        let word = match self.current_kind() {
            TokenKind::Word(s) => s.as_str(),
            _ => return None,
        };
        match word {
            "-a" | "-e" => Some(conditional::CondUnaryOp::FileExists),
            "-b" => Some(conditional::CondUnaryOp::BlockDevice),
            "-c" => Some(conditional::CondUnaryOp::CharDevice),
            "-d" => Some(conditional::CondUnaryOp::IsDirectory),
            "-f" => Some(conditional::CondUnaryOp::IsFile),
            "-g" => Some(conditional::CondUnaryOp::SetGid),
            "-h" | "-L" => Some(conditional::CondUnaryOp::IsSymlink),
            "-k" => Some(conditional::CondUnaryOp::Sticky),
            "-p" => Some(conditional::CondUnaryOp::IsPipe),
            "-r" => Some(conditional::CondUnaryOp::IsReadable),
            "-s" => Some(conditional::CondUnaryOp::NonEmpty),
            "-t" => Some(conditional::CondUnaryOp::IsTerminal),
            "-u" => Some(conditional::CondUnaryOp::SetUid),
            "-w" => Some(conditional::CondUnaryOp::IsWritable),
            "-x" => Some(conditional::CondUnaryOp::IsExecutable),
            "-G" => Some(conditional::CondUnaryOp::OwnedByGroup),
            "-N" => Some(conditional::CondUnaryOp::ModifiedSinceRead),
            "-O" => Some(conditional::CondUnaryOp::OwnedByUser),
            "-S" => Some(conditional::CondUnaryOp::IsSocket),
            "-z" => Some(conditional::CondUnaryOp::StringEmpty),
            "-n" => Some(conditional::CondUnaryOp::StringNonEmpty),
            "-o" => Some(conditional::CondUnaryOp::OptionSet),
            "-v" => Some(conditional::CondUnaryOp::VariableSet),
            "-R" => Some(conditional::CondUnaryOp::IsNameref),
            _ => None,
        }
    }

    pub(super) fn try_cond_binary_op(&self) -> Option<conditional::CondBinaryOp> {
        let word = match self.current_kind() {
            TokenKind::Word(s) => s.as_str(),
            _ => return None,
        };
        match word {
            "=" | "==" => Some(conditional::CondBinaryOp::Eq),
            "!=" => Some(conditional::CondBinaryOp::Ne),
            "=~" => Some(conditional::CondBinaryOp::RegexMatch),
            "<" => Some(conditional::CondBinaryOp::StrLt),
            ">" => Some(conditional::CondBinaryOp::StrGt),
            "-eq" => Some(conditional::CondBinaryOp::IntEq),
            "-ne" => Some(conditional::CondBinaryOp::IntNe),
            "-lt" => Some(conditional::CondBinaryOp::IntLt),
            "-le" => Some(conditional::CondBinaryOp::IntLe),
            "-gt" => Some(conditional::CondBinaryOp::IntGt),
            "-ge" => Some(conditional::CondBinaryOp::IntGe),
            "-nt" => Some(conditional::CondBinaryOp::NewerThan),
            "-ot" => Some(conditional::CondBinaryOp::OlderThan),
            "-ef" => Some(conditional::CondBinaryOp::SameFile),
            _ => None,
        }
    }

    pub(super) fn is_function_def(&self) -> bool {
        if !self.is_word() {
            return false;
        }
        let next = self.peek_kind(1);
        next == &TokenKind::LParen && self.peek_kind(2) == &TokenKind::RParen
    }

    pub(super) fn parse_function_def(&mut self) -> Result<Command, ParseError> {
        let name = self.expect_word_value()?;
        self.expect(&TokenKind::LParen)?;
        self.expect(&TokenKind::RParen)?;
        self.skip_newlines();
        let body = self.parse_function_body()?;
        let redirections = self.parse_optional_redirections()?;

        Ok(Command::FunctionDef(FunctionDef {
            name,
            body: Box::new(body),
            redirections,
        }))
    }

    pub(super) fn parse_function_keyword(&mut self) -> Result<Command, ParseError> {
        self.expect(&TokenKind::Function)?;
        let name = self.expect_word_value()?;
        if self.check(&TokenKind::LParen) {
            self.advance();
            self.expect(&TokenKind::RParen)?;
        }
        self.skip_newlines();
        let body = self.parse_function_body()?;
        let redirections = self.parse_optional_redirections()?;

        Ok(Command::FunctionDef(FunctionDef {
            name,
            body: Box::new(body),
            redirections,
        }))
    }

    pub(super) fn parse_function_body(&mut self) -> Result<CompoundCommand, ParseError> {
        match self.current_kind() {
            TokenKind::LBrace => self.parse_group().map(CompoundCommand::Group),
            TokenKind::LParen => self.parse_subshell().map(CompoundCommand::Subshell),
            _ => Err(self.error("expected '{' or '(' for function body")),
        }
    }
}
