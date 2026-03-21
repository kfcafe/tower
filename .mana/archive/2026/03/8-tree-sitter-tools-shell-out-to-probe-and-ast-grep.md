---
id: '8'
title: Tree-sitter tools — shell out to probe and ast-grep CLIs
slug: tree-sitter-tools-shell-out-to-probe-and-ast-grep
status: closed
priority: 1
created_at: '2026-03-20T17:41:44.631450Z'
updated_at: '2026-03-21T07:44:51.234050Z'
notes: |-
  ---
  2026-03-21T07:24:38.277174+00:00
  Verified gate fails as expected; reading imp-core tool patterns and implementing CLI shims for probe_search/probe_extract/scan/ast_grep with module-local tests using fake probe/sg binaries.
closed_at: '2026-03-21T07:44:51.234050Z'
parent: '2'
verify: 'cd /Users/asher/tower && cargo test -p imp-core -- tools::tree_sitter::tests 2>&1 | grep -q "test result: ok" && ! grep -q "not yet implemented" imp/crates/imp-core/src/tools/tree_sitter.rs'
fail_first: true
checkpoint: '3418a0cc774ebcb6f18bd6607331f6e6a982501e'
claimed_by: pi-agent
claimed_at: '2026-03-21T07:22:28.723206Z'
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-21T07:44:51.236631Z'
  finished_at: '2026-03-21T07:44:52.547441Z'
  duration_secs: 1.31
  result: pass
  exit_code: 0
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-21T07:22:28.723206Z'
  finished_at: '2026-03-21T07:44:51.234050Z'
---

## Problem
The 4 tree-sitter tools in `imp/crates/imp-core/src/tools/tree_sitter.rs` are stubs that
return "not yet implemented" errors. They need real implementations.

## Approach: Shell out to CLI tools
Native tree-sitter integration (compiling 16+ grammars into the binary) is a large effort.
For v0.1, shell out to the `probe` CLI and `ast-grep` CLI, similar to how the grep tool
shells out to `rg`. Upgrade to native later.

### probe_search
Shell out to `probe` CLI:
```
probe search "query" --path <path> --max-results <n> --format json
```
Parse JSON output, format as text response with file paths and code blocks.
If `probe` is not installed, return helpful error: "Install probe: cargo install probe-search"

### probe_extract
Shell out to `probe` CLI:
```
probe extract "file:line" --context <n> --format json
```
Multiple targets: run probe once per target or use probe's multi-target syntax.

### scan
Shell out to `probe`:
```
probe scan --action <action> --files <files> --format json
```
Or implement a simpler version: use tree-sitter CLI or basic regex-based extraction
for function/type/import listing from source files.

If `probe` does not support `scan`, implement a basic version:
- For "extract": regex-based function/type extraction
- For "scan": walk directory, extract structure from each file

### ast_grep
Shell out to `ast-grep` CLI (also called `sg`):
```
sg --pattern "<pattern>" --lang <lang> <path> --json
```
For replace mode: `sg --pattern "<pattern>" --rewrite "<replace>" --lang <lang> <path>`

If `ast-grep`/`sg` is not installed, return helpful error: "Install ast-grep: cargo install ast-grep"

## Implementation pattern
Follow the same pattern as `grep.rs`:
1. Build command args from tool params
2. Spawn process with `tokio::process::Command`
3. Capture stdout, parse or format
4. Apply truncation
5. Return as ToolOutput

## Tests
- Test each tool with a small Rust source file in a temp directory
- Test graceful handling when CLI tool is not installed (mock or check)
- Test parameter mapping (query, path, language, maxResults)

## Files
- `imp/crates/imp-core/src/tools/tree_sitter.rs` — MODIFY: replace stubs with CLI shims
- `imp/crates/imp-core/src/tools/grep.rs` — READ: reference for shelling out to CLI
- `imp/crates/imp-core/src/tools/bash.rs` — READ: reference for command execution

## Do NOT
- Do not add tree-sitter crate dependencies (that is a future native implementation)
- Do not change the Tool trait signatures or parameter schemas (keep existing JSON schemas)
- Do not block on missing CLIs — return actionable error messages
