//! Arithmetic expression AST nodes for `$((...))` and `((...))`.
//!
//! Most arithmetic is currently evaluated via string parsing in the interpreter.
//! These AST nodes are used for simple cases (C-style for, `(( ))` commands)
//! and serve as the foundation for a full AST-based arithmetic parser.

use super::Word;

/// An arithmetic expression.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
pub enum ArithUnaryOp {
    Negate,
    Plus,
    LogicalNot,
    BitNot,
    Increment,
    Decrement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
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
