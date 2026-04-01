//! AST types for parsed bash scripts.

pub mod arithmetic;
pub mod conditional;
pub mod word;

pub use arithmetic::ArithExpr;
pub use conditional::CondExpr;
pub use word::{Word, WordPart};

/// Root node: a complete script is a sequence of statements.
#[derive(Debug, Clone, PartialEq)]
pub struct Script {
    pub statements: Vec<Statement>,
}

/// A list of pipelines joined by `&&`, `||`, or executed sequentially.
#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub pipelines: Vec<Pipeline>,
    /// Operators between adjacent pipelines (`&&` or `||`).
    /// Length is always `pipelines.len() - 1`.
    pub operators: Vec<ListOp>,
    /// True if the statement ends with `&` (background execution).
    pub background: bool,
}

/// Operator joining two pipelines in a statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListOp {
    And,
    Or,
}

/// A sequence of commands connected by pipes.
#[derive(Debug, Clone, PartialEq)]
pub struct Pipeline {
    pub commands: Vec<Command>,
    /// `!` prefix negates the exit code.
    pub negated: bool,
    /// Per-pipe flag: true means `|&` (pipe stderr too).
    /// Length is `commands.len() - 1`.
    pub pipe_stderr: Vec<bool>,
    /// `time` prefix measures execution time.
    pub timed: bool,
}

/// A single command in a pipeline.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Simple(SimpleCommand),
    Compound(CompoundCommand),
    FunctionDef(FunctionDef),
}

/// `name arg1 arg2 ... >file` with optional leading assignments.
#[derive(Debug, Clone, PartialEq)]
pub struct SimpleCommand {
    pub assignments: Vec<Assignment>,
    pub name: Option<Word>,
    pub args: Vec<Word>,
    pub redirections: Vec<Redirection>,
}

/// `VAR=value` or `VAR+=value` or `VAR=(array elements)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    pub name: String,
    pub value: Option<Word>,
    pub append: bool,
    pub array: Option<Vec<Word>>,
}

/// Control structures, grouping, and test constructs.
#[derive(Debug, Clone, PartialEq)]
pub enum CompoundCommand {
    If(IfCmd),
    For(ForCmd),
    CStyleFor(CStyleForCmd),
    While(WhileCmd),
    Until(UntilCmd),
    Case(CaseCmd),
    Subshell(SubshellCmd),
    Group(GroupCmd),
    Arithmetic(ArithmeticCmd),
    Conditional(ConditionalCmd),
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfCmd {
    pub clauses: Vec<IfClause>,
    pub else_body: Option<Vec<Statement>>,
    pub redirections: Vec<Redirection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfClause {
    pub condition: Vec<Statement>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForCmd {
    pub variable: String,
    /// `None` means iterate over `"$@"`.
    pub words: Option<Vec<Word>>,
    pub body: Vec<Statement>,
    pub redirections: Vec<Redirection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CStyleForCmd {
    pub init: Option<ArithExpr>,
    pub condition: Option<ArithExpr>,
    pub update: Option<ArithExpr>,
    pub body: Vec<Statement>,
    pub redirections: Vec<Redirection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhileCmd {
    pub condition: Vec<Statement>,
    pub body: Vec<Statement>,
    pub redirections: Vec<Redirection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UntilCmd {
    pub condition: Vec<Statement>,
    pub body: Vec<Statement>,
    pub redirections: Vec<Redirection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaseCmd {
    pub word: Word,
    pub items: Vec<CaseItem>,
    pub redirections: Vec<Redirection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaseItem {
    pub patterns: Vec<Word>,
    pub body: Vec<Statement>,
    pub terminator: CaseTerminator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseTerminator {
    /// `;;` - stop matching.
    Break,
    /// `;&` - fall through to next body without testing.
    FallThrough,
    /// `;;&` - continue testing remaining patterns.
    Continue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubshellCmd {
    pub body: Vec<Statement>,
    pub redirections: Vec<Redirection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupCmd {
    pub body: Vec<Statement>,
    pub redirections: Vec<Redirection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArithmeticCmd {
    pub expression: ArithExpr,
    pub redirections: Vec<Redirection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConditionalCmd {
    pub expression: CondExpr,
    pub redirections: Vec<Redirection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDef {
    pub name: String,
    pub body: Box<CompoundCommand>,
    pub redirections: Vec<Redirection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Redirection {
    /// Source file descriptor (`None` uses the default for the operator).
    pub fd: Option<u32>,
    /// `{varname}` dynamic FD allocation.
    pub fd_variable: Option<String>,
    pub operator: RedirectOp,
    pub target: RedirectTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedirectOp {
    /// `<`
    Input,
    /// `>`
    Output,
    /// `>>`
    Append,
    /// `>&`
    DupOutput,
    /// `<&`
    DupInput,
    /// `<>`
    ReadWrite,
    /// `>|`
    Clobber,
    /// `&>`
    OutputAll,
    /// `&>>`
    AppendAll,
    /// `<<<`
    HereString,
    /// `<<`
    HereDoc,
    /// `<<-`
    HereDocStrip,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RedirectTarget {
    /// A filename or FD number to redirect to/from.
    Word(Word),
    /// Here-document content.
    HereDoc(HereDoc),
}

#[derive(Debug, Clone, PartialEq)]
pub struct HereDoc {
    pub delimiter: String,
    pub content: Word,
    pub strip_tabs: bool,
    /// Quoted delimiter means no expansion inside content.
    pub quoted: bool,
}
