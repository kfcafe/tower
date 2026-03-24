---
id: '18'
title: 'Consolidate imp tools: merge diff, probe, and edit pairs'
slug: consolidate-imp-tools-merge-diff-probe-and-edit-pa
status: closed
priority: 2
created_at: '2026-03-22T16:32:36.979656Z'
updated_at: '2026-03-24T07:09:55.440933Z'
notes: |2

  ## Attempt 1 — 2026-03-24T07:09:38Z
  Exit code: 1

  ```

  ```
closed_at: '2026-03-24T07:09:55.440933Z'
close_reason: 'Auto-closed: all children completed'
verify: cd /Users/asher/tower && cargo test -p imp-core --lib 2>&1 | tail -1 | grep -q 'ok' && test $(grep -c 'agent.tools.register' imp/crates/imp-cli/src/main.rs) -le 13
attempts: 1
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-24T07:09:36.061105Z'
  finished_at: '2026-03-24T07:09:38.398147Z'
  duration_secs: 2.337
  result: fail
  exit_code: 1
---

Merge 3 tool pairs to reduce imp's tool count from 16 to 13:

1. diff_show + diff_apply → unified `diff` tool with action param
2. probe_search + probe_extract → unified `probe` tool with action param  
3. edit + multi_edit → unified `edit` tool (detect from params)

Each merge creates a new facade tool struct that dispatches to existing implementations.
Keep the implementation files (diff.rs, tree_sitter.rs, edit.rs, multi_edit.rs) unchanged.
Only the Tool trait impl and registration change.

## Files
- imp/crates/imp-core/src/tools/diff.rs (add unified DiffTool)
- imp/crates/imp-core/src/tools/tree_sitter.rs (add unified ProbeTool)
- imp/crates/imp-core/src/tools/edit.rs (merge multi_edit behavior)
- imp/crates/imp-cli/src/main.rs (update registrations)

## Acceptance
- cargo check -p imp-cli passes
- cargo test -p imp-core --lib passes
- 13 or fewer tool registrations in main.rs
- Each merged tool's description is concise and clear
