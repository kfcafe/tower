---
id: '22'
title: 'imp: Agent modes (full, worker, orchestrator, planner, reviewer, auditor)'
slug: imp-agent-modes-full-worker-orchestrator-planner-r
status: closed
priority: 1
created_at: '2026-03-23T09:05:42.106562Z'
updated_at: '2026-03-23T09:53:42.092900Z'
labels:
- imp
- architecture
closed_at: '2026-03-23T09:53:42.092900Z'
close_reason: 'Auto-closed: all children completed'
verify: cd /Users/asher/tower && cargo test -p imp-core agent_mode -- --test-threads=1 2>&1 | grep -E "([4-9]|[1-9][0-9]+) passed"
fail_first: true
is_archived: true
---

## Goal

Add a `mode` system to imp that controls what tools an agent has access to, with execution-time enforcement. Modes define the *purpose* of the agent — what it's allowed to do.

## Background

Currently imp has a `Role` system that can filter tools via `ToolSet::Only(vec![...])`, but:
1. Filtering only happens at prompt generation time (tools hidden from LLM description) — no execution-time guard
2. Roles are user-defined in config with manual tool lists — no semantic categories
3. There's no concept of "this agent should only plan, not execute" or "this agent should only coordinate through mana"

## Modes

| Mode | Tools | Purpose |
|------|-------|---------|
| `full` | everything | Default. Interactive use, trusted user. |
| `worker` | read + write + bash (no mana create/run) | Unit executor. Gets work done. Uses `mana update` for progress. |
| `orchestrator` | read + mana (all actions) + ask | Plans AND executes via mana. Creates units, runs them, monitors. Cannot touch files directly. |
| `planner` | read + mana (read + create only) + ask | Decomposes work. Cannot run units. Human approves before execution. |
| `reviewer` | read + ask | Reads and reports. No mutations. |
| `auditor` | read + mana (read-only: status, list, show) | Batch inspector. Reads code and mana state, produces reports. |

Tool categories for mode filtering:
- **read**: read, grep, find, ls, scan, web, diff_show
- **write**: write, edit, multi_edit, diff_apply
- **execute**: bash
- **coordinate**: mana (with sub-action filtering)
- **interact**: ask

## Implementation

### 1. `AgentMode` enum in `config.rs`
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AgentMode {
    #[default]
    Full,
    Worker,
    Orchestrator,
    Planner,
    Reviewer,
    Auditor,
}
```

Add `mode: AgentMode` to `Config`.

### 2. `AgentMode::allowed_tools()` method
Returns the list of allowed tool names for the mode. This is the single source of truth for what each mode can do.

### 3. `AgentMode::allowed_mana_actions()` method
Returns which mana sub-actions are allowed (some modes can read mana state but not create/run units).

### 4. Execution-time enforcement in `agent.rs`
In `execute_one_tool()`, before running any tool:
- Check if the tool name is allowed by the current mode
- For the `mana` tool specifically, check if the requested action is allowed
- Return a clear error message: "Tool 'write' is not available in orchestrator mode"

### 5. Wire mode into `AgentBuilder`
- `AgentBuilder` reads `config.mode` and stores it on `Agent`
- `register_native_tools` filters based on mode (don't even register disallowed tools)
- System prompt assembly uses mode to filter tool definitions
- Mode can be overridden via `IMP_MODE` env var or `--mode` CLI flag

### 6. Mode-aware system prompt instructions
Each mode gets a brief behavioral instruction appended:
- orchestrator: "You coordinate work through mana units. Do not modify files directly."
- planner: "You decompose work into mana units. Do not run them — a human will approve."
- worker: "Focus on your assigned task. Use mana update to log progress."
- reviewer: "Read and analyze. Report findings. Do not modify anything."
- auditor: "Inspect code and unit state. Produce reports."

## Files to modify
- `imp/crates/imp-core/src/config.rs` — AgentMode enum, add to Config
- `imp/crates/imp-core/src/roles.rs` — Mode can override role tool filtering
- `imp/crates/imp-core/src/agent.rs` — Add mode field, execution-time guard in execute_one_tool
- `imp/crates/imp-core/src/builder.rs` — Wire mode, filter tool registration
- `imp/crates/imp-core/src/system_prompt.rs` — Mode-aware tool listing + instructions
- `imp/crates/imp-core/src/tools/mana.rs` — Sub-action filtering based on mode

## Existing code context

**ToolRegistry** (`tools/mod.rs`):
- `register(&mut self, tool)` — adds tool
- `definitions()` / `readonly_definitions()` — returns tool defs for LLM
- `get(name)` — lookup by name

**Tool trait** (`tools/mod.rs`):
- `is_readonly()` — already categorizes tools
- No `category()` method yet

**Role system** (`roles.rs`):
- `ToolSet::All` or `ToolSet::Only(vec![...])`
- Role can set `readonly: true` which filters to read-only tools
- Builtin roles: worker, explorer, reviewer

**AgentBuilder** (`builder.rs`):
- `register_native_tools()` — canonical tool registration, 11 tools
- `build()` — wires config → agent

**ManaTool** (`tools/mana.rs`):
- Single tool with `action` parameter: status, list, show, create, close, update
- Sub-action filtering needed: planner can create but not close; auditor can only read

**execute_one_tool** (`agent.rs` L451-555):
- Has hook-based blocking already (`before_tool_call`)
- Mode guard should go right after the hook check, before arg validation

## Config example
```toml
mode = "orchestrator"

# or in a role definition:
[roles.planner]
mode = "planner"
model = "sonnet"
thinking = "high"
```

## Env override
`IMP_MODE=orchestrator imp run "plan the refactor"`
