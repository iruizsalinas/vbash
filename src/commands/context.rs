//! Command execution context - everything a command needs to do its job.

use std::collections::HashMap;

use crate::error::Error;
use crate::fs::VirtualFs;
use crate::ExecResult;

/// Callback type for nested command execution (used by xargs, find -exec, etc.).
pub type ExecCallback<'a> = &'a dyn Fn(&str) -> Result<ExecResult, Error>;

/// Provides filesystem, environment, and I/O access to commands.
///
/// Commands receive this as `&mut` so they can write to stdout/stderr.
/// The filesystem and environment are borrowed from the interpreter.
pub struct CommandContext<'a> {
    pub fs: &'a dyn VirtualFs,
    pub cwd: &'a str,
    pub env: &'a HashMap<String, String>,
    pub stdin: &'a str,
    pub stdout: &'a mut String,
    pub stderr: &'a mut String,
    pub exec_fn: Option<ExecCallback<'a>>,
    pub limits: &'a crate::ExecutionLimits,
    #[cfg(feature = "network")]
    pub network_policy: Option<&'a crate::NetworkPolicy>,
}
