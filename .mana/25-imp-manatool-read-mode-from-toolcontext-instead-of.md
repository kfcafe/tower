---
id: '25'
title: 'imp ManaTool: read mode from ToolContext instead of env var'
slug: imp-manatool-read-mode-from-toolcontext-instead-of
status: open
priority: 2
created_at: '2026-03-23T19:33:41.390899Z'
updated_at: '2026-03-23T19:53:49.665938Z'
notes: |-
  ---
  2026-03-23T19:53:49.665926+00:00
  ## Attempt Failed (20m1s, 1.7M tokens, $0.889)

  ### What was tried

  - 0 tool calls over 35 turns in 20m1s

  ### Why it failed

  - Timeout (20m)

  ### Verify command

  `cd /Users/asher/tower && cargo test -p imp-core agent_mode_mana_ctx 2>&1 | grep -E '[1-9][0-9]* passed'`

  ### Suggestion for next attempt

  - Agent ran out of time. Consider increasing the timeout or simplifying the task scope.
labels:
- imp
verify: cd /Users/asher/tower && cargo test -p imp-core agent_mode_mana_ctx 2>&1 | grep -E '[1-9][0-9]* passed'
fail_first: true
checkpoint: '41a81ab32bbf1bf894422bec1238253733f75e89'
attempt_log:
- num: 1
  outcome: abandoned
  agent: pi-agent
  started_at: '2026-03-23T19:33:48.867644Z'
  finished_at: '2026-03-23T19:53:49.516838Z'
---

## What to implement

ManaTool currently reads the agent mode from `IMP_MODE` env var (mana.rs line 73). This is fragile — it can diverge from the Agent's actual mode field. Instead, pass the mode through ToolContext so ManaTool reads `ctx.mode`.

### 1. Add mode to ToolContext

In `imp/crates/imp-core/src/tools/mod.rs`, add `pub mode: AgentMode` to the `ToolContext` struct:

```rust
pub struct ToolContext {
    pub cwd: PathBuf,
    pub cancelled: Arc<std::sync::atomic::AtomicBool>,
    pub update_tx: tokio::sync::mpsc::Sender<ToolUpdate>,
    pub ui: Arc<dyn UserInterface>,
    pub file_cache: Arc<FileCache>,
    pub mode: AgentMode,  // <-- add this
}
```

Import AgentMode: `use crate::config::AgentMode;`

### 2. Wire mode into ToolContext in agent.rs

In `agent.rs` `execute_one_tool()`, where ToolContext is constructed (around line 575), add:
```rust
mode: self.mode,
```

### 3. Update ManaTool to use ctx.mode

In `imp/crates/imp-core/src/tools/mana.rs`, replace the env var reading (lines 73-76):

```rust
// BEFORE:
let mode = std::env::var("IMP_MODE")
    .ok()
    .and_then(|v| AgentMode::from_name(&v))
    .unwrap_or(AgentMode::Full);

// AFTER:
let mode = ctx.mode;
```

### 4. Update all ToolContext construction sites

Search for `ToolContext {` and add `mode: AgentMode::Full` (or appropriate mode) at each construction site. These are mainly in tests.

### 5. Update ManaTool tests

The mana tests currently set/unset `IMP_MODE` env var with a static mutex. Replace with constructing ToolContext with the desired mode directly — cleaner and no env var races.

### Files
- `imp/crates/imp-core/src/tools/mod.rs` — add mode to ToolContext
- `imp/crates/imp-core/src/agent.rs` — wire self.mode into ToolContext
- `imp/crates/imp-core/src/tools/mana.rs` — read ctx.mode instead of env var, update tests

### Existing code

ToolContext (tools/mod.rs L57-62):
```rust
pub struct ToolContext {
    pub cwd: PathBuf,
    pub cancelled: Arc<std::sync::atomic::AtomicBool>,
    pub update_tx: tokio::sync::mpsc::Sender<ToolUpdate>,
    pub ui: Arc<dyn UserInterface>,
    pub file_cache: Arc<FileCache>,
}
```

ToolContext construction in execute_one_tool (agent.rs ~L575):
```rust
let ctx = crate::tools::ToolContext {
    cwd: self.cwd.clone(),
    cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    update_tx,
    ui: self.ui.clone(),
    file_cache: self.file_cache.clone(),
};
```

ManaTool env reading (mana.rs L73-76):
```rust
let mode = std::env::var("IMP_MODE")
    .ok()
    .and_then(|v| AgentMode::from_name(&v))
    .unwrap_or(AgentMode::Full);
```

### Tests (prefix with `agent_mode_mana_ctx_`)
- `agent_mode_mana_ctx_reads_from_context` — ManaTool respects ctx.mode without env var
- `agent_mode_mana_ctx_worker_blocks_create` — Worker mode via ctx blocks mana create
- `agent_mode_mana_ctx_full_allows_all` — Full mode via ctx allows everything
