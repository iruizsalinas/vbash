#[derive(Debug, Clone)]
pub(super) enum Address {
    Line(usize),
    Last,
    Regex(String),
    Step(usize, usize),
}

#[derive(Debug, Clone)]
pub(super) struct SubFlags {
    pub global: bool,
    pub print: bool,
    pub ignore_case: bool,
    pub nth: Option<usize>,
    pub write_file: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) enum SedAction {
    Substitute { pattern: String, replacement: String, flags: SubFlags },
    Transliterate { from: Vec<char>, to: Vec<char> },
    Delete,
    DeleteFirst,
    Print,
    PrintFirst,
    Quit(i32),
    QuitSilent(i32),
    Append(String),
    Insert(String),
    Change(String),
    PrintLineNumber,
    Next,
    NextAppend,
    HoldCopy,
    HoldAppend,
    GetCopy,
    GetAppend,
    Exchange,
    Branch(Option<String>),
    BranchIfSub(Option<String>),
    BranchIfNotSub(Option<String>),
    Label(String),
    ReadFile(String),
    WriteFile(String),
    Group(Vec<SedCommand>),
}

#[derive(Debug, Clone)]
pub(super) struct SedCommand {
    pub addr1: Option<Address>,
    pub addr2: Option<Address>,
    pub negated: bool,
    pub action: SedAction,
}

pub(super) struct SedProgram {
    pub commands: Vec<SedCommand>,
    pub suppress_print: bool,
    pub use_ere: bool,
}
