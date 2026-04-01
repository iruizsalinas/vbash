use serde_json::Value;

use super::ast::{BinOp, JqExpr, UnaryOp, UpdateOp};
use super::lexer::{Token, INTERP_PARTS};

pub(super) struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, expected: &Token) -> Result<(), String> {
        match self.next() {
            Some(tok) if tok == expected => Ok(()),
            Some(tok) => Err(format!("expected {expected:?}, got {tok:?}")),
            None => Err(format!("expected {expected:?}, got end of input")),
        }
    }

    fn parse_expr(&mut self) -> Result<JqExpr, String> {
        self.parse_pipe()
    }

    fn parse_pipe(&mut self) -> Result<JqExpr, String> {
        let mut left = self.parse_comma()?;
        while matches!(self.peek(), Some(Token::Pipe)) {
            self.next();
            let right = self.parse_comma()?;
            left = JqExpr::Pipe(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_comma(&mut self) -> Result<JqExpr, String> {
        let mut left = self.parse_assign()?;
        while matches!(self.peek(), Some(Token::Comma)) {
            self.next();
            let right = self.parse_assign()?;
            left = JqExpr::Comma(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_assign(&mut self) -> Result<JqExpr, String> {
        let left = self.parse_binding()?;
        match self.peek() {
            Some(Token::Eq) => {
                self.next();
                let right = self.parse_binding()?;
                Ok(JqExpr::UpdateOp(UpdateOp::Assign, Box::new(left), Box::new(right)))
            }
            Some(Token::PipeEq) => {
                self.next();
                let right = self.parse_binding()?;
                Ok(JqExpr::UpdateOp(UpdateOp::Update, Box::new(left), Box::new(right)))
            }
            Some(Token::PlusEq) => {
                self.next();
                let right = self.parse_binding()?;
                Ok(JqExpr::UpdateOp(UpdateOp::AddUpdate, Box::new(left), Box::new(right)))
            }
            Some(Token::MinusEq) => {
                self.next();
                let right = self.parse_binding()?;
                Ok(JqExpr::UpdateOp(UpdateOp::SubUpdate, Box::new(left), Box::new(right)))
            }
            Some(Token::MulEq) => {
                self.next();
                let right = self.parse_binding()?;
                Ok(JqExpr::UpdateOp(UpdateOp::MulUpdate, Box::new(left), Box::new(right)))
            }
            Some(Token::DivEq) => {
                self.next();
                let right = self.parse_binding()?;
                Ok(JqExpr::UpdateOp(UpdateOp::DivUpdate, Box::new(left), Box::new(right)))
            }
            Some(Token::ModEq) => {
                self.next();
                let right = self.parse_binding()?;
                Ok(JqExpr::UpdateOp(UpdateOp::ModUpdate, Box::new(left), Box::new(right)))
            }
            _ => Ok(left),
        }
    }

    fn parse_binding(&mut self) -> Result<JqExpr, String> {
        if matches!(self.peek(), Some(Token::Label)) {
            self.next();
            if let Some(Token::Var(name)) = self.peek().cloned() {
                self.next();
                self.expect(&Token::Pipe)?;
                let body = self.parse_pipe()?;
                return Ok(JqExpr::Label(name, Box::new(body)));
            }
            return Err("expected variable after label".to_string());
        }
        if matches!(self.peek(), Some(Token::Def)) {
            return self.parse_funcdef();
        }
        let left = self.parse_alternative()?;
        if matches!(self.peek(), Some(Token::As)) {
            self.next();
            if let Some(Token::Var(name)) = self.peek().cloned() {
                self.next();
                self.expect(&Token::Pipe)?;
                let body = self.parse_pipe()?;
                return Ok(JqExpr::VarBind {
                    expr: Box::new(left),
                    name,
                    body: Box::new(body),
                });
            }
            return Err("expected variable after 'as'".to_string());
        }
        Ok(left)
    }

    fn parse_funcdef(&mut self) -> Result<JqExpr, String> {
        self.next();
        let name = match self.next() {
            Some(Token::Ident(n)) => n.clone(),
            other => return Err(format!("expected function name after def, got {other:?}")),
        };
        let mut args = Vec::new();
        if matches!(self.peek(), Some(Token::LParen)) {
            self.next();
            loop {
                match self.peek().cloned() {
                    Some(Token::RParen) => { self.next(); break; }
                    Some(Token::Ident(a)) => { self.next(); args.push(a); }
                    Some(Token::Var(a)) => { self.next(); args.push(format!("${a}")); }
                    _ => return Err("expected argument name in def".to_string()),
                }
                match self.peek() {
                    Some(Token::Semicolon) => { self.next(); }
                    Some(Token::RParen) => {}
                    _ => return Err("expected ; or ) in def arguments".to_string()),
                }
            }
        }
        self.expect(&Token::Colon)?;
        let body = self.parse_pipe()?;
        self.expect(&Token::Semicolon)?;
        let next = if self.peek().is_some() {
            Some(Box::new(self.parse_pipe()?))
        } else {
            None
        };
        Ok(JqExpr::FuncDef { name, args, body: Box::new(body), next })
    }

    fn parse_alternative(&mut self) -> Result<JqExpr, String> {
        let mut left = self.parse_or()?;
        while matches!(self.peek(), Some(Token::SlashSlash)) {
            self.next();
            let right = self.parse_or()?;
            left = JqExpr::BinaryOp(BinOp::Alt, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_or(&mut self) -> Result<JqExpr, String> {
        let mut left = self.parse_and()?;
        while matches!(self.peek(), Some(Token::Or)) {
            self.next();
            let right = self.parse_and()?;
            left = JqExpr::BinaryOp(BinOp::Or, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<JqExpr, String> {
        let mut left = self.parse_comparison()?;
        while matches!(self.peek(), Some(Token::And)) {
            self.next();
            let right = self.parse_comparison()?;
            left = JqExpr::BinaryOp(BinOp::And, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<JqExpr, String> {
        let mut left = self.parse_addition()?;
        loop {
            match self.peek() {
                Some(Token::EqEq) => { self.next(); let r = self.parse_addition()?; left = JqExpr::BinaryOp(BinOp::Eq, Box::new(left), Box::new(r)); }
                Some(Token::NotEq) => { self.next(); let r = self.parse_addition()?; left = JqExpr::BinaryOp(BinOp::Ne, Box::new(left), Box::new(r)); }
                Some(Token::Lt) => { self.next(); let r = self.parse_addition()?; left = JqExpr::BinaryOp(BinOp::Lt, Box::new(left), Box::new(r)); }
                Some(Token::Le) => { self.next(); let r = self.parse_addition()?; left = JqExpr::BinaryOp(BinOp::Le, Box::new(left), Box::new(r)); }
                Some(Token::Gt) => { self.next(); let r = self.parse_addition()?; left = JqExpr::BinaryOp(BinOp::Gt, Box::new(left), Box::new(r)); }
                Some(Token::Ge) => { self.next(); let r = self.parse_addition()?; left = JqExpr::BinaryOp(BinOp::Ge, Box::new(left), Box::new(r)); }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_addition(&mut self) -> Result<JqExpr, String> {
        let mut left = self.parse_multiplication()?;
        loop {
            match self.peek() {
                Some(Token::Plus) => { self.next(); let r = self.parse_multiplication()?; left = JqExpr::BinaryOp(BinOp::Add, Box::new(left), Box::new(r)); }
                Some(Token::Minus) => { self.next(); let r = self.parse_multiplication()?; left = JqExpr::BinaryOp(BinOp::Sub, Box::new(left), Box::new(r)); }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_multiplication(&mut self) -> Result<JqExpr, String> {
        let mut left = self.parse_unary()?;
        loop {
            match self.peek() {
                Some(Token::Mul) => { self.next(); let r = self.parse_unary()?; left = JqExpr::BinaryOp(BinOp::Mul, Box::new(left), Box::new(r)); }
                Some(Token::Div) => { self.next(); let r = self.parse_unary()?; left = JqExpr::BinaryOp(BinOp::Div, Box::new(left), Box::new(r)); }
                Some(Token::Mod) => { self.next(); let r = self.parse_unary()?; left = JqExpr::BinaryOp(BinOp::Mod, Box::new(left), Box::new(r)); }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<JqExpr, String> {
        if matches!(self.peek(), Some(Token::Minus)) {
            self.next();
            let expr = self.parse_postfix()?;
            return Ok(JqExpr::UnaryOp(UnaryOp::Neg, Box::new(expr)));
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<JqExpr, String> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.peek() {
                Some(Token::Dot) => {
                    self.next();
                    match self.peek().cloned() {
                        Some(Token::Ident(name)) => {
                            self.next();
                            if matches!(self.peek(), Some(Token::Question)) {
                                self.next();
                                expr = JqExpr::Pipe(Box::new(expr), Box::new(JqExpr::OptionalField(name)));
                            } else {
                                expr = JqExpr::Pipe(Box::new(expr), Box::new(JqExpr::Field(name)));
                            }
                        }
                        Some(Token::Str(s)) => {
                            self.next();
                            if matches!(self.peek(), Some(Token::Question)) {
                                self.next();
                                expr = JqExpr::Pipe(Box::new(expr), Box::new(JqExpr::OptionalField(s)));
                            } else {
                                expr = JqExpr::Pipe(Box::new(expr), Box::new(JqExpr::Field(s)));
                            }
                        }
                        _ => {
                            expr = JqExpr::Pipe(Box::new(expr), Box::new(JqExpr::Identity));
                        }
                    }
                }
                Some(Token::LBracket) => {
                    self.next();
                    expr = self.parse_index_suffix(expr)?;
                }
                Some(Token::Question) => {
                    self.next();
                    expr = JqExpr::Optional(Box::new(expr));
                }
                Some(Token::Not) => {
                    self.next();
                    expr = JqExpr::UnaryOp(UnaryOp::Not, Box::new(expr));
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_index_suffix(&mut self, base: JqExpr) -> Result<JqExpr, String> {
        if matches!(self.peek(), Some(Token::RBracket)) {
            self.next();
            if matches!(self.peek(), Some(Token::Question)) {
                self.next();
                return Ok(JqExpr::Pipe(Box::new(base), Box::new(JqExpr::OptionalIterate)));
            }
            return Ok(JqExpr::Pipe(Box::new(base), Box::new(JqExpr::Iterate)));
        }

        if matches!(self.peek(), Some(Token::Colon)) {
            self.next();
            let end = if matches!(self.peek(), Some(Token::RBracket)) {
                None
            } else {
                Some(Box::new(self.parse_expr()?))
            };
            self.expect(&Token::RBracket)?;
            return Ok(JqExpr::Pipe(Box::new(base), Box::new(JqExpr::Slice(None, end))));
        }

        let idx = self.parse_expr()?;

        if matches!(self.peek(), Some(Token::Colon)) {
            self.next();
            let end = if matches!(self.peek(), Some(Token::RBracket)) {
                None
            } else {
                Some(Box::new(self.parse_expr()?))
            };
            self.expect(&Token::RBracket)?;
            return Ok(JqExpr::Pipe(Box::new(base), Box::new(JqExpr::Slice(Some(Box::new(idx)), end))));
        }

        self.expect(&Token::RBracket)?;
        if matches!(self.peek(), Some(Token::Question)) {
            self.next();
            let inner = JqExpr::Optional(Box::new(JqExpr::Index(Box::new(idx))));
            return Ok(JqExpr::Pipe(Box::new(base), Box::new(inner)));
        }
        Ok(JqExpr::Pipe(Box::new(base), Box::new(JqExpr::Index(Box::new(idx)))))
    }

    fn parse_primary(&mut self) -> Result<JqExpr, String> {
        match self.peek().cloned() {
            Some(Token::Dot) => {
                self.next();
                match self.peek().cloned() {
                    Some(Token::Ident(name)) => {
                        self.next();
                        if matches!(self.peek(), Some(Token::Question)) {
                            self.next();
                            Ok(JqExpr::OptionalField(name))
                        } else {
                            Ok(JqExpr::Field(name))
                        }
                    }
                    Some(Token::Str(s)) => {
                        self.next();
                        if matches!(self.peek(), Some(Token::Question)) {
                            self.next();
                            Ok(JqExpr::OptionalField(s))
                        } else {
                            Ok(JqExpr::Field(s))
                        }
                    }
                    Some(Token::LBracket) => {
                        self.next();
                        self.parse_index_suffix(JqExpr::Identity)
                    }
                    _ => Ok(JqExpr::Identity),
                }
            }
            Some(Token::DotDot) => {
                self.next();
                Ok(JqExpr::Recurse)
            }
            Some(Token::Number(n)) => {
                self.next();
                if n.fract() == 0.0 && n >= i64::MIN as f64 && n <= i64::MAX as f64 {
                    Ok(JqExpr::Literal(Value::Number(serde_json::Number::from(n as i64))))
                } else {
                    Ok(JqExpr::Literal(serde_json::Number::from_f64(n).map_or(Value::Null, Value::Number)))
                }
            }
            Some(Token::Str(ref s)) if s.starts_with("\x00INTERP:") => {
                self.next();
                let parts = INTERP_PARTS.with(|cell| {
                    let mut v = cell.borrow_mut();
                    if v.is_empty() { Vec::new() } else { v.remove(0) }
                });
                Ok(JqExpr::StringInterp(parts))
            }
            Some(Token::Str(s)) => {
                self.next();
                Ok(JqExpr::Literal(Value::String(s)))
            }
            Some(Token::True) => { self.next(); Ok(JqExpr::Literal(Value::Bool(true))) }
            Some(Token::False) => { self.next(); Ok(JqExpr::Literal(Value::Bool(false))) }
            Some(Token::Null) => { self.next(); Ok(JqExpr::Literal(Value::Null)) }
            Some(Token::LParen) => {
                self.next();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(JqExpr::Paren(Box::new(expr)))
            }
            Some(Token::LBracket) => {
                self.next();
                if matches!(self.peek(), Some(Token::RBracket)) {
                    self.next();
                    Ok(JqExpr::ArrayConstruct(None))
                } else {
                    let inner = self.parse_expr()?;
                    self.expect(&Token::RBracket)?;
                    Ok(JqExpr::ArrayConstruct(Some(Box::new(inner))))
                }
            }
            Some(Token::LBrace) => {
                self.next();
                let mut pairs = Vec::new();
                if !matches!(self.peek(), Some(Token::RBrace)) {
                    loop {
                        let (key, val) = self.parse_object_entry()?;
                        pairs.push((key, val));
                        if matches!(self.peek(), Some(Token::Comma)) {
                            self.next();
                        } else {
                            break;
                        }
                    }
                }
                self.expect(&Token::RBrace)?;
                Ok(JqExpr::ObjectConstruct(pairs))
            }
            Some(Token::If) => {
                self.next();
                let cond = self.parse_expr()?;
                self.expect(&Token::Then)?;
                let then_branch = self.parse_expr()?;
                let mut elif_branches = Vec::new();
                while matches!(self.peek(), Some(Token::Elif)) {
                    self.next();
                    let ec = self.parse_expr()?;
                    self.expect(&Token::Then)?;
                    let et = self.parse_expr()?;
                    elif_branches.push((ec, et));
                }
                let else_branch = if matches!(self.peek(), Some(Token::Else)) {
                    self.next();
                    Some(Box::new(self.parse_expr()?))
                } else {
                    None
                };
                self.expect(&Token::End)?;
                Ok(JqExpr::Conditional {
                    cond: Box::new(cond),
                    then_branch: Box::new(then_branch),
                    elif_branches,
                    else_branch,
                })
            }
            Some(Token::Try) => {
                self.next();
                let try_expr = self.parse_postfix()?;
                let catch_expr = if matches!(self.peek(), Some(Token::Catch)) {
                    self.next();
                    Some(Box::new(self.parse_postfix()?))
                } else {
                    None
                };
                Ok(JqExpr::TryCatch {
                    try_expr: Box::new(try_expr),
                    catch_expr,
                })
            }
            Some(Token::Reduce) => {
                self.next();
                let expr = self.parse_postfix()?;
                self.expect(&Token::As)?;
                let name = match self.next() {
                    Some(Token::Var(n)) => n.clone(),
                    _ => return Err("expected variable in reduce".to_string()),
                };
                self.expect(&Token::LParen)?;
                let init = self.parse_expr()?;
                self.expect(&Token::Semicolon)?;
                let update = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(JqExpr::Reduce {
                    expr: Box::new(expr),
                    name,
                    init: Box::new(init),
                    update: Box::new(update),
                })
            }
            Some(Token::Foreach) => {
                self.next();
                let expr = self.parse_postfix()?;
                self.expect(&Token::As)?;
                let name = match self.next() {
                    Some(Token::Var(n)) => n.clone(),
                    _ => return Err("expected variable in foreach".to_string()),
                };
                self.expect(&Token::LParen)?;
                let init = self.parse_expr()?;
                self.expect(&Token::Semicolon)?;
                let update = self.parse_expr()?;
                let extract = if matches!(self.peek(), Some(Token::Semicolon)) {
                    self.next();
                    Some(Box::new(self.parse_expr()?))
                } else {
                    None
                };
                self.expect(&Token::RParen)?;
                Ok(JqExpr::Foreach {
                    expr: Box::new(expr),
                    name,
                    init: Box::new(init),
                    update: Box::new(update),
                    extract,
                })
            }
            Some(Token::Var(name)) => {
                self.next();
                Ok(JqExpr::VarRef(name))
            }
            Some(Token::Ident(name)) => {
                self.next();
                if matches!(self.peek(), Some(Token::LParen)) {
                    self.next();
                    let mut func_args = Vec::new();
                    if !matches!(self.peek(), Some(Token::RParen)) {
                        loop {
                            func_args.push(self.parse_expr()?);
                            if matches!(self.peek(), Some(Token::Semicolon)) {
                                self.next();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(&Token::RParen)?;
                    Ok(JqExpr::FuncCall(name, func_args))
                } else {
                    Ok(JqExpr::FuncCall(name, Vec::new()))
                }
            }
            Some(Token::Format(name)) => {
                self.next();
                let arg = if matches!(self.peek(), Some(Token::Str(_))) {
                    if let Some(Token::Str(ref s)) = self.peek().cloned() {
                        if s.starts_with("\x00INTERP:") {
                            self.next();
                            let parts = INTERP_PARTS.with(|cell| {
                                let mut v = cell.borrow_mut();
                                if v.is_empty() { Vec::new() } else { v.remove(0) }
                            });
                            Some(Box::new(JqExpr::StringInterp(parts)))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };
                Ok(JqExpr::Format(name, arg))
            }
            Some(Token::Not) => {
                self.next();
                Ok(JqExpr::UnaryOp(UnaryOp::Not, Box::new(JqExpr::Identity)))
            }
            Some(Token::Break) => {
                self.next();
                let label = match self.peek().cloned() {
                    Some(Token::Var(name)) => { self.next(); name }
                    _ => return Err("expected variable after break".to_string()),
                };
                Ok(JqExpr::BreakExpr(label))
            }
            Some(Token::Minus) => {
                self.next();
                let expr = self.parse_postfix()?;
                Ok(JqExpr::UnaryOp(UnaryOp::Neg, Box::new(expr)))
            }
            Some(tok) => Err(format!("unexpected token: {tok:?}")),
            None => Err("unexpected end of input".to_string()),
        }
    }

    fn parse_object_entry(&mut self) -> Result<(JqExpr, JqExpr), String> {
        let key = match self.peek().cloned() {
            Some(Token::Ident(name)) => {
                self.next();
                if matches!(self.peek(), Some(Token::Colon)) {
                    self.next();
                    let val = self.parse_assign()?;
                    return Ok((JqExpr::Literal(Value::String(name)), val));
                }
                return Ok((
                    JqExpr::Literal(Value::String(name.clone())),
                    JqExpr::Field(name),
                ));
            }
            Some(Token::Var(name)) => {
                self.next();
                if matches!(self.peek(), Some(Token::Colon)) {
                    self.next();
                    let val = self.parse_assign()?;
                    return Ok((JqExpr::Literal(Value::String(name)), val));
                }
                return Ok((
                    JqExpr::Literal(Value::String(name.clone())),
                    JqExpr::VarRef(name),
                ));
            }
            Some(Token::Str(s)) => {
                self.next();
                JqExpr::Literal(Value::String(s))
            }
            Some(Token::LParen) => {
                self.next();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                expr
            }
            Some(Token::Format(ref _name)) => {
                self.parse_primary()?
            }
            _ => return Err("expected object key".to_string()),
        };
        self.expect(&Token::Colon)?;
        let val = self.parse_assign()?;
        Ok((key, val))
    }
}

pub(super) fn parse_tokens(tokens: &[Token]) -> Result<JqExpr, String> {
    if tokens.is_empty() {
        return Ok(JqExpr::Identity);
    }
    let mut parser = Parser::new(tokens.to_vec());
    let expr = parser.parse_expr()?;
    if parser.pos < parser.tokens.len() {
        return Err(format!("unexpected token: {:?}", parser.tokens[parser.pos]));
    }
    Ok(expr)
}
