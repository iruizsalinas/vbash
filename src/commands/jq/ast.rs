use serde_json::Value;

#[derive(Debug, Clone)]
pub(super) enum StringPart {
    Literal(String),
    Expr(JqExpr),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Alt,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum UnaryOp {
    Not,
    Neg,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum UpdateOp {
    Assign,
    Update,
    AddUpdate,
    SubUpdate,
    MulUpdate,
    DivUpdate,
    ModUpdate,
}

#[derive(Debug, Clone)]
pub(super) enum JqExpr {
    Identity,
    Field(String),
    OptionalField(String),
    Index(Box<JqExpr>),
    Slice(Option<Box<JqExpr>>, Option<Box<JqExpr>>),
    Iterate,
    OptionalIterate,
    Pipe(Box<JqExpr>, Box<JqExpr>),
    Comma(Box<JqExpr>, Box<JqExpr>),
    Literal(Value),
    ArrayConstruct(Option<Box<JqExpr>>),
    ObjectConstruct(Vec<(JqExpr, JqExpr)>),
    Paren(Box<JqExpr>),
    BinaryOp(BinOp, Box<JqExpr>, Box<JqExpr>),
    UnaryOp(UnaryOp, Box<JqExpr>),
    Conditional {
        cond: Box<JqExpr>,
        then_branch: Box<JqExpr>,
        elif_branches: Vec<(JqExpr, JqExpr)>,
        else_branch: Option<Box<JqExpr>>,
    },
    TryCatch {
        try_expr: Box<JqExpr>,
        catch_expr: Option<Box<JqExpr>>,
    },
    FuncCall(String, Vec<JqExpr>),
    VarRef(String),
    VarBind {
        expr: Box<JqExpr>,
        name: String,
        body: Box<JqExpr>,
    },
    Reduce {
        expr: Box<JqExpr>,
        name: String,
        init: Box<JqExpr>,
        update: Box<JqExpr>,
    },
    Foreach {
        expr: Box<JqExpr>,
        name: String,
        init: Box<JqExpr>,
        update: Box<JqExpr>,
        extract: Option<Box<JqExpr>>,
    },
    Recurse,
    StringInterp(Vec<StringPart>),
    UpdateOp(UpdateOp, Box<JqExpr>, Box<JqExpr>),
    Optional(Box<JqExpr>),
    Label(String, Box<JqExpr>),
    FuncDef {
        name: String,
        args: Vec<String>,
        body: Box<JqExpr>,
        next: Option<Box<JqExpr>>,
    },
    Format(String, Option<Box<JqExpr>>),
    BreakExpr(String),
}
