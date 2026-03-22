---
id: '19'
title: 'imp: Rush shell backend for BashTool'
slug: imp-rush-shell-backend-for-bashtool
status: open
priority: 2
created_at: '2026-03-22T22:05:44.641297Z'
updated_at: '2026-03-22T22:05:44.641297Z'
labels:
- imp
- integration
- rush
verify: cd /Users/asher/tower && cargo test -p imp-core test_rush_backend 2>&1 | grep -E "[1-9][0-9]* passed"
fail_first: true
---

Add rush as an optional shell backend for imp's BashTool, replacing `sh -c` with rush's programmatic API or daemon protocol.

## Why

imp's BashTool currently spawns `Command::new("sh").arg("-c").arg(command)` for every tool call. An agent session runs hundreds of commands, mostly file operations (ls, grep, cat, find, git) that rush has as built-ins. Using rush eliminates fork/exec overhead for these common commands.

Two integration modes (implement whichever is ready first, preferring library):

### Mode A: Library (preferred)
If rush's `run()` API (rush unit 7.1) is available, add rush as a path/git dependency to imp-core and call it directly:
```rust
let result = rush::run(command, &rush::RunOptions {
    cwd: Some(ctx.cwd.clone()),
    timeout: Some(timeout_secs),
    json_output: false,
    max_output_bytes: Some(MAX_OUTPUT_BYTES),
    ..Default::default()
});
```

### Mode B: Daemon
Connect to rush daemon over Unix socket, send Execute messages, get ExecutionResult back.

### Configuration
Add to imp config:
```toml
[shell]
backend = "sh"  # default, "rush", or "rush-daemon"
```

### Fallback
If rush backend is configured but unavailable, fall back to `sh -c` with a warning.

## Files to modify
- `imp/crates/imp-core/src/tools/bash.rs` — add rush backend
- `imp/crates/imp-core/Cargo.toml` — add optional rush dependency

## Dependencies
- Requires rush unit 7.1 (rush::run API) for library mode
- Requires rush unit 7.3 (output budget) for native truncation
