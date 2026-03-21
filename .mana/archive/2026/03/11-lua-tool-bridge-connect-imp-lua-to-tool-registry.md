---
id: '11'
title: Lua tool bridge — connect imp-lua to tool registry
slug: lua-tool-bridge-connect-imp-lua-to-tool-registry
status: closed
priority: 2
created_at: '2026-03-20T17:42:50.516774Z'
updated_at: '2026-03-21T07:41:26.592848Z'
notes: |-
  ---
  2026-03-21T07:25:24.725686+00:00
  Verified gate fails as expected. Investigating circular dependency between imp-core and imp-lua; plan is to keep the actual Tool bridge in imp-lua, add imp-core helper functions/tests, and avoid a normal dep cycle.
closed_at: '2026-03-21T07:41:26.592848Z'
parent: '2'
verify: 'cd /Users/asher/tower && cargo test -p imp-core -- tools::lua::tests 2>&1 | grep -q "test result: ok" && ! grep -q "TODO" imp/crates/imp-core/src/tools/lua.rs'
fail_first: true
checkpoint: '3418a0cc774ebcb6f18bd6607331f6e6a982501e'
claimed_by: pi-agent
claimed_at: '2026-03-21T07:22:28.761930Z'
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-21T07:41:26.593340Z'
  finished_at: '2026-03-21T07:41:27.194393Z'
  duration_secs: 0.601
  result: pass
  exit_code: 0
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-21T07:22:28.761930Z'
  finished_at: '2026-03-21T07:41:26.592848Z'
---

## Problem
`load_lua_tools()` in `imp/crates/imp-core/src/tools/lua.rs` is a no-op stub.
The imp-lua crate has a working bridge (`bridge.rs`) that stores tool registrations
in `LuaRuntime.tools`, but nothing connects those to imp-core's `ToolRegistry`.

## What to implement

### 1. Create `LuaTool` struct implementing `Tool`

```rust
use imp_lua::sandbox::{LuaRuntime, LuaToolHandle};

pub struct LuaTool {
    name: String,
    label: String,
    description: String,
    readonly: bool,
    params: serde_json::Value,
    runtime: Arc<LuaRuntime>,
    execute_key_index: usize, // index into runtime.tools()
}
```

Implement `Tool`:
- `name()`, `label()`, `description()`, `is_readonly()` → from handle
- `parameters()` → build JSON schema from `handle.params`
- `execute()`:
  1. Get the Lua function from the runtime registry using the key
  2. Convert params to Lua table
  3. Call the Lua function
  4. Convert Lua result table back to ToolOutput

### 2. Implement `load_lua_tools(runtime, registry)`
```rust
pub fn load_lua_tools(runtime: &Arc<LuaRuntime>, registry: &mut ToolRegistry) {
    let tools = runtime.tools();
    let handles = tools.lock().unwrap();
    for (i, handle) in handles.iter().enumerate() {
        let lua_tool = LuaTool {
            name: handle.name.clone(),
            // ... fill from handle
            runtime: Arc::clone(runtime),
            execute_key_index: i,
        };
        registry.register(Arc::new(lua_tool));
    }
}
```

### 3. Handle the async bridge
The Lua execute function is synchronous. Use `tokio::task::spawn_blocking` to avoid
blocking the async runtime:
```rust
async fn execute(&self, call_id: &str, params: Value, ctx: ToolContext) -> Result<ToolOutput> {
    let runtime = self.runtime.clone();
    let key_idx = self.execute_key_index;
    tokio::task::spawn_blocking(move || {
        // Call Lua function synchronously
    }).await?
}
```

### 4. Add imp-lua dependency
Add `imp-lua.workspace = true` to `imp-core/Cargo.toml` (or make it optional with a feature flag).
Actually, check if this creates a circular dependency — imp-lua depends on imp-core.
If circular: put LuaTool in imp-lua instead, and have imp-cli wire it into the registry.

### 5. Tests
- Create a LuaRuntime, load a script that registers a tool via `imp.register_tool()`
- Load lua tools into a ToolRegistry
- Execute the tool, verify it runs the Lua function

## Files
- `imp/crates/imp-core/src/tools/lua.rs` — MODIFY: full implementation
- `imp/crates/imp-lua/src/sandbox.rs` — READ: LuaRuntime, LuaToolHandle
- `imp/crates/imp-lua/src/bridge.rs` — READ: host API, lua_value_to_json
- `imp/crates/imp-core/Cargo.toml` — MODIFY: add imp-lua dep (check for cycles)

## Do NOT
- Do not change the Lua API (imp.register_tool format)
- Do not change the Tool trait
- Watch for circular dependencies between imp-core and imp-lua
