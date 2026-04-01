//! Arithmetic expression AST nodes for `$((...))` and `((...))`.

use super::Word;

/// An arithmetic expression.
#[derive(Debug, Clone, PartialEq)]
pub enum ArithExpr {
    /// Integer literal.
    Number(i64),
    /// Variable reference (name without `$` prefix).
    Variable(String),
    /// `arr[index]`.
    ArrayElement {
        array: String,
        index: Box<ArithExpr>,
    },
    /// Binary operation: `left op right`.
    Binary {
        op: ArithBinaryOp,
        left: Box<ArithExpr>,
        right: Box<ArithExpr>,
    },
    /// Unary prefix or postfix operation.
    Unary {
        op: ArithUnaryOp,
        operand: Box<ArithExpr>,
        prefix: bool,
    },
    /// `condition ? consequent : alternate`.
    Ternary {
        condition: Box<ArithExpr>,
        consequent: Box<ArithExpr>,
        alternate: Box<ArithExpr>,
    },
    /// `variable = expr` or `variable += expr` etc.
    Assign {
        name: String,
        subscript: Option<Box<ArithExpr>>,
        op: ArithAssignOp,
        value: Box<ArithExpr>,
    },
    /// Parenthesized grouping.
    Group(Box<ArithExpr>),
    /// `$(command)` inside arithmetic.
    CommandSubst(String),
    /// Comma operator: evaluates both, returns the second.
    Comma {
        left: Box<ArithExpr>,
        right: Box<ArithExpr>,
    },
    /// Nested `$((expr))` inside arithmetic.
    Nested(Box<ArithExpr>),
    /// `$name` inside arithmetic - expanded as parameter then coerced to number.
    ParameterExpansion(Box<Word>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArithBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    ShiftLeft,
    ShiftRight,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    BitAnd,
    BitOr,
    BitXor,
    LogicalAnd,
    LogicalOr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArithUnaryOp {
    Negate,
    Plus,
    LogicalNot,
    BitNot,
    Increment,
    Decrement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArithAssignOp {
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
    ShiftLeftAssign,
    ShiftRightAssign,
    BitAndAssign,
    BitOrAssign,
    BitXorAssign,
}
