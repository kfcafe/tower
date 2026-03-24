---
id: '33'
title: 'TUI polish: compact tool output rendering'
slug: tui-polish-compact-tool-output-rendering
status: closed
priority: 2
created_at: '2026-03-24T05:29:48.250333Z'
updated_at: '2026-03-24T07:04:03.183095Z'
labels:
- imp-tui
- polish
closed_at: '2026-03-24T07:04:03.183095Z'
verify: cd /Users/asher/tower && cargo test -p imp-tui --lib 2>&1 | grep "test result" | grep "0 failed"
claimed_by: pi-agent
claimed_at: '2026-03-24T06:56:42.698182Z'
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-24T07:04:01.597572Z'
  finished_at: '2026-03-24T07:04:03.119186Z'
  duration_secs: 1.521
  result: pass
  exit_code: 0
outputs:
  text: 'test result: ok. 54 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.91s'
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-24T06:56:42.698182Z'
  finished_at: '2026-03-24T07:04:03.183095Z'
---

Make tool call display in the chat view more compact and less noisy.

Current: each tool call takes a full line with `│ ✓ read path/to/file  42 lines`. When there are 5-10 tool calls in a turn, they dominate the chat visually.

Ideas:
- Group consecutive tool calls into a single compact block
- When collapsed (not peeked): show as `  ✓ read path  ✓ grep pattern  ✓ edit file` — multiple on one line
- When expanded (Tab peek): show full detail per tool
- Errors always auto-expand
- Running tool gets the spinner, completed ones are one-liners

Files:
- `imp/crates/imp-tui/src/views/chat.rs` — tool call rendering in chat
- `imp/crates/imp-tui/src/views/tools.rs` — DisplayToolCall header/formatting
