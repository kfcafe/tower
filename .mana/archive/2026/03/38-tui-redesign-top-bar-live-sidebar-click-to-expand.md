---
id: '38'
title: 'TUI redesign: top bar, live sidebar, click-to-expand'
slug: tui-redesign-top-bar-live-sidebar-click-to-expand
status: closed
priority: 1
created_at: '2026-03-24T06:46:31.248127Z'
updated_at: '2026-03-24T07:13:38.634799Z'
labels:
- imp-tui
- redesign
closed_at: '2026-03-24T07:13:38.634799Z'
close_reason: verify passed (tidy sweep)
verify: cd /Users/asher/tower && cargo test -p imp-tui --lib 2>&1 | grep "test result" | grep "0 failed"
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-24T07:13:28.819194Z'
  finished_at: '2026-03-24T07:13:38.596983Z'
  duration_secs: 9.777
  result: pass
  exit_code: 0
outputs:
  text: 'test result: ok. 73 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.04s'
---

Major TUI layout redesign for imp. This is the parent unit — children handle individual pieces.

## Vision

Alternate screen with three zones:
- **Top bar** (1 line): model · context gauge · cost · cwd · session name
- **Main area** (split): left chat+editor, right live tool preview sidebar
- **No bottom status bar** — top bar replaces it

## Sidebar (toggleable, Ctrl+P)
- Auto-follows the active tool call
- bash → streaming terminal output
- edit/multi_edit → unified diff
- read → file content
- grep → matches with context
- write → content being written
- When idle: shows last tool output, dimmed
- Own scroll context (mouse wheel when hovered, or Ctrl+↑/↓)
- Auto-opens if terminal ≥120 cols on first tool call
- Width: 40% of terminal, min 30 cols

## Click-to-expand
- Click on any tool call line in chat → shows that tool's output in sidebar
- Mouse click events already captured, just need Y-coordinate mapping

## Theme
- Already done: dungeon stone default with config-driven overrides

## Files
- `imp/crates/imp-tui/src/app.rs` — layout, event routing
- `imp/crates/imp-tui/src/views/chat.rs` — click target tracking
- `imp/crates/imp-tui/src/views/sidebar.rs` — new: tool preview pane
- `imp/crates/imp-tui/src/views/top_bar.rs` — new: replaces status bar
- `imp/crates/imp-tui/src/views/status.rs` — deprecate or repurpose
