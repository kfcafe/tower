---
id: '35'
title: Learning loop — integrate memory.rs and learning.rs into agent
slug: learning-loop-integrate-memoryrs-and-learningrs-in
status: closed
priority: 3
created_at: '2026-03-24T05:30:08.983949Z'
updated_at: '2026-03-24T07:13:28.263733Z'
labels:
- imp-core
- learning
closed_at: '2026-03-24T07:13:28.263733Z'
close_reason: verify passed (tidy sweep)
verify: cd /Users/asher/tower && cargo test -p imp-core --lib learning 2>&1 | grep "test result" | grep "0 failed" && cargo test -p imp-core --lib memory 2>&1 | grep "test result" | grep "0 failed"
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-24T07:12:40.561847Z'
  finished_at: '2026-03-24T07:13:28.230489Z'
  duration_secs: 47.668
  result: pass
  exit_code: 0
outputs:
  text: |-
    test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 412 filtered out; finished in 0.01s
    test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 397 filtered out; finished in 0.02s
---

Other agents created memory.rs, learning.rs, and skill_manage.rs. These need to be integrated into the agent loop:

1. Memory store: persistent key-value memory across sessions (`~/.local/share/imp/memory.json`)
2. Learning: agent can observe patterns and store them as learned preferences
3. The system prompt already has a memory layer — wire it to the actual store
4. Add `/memory` slash command to view/clear learned preferences

Files:
- `imp/crates/imp-core/src/memory.rs` — exists
- `imp/crates/imp-core/src/learning.rs` — exists
- `imp/crates/imp-core/src/builder.rs` — wire memory into system prompt assembly
- `imp/crates/imp-tui/src/app.rs` — /memory command
