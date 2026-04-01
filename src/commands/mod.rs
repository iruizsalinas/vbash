//! Command registry and dispatch.

#[cfg(feature = "network")]
pub mod network;
pub mod archive;
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::many_single_char_names,
    clippy::redundant_closure,
    clippy::unnested_or_patterns,
    clippy::match_same_arms,
    clippy::similar_names,
    clippy::unreadable_literal,
    clippy::items_after_statements,
    clippy::needless_pass_by_value,
)]
pub mod awk;
pub mod test_cmd;
pub mod context;
pub mod file_ops;
pub mod hash;
pub mod jq;
pub mod search;
pub mod sed;
pub mod text;
pub mod util;
pub mod yq;

pub use context::CommandContext;

use std::collections::HashMap;

use crate::error::Error;
use crate::ExecResult;

/// Signature for all command implementations.
pub type CommandFn = fn(args: &[&str], ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error>;

/// Maps command names to their implementations.
pub(crate) struct CommandRegistry {
    commands: HashMap<String, CommandFn>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        let mut reg = Self { commands: HashMap::new() };
        reg.register("echo", text::echo);
        reg.register("printf", text::printf);
        reg.register("true", util::cmd_true);
        reg.register("false", util::cmd_false);
        reg.register("pwd", util::pwd);
        reg.register("env", util::env);
        reg.register("printenv", util::printenv);
        reg.register("test", test_cmd::test_cmd);
        reg.register("[", test_cmd::test_cmd);
        reg.register("cat", text::cat);
        reg.register("wc", text::wc);
        reg.register("head", text::head);
        reg.register("tail", text::tail);
        reg.register("sort", text::sort_cmd);
        reg.register("grep", text::grep_cmd);
        reg.register("seq", util::seq_cmd);
        reg.register("basename", file_ops::basename_cmd);
        reg.register("dirname", file_ops::dirname_cmd);
        reg.register("mkdir", file_ops::mkdir_cmd);
        reg.register("rm", file_ops::rm_cmd);
        reg.register("touch", file_ops::touch_cmd);
        reg.register("cp", file_ops::cp_cmd);
        reg.register("mv", file_ops::mv_cmd);
        reg.register("ls", file_ops::ls);
        reg.register("stat", file_ops::stat_cmd);
        reg.register("chmod", file_ops::chmod_cmd);
        reg.register("ln", file_ops::ln_cmd);
        reg.register("readlink", file_ops::readlink_cmd);
        reg.register("rmdir", file_ops::rmdir_cmd);
        reg.register("tree", file_ops::tree_cmd);
        reg.register("file", file_ops::file_cmd);
        reg.register("split", file_ops::split_cmd);
        reg.register("cut", text::cut);
        reg.register("tr", text::tr);
        reg.register("uniq", text::uniq);
        reg.register("rev", text::rev);
        reg.register("tac", text::tac);
        reg.register("paste", text::paste);
        reg.register("fold", text::fold);
        reg.register("expand", text::expand);
        reg.register("column", text::column);
        reg.register("unexpand", text::unexpand);
        reg.register("comm", text::comm);
        reg.register("join", text::join);
        reg.register("nl", text::nl);
        reg.register("od", text::od);
        reg.register("base64", text::base64);
        reg.register("strings", text::strings);
        reg.register("diff", text::diff);
        reg.register("tee", text::tee);
        reg.register("find", search::find_cmd);
        reg.register("which", search::which_cmd);
        reg.register("egrep", search::egrep_cmd);
        reg.register("fgrep", search::fgrep_cmd);
        reg.register("date", util::date_cmd);
        reg.register("sleep", util::sleep_cmd);
        reg.register("yes", util::yes_cmd);
        reg.register("expr", util::expr_cmd);
        reg.register("bc", util::bc_cmd);
        reg.register("realpath", util::realpath_cmd);
        reg.register("mktemp", util::mktemp_cmd);
        reg.register("whoami", util::whoami_cmd);
        reg.register("hostname", util::hostname_cmd);
        reg.register("uname", util::uname_cmd);
        reg.register("du", util::du_cmd);
        reg.register("timeout", util::timeout_cmd);
        reg.register("nohup", util::nohup_cmd);
        reg.register("xargs", util::xargs_cmd);
        reg.register("sed", sed::sed_cmd);
        reg.register("tar", archive::tar);
        reg.register("gzip", archive::gzip_cmd);
        reg.register("gunzip", archive::gunzip);
        reg.register("zcat", archive::zcat);
        reg.register("yq", yq::yq);
        reg.register("awk", awk::awk_cmd);
        reg.register("jq", jq::jq_cmd);
        reg.register("md5sum", hash::md5sum_cmd);
        reg.register("sha1sum", hash::sha1sum_cmd);
        reg.register("sha256sum", hash::sha256sum_cmd);
        reg.register("sha512sum", hash::sha512sum_cmd);
        #[cfg(feature = "network")]
        reg.register("curl", network::curl_cmd);
        reg
    }

    pub fn register(&mut self, name: impl Into<String>, func: CommandFn) {
        self.commands.insert(name.into(), func);
    }

    pub fn lookup(&self, name: &str) -> Option<CommandFn> {
        self.commands.get(name).copied()
    }
}
