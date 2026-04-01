//! A virtual bash environment for AI agents.
//!
//! Runs bash scripts in-process with a virtual filesystem. Includes
//! `sed`, `awk`, `jq`, and most common Unix commands.
//!
//! ```rust,no_run
//! use vbash::Shell;
//!
//! let mut shell = Shell::builder()
//!     .file("/data/names.txt", "alice\nbob\ncharlie")
//!     .build();
//!
//! let result = shell.exec("cat /data/names.txt | sort | head -n 2").unwrap();
//! assert_eq!(result.stdout, "alice\nbob\n");
//! assert_eq!(result.exit_code, 0);
//! ```

#![forbid(unsafe_code)]

pub(crate) mod ast;
pub(crate) mod commands;
pub(crate) mod error;
pub(crate) mod fs;
pub(crate) mod interpreter;
pub(crate) mod lexer;
pub(crate) mod parser;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub use error::{Error, ExecError, FsError, LimitKind, ParseError};
pub use commands::{CommandFn, CommandContext};
pub use fs::VirtualFs;
pub use fs::memory::InMemoryFs;
pub use fs::mountable::MountableFs;
pub use fs::overlay::OverlayFs;
pub use fs::readwrite::ReadWriteFs;

/// Result of executing a bash command string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub env: HashMap<String, String>,
}

/// Options for a single `exec` call.
#[derive(Default)]
pub struct ExecOptions<'a> {
    /// Standard input provided to the command.
    pub stdin: Option<&'a str>,
    /// Override environment variables for this call only.
    pub env: Option<&'a HashMap<String, String>>,
    /// Override working directory for this call only.
    pub cwd: Option<&'a str>,
    /// Cancellation flag. When set to `true`, execution will stop at the next command boundary.
    pub cancel: Option<Arc<AtomicBool>>,
}

impl std::fmt::Debug for ExecOptions<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecOptions")
            .field("stdin", &self.stdin)
            .field("env", &self.env)
            .field("cwd", &self.cwd)
            .field("cancel", &self.cancel.as_ref().map(|c| c.load(std::sync::atomic::Ordering::Relaxed)))
            .finish()
    }
}

/// Network access policy for the `curl` command (requires `network` feature).
///
/// Controls which URLs the sandboxed shell is allowed to contact.
/// If no policy is set, all network requests are blocked (secure by default).
#[cfg(feature = "network")]
#[derive(Debug, Clone)]
pub struct NetworkPolicy {
    /// If non-empty, only URLs starting with one of these prefixes are allowed.
    /// Uses segment-aware matching to prevent path traversal attacks.
    pub allowed_url_prefixes: Vec<String>,
    /// Block requests to private/loopback IP addresses and `localhost`.
    /// Defaults to `true`.
    pub block_private_ips: bool,
    /// HTTP methods that are allowed. Requests with other methods are rejected.
    pub allowed_methods: Vec<String>,
    /// Maximum response body size in bytes. Responses exceeding this are aborted.
    pub max_response_size: usize,
    /// Maximum number of HTTP redirects to follow.
    pub max_redirects: u32,
}

#[cfg(feature = "network")]
impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            allowed_url_prefixes: Vec::new(),
            block_private_ips: true,
            allowed_methods: vec![
                "GET".into(),
                "HEAD".into(),
                "POST".into(),
                "PUT".into(),
                "DELETE".into(),
                "PATCH".into(),
            ],
            max_response_size: 10 * 1024 * 1024, // 10MB
            max_redirects: 20,
        }
    }
}

/// Configurable execution limits to prevent runaway scripts.
#[derive(Debug, Clone)]
pub struct ExecutionLimits {
    pub max_call_depth: u32,
    pub max_command_count: u32,
    pub max_loop_iterations: u32,
    pub max_output_size: usize,
    pub max_substitution_depth: u32,
    pub max_brace_expansion: u32,
    pub max_glob_operations: u32,
    pub max_string_length: usize,
    pub max_array_elements: usize,
    pub max_source_depth: u32,
    pub max_input_size: usize,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            max_call_depth: 100,
            max_command_count: 10_000,
            max_loop_iterations: 10_000,
            max_output_size: 10 * 1024 * 1024,
            max_substitution_depth: 50,
            max_brace_expansion: 10_000,
            max_glob_operations: 100_000,
            max_string_length: 10 * 1024 * 1024,
            max_array_elements: 100_000,
            max_source_depth: 100,
            max_input_size: 1_000_000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionLimits {
    pub max_total_commands: u64,
    pub max_exec_calls: u64,
}

impl Default for SessionLimits {
    fn default() -> Self {
        Self {
            max_total_commands: 100_000,
            max_exec_calls: 1_000,
        }
    }
}

/// A virtual Bash environment.
///
/// Create one with [`Shell::new`] for defaults or [`Shell::builder`] for
/// full control. The filesystem persists across [`exec`](Shell::exec) calls;
/// shell state (variables, functions, cwd) is isolated per call.
pub struct Shell {
    fs: Box<dyn VirtualFs>,
    default_env: HashMap<String, String>,
    cwd: String,
    limits: ExecutionLimits,
    registry: commands::CommandRegistry,
    #[cfg(feature = "network")]
    network_policy: Option<NetworkPolicy>,
    session_limits: Option<SessionLimits>,
    session_command_count: u64,
    session_exec_count: u64,
}

impl Shell {
    /// Create an instance with default settings and an empty in-memory filesystem.
    pub fn new() -> Self {
        Builder::new().build()
    }

    /// Start building a configured instance.
    pub fn builder() -> Builder {
        Builder::new()
    }

    /// Execute a bash command string.
    ///
    /// Shell state (variables, functions, working directory) is isolated
    /// per call - each invocation starts fresh. The filesystem is shared
    /// and persists across calls.
    ///
    /// # Errors
    ///
    /// Returns `Error::Parse` for syntax errors, `Error::LimitExceeded`
    /// if an execution limit is hit. Note that a non-zero exit code is **not**
    /// an error - it's reported in [`ExecResult::exit_code`].
    pub fn exec(&mut self, command: &str) -> Result<ExecResult, Error> {
        self.check_session_limits()?;
        if command.len() > self.limits.max_input_size {
            return Err(Error::LimitExceeded(crate::error::LimitKind::InputSize));
        }
        let script = parser::parse(command)?;
        let (result, cmd_count) = interpreter::execute(
            &script,
            &*self.fs,
            &self.default_env,
            &self.cwd,
            &self.limits,
            &self.registry,
            "",
            None,
            #[cfg(feature = "network")]
            self.network_policy.as_ref(),
        );
        self.update_session_counters(cmd_count);
        result
    }

    /// Execute with custom options (stdin, env overrides, cwd override).
    #[allow(clippy::needless_pass_by_value)]
    pub fn exec_with(&mut self, command: &str, options: ExecOptions<'_>) -> Result<ExecResult, Error> {
        self.check_session_limits()?;
        if command.len() > self.limits.max_input_size {
            return Err(Error::LimitExceeded(crate::error::LimitKind::InputSize));
        }
        let script = parser::parse(command)?;
        let env = match options.env {
            Some(override_env) => {
                let mut e = self.default_env.clone();
                e.extend(override_env.iter().map(|(k, v)| (k.clone(), v.clone())));
                e
            }
            None => self.default_env.clone(),
        };
        let cwd = options.cwd.unwrap_or(&self.cwd);
        let stdin = options.stdin.unwrap_or("");
        let (result, cmd_count) = interpreter::execute(
            &script,
            &*self.fs,
            &env,
            cwd,
            &self.limits,
            &self.registry,
            stdin,
            options.cancel,
            #[cfg(feature = "network")]
            self.network_policy.as_ref(),
        );
        self.update_session_counters(cmd_count);
        result
    }

    /// Register a custom command after construction.
    pub fn register_command(&mut self, name: impl Into<String>, func: CommandFn) {
        self.registry.register(name.into(), func);
    }

    /// Execute a command with a timeout. Spawns a timer thread that cancels
    /// execution after the given duration.
    pub fn exec_with_timeout(&mut self, command: &str, timeout: std::time::Duration) -> Result<ExecResult, Error> {
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(timeout);
            cancel_clone.store(true, std::sync::atomic::Ordering::Relaxed);
        });
        self.exec_with(command, ExecOptions {
            cancel: Some(cancel),
            ..Default::default()
        })
    }

    /// Direct access to the virtual filesystem.
    pub fn fs(&self) -> &dyn VirtualFs {
        &*self.fs
    }

    /// Read a file as a UTF-8 string.
    pub fn read_file(&self, path: &str) -> Result<String, Error> {
        self.fs.read_file_string(path).map_err(Error::Fs)
    }

    /// Write a UTF-8 string to a file (creates parent directories).
    pub fn write_file(&self, path: &str, content: &str) -> Result<(), Error> {
        self.fs.write_file(path, content.as_bytes()).map_err(Error::Fs)
    }

    /// Current working directory.
    pub fn cwd(&self) -> &str {
        &self.cwd
    }

    /// Current default environment variables.
    pub fn env(&self) -> &HashMap<String, String> {
        &self.default_env
    }

    fn check_session_limits(&self) -> Result<(), Error> {
        if let Some(ref sl) = self.session_limits {
            if self.session_exec_count >= sl.max_exec_calls {
                return Err(Error::LimitExceeded(crate::error::LimitKind::SessionExecCalls));
            }
            if self.session_command_count >= sl.max_total_commands {
                return Err(Error::LimitExceeded(crate::error::LimitKind::SessionCommands));
            }
        }
        Ok(())
    }

    fn update_session_counters(&mut self, commands_run: u32) {
        self.session_exec_count += 1;
        self.session_command_count += u64::from(commands_run);
    }
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Shell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shell")
            .field("cwd", &self.cwd)
            .field("env_count", &self.default_env.len())
            .finish_non_exhaustive()
    }
}

/// Builder for configuring an [`Shell`] instance.
pub struct Builder {
    fs: Option<Box<dyn VirtualFs>>,
    env: HashMap<String, String>,
    files: Vec<(String, String)>,
    cwd: String,
    limits: ExecutionLimits,
    custom_commands: Vec<(String, CommandFn)>,
    #[cfg(feature = "network")]
    network_policy: Option<NetworkPolicy>,
    session_limits: Option<SessionLimits>,
}

impl Builder {
    /// Create a new builder with default settings.
    pub fn new() -> Self {
        Self {
            fs: None,
            env: HashMap::new(),
            files: Vec::new(),
            cwd: String::from("/home/user"),
            limits: ExecutionLimits::default(),
            custom_commands: Vec::new(),
            #[cfg(feature = "network")]
            network_policy: None,
            session_limits: None,
        }
    }

    /// Use a custom filesystem implementation.
    #[must_use]
    pub fn fs(mut self, fs: impl VirtualFs + 'static) -> Self {
        self.fs = Some(Box::new(fs));
        self
    }

    /// Set an environment variable.
    #[must_use]
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set multiple environment variables.
    #[must_use]
    pub fn envs(mut self, vars: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>) -> Self {
        for (k, v) in vars {
            self.env.insert(k.into(), v.into());
        }
        self
    }

    /// Set the initial working directory.
    #[must_use]
    pub fn cwd(mut self, dir: impl Into<String>) -> Self {
        self.cwd = dir.into();
        self
    }

    /// Pre-populate a file in the virtual filesystem.
    #[must_use]
    pub fn file(mut self, path: impl Into<String>, content: impl Into<String>) -> Self {
        self.files.push((path.into(), content.into()));
        self
    }

    /// Set execution limits.
    #[must_use]
    pub fn limits(mut self, limits: ExecutionLimits) -> Self {
        self.limits = limits;
        self
    }

    #[must_use]
    pub fn session_limits(mut self, limits: SessionLimits) -> Self {
        self.session_limits = Some(limits);
        self
    }

    /// Register a custom command that will be available in the shell.
    #[must_use]
    pub fn command(mut self, name: impl Into<String>, func: CommandFn) -> Self {
        self.custom_commands.push((name.into(), func));
        self
    }

    /// Set a network policy for the `curl` command.
    ///
    /// When the `network` feature is enabled, all network requests are blocked
    /// by default unless a policy is explicitly configured via this method.
    #[cfg(feature = "network")]
    #[must_use]
    pub fn network_policy(mut self, policy: NetworkPolicy) -> Self {
        self.network_policy = Some(policy);
        self
    }

    /// Build the configured [`Shell`] instance.
    pub fn build(self) -> Shell {
        let fs: Box<dyn VirtualFs> = match self.fs {
            Some(fs) => fs,
            None => Box::new(InMemoryFs::new()),
        };

        let _ = fs.mkdir("/bin", true);
        let _ = fs.mkdir("/usr/bin", true);
        let _ = fs.mkdir("/tmp", true);
        let _ = fs.mkdir("/dev", true);
        let _ = fs.mkdir("/home/user", true);
        let _ = fs.mkdir("/proc", true);
        let _ = fs.mkdir("/proc/self", true);
        let _ = fs.write_file("/dev/null", b"");
        let _ = fs.write_file("/proc/version", b"Linux vbash 5.15.0 x86_64\n");
        let _ = fs.write_file("/proc/self/exe", b"/bin/bash\n");

        for (path, content) in &self.files {
            if let Some(parent_end) = path.rfind('/') {
                if parent_end > 0 {
                    let _ = fs.mkdir(&path[..parent_end], true);
                }
            }
            let _ = fs.write_file(path, content.as_bytes());
        }

        let mut env = self.env;
        env.entry("HOME".to_string())
            .or_insert_with(|| "/home/user".to_string());
        env.entry("PATH".to_string())
            .or_insert_with(|| "/usr/bin:/bin".to_string());
        env.entry("PWD".to_string())
            .or_insert_with(|| self.cwd.clone());
        env.entry("USER".to_string())
            .or_insert_with(|| "user".to_string());
        env.entry("HOSTNAME".to_string())
            .or_insert_with(|| "vbash".to_string());
        env.entry("SHELL".to_string())
            .or_insert_with(|| "/bin/bash".to_string());
        env.entry("TERM".to_string())
            .or_insert_with(|| "xterm-256color".to_string());
        env.entry("IFS".to_string())
            .or_insert_with(|| " \t\n".to_string());
        env.entry("OLDPWD".to_string())
            .or_insert_with(|| self.cwd.clone());
        env.entry("OSTYPE".to_string())
            .or_insert_with(|| "linux-gnu".to_string());
        env.entry("MACHTYPE".to_string())
            .or_insert_with(|| "x86_64-pc-linux-gnu".to_string());
        env.entry("HOSTTYPE".to_string())
            .or_insert_with(|| "x86_64".to_string());
        env.entry("BASH_VERSION".to_string())
            .or_insert_with(|| "5.2.0-vbash".to_string());

        let mut registry = commands::CommandRegistry::new();
        for (name, func) in self.custom_commands {
            registry.register(name, func);
        }

        Shell {
            fs,
            default_env: env,
            cwd: self.cwd,
            limits: self.limits,
            registry,
            #[cfg(feature = "network")]
            network_policy: self.network_policy,
            session_limits: self.session_limits,
            session_command_count: 0,
            session_exec_count: 0,
        }
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}
