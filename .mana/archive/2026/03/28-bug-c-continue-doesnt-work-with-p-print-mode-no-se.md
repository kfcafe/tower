---
id: '28'
title: 'bug: -c (continue) doesn''t work with -p (print mode) — no session resume'
slug: bug-c-continue-doesnt-work-with-p-print-mode-no-se
status: closed
priority: 2
created_at: '2026-03-24T02:36:59.526205Z'
updated_at: '2026-03-24T02:54:37.729910Z'
labels:
- bug
- imp-cli
closed_at: '2026-03-24T02:54:37.729910Z'
verify: 'cd /Users/asher/tower && echo "test1" | imp -p "Remember the word: elephant" 2>/dev/null && imp -p "What word did I tell you to remember?" -c 2>&1 | grep -i elephant'
claimed_by: pi-agent
claimed_at: '2026-03-24T02:43:28.124839Z'
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-24T02:54:32.330583Z'
  finished_at: '2026-03-24T02:54:37.654690Z'
  duration_secs: 5.324
  result: pass
  exit_code: 0
outputs:
  text: |-
    Remembered: **elephant** 🐘The user told me to remember the word "elephant".
    The word you told me to remember is **elephant**.
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-24T02:43:28.124839Z'
  finished_at: '2026-03-24T02:54:37.729910Z'
---

## Bug

Print mode (`imp -p "prompt"`) ignores `-c` / `--cont`. It always starts a fresh session with no history, so continue has no effect.

### Repro

```sh
imp -p "Remember the word: elephant"
imp -p "What word did I tell you to remember?" -c
# Expected: mentions elephant (loaded from previous session)
# Actual: says it has no memory of any word
```

### Root Cause

`run_print_mode()` in `imp/crates/imp-cli/src/main.rs` (~line 1413) has no session handling at all — no `SessionManager`, no loading previous messages. The `-c` flag is only checked in the interactive TUI path (~line 1592).

### Fix

1. In `run_print_mode`, when `cli.cont` is true (or `cli.session` is set), load the session via `SessionManager`
2. Prepend loaded messages to the agent's conversation before calling `agent.run()`
3. Save the new messages back to the session file after the agent completes
4. When `cli.no_session` is true, skip all session handling (already the default behavior)

### Files
- `imp/crates/imp-cli/src/main.rs` — `run_print_mode()` function
- `imp/crates/imp-core/src/session.rs` — `SessionManager` API
