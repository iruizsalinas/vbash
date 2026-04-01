//! Error types for parsing, execution, and filesystem operations.

use std::fmt;

/// Top-level error type for all vbash operations.
#[derive(Debug)]
pub enum Error {
    /// A syntax error encountered during parsing.
    Parse(ParseError),
    /// An error during command execution.
    Exec(ExecError),
    /// A filesystem operation failed.
    Fs(FsError),
    /// An execution limit was exceeded.
    LimitExceeded(LimitKind),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "{e}"),
            Self::Exec(e) => write!(f, "{e}"),
            Self::Fs(e) => write!(f, "{e}"),
            Self::LimitExceeded(kind) => write!(f, "execution limit exceeded: {kind}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<ParseError> for Error {
    fn from(e: ParseError) -> Self {
        Self::Parse(e)
    }
}

impl From<ExecError> for Error {
    fn from(e: ExecError) -> Self {
        Self::Exec(e)
    }
}

impl From<FsError> for Error {
    fn from(e: FsError) -> Self {
        Self::Fs(e)
    }
}

impl From<LimitKind> for Error {
    fn from(kind: LimitKind) -> Self {
        Self::LimitExceeded(kind)
    }
}

/// A syntax error with source location.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "syntax error at {}:{}: {}", self.line, self.column, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Errors that can occur during command execution.
#[derive(Debug, Clone)]
pub enum ExecError {
    DivisionByZero,
    UnboundVariable(String),
    /// A generic execution error with a message.
    Other(String),
}

impl fmt::Display for ExecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DivisionByZero => write!(f, "division by 0"),
            Self::UnboundVariable(name) => write!(f, "{name}: unbound variable"),
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ExecError {}

/// Filesystem errors modeled after POSIX errno values.
#[derive(Debug, Clone)]
pub enum FsError {
    /// ENOENT - path does not exist.
    NotFound(String),
    /// EEXIST - path already exists.
    AlreadyExists(String),
    /// ENOTDIR - expected a directory.
    NotADirectory(String),
    /// EISDIR - expected a file, got a directory.
    IsADirectory(String),
    /// EACCES - permission denied or path escapes sandbox.
    PermissionDenied(String),
    /// ELOOP - too many symlink levels.
    SymlinkLoop(String),
    /// EFBIG - file too large.
    TooLarge(String),
    /// EXDEV - cross-device link.
    CrossDevice(String),
    /// EBUSY - resource busy (e.g. mount point).
    Busy(String),
    /// EINVAL - invalid argument.
    InvalidArgument(String),
}

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(p) => write!(f, "{p}: No such file or directory"),
            Self::AlreadyExists(p) => write!(f, "{p}: File exists"),
            Self::NotADirectory(p) => write!(f, "{p}: Not a directory"),
            Self::IsADirectory(p) => write!(f, "{p}: Is a directory"),
            Self::PermissionDenied(p) => write!(f, "{p}: Permission denied"),
            Self::SymlinkLoop(p) => write!(f, "{p}: Too many levels of symbolic links"),
            Self::TooLarge(p) => write!(f, "{p}: File too large"),
            Self::CrossDevice(p) => write!(f, "{p}: Invalid cross-device link"),
            Self::Busy(p) => write!(f, "{p}: Device or resource busy"),
            Self::InvalidArgument(p) => write!(f, "{p}: Invalid argument"),
        }
    }
}

impl std::error::Error for FsError {}

/// Which resource limit was exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitKind {
    CallDepth,
    CommandCount,
    LoopIterations,
    OutputSize,
    SubstitutionDepth,
    BraceExpansion,
    StringLength,
    ArrayElements,
    SourceDepth,
    InputSize,
    Cancelled,
    SessionExecCalls,
    SessionCommands,
}

impl fmt::Display for LimitKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::CallDepth => "maximum call depth",
            Self::CommandCount => "maximum command count",
            Self::LoopIterations => "maximum loop iterations",
            Self::OutputSize => "maximum output size",
            Self::SubstitutionDepth => "maximum substitution depth",
            Self::BraceExpansion => "maximum brace expansion results",
            Self::StringLength => "maximum string length",
            Self::ArrayElements => "maximum array elements",
            Self::SourceDepth => "maximum source depth",
            Self::InputSize => "maximum input size",
            Self::Cancelled => "execution cancelled",
            Self::SessionExecCalls => "session exec call limit",
            Self::SessionCommands => "session command limit",
        };
        write!(f, "{name}")
    }
}

/// Shell control flow signals caught at scope boundaries.
///
/// These are not errors - they represent normal bash control flow like
/// `break`, `continue`, `return`, and `exit`. The interpreter catches
/// them at the appropriate scope (loops, functions, top-level).
#[derive(Debug)]
pub(crate) enum ControlFlow {
    Break { n: u32, stdout: String, stderr: String },
    Continue { n: u32, stdout: String, stderr: String },
    Return { code: i32, stdout: String, stderr: String },
    Exit { code: i32, stdout: String, stderr: String },
}

/// Unified internal result type combining control flow with real errors.
#[derive(Debug)]
pub(crate) enum ShellSignal {
    Flow(ControlFlow),
    Error(Error),
}

impl From<ControlFlow> for ShellSignal {
    fn from(cf: ControlFlow) -> Self {
        Self::Flow(cf)
    }
}

impl From<Error> for ShellSignal {
    fn from(e: Error) -> Self {
        Self::Error(e)
    }
}

impl From<FsError> for ShellSignal {
    fn from(e: FsError) -> Self {
        Self::Error(Error::Fs(e))
    }
}

impl From<ExecError> for ShellSignal {
    fn from(e: ExecError) -> Self {
        Self::Error(Error::Exec(e))
    }
}

impl From<ParseError> for ShellSignal {
    fn from(e: ParseError) -> Self {
        Self::Error(Error::Parse(e))
    }
}

impl From<LimitKind> for ShellSignal {
    fn from(kind: LimitKind) -> Self {
        Self::Error(Error::LimitExceeded(kind))
    }
}
