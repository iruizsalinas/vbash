use super::{ExecOutput, Interpreter};

impl Interpreter<'_> {
    pub(super) fn builtin_read(&mut self, args: &[&str], stdin: &str) -> ExecOutput {
        let mut raw = false;
        let mut array_name: Option<String> = None;
        let mut delimiter = '\n';
        let mut nchars: Option<usize> = None;
        let mut var_names = Vec::new();
        let mut i = 0;

        while i < args.len() {
            match args[i] {
                "-r" => raw = true,
                "-a" => {
                    i += 1;
                    if i < args.len() {
                        array_name = Some(args[i].to_string());
                    }
                }
                "-d" => {
                    i += 1;
                    if i < args.len() {
                        delimiter = args[i].chars().next().unwrap_or('\n');
                    }
                }
                "-p" => {
                    i += 1;
                }
                "-n" => {
                    i += 1;
                    if i < args.len() {
                        nchars = args[i].parse().ok();
                    }
                }
                arg if arg.starts_with('-') => {}
                arg => var_names.push(arg.to_string()),
            }
            i += 1;
        }

        // If the pipeline provides explicit stdin, use it.
        // Otherwise, fall back to the interpreter's global stdin (set by
        // compound-command redirections like `while read; do ...; done < file`).
        let use_global = stdin.is_empty() && !self.stdin.is_empty();
        let effective_stdin: String = if use_global {
            self.stdin.clone()
        } else {
            stdin.to_string()
        };

        if effective_stdin.is_empty() {
            return ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 1 };
        }

        let input: String = if let Some(n) = nchars {
            let end = effective_stdin.char_indices().nth(n).map_or(effective_stdin.len(), |(idx, _)| idx);
            effective_stdin[..end].to_string()
        } else {
            let delim_str = delimiter.to_string();
            effective_stdin.split(&*delim_str).next().unwrap_or("").to_string()
        };

        // Consume the read portion from the global stdin so the next
        // call to `read` gets the remaining lines.
        if use_global {
            let consumed = input.len();
            let rest = &effective_stdin[consumed..];
            // Skip the delimiter that follows the consumed input
            let rest = if rest.starts_with(delimiter) {
                &rest[delimiter.len_utf8()..]
            } else {
                rest
            };
            self.stdin = rest.to_string();
        }

        let input = if raw {
            input.clone()
        } else {
            input.replace("\\\\", "\x00")
                 .replace('\\', "")
                 .replace('\x00', "\\")
        };

        let input = input.trim_end_matches('\n');

        if let Some(ref arr_name) = array_name {
            let ifs = self.state.get_var("IFS").unwrap_or(" \t\n").to_string();
            let elements: Vec<String> = input
                .split(|c: char| ifs.contains(c))
                .filter(|s| !s.is_empty())
                .map(std::string::ToString::to_string)
                .collect();
            self.state.arrays.insert(arr_name.clone(), elements);
        } else if var_names.is_empty() {
            let _ = self.state.set_var("REPLY", input.to_string());
        } else {
            let ifs = self.state.get_var("IFS").unwrap_or(" \t\n").to_string();
            let parts: Vec<&str> = input
                .splitn(var_names.len(), |c: char| ifs.contains(c))
                .collect();
            for (j, name) in var_names.iter().enumerate() {
                let value = parts.get(j).unwrap_or(&"");
                let _ = self.state.set_var(name, value.to_string());
            }
        }

        ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 0 }
    }

    pub(super) fn builtin_getopts(&mut self, args: &[&str]) -> ExecOutput {
        if args.len() < 2 {
            return ExecOutput {
                stdout: String::new(),
                stderr: "getopts: usage: getopts optstring name [arg ...]\n".to_string(),
                exit_code: 2,
            };
        }
        let optstring = args[0];
        let var_name = args[1];

        let optind: usize = self.state.get_var("OPTIND")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);

        let owned_params = self.state.positional_params.clone();
        let parse_args: Vec<&str> = if args.len() > 2 {
            args[2..].to_vec()
        } else {
            owned_params.iter().map(String::as_str).collect()
        };

        if optind == 0 || optind > parse_args.len() {
            let _ = self.state.set_var(var_name, "?".to_string());
            return ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 1 };
        }

        let current = parse_args[optind - 1];
        if !current.starts_with('-') || current == "-" || current == "--" {
            let _ = self.state.set_var(var_name, "?".to_string());
            if current == "--" {
                let _ = self.state.set_var("OPTIND", (optind + 1).to_string());
            }
            return ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 1 };
        }

        let opt_char = current.chars().nth(1).unwrap_or('?');
        let silent = optstring.starts_with(':');

        if let Some(pos) = optstring.find(opt_char) {
            let needs_arg = optstring.as_bytes().get(pos + 1).copied() == Some(b':');
            if needs_arg {
                if current.len() > 2 {
                    let _ = self.state.set_var("OPTARG", current[2..].to_string());
                    let _ = self.state.set_var("OPTIND", (optind + 1).to_string());
                } else if optind < parse_args.len() {
                    let _ = self.state.set_var("OPTARG", parse_args[optind].to_string());
                    let _ = self.state.set_var("OPTIND", (optind + 2).to_string());
                } else {
                    if silent {
                        let _ = self.state.set_var(var_name, ":".to_string());
                        let _ = self.state.set_var("OPTARG", opt_char.to_string());
                    } else {
                        let _ = self.state.set_var(var_name, "?".to_string());
                        let _ = self.state.unset_var("OPTARG");
                    }
                    let _ = self.state.set_var("OPTIND", (optind + 1).to_string());
                    return ExecOutput {
                        stdout: String::new(),
                        stderr: if silent {
                            String::new()
                        } else {
                            format!("getopts: option requires an argument -- '{opt_char}'\n")
                        },
                        exit_code: 0,
                    };
                }
            } else {
                let _ = self.state.unset_var("OPTARG");
                let _ = self.state.set_var("OPTIND", (optind + 1).to_string());
            }
            let _ = self.state.set_var(var_name, opt_char.to_string());
            ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 0 }
        } else {
            let _ = self.state.set_var("OPTIND", (optind + 1).to_string());
            if silent {
                let _ = self.state.set_var(var_name, "?".to_string());
                let _ = self.state.set_var("OPTARG", opt_char.to_string());
                ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 0 }
            } else {
                let _ = self.state.set_var(var_name, "?".to_string());
                let _ = self.state.unset_var("OPTARG");
                ExecOutput {
                    stdout: String::new(),
                    stderr: format!("getopts: illegal option -- '{opt_char}'\n"),
                    exit_code: 0,
                }
            }
        }
    }

    pub(super) fn builtin_mapfile(&mut self, args: &[&str], stdin: &str) -> ExecOutput {
        let mut strip_newline = false;
        let mut max_count: Option<usize> = None;
        let mut origin = 0usize;
        let mut skip = 0usize;
        let mut array_name = "MAPFILE".to_string();

        let mut i = 0;
        while i < args.len() {
            match args[i] {
                "-t" => strip_newline = true,
                "-n" if i + 1 < args.len() => {
                    i += 1;
                    max_count = args[i].parse().ok();
                }
                "-O" if i + 1 < args.len() => {
                    i += 1;
                    origin = args[i].parse().unwrap_or(0);
                }
                "-s" if i + 1 < args.len() => {
                    i += 1;
                    skip = args[i].parse().unwrap_or(0);
                }
                arg if !arg.starts_with('-') => {
                    array_name = arg.to_string();
                }
                _ => {}
            }
            i += 1;
        }

        let lines: Vec<&str> = stdin.lines().collect();
        let selected: Vec<String> = lines
            .iter()
            .skip(skip)
            .take(max_count.unwrap_or(usize::MAX))
            .map(|line| {
                if strip_newline {
                    (*line).to_string()
                } else {
                    format!("{line}\n")
                }
            })
            .collect();

        let arr = self.state.arrays.entry(array_name).or_default();
        while arr.len() < origin {
            arr.push(String::new());
        }
        for (j, val) in selected.into_iter().enumerate() {
            let idx = origin + j;
            if idx < arr.len() {
                arr[idx] = val;
            } else {
                arr.push(val);
            }
        }

        ExecOutput { stdout: String::new(), stderr: String::new(), exit_code: 0 }
    }
}
