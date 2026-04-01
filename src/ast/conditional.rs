//! Conditional expression AST nodes for `[[ ... ]]`.

use super::Word;

/// A conditional expression inside `[[ ]]`.
#[derive(Debug, Clone, PartialEq)]
pub enum CondExpr {
    /// Binary test: `word1 op word2`.
    Binary {
        op: CondBinaryOp,
        left: Word,
        right: Word,
    },
    /// Unary test: `op word`.
    Unary {
        op: CondUnaryOp,
        operand: Word,
    },
    /// `! expr`.
    Not(Box<CondExpr>),
    /// `expr1 && expr2`.
    And(Box<CondExpr>, Box<CondExpr>),
    /// `expr1 || expr2`.
    Or(Box<CondExpr>, Box<CondExpr>),
    /// `( expr )` - parenthesized grouping.
    Group(Box<CondExpr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CondBinaryOp {
    /// `=` or `==` - string/pattern match.
    Eq,
    /// `!=` - string/pattern not match.
    Ne,
    /// `=~` - regex match.
    RegexMatch,
    /// `<` - string less than.
    StrLt,
    /// `>` - string greater than.
    StrGt,
    /// `-eq` - integer equal.
    IntEq,
    /// `-ne` - integer not equal.
    IntNe,
    /// `-lt` - integer less than.
    IntLt,
    /// `-le` - integer less or equal.
    IntLe,
    /// `-gt` - integer greater than.
    IntGt,
    /// `-ge` - integer greater or equal.
    IntGe,
    /// `-nt` - file newer than.
    NewerThan,
    /// `-ot` - file older than.
    OlderThan,
    /// `-ef` - same file (device + inode).
    SameFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CondUnaryOp {
    /// `-a` or `-e` - file exists.
    FileExists,
    /// `-b` - block device.
    BlockDevice,
    /// `-c` - character device.
    CharDevice,
    /// `-d` - directory.
    IsDirectory,
    /// `-f` - regular file.
    IsFile,
    /// `-g` - setgid bit.
    SetGid,
    /// `-h` or `-L` - symbolic link.
    IsSymlink,
    /// `-k` - sticky bit.
    Sticky,
    /// `-p` - named pipe.
    IsPipe,
    /// `-r` - readable.
    IsReadable,
    /// `-s` - non-empty file.
    NonEmpty,
    /// `-t` - terminal file descriptor.
    IsTerminal,
    /// `-u` - setuid bit.
    SetUid,
    /// `-w` - writable.
    IsWritable,
    /// `-x` - executable.
    IsExecutable,
    /// `-G` - owned by effective group.
    OwnedByGroup,
    /// `-N` - modified since last read.
    ModifiedSinceRead,
    /// `-O` - owned by effective user.
    OwnedByUser,
    /// `-S` - socket.
    IsSocket,
    /// `-z` - string is empty.
    StringEmpty,
    /// `-n` - string is non-empty.
    StringNonEmpty,
    /// `-o` - shell option is set.
    OptionSet,
    /// `-v` - variable is set.
    VariableSet,
    /// `-R` - variable is a nameref.
    IsNameref,
}
