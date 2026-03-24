---
id: '36'
title: Subagent tool — dispatch child agents from within a session
slug: subagent-tool-dispatch-child-agents-from-within-a
status: closed
priority: 3
created_at: '2026-03-24T05:30:20.790570Z'
updated_at: '2026-03-24T05:40:47.616329Z'
notes: |-
  ---
  2026-03-24T05:40:46.756662+00:00
  Not needed — mana run is our subagent system
labels:
- imp-core
- tools
closed_at: '2026-03-24T05:40:47.616329Z'
verify: cd /Users/asher/tower && cargo test -p imp-core --lib tools::subagent 2>&1 | grep "test result" | grep "0 failed"
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-24T05:40:46.785724Z'
  finished_at: '2026-03-24T05:40:47.585361Z'
  duration_secs: 0.799
  result: pass
  exit_code: 0
outputs:
  text: 'test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 425 filtered out; finished in 0.00s'
---

Add a `subagent` tool that lets the agent spawn child agents for parallel or sequential work.

Modes:
- `single` — spawn one agent with a task, wait for result
- `parallel` — spawn N agents with different tasks, collect results
- `chain` — sequential agents where each gets the previous output via {previous}

The child agents:
- Get their own tool set (configurable: full, readonly, none)
- Run headlessly (no TUI)
- Share the same cwd and file system
- Results are returned as tool output to the parent

This is the last tool needed for full pi parity (pi has subagent via its extension system).

Files:
- `imp/crates/imp-core/src/tools/subagent.rs` — new tool
- `imp/crates/imp-core/src/tools/mod.rs` — register
- `imp/crates/imp-core/src/builder.rs` — register in tool set
