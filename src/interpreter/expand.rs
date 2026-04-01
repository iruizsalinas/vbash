use crate::ast::word::{BraceExpansion, ParamOp, WordPart};
use crate::ast::Word;
use crate::error::{ExecError, Error, ShellSignal};

use super::pattern::{
    expand_ansi_c, glob_match, glob_part_to_string, ifs_split,
    modify_case, remove_pattern, replace_pattern,
};
use super::{contains_glob_chars, Interpreter};

impl Interpreter<'_> {
    pub(super) fn expand_word(&mut self, word: &Word) -> Result<String, ShellSignal> {
        let mut result = String::new();
        for part in &word.parts {
            result.push_str(&self.expand_word_part(part)?);
        }
        if result.len() > self.limits.max_string_length {
            return Err(crate::error::LimitKind::StringLength.into());
        }
        Ok(result)
    }

    pub(super) fn expand_word_part(&mut self, part: &WordPart) -> Result<String, ShellSignal> {
        match part {
            WordPart::Literal(s) | WordPart::SingleQuoted(s) => Ok(s.clone()),
            WordPart::DoubleQuoted(parts) => {
                let mut result = String::new();
                for p in parts {
                    result.push_str(&self.expand_word_part(p)?);
                }
                Ok(result)
            }
            WordPart::Escaped(c) => Ok(String::from(*c)),
            WordPart::AnsiCQuoted(s) => Ok(expand_ansi_c(s)),
            WordPart::Parameter(p) => self.expand_parameter(p),
            WordPart::CommandSubstitution(cmd) => {
                let result = self.expand_command_subst(cmd)?;
                if result.len() > self.limits.max_string_length {
                    return Err(crate::error::LimitKind::StringLength.into());
                }
                Ok(result)
            }
            WordPart::ArithmeticExpansion(expr) => {
                let val = self.evaluate_arith(expr)?;
                let result = val.to_string();
                if result.len() > self.limits.max_string_length {
                    return Err(crate::error::LimitKind::StringLength.into());
                }
                Ok(result)
            }
            WordPart::TildeExpansion(user) => {
                match user.as_str() {
                    "" => Ok(self.state.get_var("HOME").unwrap_or("/").to_string()),
                    "+" => Ok(self.state.cwd.clone()),
                    "-" => Ok(self.state.previous_dir.clone()),
                    _ => Ok(format!("~{user}")),
                }
            }
            WordPart::Glob(_) => {
                // Glob expansion happens at a higher level during word splitting
                Ok(glob_part_to_string(part))
            }
            WordPart::BraceExpansion(be) => {
                let fields = self.expand_brace(be)?;
                Ok(fields.join(" "))
            }
            WordPart::ProcessSubstitution { command, direction } => {
                use crate::ast::word::ProcessDirection;
                match direction {
                    ProcessDirection::In => {
                        let script = crate::parser::parse(command)
                            .map_err(|e| ShellSignal::Error(Error::Parse(e)))?;
                        let out = self.execute_script(&script)?;
                        let temp_path = format!("/dev/fd/{}", self.state.lineno + 63);
                        let _ = self.fs.mkdir("/dev/fd", true);
                        let _ = self.fs.write_file(&temp_path, out.stdout.as_bytes());
                        Ok(temp_path)
                    }
                    ProcessDirection::Out => {
                        let _ = self.fs.mkdir("/dev/fd", true);
                        Ok(format!("/dev/fd/{}", self.state.lineno + 63))
                    }
                }
            }
        }
    }

    pub(super) fn expand_word_splitting(&mut self, word: &Word) -> Result<Vec<String>, ShellSignal> {
        if is_quoted_at(word) {
            return Ok(self.state.positional_params.clone());
        }

        if word_has_brace_expansion(word) {
            return self.expand_word_to_fields(word);
        }

        let expanded = self.expand_word(word)?;
        if expanded.is_empty() {
            if word_is_fully_quoted(word) {
                return Ok(vec![String::new()]);
            }
            return Ok(vec![]);
        }

        let needs_split = word_has_unquoted_expansion(word);

        let fields = if needs_split {
            ifs_split(&expanded, self.state.get_var("IFS"))
        } else {
            vec![expanded]
        };

        if self.state.options.noglob || word_is_fully_quoted(word) {
            return Ok(fields);
        }

        let mut result = Vec::new();
        for field in fields {
            if contains_glob_chars(&field) {
                let matches = self.expand_glob(&field);
                if !matches.is_empty() {
                    result.extend(matches);
                } else if self.state.shopt.nullglob {
                    // skip: nullglob means unmatched patterns produce nothing
                } else if self.state.shopt.failglob {
                    return Err(ExecError::Other(format!("no match: {field}")).into());
                } else {
                    result.push(field);
                }
            } else {
                result.push(field);
            }
        }
        Ok(result)
    }

    fn expand_word_to_fields(&mut self, word: &Word) -> Result<Vec<String>, ShellSignal> {
        let mut proto_words: Vec<Vec<WordPart>> = vec![vec![]];

        for part in &word.parts {
            if let WordPart::BraceExpansion(be) = part {
                let alternatives = self.expand_brace(be)?;
                let mut new_protos = Vec::new();
                for existing in &proto_words {
                    for alt in &alternatives {
                        let mut pw = existing.clone();
                        pw.push(WordPart::Literal(alt.clone()));
                        new_protos.push(pw);
                    }
                }
                proto_words = new_protos;
            } else {
                for pw in &mut proto_words {
                    pw.push(part.clone());
                }
            }
        }

        let mut result = Vec::new();
        for parts in proto_words {
            let w = Word { parts };
            let expanded = self.expand_word(&w)?;
            if expanded.is_empty() {
                continue;
            }
            if contains_glob_chars(&expanded) && !self.state.options.noglob {
                let matches = self.expand_glob(&expanded);
                if !matches.is_empty() {
                    result.extend(matches);
                } else if self.state.shopt.nullglob {
                    // skip: nullglob means unmatched patterns produce nothing
                } else if self.state.shopt.failglob {
                    return Err(ExecError::Other(format!("no match: {expanded}")).into());
                } else {
                    result.push(expanded);
                }
            } else {
                result.push(expanded);
            }
        }
        Ok(result)
    }

    fn expand_brace(&mut self, be: &BraceExpansion) -> Result<Vec<String>, ShellSignal> {
        match be {
            BraceExpansion::List(words) => {
                let mut results = Vec::new();
                for w in words {
                    results.push(self.expand_word(w)?);
                }
                Ok(results)
            }
            BraceExpansion::Range { start, end, step } => {
                let step_val: i64 = step.as_deref().and_then(|s| s.parse().ok()).unwrap_or(1);
                let step_val = if step_val == 0 { 1 } else { step_val.abs() };

                if let (Ok(s), Ok(e)) = (start.parse::<i64>(), end.parse::<i64>()) {
                    let mut results = Vec::new();
                    if s <= e {
                        let mut i = s;
                        while i <= e {
                            if results.len() >= self.limits.max_brace_expansion as usize {
                                return Err(crate::error::LimitKind::BraceExpansion.into());
                            }
                            results.push(i.to_string());
                            i += step_val;
                        }
                    } else {
                        let mut i = s;
                        while i >= e {
                            if results.len() >= self.limits.max_brace_expansion as usize {
                                return Err(crate::error::LimitKind::BraceExpansion.into());
                            }
                            results.push(i.to_string());
                            i -= step_val;
                        }
                    }
                    Ok(results)
                } else if start.len() == 1 && end.len() == 1 {
                    let s = start.as_bytes()[0];
                    let e = end.as_bytes()[0];
                    let step_u8 = u8::try_from(step_val).unwrap_or(1);
                    let mut results = Vec::new();
                    if s <= e {
                        let mut c = s;
                        while c <= e {
                            if results.len() >= self.limits.max_brace_expansion as usize {
                                return Err(crate::error::LimitKind::BraceExpansion.into());
                            }
                            results.push(String::from(c as char));
                            match c.checked_add(step_u8) {
                                Some(next) => c = next,
                                None => break,
                            }
                        }
                    } else {
                        let mut c = s;
                        while c >= e {
                            if results.len() >= self.limits.max_brace_expansion as usize {
                                return Err(crate::error::LimitKind::BraceExpansion.into());
                            }
                            results.push(String::from(c as char));
                            match c.checked_sub(step_u8) {
                                Some(next) => c = next,
                                None => break,
                            }
                        }
                    }
                    Ok(results)
                } else {
                    Ok(vec![format!("{{{start}..{end}}}")])
                }
            }
        }
    }

    pub(super) fn expand_parameter(&mut self, param: &crate::ast::word::ParameterExpansion) -> Result<String, ShellSignal> {
        if let Some(ref op) = param.operation {
            if let ParamOp::Length = op {
                if let Some(ref sub) = param.subscript {
                    let sub_str = self.expand_word(sub)?;
                    if sub_str == "@" || sub_str == "*" {
                        return Ok(self.state.array_len(&param.name).to_string());
                    }
                }
            }
            if let ParamOp::ArrayKeys { .. } = op {
                let len = self.state.array_len(&param.name);
                let indices: Vec<String> = (0..len).map(|i| i.to_string()).collect();
                return Ok(indices.join(" "));
            }
        }

        if let Some(ref sub) = param.subscript {
            let sub_str = self.expand_word(sub)?;
            let arr_value = if sub_str == "@" || sub_str == "*" {
                self.state
                    .get_array(&param.name)
                    .map(|arr| arr.join(" "))
            } else if let Some(assoc) = self.state.assoc_arrays.get(&param.name) {
                assoc.get(&sub_str).cloned()
            } else if let Ok(idx) = sub_str.parse::<usize>() {
                self.state
                    .get_array_element(&param.name, idx)
                    .map(String::from)
            } else {
                None
            };

            let value = arr_value.unwrap_or_default();

            if let Some(ref op) = param.operation {
                return self.apply_param_op(&param.name, &value, op);
            }

            return Ok(value);
        }

        let raw_value = match param.name.as_str() {
            "?" => Some(self.state.last_exit_code.to_string()),
            "$" => Some(self.state.pid.to_string()),
            "#" => Some(self.state.positional_params.len().to_string()),
            "*" => {
                let ifs_char = self.state.get_var("IFS")
                    .map_or(' ', |s| s.chars().next().unwrap_or(' '));
                Some(self.state.positional_params.join(&ifs_char.to_string()))
            }
            "@" => Some(self.state.positional_params.join(" ")),
            "!" => Some("0".to_string()),
            "-" => Some(self.state.options_string()),
            "_" => Some(self.state.last_arg.clone()),
            "RANDOM" => {
                self.state.random_seed = self.state.random_seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                Some(((self.state.random_seed >> 16) & 0x7FFF).to_string())
            }
            "SECONDS" => Some(self.state.start_time.elapsed().as_secs().to_string()),
            "LINENO" => Some(self.state.lineno.to_string()),
            "BASH_SUBSHELL" => Some(self.state.subshell_depth.to_string()),
            "FUNCNAME" => Some(self.state.func_name_stack.last().cloned().unwrap_or_default()),
            "BASH_SOURCE" => Some(self.state.source_file.clone()),
            "0" => Some("vbash".to_string()),
            n if n.len() == 1 && n.as_bytes()[0].is_ascii_digit() => {
                let idx = (n.as_bytes()[0] - b'0') as usize;
                if idx == 0 {
                    Some("vbash".to_string())
                } else {
                    Some(
                        self.state
                            .positional_params
                            .get(idx - 1)
                            .cloned()
                            .unwrap_or_default(),
                    )
                }
            }
            name => {
                if let Some(arr) = self.state.get_array(name) {
                    arr.first().cloned()
                } else {
                    self.state.get_var(name).map(String::from)
                }
            }
        };

        let value = raw_value.unwrap_or_default();

        if let Some(ref op) = param.operation {
            return self.apply_param_op(&param.name, &value, op);
        }

        if value.is_empty()
            && self.state.options.nounset
            && !matches!(param.name.as_str(), "?" | "$" | "#" | "@" | "*" | "!" | "-" | "_")
            && self.state.get_var(&param.name).is_none()
        {
            return Err(ExecError::UnboundVariable(param.name.clone()).into());
        }

        Ok(value)
    }

    pub(super) fn apply_param_op(&mut self, name: &str, value: &str, op: &ParamOp) -> Result<String, ShellSignal> {
        match op {
            ParamOp::Default { word, colon } => {
                let empty = if *colon { value.is_empty() } else { self.state.get_var(name).is_none() };
                if empty {
                    self.expand_word(word)
                } else {
                    Ok(value.to_string())
                }
            }
            ParamOp::AssignDefault { word, colon } => {
                let empty = if *colon { value.is_empty() } else { self.state.get_var(name).is_none() };
                if empty {
                    let default = self.expand_word(word)?;
                    let _ = self.state.set_var(name, default.clone());
                    Ok(default)
                } else {
                    Ok(value.to_string())
                }
            }
            ParamOp::Error { word, colon } => {
                let empty = if *colon { value.is_empty() } else { self.state.get_var(name).is_none() };
                if empty {
                    let msg = self.expand_word(word)?;
                    Err(ExecError::Other(format!("{name}: {msg}")).into())
                } else {
                    Ok(value.to_string())
                }
            }
            ParamOp::Alternative { word, colon } => {
                let empty = if *colon { value.is_empty() } else { self.state.get_var(name).is_none() };
                if empty {
                    Ok(String::new())
                } else {
                    self.expand_word(word)
                }
            }
            ParamOp::Length => Ok(value.len().to_string()),
            ParamOp::Substring { offset, length } => {
                let off: i64 = self.expand_word(offset)?.trim().parse().unwrap_or(0);
                let vlen = value.len();
                let abs_off = usize::try_from(off.unsigned_abs()).unwrap_or(usize::MAX);
                let start = if off < 0 {
                    vlen.saturating_sub(abs_off)
                } else {
                    abs_off.min(vlen)
                };
                if let Some(len_word) = length {
                    let len: usize = self.expand_word(len_word)?.parse().unwrap_or(0);
                    Ok(value.chars().skip(start).take(len).collect())
                } else {
                    Ok(value.chars().skip(start).collect())
                }
            }
            ParamOp::PatternRemoval { pattern, side, greedy } => {
                let pat = self.expand_word(pattern)?;
                Ok(remove_pattern(value, &pat, *side, *greedy))
            }
            ParamOp::PatternReplace { pattern, replacement, all, anchor } => {
                let pat = self.expand_word(pattern)?;
                let repl = if let Some(r) = replacement {
                    self.expand_word(r)?
                } else {
                    String::new()
                };
                Ok(replace_pattern(value, &pat, &repl, *all, *anchor))
            }
            ParamOp::CaseModify { direction, all } => {
                Ok(modify_case(value, *direction, *all))
            }
            ParamOp::Indirection => {
                let indirect_name = value;
                Ok(self.state.get_var(indirect_name).unwrap_or("").to_string())
            }
            _ => Ok(value.to_string()),
        }
    }

    pub(super) fn expand_command_subst(&mut self, cmd: &str) -> Result<String, ShellSignal> {
        self.state.substitution_depth += 1;
        if self.state.substitution_depth > self.limits.max_substitution_depth {
            self.state.substitution_depth -= 1;
            return Err(crate::error::LimitKind::SubstitutionDepth.into());
        }
        let script = crate::parser::parse(cmd)
            .map_err(|e| ShellSignal::Error(Error::Parse(e)))?;
        let out = self.execute_script(&script);
        self.state.substitution_depth -= 1;
        let out = out?;
        Ok(out.stdout.trim_end_matches('\n').to_string())
    }

    pub(super) fn expand_glob(&self, pattern: &str) -> Vec<String> {
        let extglob = self.state.shopt.extglob;
        let nocaseglob = self.state.shopt.nocaseglob;
        let max_results = self.limits.max_glob_operations as usize;

        if self.state.shopt.globstar && pattern.contains("**") {
            return self.expand_globstar(pattern, extglob);
        }

        let (dir, file_pat) = if let Some(pos) = pattern.rfind('/') {
            let dir = if pos == 0 { "/" } else { &pattern[..pos] };
            (dir.to_string(), &pattern[pos + 1..])
        } else {
            (self.state.cwd.clone(), pattern)
        };

        let Ok(entries) = self.fs.readdir(&dir) else {
            return vec![];
        };

        let mut matches: Vec<String> = entries
            .iter()
            .filter(|e| {
                if !self.state.shopt.dotglob && e.name.starts_with('.') && !file_pat.starts_with('.') {
                    return false;
                }
                glob_match(file_pat, &e.name, extglob, nocaseglob)
            })
            .take(max_results)
            .map(|e| {
                if dir == self.state.cwd {
                    e.name.clone()
                } else if dir == "/" {
                    format!("/{}", e.name)
                } else {
                    format!("{}/{}", dir, e.name)
                }
            })
            .collect();

        matches.sort_unstable();
        matches
    }

    fn expand_globstar(&self, pattern: &str, extglob: bool) -> Vec<String> {
        let max_results = self.limits.max_glob_operations as usize;
        let (dir, rest_pat) = if let Some(pos) = pattern.find("**") {
            let dir_part = &pattern[..pos];
            let after = &pattern[pos + 2..];
            let after = after.strip_prefix('/').unwrap_or(after);
            let dir = if dir_part.is_empty() {
                self.state.cwd.clone()
            } else if dir_part == "/" {
                "/".to_string()
            } else {
                dir_part.trim_end_matches('/').to_string()
            };
            (dir, after.to_string())
        } else {
            return vec![];
        };

        let mut all_paths = Vec::new();
        self.walk_dir_recursive(&dir, &mut all_paths);

        let mut matches: Vec<String> = all_paths
            .into_iter()
            .filter(|path| {
                let name = path.rsplit('/').next().unwrap_or(path);
                if !self.state.shopt.dotglob && name.starts_with('.') {
                    return false;
                }
                if rest_pat.is_empty() {
                    return true;
                }
                let rel = if dir == "/" {
                    path.strip_prefix('/').unwrap_or(path)
                } else {
                    path.strip_prefix(&dir)
                        .and_then(|p| p.strip_prefix('/'))
                        .unwrap_or(path)
                };
                let nocaseglob = self.state.shopt.nocaseglob;
                glob_match(&rest_pat, rel, extglob, nocaseglob)
                    || glob_match(&rest_pat, name, extglob, nocaseglob)
            })
            .take(max_results)
            .collect();

        matches.sort_unstable();
        matches
    }

    fn walk_dir_recursive(&self, dir: &str, results: &mut Vec<String>) {
        if results.len() > self.limits.max_glob_operations as usize {
            return;
        }
        let Ok(entries) = self.fs.readdir(dir) else {
            return;
        };
        for entry in &entries {
            if results.len() > self.limits.max_glob_operations as usize {
                return;
            }
            let path = if dir == "/" {
                format!("/{}", entry.name)
            } else {
                format!("{}/{}", dir, entry.name)
            };
            results.push(path.clone());
            if entry.file_type == crate::fs::FileType::Directory {
                self.walk_dir_recursive(&path, results);
            }
        }
    }
}

fn is_quoted_at(word: &Word) -> bool {
    if word.parts.len() != 1 {
        return false;
    }
    if let WordPart::DoubleQuoted(inner) = &word.parts[0] {
        if inner.len() != 1 {
            return false;
        }
        if let WordPart::Parameter(p) = &inner[0] {
            return p.name == "@" && p.operation.is_none();
        }
    }
    false
}

fn word_has_unquoted_expansion(word: &Word) -> bool {
    for part in &word.parts {
        match part {
            WordPart::Parameter(_) | WordPart::CommandSubstitution(_) => return true,
            _ => {}
        }
    }
    false
}

fn word_has_brace_expansion(word: &Word) -> bool {
    word.parts.iter().any(|p| matches!(p, WordPart::BraceExpansion(_)))
}

fn word_is_fully_quoted(word: &Word) -> bool {
    word.parts.iter().all(|p| matches!(p, WordPart::DoubleQuoted(_) | WordPart::SingleQuoted(_)))
}

