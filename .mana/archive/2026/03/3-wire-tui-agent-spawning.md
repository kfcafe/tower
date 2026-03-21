---
id: '3'
title: Wire TUI agent spawning
slug: wire-tui-agent-spawning
status: closed
priority: 0
created_at: '2026-03-20T17:39:57.080736Z'
updated_at: '2026-03-21T07:35:45.533506Z'
notes: |-
  ---
  2026-03-21T07:35:43.008431+00:00
  Discoveries: imp-tui can construct a usable agent entirely from imp-core/imp-llm pieces; imp-tui did not actually use imp-lua, so dropping that dependency avoids unrelated build failures while the Lua bridge is still incomplete.
closed_at: '2026-03-21T07:35:45.533506Z'
parent: '2'
verify: cd /Users/asher/tower && cargo check -p imp-tui 2>&1 | grep -q Finished && grep -q "Agent::new" imp/crates/imp-tui/src/app.rs && grep -q "agent.*run\|tokio::spawn" imp/crates/imp-tui/src/app.rs
fail_first: true
checkpoint: '3418a0cc774ebcb6f18bd6607331f6e6a982501e'
claimed_by: pi-agent
claimed_at: '2026-03-21T07:22:29.032101Z'
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-21T07:35:45.534289Z'
  finished_at: '2026-03-21T07:36:08.722587Z'
  duration_secs: 23.188
  result: pass
  exit_code: 0
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-21T07:22:29.032101Z'
  finished_at: '2026-03-21T07:35:45.533506Z'
---

## Problem
The TUI `send_message()` in `imp/crates/imp-tui/src/app.rs` adds a display message and sets
`is_streaming = true`, but NEVER creates an Agent or spawns the agent loop. The `agent_handle`
field is always `None`. This means the TUI is completely non-functional — no messages reach the LLM.

## What to implement
In `send_message()` (or a helper it calls), wire up the full agent lifecycle:

1. **Resolve model**: Use `self.model_registry.find_by_alias(&self.model_name)` to get `ModelMeta`,
   then create a provider via `imp_llm::providers::create_provider(&meta.provider)`.
   Build a `Model { meta, provider }`.

2. **Resolve API key**: Load `AuthStore` from config dir, call `auth_store.resolve(&provider_name)`.

3. **Create Agent**: Call `Agent::new(model, self.cwd.clone())` — this returns `(Agent, AgentHandle)`.

4. **Configure Agent**:
   - Set `agent.thinking_level = self.thinking_level`
   - Set `agent.api_key = api_key`
   - Set `agent.system_prompt` — at minimum use identity layer. Can call system_prompt::assemble()
     or just set a basic string for now.
   - Register all native tools on `agent.tools`:
     ```rust
     use imp_core::tools::{read::ReadTool, write::WriteTool, edit::EditTool, ...};
     agent.tools.register(Arc::new(ReadTool));
     agent.tools.register(Arc::new(WriteTool));
     // etc for all tools
     ```
   - Copy existing messages from the session into `agent.messages` for context continuity.

5. **Spawn the agent loop**: `tokio::spawn(async move { agent.run(prompt).await; })`.
   The agent owns itself and runs independently.

6. **Store the handle**: `self.agent_handle = Some(handle);`
   The existing `drain_agent_events()` and `handle_agent_event()` already work correctly —
   they just need a non-None handle.

## Key types to know
```rust
// From imp-core/src/agent.rs
pub struct Agent { pub model, pub tools, pub messages, pub system_prompt, pub api_key, ... }
pub struct AgentHandle { pub event_rx: mpsc::Receiver<AgentEvent>, pub command_tx: mpsc::Sender<AgentCommand> }
impl Agent { pub fn new(model: Model, cwd: PathBuf) -> (Self, AgentHandle) }
impl Agent { pub async fn run(&mut self, prompt: String) -> Result<()> }

// From imp-llm/src/providers/mod.rs
pub fn create_provider(name: &str) -> Option<Box<dyn Provider>>

// From imp-llm/src/auth.rs
pub struct AuthStore { ... }
impl AuthStore { pub fn resolve(&self, provider: &str) -> Result<String> }
```

## Files
- `imp/crates/imp-tui/src/app.rs` — MODIFY: wire send_message to create + spawn agent
- `imp/crates/imp-tui/Cargo.toml` — MODIFY: may need to add imp-llm as a dependency
- `imp/crates/imp-core/src/agent.rs` — READ: Agent API
- `imp/crates/imp-core/src/tools/*.rs` — READ: tool structs to register
- `imp/crates/imp-llm/src/providers/mod.rs` — READ: create_provider
- `imp/crates/imp-llm/src/auth.rs` — READ: AuthStore
- `imp/crates/imp-core/src/config.rs` — READ: Config::user_config_dir()

## Acceptance
- `cargo check -p imp-tui` passes
- `send_message()` creates an Agent with registered tools and spawns it
- `self.agent_handle` is set to Some(handle) after spawning
- The existing drain_agent_events / handle_agent_event already handle the rest

## Do NOT
- Do not change the Agent struct, AgentHandle, or AgentEvent types
- Do not change how drain_agent_events or handle_agent_event work — they are correct
- Do not add mana-core integration — just basic agent + tools + provider
