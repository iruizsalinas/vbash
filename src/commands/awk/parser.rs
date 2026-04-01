use std::collections::HashMap;

use super::ast::{AwkFunction, Expr, Pattern, Program, Rule, Stmt};
use super::lexer::Token;

pub(super) struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> &Token {
        let t = self.tokens.get(self.pos).unwrap_or(&Token::Eof);
        self.pos += 1;
        t
    }

    fn eat(&mut self, expected: &Token) -> Result<(), String> {
        if self.peek() == expected {
            self.advance();
            Ok(())
        } else {
            Err(format!("expected {expected:?}, got {:?}", self.peek()))
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek(), Token::Newline | Token::Semi) {
            self.advance();
        }
    }

    fn parse_program(&mut self) -> Result<Program, String> {
        let mut rules = Vec::new();
        let mut functions = HashMap::new();

        self.skip_newlines();
        while !matches!(self.peek(), Token::Eof) {
            if matches!(self.peek(), Token::Function) {
                self.advance();
                let name = match self.advance().clone() {
                    Token::Ident(n) => n,
                    t => return Err(format!("expected function name, got {t:?}")),
                };
                self.eat(&Token::LParen)?;
                let mut params = Vec::new();
                while !matches!(self.peek(), Token::RParen | Token::Eof) {
                    match self.advance().clone() {
                        Token::Ident(p) => params.push(p),
                        Token::Comma => {}
                        t => return Err(format!("expected param name, got {t:?}")),
                    }
                }
                self.eat(&Token::RParen)?;
                self.skip_newlines();
                let body = self.parse_action()?;
                functions.insert(name, AwkFunction { params, body });
            } else {
                let rule = self.parse_rule()?;
                rules.push(rule);
            }
            self.skip_newlines();
        }

        Ok(Program { rules, functions })
    }

    fn parse_rule(&mut self) -> Result<Rule, String> {
        let pattern = self.parse_pattern()?;
        self.skip_newlines();

        let action = if matches!(self.peek(), Token::LBrace) {
            self.parse_action()?
        } else {
            match &pattern {
                Pattern::Begin | Pattern::End => {
                    return Err("BEGIN/END requires action block".to_string());
                }
                _ => vec![Stmt::Print(vec![], None)],
            }
        };

        Ok(Rule { pattern, action })
    }

    fn parse_pattern(&mut self) -> Result<Pattern, String> {
        let pat = match self.peek() {
            Token::Begin => {
                self.advance();
                Pattern::Begin
            }
            Token::End => {
                self.advance();
                Pattern::End
            }
            Token::LBrace => Pattern::All,
            Token::Regex(r) => {
                let r = r.clone();
                self.advance();
                Pattern::Regex(r)
            }
            Token::Eof | Token::Newline | Token::Semi => Pattern::All,
            _ => {
                let expr = self.parse_expr()?;
                Pattern::Expr(expr)
            }
        };

        if matches!(self.peek(), Token::Comma) {
            self.advance();
            self.skip_newlines();
            let pat2 = self.parse_pattern()?;
            Ok(Pattern::Range(Box::new(pat), Box::new(pat2)))
        } else {
            Ok(pat)
        }
    }

    fn parse_action(&mut self) -> Result<Vec<Stmt>, String> {
        self.eat(&Token::LBrace)?;
        self.skip_newlines();
        let mut stmts = Vec::new();
        while !matches!(self.peek(), Token::RBrace | Token::Eof) {
            stmts.push(self.parse_stmt()?);
            self.skip_newlines();
        }
        self.eat(&Token::RBrace)?;
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        self.skip_newlines();
        match self.peek().clone() {
            Token::If => self.parse_if(),
            Token::While => self.parse_while(),
            Token::For => self.parse_for(),
            Token::Do => self.parse_do_while(),
            Token::LBrace => {
                let stmts = self.parse_action()?;
                Ok(Stmt::Block(stmts))
            }
            Token::Break => {
                self.advance();
                self.eat_terminator();
                Ok(Stmt::Break)
            }
            Token::Continue => {
                self.advance();
                self.eat_terminator();
                Ok(Stmt::Continue)
            }
            Token::Next => {
                self.advance();
                self.eat_terminator();
                Ok(Stmt::Next)
            }
            Token::Exit => {
                self.advance();
                let code = if self.is_expr_start() {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                self.eat_terminator();
                Ok(Stmt::Exit(code))
            }
            Token::Return => {
                self.advance();
                let val = if self.is_expr_start() {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                self.eat_terminator();
                Ok(Stmt::Return(val))
            }
            Token::Delete => {
                self.advance();
                let name = match self.advance().clone() {
                    Token::Ident(n) => n,
                    t => return Err(format!("expected array name after delete, got {t:?}")),
                };
                if matches!(self.peek(), Token::LBracket) {
                    self.advance();
                    let mut subs = Vec::new();
                    if !matches!(self.peek(), Token::RBracket) {
                        subs.push(self.parse_expr()?);
                        while matches!(self.peek(), Token::Comma) {
                            self.advance();
                            subs.push(self.parse_expr()?);
                        }
                    }
                    self.eat(&Token::RBracket)?;
                    self.eat_terminator();
                    Ok(Stmt::Delete(name, subs))
                } else {
                    self.eat_terminator();
                    Ok(Stmt::Delete(name, vec![]))
                }
            }
            Token::Print => self.parse_print(),
            Token::Printf => self.parse_printf(),
            _ => {
                let expr = self.parse_expr()?;
                self.eat_terminator();
                Ok(Stmt::Expr(expr))
            }
        }
    }

    fn eat_terminator(&mut self) {
        if matches!(self.peek(), Token::Semi | Token::Newline) {
            self.advance();
        }
    }

    fn is_expr_start(&self) -> bool {
        matches!(
            self.peek(),
            Token::Number(_)
                | Token::Str(_)
                | Token::Ident(_)
                | Token::Dollar
                | Token::LParen
                | Token::Not
                | Token::Minus
                | Token::Plus
                | Token::PlusPlus
                | Token::MinusMinus
                | Token::Regex(_)
        )
    }

    fn parse_print(&mut self) -> Result<Stmt, String> {
        self.advance();
        let mut exprs = Vec::new();
        let mut output_redir = None;

        if self.is_expr_start() {
            exprs.push(self.parse_non_assign_expr()?);
            while matches!(self.peek(), Token::Comma) {
                self.advance();
                exprs.push(self.parse_non_assign_expr()?);
            }
        }

        if matches!(self.peek(), Token::Gt | Token::Append | Token::Pipe) {
            self.advance();
            output_redir = Some(Box::new(self.parse_primary()?));
        }

        self.eat_terminator();
        Ok(Stmt::Print(exprs, output_redir))
    }

    fn parse_printf(&mut self) -> Result<Stmt, String> {
        self.advance();
        let mut exprs = Vec::new();
        let mut output_redir = None;

        if self.is_expr_start() {
            exprs.push(self.parse_non_assign_expr()?);
            while matches!(self.peek(), Token::Comma) {
                self.advance();
                exprs.push(self.parse_non_assign_expr()?);
            }
        }

        if matches!(self.peek(), Token::Gt | Token::Append | Token::Pipe) {
            self.advance();
            output_redir = Some(Box::new(self.parse_primary()?));
        }

        self.eat_terminator();
        Ok(Stmt::Printf(exprs, output_redir))
    }

    fn parse_if(&mut self) -> Result<Stmt, String> {
        self.advance();
        self.eat(&Token::LParen)?;
        let cond = self.parse_expr()?;
        self.eat(&Token::RParen)?;
        self.skip_newlines();
        let then_body = if matches!(self.peek(), Token::LBrace) {
            self.parse_action()?
        } else {
            vec![self.parse_stmt()?]
        };
        self.skip_newlines();
        let else_body = if matches!(self.peek(), Token::Else) {
            self.advance();
            self.skip_newlines();
            if matches!(self.peek(), Token::LBrace) {
                Some(self.parse_action()?)
            } else {
                Some(vec![self.parse_stmt()?])
            }
        } else {
            None
        };
        Ok(Stmt::If(cond, then_body, else_body))
    }

    fn parse_while(&mut self) -> Result<Stmt, String> {
        self.advance();
        self.eat(&Token::LParen)?;
        let cond = self.parse_expr()?;
        self.eat(&Token::RParen)?;
        self.skip_newlines();
        let body = if matches!(self.peek(), Token::LBrace) {
            self.parse_action()?
        } else {
            vec![self.parse_stmt()?]
        };
        Ok(Stmt::While(cond, body))
    }

    fn parse_do_while(&mut self) -> Result<Stmt, String> {
        self.advance();
        self.skip_newlines();
        let body = if matches!(self.peek(), Token::LBrace) {
            self.parse_action()?
        } else {
            vec![self.parse_stmt()?]
        };
        self.skip_newlines();
        if !matches!(self.peek(), Token::While) {
            return Err("expected 'while' after 'do' block".to_string());
        }
        self.advance();
        self.eat(&Token::LParen)?;
        let cond = self.parse_expr()?;
        self.eat(&Token::RParen)?;
        self.eat_terminator();
        Ok(Stmt::DoWhile(body, cond))
    }

    fn parse_for(&mut self) -> Result<Stmt, String> {
        self.advance();
        self.eat(&Token::LParen)?;

        if let Token::Ident(name) = self.peek().clone() {
            let saved = self.pos;
            self.advance();
            if matches!(self.peek(), Token::In) {
                self.advance();
                let arr = match self.advance().clone() {
                    Token::Ident(a) => a,
                    t => return Err(format!("expected array name, got {t:?}")),
                };
                self.eat(&Token::RParen)?;
                self.skip_newlines();
                let body = if matches!(self.peek(), Token::LBrace) {
                    self.parse_action()?
                } else {
                    vec![self.parse_stmt()?]
                };
                return Ok(Stmt::ForIn(name, arr, body));
            }
            self.pos = saved;
        }

        let init = if matches!(self.peek(), Token::Semi) {
            None
        } else {
            let e = self.parse_expr()?;
            Some(Box::new(Stmt::Expr(e)))
        };
        self.eat(&Token::Semi)?;
        let cond = if matches!(self.peek(), Token::Semi) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.eat(&Token::Semi)?;
        let update = if matches!(self.peek(), Token::RParen) {
            None
        } else {
            let e = self.parse_expr()?;
            Some(Box::new(Stmt::Expr(e)))
        };
        self.eat(&Token::RParen)?;
        self.skip_newlines();
        let body = if matches!(self.peek(), Token::LBrace) {
            self.parse_action()?
        } else {
            vec![self.parse_stmt()?]
        };
        Ok(Stmt::For(init, cond, update, body))
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_assign()
    }

    fn parse_non_assign_expr(&mut self) -> Result<Expr, String> {
        self.parse_ternary()
    }

    fn parse_assign(&mut self) -> Result<Expr, String> {
        let left = self.parse_ternary()?;

        match self.peek() {
            Token::Assign => {
                self.advance();
                let right = self.parse_assign()?;
                Ok(Expr::Assign(Box::new(left), Box::new(right)))
            }
            Token::PlusAssign => {
                self.advance();
                let right = self.parse_assign()?;
                Ok(Expr::CompoundAssign("+".to_string(), Box::new(left), Box::new(right)))
            }
            Token::MinusAssign => {
                self.advance();
                let right = self.parse_assign()?;
                Ok(Expr::CompoundAssign("-".to_string(), Box::new(left), Box::new(right)))
            }
            Token::StarAssign => {
                self.advance();
                let right = self.parse_assign()?;
                Ok(Expr::CompoundAssign("*".to_string(), Box::new(left), Box::new(right)))
            }
            Token::SlashAssign => {
                self.advance();
                let right = self.parse_assign()?;
                Ok(Expr::CompoundAssign("/".to_string(), Box::new(left), Box::new(right)))
            }
            Token::PercentAssign => {
                self.advance();
                let right = self.parse_assign()?;
                Ok(Expr::CompoundAssign("%".to_string(), Box::new(left), Box::new(right)))
            }
            Token::CaretAssign => {
                self.advance();
                let right = self.parse_assign()?;
                Ok(Expr::CompoundAssign("^".to_string(), Box::new(left), Box::new(right)))
            }
            _ => Ok(left),
        }
    }

    fn parse_ternary(&mut self) -> Result<Expr, String> {
        let cond = self.parse_or()?;
        if matches!(self.peek(), Token::Question) {
            self.advance();
            let then_expr = self.parse_assign()?;
            self.eat(&Token::Colon)?;
            let else_expr = self.parse_assign()?;
            Ok(Expr::Ternary(Box::new(cond), Box::new(then_expr), Box::new(else_expr)))
        } else {
            Ok(cond)
        }
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_and()?;
        while matches!(self.peek(), Token::Or) {
            self.advance();
            let right = self.parse_and()?;
            left = Expr::BinOp("||".to_string(), Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_in_expr()?;
        while matches!(self.peek(), Token::And) {
            self.advance();
            let right = self.parse_in_expr()?;
            left = Expr::BinOp("&&".to_string(), Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_in_expr(&mut self) -> Result<Expr, String> {
        let left = self.parse_match()?;
        if matches!(self.peek(), Token::In) {
            self.advance();
            let arr = match self.advance().clone() {
                Token::Ident(a) => a,
                t => return Err(format!("expected array name after 'in', got {t:?}")),
            };
            if let Expr::Var(name) = left {
                Ok(Expr::InArray(arr, vec![Expr::Str(name)]))
            } else {
                Ok(Expr::InArray(arr, vec![left]))
            }
        } else {
            Ok(left)
        }
    }

    fn parse_match(&mut self) -> Result<Expr, String> {
        let left = self.parse_comparison()?;
        match self.peek() {
            Token::Match => {
                self.advance();
                let right = self.parse_comparison()?;
                Ok(Expr::MatchOp(Box::new(left), Box::new(right)))
            }
            Token::NotMatch => {
                self.advance();
                let right = self.parse_comparison()?;
                Ok(Expr::NotMatchOp(Box::new(left), Box::new(right)))
            }
            _ => Ok(left),
        }
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let left = self.parse_concat()?;
        match self.peek() {
            Token::Lt => {
                self.advance();
                let right = self.parse_concat()?;
                Ok(Expr::BinOp("<".to_string(), Box::new(left), Box::new(right)))
            }
            Token::Le => {
                self.advance();
                let right = self.parse_concat()?;
                Ok(Expr::BinOp("<=".to_string(), Box::new(left), Box::new(right)))
            }
            Token::Gt => {
                self.advance();
                let right = self.parse_concat()?;
                Ok(Expr::BinOp(">".to_string(), Box::new(left), Box::new(right)))
            }
            Token::Ge => {
                self.advance();
                let right = self.parse_concat()?;
                Ok(Expr::BinOp(">=".to_string(), Box::new(left), Box::new(right)))
            }
            Token::Eq => {
                self.advance();
                let right = self.parse_concat()?;
                Ok(Expr::BinOp("==".to_string(), Box::new(left), Box::new(right)))
            }
            Token::Ne => {
                self.advance();
                let right = self.parse_concat()?;
                Ok(Expr::BinOp("!=".to_string(), Box::new(left), Box::new(right)))
            }
            _ => Ok(left),
        }
    }

    fn parse_concat(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_addition()?;
        while self.is_concat_start() {
            let right = self.parse_addition()?;
            left = Expr::Concat(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn is_concat_start(&self) -> bool {
        matches!(
            self.peek(),
            Token::Dollar
                | Token::Number(_)
                | Token::Str(_)
                | Token::Ident(_)
                | Token::LParen
                | Token::Not
                | Token::PlusPlus
                | Token::MinusMinus
        )
    }

    fn parse_addition(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_multiplication()?;
        loop {
            match self.peek() {
                Token::Plus => {
                    self.advance();
                    let right = self.parse_multiplication()?;
                    left = Expr::BinOp("+".to_string(), Box::new(left), Box::new(right));
                }
                Token::Minus => {
                    self.advance();
                    let right = self.parse_multiplication()?;
                    left = Expr::BinOp("-".to_string(), Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_multiplication(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_power()?;
        loop {
            match self.peek() {
                Token::Star => {
                    self.advance();
                    let right = self.parse_power()?;
                    left = Expr::BinOp("*".to_string(), Box::new(left), Box::new(right));
                }
                Token::Slash => {
                    self.advance();
                    let right = self.parse_power()?;
                    left = Expr::BinOp("/".to_string(), Box::new(left), Box::new(right));
                }
                Token::Percent => {
                    self.advance();
                    let right = self.parse_power()?;
                    left = Expr::BinOp("%".to_string(), Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_power(&mut self) -> Result<Expr, String> {
        let base = self.parse_unary()?;
        if matches!(self.peek(), Token::Caret) {
            self.advance();
            let exp = self.parse_unary()?;
            Ok(Expr::BinOp("^".to_string(), Box::new(base), Box::new(exp)))
        } else {
            Ok(base)
        }
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        match self.peek() {
            Token::Not => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::UnaryOp("!".to_string(), Box::new(expr)))
            }
            Token::Minus => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::UnaryOp("-".to_string(), Box::new(expr)))
            }
            Token::Plus => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::UnaryOp("+".to_string(), Box::new(expr)))
            }
            Token::PlusPlus => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::PreInc(Box::new(expr)))
            }
            Token::MinusMinus => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::PreDec(Box::new(expr)))
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.peek() {
                Token::PlusPlus => {
                    self.advance();
                    expr = Expr::PostInc(Box::new(expr));
                }
                Token::MinusMinus => {
                    self.advance();
                    expr = Expr::PostDec(Box::new(expr));
                }
                Token::LBracket => {
                    if let Expr::Var(name) = expr {
                        self.advance();
                        let mut subs = vec![self.parse_expr()?];
                        while matches!(self.peek(), Token::Comma) {
                            self.advance();
                            subs.push(self.parse_expr()?);
                        }
                        self.eat(&Token::RBracket)?;
                        expr = Expr::Array(name, subs);
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.peek().clone() {
            Token::Number(n) => {
                self.advance();
                Ok(Expr::Num(n))
            }
            Token::Str(s) => {
                self.advance();
                Ok(Expr::Str(s))
            }
            Token::Regex(r) => {
                self.advance();
                Ok(Expr::Regex(r))
            }
            Token::Dollar => {
                self.advance();
                let expr = self.parse_primary()?;
                Ok(Expr::Field(Box::new(expr)))
            }
            Token::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.eat(&Token::RParen)?;
                Ok(expr)
            }
            Token::Getline => {
                self.advance();
                Ok(Expr::Getline)
            }
            Token::Ident(name) => {
                self.advance();
                if matches!(self.peek(), Token::LParen) {
                    self.advance();
                    let mut args = Vec::new();
                    if !matches!(self.peek(), Token::RParen) {
                        args.push(self.parse_expr()?);
                        while matches!(self.peek(), Token::Comma) {
                            self.advance();
                            args.push(self.parse_expr()?);
                        }
                    }
                    self.eat(&Token::RParen)?;
                    if name == "sprintf" {
                        Ok(Expr::Sprintf(args))
                    } else {
                        Ok(Expr::Call(name, args))
                    }
                } else {
                    Ok(Expr::Var(name))
                }
            }
            t => Err(format!("unexpected token: {t:?}")),
        }
    }
}

pub(super) fn parse(tokens: &[Token]) -> Result<Program, String> {
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}
