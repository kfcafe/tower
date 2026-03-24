---
id: '41'
title: Lua extension system — load .lua plugins as tools and slash commands
slug: lua-extension-system-load-lua-plugins-as-tools-and
status: open
priority: 1
created_at: '2026-03-24T08:00:10.698926Z'
updated_at: '2026-03-24T08:02:14.259916Z'
notes: |-
  ---
  2026-03-24T08:02:14.259559+00:00
  imp-lua crate already has 1349 lines: VM, sandbox, bridge, loader, event bus, 31 tests. Missing: ctx.tool() bridge to native tools, ctx.http, ctx.env scoping, builder integration, TUI command dispatch, trust store, /reload wiring.
labels:
- imp-lua
- extensions
verify: cd /Users/asher/tower && cargo test -p imp-lua --lib 2>&1 | grep "test result" | grep -v "0 passed" | grep "0 failed"
---

Build the Lua extension system for imp. This is the parent unit.

## Design

Extensions are .lua files that define tools and/or slash commands. Drop a file in, it's live.

### Discovery
- `~/.config/imp/extensions/` — user extensions (global)
- `.imp/extensions/` — project extensions (per-repo, trust-prompted)

### Format

Single tool shorthand:
```lua
return {
    name = "weather",
    description = "Get weather for a location",
    parameters = { ... },
    execute = function(params, ctx) ... end
}
```

Multi-tool + commands:
```lua
return {
    tools = { { name = ..., execute = ... }, ... },
    commands = { { name = ..., execute = ... }, ... }
}
```

### ctx API
```
ctx.tool(name, params)       -- call any native imp tool
ctx.http.get(url, headers?)  -- HTTP GET, returns { status, body }
ctx.http.post(url, body, headers?)
ctx.env(name)                -- env var (scoped to declared vars in sandbox mode)
ctx.cwd                      -- working directory
ctx.home                     -- home directory
```

### Security
- Sandboxed by default: no raw os/io/loadfile, only ctx API
- `trust = true` in extension definition unlocks full Lua stdlib
- Project extensions require one-time user approval (stored in ~/.config/imp/trusted_extensions.json)
- Extensions declare env vars they need: `env = { "API_KEY" }`

### Runtime
- Lua 5.4 via mlua crate
- One VM, separate environments per extension
- 30s default timeout per execution
- Reload on /reload slash command

### Files
- `imp/crates/imp-lua/src/lib.rs` — VM setup, sandbox, ctx bindings
- `imp/crates/imp-lua/src/loader.rs` — scan dirs, evaluate .lua, extract definitions
- `imp/crates/imp-lua/src/bridge.rs` — LuaTool implements Tool trait
- `imp/crates/imp-lua/src/http.rs` — ctx.http bindings
- `imp/crates/imp-core/src/builder.rs` — register Lua tools alongside native
- `imp/crates/imp-tui/src/app.rs` — dispatch Lua slash commands
