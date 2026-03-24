---
id: '37'
title: Lua extension system — load and run .lua plugins as tools
slug: lua-extension-system-load-and-run-lua-plugins-as-t
status: closed
priority: 3
created_at: '2026-03-24T05:30:30.270404Z'
updated_at: '2026-03-24T07:13:28.795598Z'
labels:
- imp-lua
- extensions
closed_at: '2026-03-24T07:13:28.795598Z'
close_reason: verify passed (tidy sweep)
verify: cd /Users/asher/tower && cargo test -p imp-lua --lib 2>&1 | grep "test result" | grep "0 failed"
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-24T07:13:28.284830Z'
  finished_at: '2026-03-24T07:13:28.769523Z'
  duration_secs: 0.484
  result: pass
  exit_code: 0
outputs:
  text: 'test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s'
---

The imp-lua crate exists but is empty. Build a plugin system that:

1. Scans `~/.config/imp/extensions/` and `.imp/extensions/` for .lua files
2. Each .lua file defines a tool: name, description, parameters, execute function
3. Lua tools are registered alongside native tools
4. Lua has access to a sandboxed API: read/write files, run commands, HTTP requests
5. Extensions can add slash commands too

This replaces pi's JavaScript extension system with a lighter Lua approach.

Files:
- `imp/crates/imp-lua/src/lib.rs` — exists, needs implementation
- `imp/crates/imp-core/src/builder.rs` — load and register Lua tools
- `~/.config/imp/extensions/` — user extension directory
