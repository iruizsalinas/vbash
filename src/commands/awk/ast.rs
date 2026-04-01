use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(super) enum Expr {
    Num(f64),
    Str(String),
    Regex(String),
    Var(String),
    Field(Box<Expr>),
    Array(String, Vec<Expr>),
    Assign(Box<Expr>, Box<Expr>),
    CompoundAssign(String, Box<Expr>, Box<Expr>),
    BinOp(String, Box<Expr>, Box<Expr>),
    UnaryOp(String, Box<Expr>),
    PreInc(Box<Expr>),
    PreDec(Box<Expr>),
    PostInc(Box<Expr>),
    PostDec(Box<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    MatchOp(Box<Expr>, Box<Expr>),
    NotMatchOp(Box<Expr>, Box<Expr>),
    Concat(Box<Expr>, Box<Expr>),
    InArray(String, Vec<Expr>),
    Getline,
    Call(String, Vec<Expr>),
    Sprintf(Vec<Expr>),
}

#[derive(Debug, Clone)]
pub(super) enum Stmt {
    Expr(Expr),
    Print(Vec<Expr>, Option<Box<Expr>>),
    Printf(Vec<Expr>, Option<Box<Expr>>),
    If(Expr, Vec<Stmt>, Option<Vec<Stmt>>),
    While(Expr, Vec<Stmt>),
    DoWhile(Vec<Stmt>, Expr),
    For(Option<Box<Stmt>>, Option<Expr>, Option<Box<Stmt>>, Vec<Stmt>),
    ForIn(String, String, Vec<Stmt>),
    Break,
    Continue,
    Next,
    Exit(Option<Expr>),
    Return(Option<Expr>),
    Delete(String, Vec<Expr>),
    Block(Vec<Stmt>),
}

#[derive(Debug, Clone)]
pub(super) enum Pattern {
    Begin,
    End,
    Expr(Expr),
    Regex(String),
    Range(Box<Pattern>, Box<Pattern>),
    All,
}

#[derive(Debug, Clone)]
pub(super) struct Rule {
    pub pattern: Pattern,
    pub action: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub(super) struct AwkFunction {
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub(super) struct Program {
    pub rules: Vec<Rule>,
    pub functions: HashMap<String, AwkFunction>,
}
