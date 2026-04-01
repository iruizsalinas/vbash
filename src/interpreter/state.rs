//! Shell state: environment variables, functions, options, and scoping.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::ast::FunctionDef;

/// All mutable state for a running shell instance.
#[derive(Debug, Clone)]
pub(crate) struct ShellState {
    /// Environment variables.
    pub env: HashMap<String, String>,
    /// Variables marked for export.
    pub exported: HashSet<String>,
    /// Read-only variables.
    pub readonly: HashSet<String>,

    /// User-defined functions.
    pub functions: HashMap<String, FunctionDef>,

    /// Stack of local variable scopes (innermost last).
    /// Each scope maps variable names to their saved outer value
    /// (`None` means the variable didn't exist before entering scope).
    pub local_scopes: Vec<HashMap<String, Option<String>>>,

    /// Shell options set via `set -o`.
    pub options: ShellOptions,
    /// Shell options set via `shopt -s`.
    pub shopt: ShoptOptions,

    /// Positional parameters ($1, $2, ...).
    pub positional_params: Vec<String>,

    /// Last command's exit code (`$?`).
    pub last_exit_code: i32,
    /// Last argument of previous command (`$_`).
    pub last_arg: String,

    /// Current function/source call depth.
    pub call_depth: u32,
    /// Total commands executed (for limit tracking).
    pub command_count: u32,
    /// Current loop nesting depth.
    pub loop_depth: u32,

    /// Current working directory.
    pub cwd: String,
    /// Previous working directory (for `cd -`).
    pub previous_dir: String,
    /// Directory stack for pushd/popd.
    pub dir_stack: Vec<String>,

    /// Indexed arrays.
    pub arrays: HashMap<String, Vec<String>>,
    /// Associative arrays.
    pub assoc_arrays: HashMap<String, HashMap<String, String>>,

    /// Alias definitions.
    pub aliases: HashMap<String, String>,

    /// Trap handlers keyed by signal name.
    pub traps: HashMap<String, String>,

    /// Virtual process ID.
    pub pid: u32,

    /// Whether we're inside a condition (suppresses errexit).
    pub in_condition: bool,

    pub start_time: Instant,
    pub lineno: u32,
    pub substitution_depth: u32,
    pub source_depth: u32,
    pub subshell_depth: u32,
    pub func_name_stack: Vec<String>,
    pub random_seed: u32,
    pub source_file: String,
}

impl ShellState {
    pub fn new(cwd: String) -> Self {
        Self {
            env: HashMap::new(),
            exported: HashSet::new(),
            readonly: HashSet::new(),
            functions: HashMap::new(),
            local_scopes: Vec::new(),
            options: ShellOptions::default(),
            shopt: ShoptOptions::default(),
            positional_params: Vec::new(),
            last_exit_code: 0,
            last_arg: String::new(),
            call_depth: 0,
            command_count: 0,
            loop_depth: 0,
            cwd: cwd.clone(),
            previous_dir: cwd,
            dir_stack: Vec::new(),
            arrays: HashMap::new(),
            assoc_arrays: HashMap::new(),
            aliases: HashMap::new(),
            traps: HashMap::new(),
            pid: 1,
            in_condition: false,
            start_time: Instant::now(),
            lineno: 0,
            substitution_depth: 0,
            source_depth: 0,
            subshell_depth: 0,
            func_name_stack: Vec::new(),
            random_seed: 1,
            source_file: String::new(),
        }
    }

    /// Look up a variable, searching local scopes first (dynamic scoping).
    pub fn get_var(&self, name: &str) -> Option<&str> {
        for scope in self.local_scopes.iter().rev() {
            if scope.get(name).is_some() {
                // The variable exists in this scope - but it might be
                // shadowed by the env value (locals modify env directly).
                // If it's in the scope map at all, it was declared local here,
                // so the current env value is the local value.
                return self.env.get(name).map(String::as_str);
            }
        }
        self.env.get(name).map(String::as_str)
    }

    /// Set a variable, respecting readonly.
    pub fn set_var(&mut self, name: &str, value: String) -> Result<(), String> {
        if self.readonly.contains(name) {
            return Err(format!("{name}: readonly variable"));
        }
        self.env.insert(name.to_string(), value);
        Ok(())
    }

    /// Unset a variable.
    pub fn unset_var(&mut self, name: &str) -> Result<(), String> {
        if self.readonly.contains(name) {
            return Err(format!("{name}: readonly variable"));
        }
        self.env.remove(name);
        self.exported.remove(name);
        Ok(())
    }

    pub fn get_array(&self, name: &str) -> Option<&Vec<String>> {
        self.arrays.get(name)
    }

    pub fn set_array(&mut self, name: &str, values: Vec<String>) {
        self.arrays.insert(name.to_string(), values);
    }

    pub fn get_array_element(&self, name: &str, index: usize) -> Option<&str> {
        self.arrays
            .get(name)
            .and_then(|arr| arr.get(index))
            .map(String::as_str)
    }

    pub fn set_array_element(&mut self, name: &str, index: usize, value: String, max_elements: usize) -> Result<(), String> {
        if index >= max_elements {
            return Err(format!("{name}[{index}]: array index exceeds maximum ({max_elements})"));
        }
        let arr = self.arrays.entry(name.to_string()).or_default();
        if index >= arr.len() {
            arr.resize(index + 1, String::new());
        }
        arr[index] = value;
        Ok(())
    }

    pub fn array_len(&self, name: &str) -> usize {
        self.arrays.get(name).map_or(0, Vec::len)
    }

    pub fn array_push(&mut self, name: &str, value: String) {
        self.arrays.entry(name.to_string()).or_default().push(value);
    }

    /// Build the `$-` string representing current shell options.
    pub fn options_string(&self) -> String {
        let mut s = String::new();
        if self.options.errexit { s.push('e'); }
        if self.options.nounset { s.push('u'); }
        if self.options.xtrace { s.push('x'); }
        if self.options.verbose { s.push('v'); }
        if self.options.noglob { s.push('f'); }
        if self.options.noclobber { s.push('C'); }
        if self.options.allexport { s.push('a'); }
        s
    }
}

/// Options controlled by `set -o` and `set +o`.
///
/// Each field maps to a POSIX/bash shell option flag. Bools are the natural
/// representation here since each option is independently on or off.
#[derive(Debug, Clone, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct ShellOptions {
    /// `-e` - exit immediately on error.
    pub errexit: bool,
    /// `-u` - treat unset variables as error.
    pub nounset: bool,
    /// `-o pipefail` - pipeline exit code from last failing command.
    pub pipefail: bool,
    /// `-x` - print commands before executing.
    pub xtrace: bool,
    /// `-v` - print input lines as they are read.
    pub verbose: bool,
    /// `-C` - prevent `>` from overwriting files.
    pub noclobber: bool,
    /// `-f` - disable pathname expansion.
    pub noglob: bool,
    /// `-a` - automatically export all variables.
    pub allexport: bool,
    /// `-n` - read commands but don't execute (syntax check only).
    pub noexec: bool,
}

/// Options controlled by `shopt -s` and `shopt -u`.
#[derive(Debug, Clone, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct ShoptOptions {
    /// Extended glob patterns `@(...)`, `*(...)`, `+(...)`, `?(...)`, `!(...)`.
    pub extglob: bool,
    /// `**` matches recursively across directories.
    pub globstar: bool,
    /// Non-matching globs expand to nothing instead of the literal pattern.
    pub nullglob: bool,
    /// Non-matching globs produce an error.
    pub failglob: bool,
    /// Globs match hidden files (starting with `.`).
    pub dotglob: bool,
    /// Case-insensitive globbing.
    pub nocaseglob: bool,
    /// Case-insensitive pattern matching in `case` and `[[`.
    pub nocasematch: bool,
    /// Enable alias expansion.
    pub expand_aliases: bool,
    /// `echo` interprets escape sequences by default.
    pub xpg_echo: bool,
}

