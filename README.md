# vbash

A virtual bash environment for AI agents. Runs bash scripts in-process with
an in-memory filesystem. No real shell, no real files unless you opt in.

Inspired by [just-bash](https://github.com/vercel-labs/just-bash) by Vercel Labs.

```rust
use vbash::Shell;

let mut shell = Shell::builder()
    .file("/data/names.txt", "alice\nbob\ncharlie")
    .build();

let result = shell.exec("cat /data/names.txt | sort | head -n 2").unwrap();
assert_eq!(result.stdout, "alice\nbob\n");
```

## What's included

Bash syntax: variables, arrays, pipes, redirections, loops, conditionals,
functions, subshells, arithmetic, globs, heredocs, brace expansion, and more.

115+ built-in commands including full `sed`, `awk`, and `jq` interpreters,
plus `grep`, `sort`, `find`, `tar`, `curl` (behind feature flag), and the
usual coreutils.

## What's not included

- `select` and `coproc` statements
- Background jobs run synchronously
- Some `jq` edge cases may differ from the C implementation

## Custom commands

```rust
use vbash::{Shell, CommandContext, ExecResult, Error};
use std::collections::HashMap;

fn greet(args: &[&str], _ctx: &mut CommandContext<'_>) -> Result<ExecResult, Error> {
    let name = args.first().copied().unwrap_or("world");
    Ok(ExecResult {
        stdout: format!("hello {name}\n"),
        stderr: String::new(),
        exit_code: 0,
        env: HashMap::new(),
    })
}

let mut shell = Shell::builder().command("greet", greet).build();
shell.exec("greet alice").unwrap();
```

You can also call `shell.register_command("name", func)` after building.

## Filesystem backends

By default everything lives in memory. You can also read from a real directory
(writes still stay in memory), or go full read-write on the host filesystem.

```rust
use vbash::{Shell, OverlayFs};

let shell = Shell::builder()
    .fs(OverlayFs::new("/path/to/project").unwrap())
    .build();
```

| Backend | Reads from | Writes to | Typical use |
|---------|-----------|-----------|-------------|
| `InMemoryFs` | Memory | Memory | Testing, sandboxing |
| `OverlayFs` | Disk, then memory | Memory | Read real files safely |
| `ReadWriteFs` | Disk | Disk | Full host access |
| `MountableFs` | Routed | Routed | Mix backends per path |

## Limits

Scripts run inside configurable limits. If any limit is exceeded the call
returns an error.

```rust
use vbash::{Shell, ExecutionLimits};

let mut shell = Shell::builder()
    .limits(ExecutionLimits {
        max_loop_iterations: 1000,
        max_command_count: 5000,
        ..ExecutionLimits::default()
    })
    .build();
```

## Cancellation

Cancel a running script from another thread, or set a timeout:

```rust
use vbash::Shell;
use std::time::Duration;

let mut shell = Shell::new();
let r = shell.exec_with_timeout("sleep 100", Duration::from_secs(1));
assert!(r.is_err());
```

## Network access

Disabled by default. Enable with the `network` feature flag and configure
which URLs are allowed:

```rust
use vbash::{Shell, NetworkPolicy};

let mut shell = Shell::builder()
    .network_policy(NetworkPolicy {
        allowed_url_prefixes: vec!["https://api.example.com/".into()],
        ..NetworkPolicy::default()
    })
    .build();
```

Private IPs are blocked by default. Redirect targets are re-validated.
