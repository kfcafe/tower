---
id: '34'
title: FTS5 session search — wire session_index.rs into /resume
slug: fts5-session-search-wire-sessionindexrs-into-resum
status: closed
priority: 2
created_at: '2026-03-24T05:29:58.371909Z'
updated_at: '2026-03-24T07:12:38.055858Z'
labels:
- imp-core
- imp-tui
- search
closed_at: '2026-03-24T07:12:38.055858Z'
close_reason: verify passed (tidy sweep)
verify: cd /Users/asher/tower && cargo test -p imp-core --lib session_index 2>&1 | grep "test result" | grep "0 failed"
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-24T07:11:56.765456Z'
  finished_at: '2026-03-24T07:12:37.062297Z'
  duration_secs: 40.296
  result: pass
  exit_code: 0
outputs:
  text: 'test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 411 filtered out; finished in 0.68s'
---

Another agent created `imp/crates/imp-core/src/session_index.rs` with rusqlite + FTS5 for full-text session search. It needs to be integrated:

1. Index sessions on startup (background task)
2. `/resume <query>` searches the FTS5 index instead of scanning files
3. Session picker shows search results ranked by relevance
4. New sessions are indexed as messages are appended

Files:
- `imp/crates/imp-core/src/session_index.rs` — exists, needs integration
- `imp/crates/imp-tui/src/views/session_picker.rs` — add search input
- `imp/crates/imp-tui/src/app.rs` — wire indexer startup + search
