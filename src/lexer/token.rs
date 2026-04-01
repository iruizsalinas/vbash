//! Token types produced by the lexer.

/// Position in the source input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub line: usize,
    pub column: usize,
}

/// A token with its kind and source location.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// All token types the lexer can produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    /// A shell word (may contain quotes, expansions, globs).
    Word(String),
    /// A variable assignment: `NAME=value` or `NAME+=value`.
    AssignmentWord(String),
    /// A file descriptor number immediately before a redirect operator.
    IoNumber(String),

    /// `|`
    Pipe,
    /// `|&`
    PipeAmp,
    /// `&&`
    And,
    /// `||`
    Or,
    /// `;`
    Semi,
    /// `&`
    Amp,
    /// `!`
    Bang,

    /// `<`
    Less,
    /// `>`
    Great,
    /// `<<`
    DLess,
    /// `>>`
    DGreat,
    /// `<&`
    LessAnd,
    /// `>&`
    GreatAnd,
    /// `<>`
    LessGreat,
    /// `<<-`
    DLessDash,
    /// `>|`
    Clobber,
    /// `<<<`
    TLess,
    /// `&>`
    AndGreat,
    /// `&>>`
    AndDGreat,

    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `((`
    DParen,
    /// `))`
    DParenClose,
    /// `[[`
    DBrack,
    /// `]]`
    DBrackClose,

    /// `;;`
    DSemi,
    /// `;&`
    SemiAnd,
    /// `;;&`
    SemiSemiAnd,

    If,
    Then,
    Else,
    Elif,
    Fi,
    For,
    While,
    Until,
    Do,
    Done,
    Case,
    Esac,
    In,
    Function,
    Select,
    Time,
    Coproc,

    Newline,
    /// Here-document content (collected after the line containing `<<`).
    HeredocContent(String),
    /// End of input.
    Eof,
}

