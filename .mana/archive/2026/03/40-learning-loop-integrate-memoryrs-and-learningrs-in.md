---
id: '40'
title: Learning loop — integrate memory.rs and learning.rs into agent
slug: learning-loop-integrate-memoryrs-and-learningrs-in
status: closed
priority: 3
created_at: '2026-03-24T07:29:03.572564Z'
updated_at: '2026-03-24T07:31:43.445358Z'
labels:
- imp-core
- learning
closed_at: '2026-03-24T07:31:43.445358Z'
verify: cd /Users/asher/tower && grep -q 'pub mod memory' imp/crates/imp-core/src/lib.rs && grep -q 'memory' imp/crates/imp-core/src/builder.rs && cargo test -p imp-core --lib memory 2>&1 | grep "test result" | grep "0 failed"
claimed_by: pi-agent
claimed_at: '2026-03-24T07:31:10.549069Z'
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-24T07:31:41.814234Z'
  finished_at: '2026-03-24T07:31:43.382319Z'
  duration_secs: 1.568
  result: pass
  exit_code: 0
outputs:
  text: 'test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 397 filtered out; finished in 0.03s'
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-24T07:31:10.549069Z'
  finished_at: '2026-03-24T07:31:43.445358Z'
---

Integrate existing memory.rs and learning.rs into the agent loop:

1. Memory store: persistent key-value at ~/.local/share/imp/memory.json
2. Builder loads memory and feeds into system prompt assembly (memory + user_profile layers already exist)
3. Learning: agent observes patterns and stores preferences
4. `/memory` slash command to view/clear

Files:
- `imp/crates/imp-core/src/memory.rs` — exists
- `imp/crates/imp-core/src/learning.rs` — exists
- `imp/crates/imp-core/src/builder.rs` — wire memory into prompt assembly
- `imp/crates/imp-tui/src/app.rs` — /memory command
