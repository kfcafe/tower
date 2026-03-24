---
id: '39'
title: FTS5 session search — wire session_index.rs into /resume
slug: fts5-session-search-wire-sessionindexrs-into-resum
status: closed
priority: 2
created_at: '2026-03-24T07:28:52.318030Z'
updated_at: '2026-03-24T07:30:17.384946Z'
labels:
- imp-core
- imp-tui
- search
closed_at: '2026-03-24T07:30:17.384946Z'
verify: cd /Users/asher/tower && grep -q 'session_index' imp/crates/imp-core/src/lib.rs && cargo test -p imp-core --lib session_index 2>&1 | grep "test result" | grep "0 failed"
claimed_by: pi-agent
claimed_at: '2026-03-24T07:29:10.631750Z'
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-24T07:30:16.295009Z'
  finished_at: '2026-03-24T07:30:17.318295Z'
  duration_secs: 1.023
  result: pass
  exit_code: 0
outputs:
  text: 'test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 411 filtered out; finished in 0.03s'
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-24T07:29:10.631750Z'
  finished_at: '2026-03-24T07:30:17.384946Z'
---

Wire the existing session_index.rs (rusqlite + FTS5) into the TUI:

1. Index sessions on startup (background tokio task)
2. `/resume <query>` searches FTS5 index instead of file scan
3. Session picker shows search results ranked by relevance
4. New sessions indexed as messages are appended

Files:
- `imp/crates/imp-core/src/session_index.rs` — exists, needs integration
- `imp/crates/imp-tui/src/views/session_picker.rs` — add search input
- `imp/crates/imp-tui/src/app.rs` — wire indexer + search
