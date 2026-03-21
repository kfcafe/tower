---
id: '6'
title: Implement shell tool loader
slug: implement-shell-tool-loader
status: closed
priority: 1
created_at: '2026-03-20T17:40:54.464492Z'
updated_at: '2026-03-21T07:39:34.282742Z'
notes: |-
  ---
  2026-03-21T07:23:42.977271+00:00
  Verified gate fails initially as expected. Reading shell/tool patterns and implementing shell loader plus focused tests in imp-core.
closed_at: '2026-03-21T07:39:34.282742Z'
parent: '2'
verify: 'cd /Users/asher/tower && cargo test -p imp-core -- tools::shell::tests 2>&1 | grep -q "test result: ok" && ! grep -q "TODO" imp/crates/imp-core/src/tools/shell.rs'
fail_first: true
checkpoint: '3418a0cc774ebcb6f18bd6607331f6e6a982501e'
claimed_by: pi-agent
claimed_at: '2026-03-21T07:22:28.575196Z'
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-21T07:39:34.283329Z'
  finished_at: '2026-03-21T07:39:35.039352Z'
  duration_secs: 0.756
  result: pass
  exit_code: 0
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-21T07:22:28.575196Z'
  finished_at: '2026-03-21T07:39:34.282742Z'
---

## Problem
`load_shell_tools()` in `imp/crates/imp-core/src/tools/shell.rs` is a no-op stub with a TODO comment.
The types (`ShellToolDef`, `ShellParamDef`, `ShellExecDef`) are already defined and correct.

## What to implement

### 1. Create `ShellTool` struct implementing `Tool`
```rust
pub struct ShellTool {
    def: ShellToolDef,
}
```

Implement `Tool` for `ShellTool`:
- `name()` → `def.name`
- `label()` → `def.label`
- `description()` → `def.description`
- `is_readonly()` → `def.readonly`
- `parameters()` → build JSON schema from `def.params`
- `execute()`:
  1. Validate required params are present
  2. Interpolate `{param}` and `{param|default}` placeholders in `def.exec.args`
  3. Run command with `tokio::process::Command`
  4. Capture stdout/stderr
  5. Apply truncation (use `truncate_head` or `truncate_tail` based on `def.exec.truncate`)
  6. If command not found, return helpful error with `def.exec.install_hint` if present

### 2. Implement `load_shell_tools(dir, registry)`
1. Walk `dir` for `*.toml` files
2. Parse each as `ShellToolDef` using `toml::from_str`
3. Create `ShellTool` instance, wrap in `Arc`, register in `registry`
4. Skip files that fail to parse (log warning, continue)

### 3. Add tests
- Create a temp dir with a test TOML tool definition (e.g., wraps `echo`)
- Load it, verify tool is registered
- Execute it, verify param interpolation and output

## Interpolation spec
- `{param}` → replaced with param value, error if missing and not optional
- `{param|default}` → replaced with param value or default if missing

## Files
- `imp/crates/imp-core/src/tools/shell.rs` — MODIFY: full implementation
- `imp/crates/imp-core/src/tools/mod.rs` — READ: Tool trait, truncation helpers
- `imp/crates/imp-core/src/tools/bash.rs` — READ: reference for command execution

## Do NOT
- Do not change ShellToolDef/ShellExecDef types
- Do not add new dependencies — use existing tokio::process::Command
