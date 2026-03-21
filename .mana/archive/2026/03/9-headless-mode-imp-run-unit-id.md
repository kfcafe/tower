---
id: '9'
title: Headless mode — imp run <unit-id>
slug: headless-mode-imp-run-unit-id
status: closed
priority: 1
created_at: '2026-03-20T17:42:06.555803Z'
updated_at: '2026-03-21T07:49:16.778554Z'
closed_at: '2026-03-21T07:49:16.778554Z'
notes: |-
  ---
  2026-03-21T07:49:55Z
  Discoveries: imp-cli main.rs already had concurrent RPC-mode edits in progress, so headless mode needed to merge into the newer command-dispatch shape. The TUI's native tool registration list in imp-tui/src/app.rs is the right source to mirror for headless setup. Mana units keep retry context in YAML frontmatter notes/attempt_log, while the markdown body is the main task description.
parent: '2'
dependencies:
- '3'
verify: cd /Users/asher/tower && cargo build -p imp-cli 2>&1 | grep -q Finished && ! grep -q "not yet implemented" imp/crates/imp-cli/src/main.rs
fail_first: true
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-21T07:49:16.780541Z'
  finished_at: '2026-03-21T07:49:17.486864Z'
  duration_secs: 0.706
  result: pass
  exit_code: 0
---

## Problem
`imp run <unit-id>` prints "headless mode not yet implemented" and exits.
This mode is critical for orchestration — wizard-orch dispatches imp processes this way.

## What to implement

In `Commands::Run` handler in `imp/crates/imp-cli/src/main.rs`:

### 1. Load the mana unit
Read the unit file from `.mana/` directory. For now, do this simply:
- Walk up from cwd looking for `.mana/` directory
- Find the unit file matching `unit_id` (glob for `*{unit_id}*.md`)
- Parse the YAML frontmatter to get title, description, verify command, notes
- If unit not found, error and exit 1

Do NOT depend on mana-core yet. Just read the YAML file directly using `serde_yaml`.
The unit file format is YAML frontmatter + markdown body:
```yaml
---
id: "15.1"
title: "Some task"
status: open
verify: "cargo test ..."
---
Description body here...
```

### 2. Assemble task prompt
Build a prompt from the unit context:
```
Task: {title}

{description}

Verify command: {verify}
```
If there are notes/previous attempts, include them.

### 3. Create and run agent
Same pattern as TUI spawning (unit 15.1 must be done first):
- Resolve model + provider + API key
- Create Agent, register all native tools
- Set system prompt (assemble from resources if available, otherwise basic identity)
- Run agent with task prompt (NOT spawned — run synchronously since this is headless)

### 4. Stream events as JSON lines to stdout
While agent runs, emit each AgentEvent as a JSON line:
```json
{"type":"turn_start","index":0}
{"type":"text_delta","text":"Looking at..."}
{"type":"tool_execution_start","tool":"read","args":{"path":"src/main.rs"}}
...
```

### 5. Run verify after agent completes
If the unit has a verify command:
- Run it with `tokio::process::Command`
- Exit 0 if verify passes, 1 if not

### 6. Add serde_yaml dependency
Add `serde_yaml = "0.9"` to imp-cli's Cargo.toml if not already present.

## Files
- `imp/crates/imp-cli/src/main.rs` — MODIFY: implement Run command
- `imp/crates/imp-cli/Cargo.toml` — MODIFY: add serde_yaml if needed
- `imp/crates/imp-core/src/agent.rs` — READ: Agent API
- `imp/crates/imp-core/src/tools/*.rs` — READ: tool structs to register

## Do NOT
- Do not depend on mana-core (yet) — read unit files directly
- Do not add interactive prompts — this is fully headless
- Do not change the Agent or AgentEvent types
