---
id: '21'
title: Replace pi with imp — interactive TUI sprint
slug: replace-pi-with-imp-interactive-tui-sprint
status: closed
priority: 2
created_at: '2026-03-23T09:05:29.295342Z'
updated_at: '2026-03-23T10:29:22.322882Z'
closed_at: '2026-03-23T10:29:22.322882Z'
close_reason: 'Auto-closed: all children completed'
verify: imp --version && imp --help | grep -q 'login'
is_archived: true
---

Sprint to make imp's TUI mode a complete replacement for pi.

## Success Criteria
- Can start imp, get a working chat session with OAuth
- Sessions persist and can be continued/resumed
- Slash commands work (/model, /settings, /compact, /new, /fork, /session, /resume, /name, /copy, /quit)
- Config settings panel (TUI overlay to edit defaults)
- AGENTS.md + project context loaded automatically
- OAuth token auto-refresh during long sessions
- Model selection works (Ctrl+L + /model) with config-driven enabled list
- Clean clippy on entire imp workspace

## Non-Goals (this sprint)
- Skills/extensions system
- RPC mode improvements
- Theme customization
- HTML export
- Subagent tool
