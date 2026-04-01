//! Shell word types and their expansion parts.

/// A shell word composed of one or more parts.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub parts: Vec<WordPart>,
}

impl Word {
    pub fn literal(s: impl Into<String>) -> Self {
        Self {
            parts: vec![WordPart::Literal(s.into())],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }
}

/// One segment of a shell word.
#[derive(Debug, Clone, PartialEq)]
pub enum WordPart {
    /// Raw text, no special meaning.
    Literal(String),
    /// `'...'` - literal content, no expansion.
    SingleQuoted(String),
    /// `"..."` - may contain expansions.
    DoubleQuoted(Vec<WordPart>),
    /// `\x` - single escaped character.
    Escaped(char),
    /// `$'...'` - ANSI-C quoting with escape sequences.
    AnsiCQuoted(String),
    /// `$VAR` or `${VAR}` with optional operation.
    Parameter(ParameterExpansion),
    /// `$(command)` or `` `command` ``.
    CommandSubstitution(String),
    /// `$((expression))`.
    ArithmeticExpansion(super::ArithExpr),
    /// `{a,b,c}` or `{1..10}` or `{1..10..2}`.
    BraceExpansion(BraceExpansion),
    /// `~` or `~user`.
    TildeExpansion(String),
    /// `*`, `?`, `[abc]` - glob metacharacter.
    Glob(GlobPart),
    /// `<(cmd)` or `>(cmd)` - process substitution (parsed but limited support).
    ProcessSubstitution { command: String, direction: ProcessDirection },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParameterExpansion {
    pub name: String,
    /// Array subscript: `${arr[idx]}`.
    pub subscript: Option<Box<Word>>,
    pub operation: Option<ParamOp>,
}

/// Operations inside `${name op ...}`.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamOp {
    /// `${name:-word}` or `${name-word}`. `colon` distinguishes empty vs unset.
    Default { word: Word, colon: bool },
    /// `${name:=word}` or `${name=word}`.
    AssignDefault { word: Word, colon: bool },
    /// `${name:?word}` or `${name?word}`.
    Error { word: Word, colon: bool },
    /// `${name:+word}` or `${name+word}`.
    Alternative { word: Word, colon: bool },
    /// `${#name}`.
    Length,
    /// `${name:offset}` or `${name:offset:length}`.
    Substring { offset: Word, length: Option<Word> },
    /// `${name#pattern}`, `${name##pattern}`, `${name%pattern}`, `${name%%pattern}`.
    PatternRemoval { pattern: Word, side: PatternSide, greedy: bool },
    /// `${name/pattern/replacement}` or `${name//pattern/replacement}`.
    PatternReplace { pattern: Word, replacement: Option<Word>, all: bool, anchor: Option<PatternAnchor> },
    /// `${name^}`, `${name^^}`, `${name,}`, `${name,,}`.
    CaseModify { direction: CaseDirection, all: bool },
    /// `${!name}` - indirect expansion.
    Indirection,
    /// `${!prefix*}` or `${!prefix@}` - variable names with prefix.
    VarNamePrefix { star: bool },
    /// `${!arr[@]}` or `${!arr[*]}` - array keys.
    ArrayKeys { star: bool },
    /// `${name@operator}` - transformations.
    Transform(TransformOp),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternSide {
    Prefix,
    Suffix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternAnchor {
    Start,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseDirection {
    Upper,
    Lower,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformOp {
    /// `@Q` - quote.
    Quote,
    /// `@E` - expand escapes.
    Escape,
    /// `@P` - prompt expansion.
    Prompt,
    /// `@A` - assignment form.
    Assignment,
    /// `@a` - attributes.
    Attributes,
    /// `@K` - quoted key-value pairs.
    KeyValue,
    /// `@k` - unquoted key-value pairs.
    KeyValueUnquoted,
    /// `@u` - uppercase first char.
    UpperFirst,
    /// `@U` - uppercase all.
    UpperAll,
    /// `@L` - lowercase all.
    LowerAll,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BraceExpansion {
    /// `{a,b,c}` - list of alternatives.
    List(Vec<Word>),
    /// `{start..end}` or `{start..end..step}`.
    Range {
        start: String,
        end: String,
        step: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlobPart {
    /// `*`
    Star,
    /// `?`
    Question,
    /// `[abc]` or `[a-z]` or `[!abc]`.
    CharClass { negated: bool, content: String },
    /// `**` (when globstar is enabled).
    GlobStar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessDirection {
    /// `<(cmd)` - read from process.
    In,
    /// `>(cmd)` - write to process.
    Out,
}
