---
id: '2'
title: imp v0.1 — End-to-end functional agent
slug: imp-v01-end-to-end-functional-agent
status: closed
priority: 0
created_at: '2026-03-20T17:39:27.026670Z'
updated_at: '2026-03-21T07:53:31.464661Z'
closed_at: '2026-03-21T07:53:31.464661Z'
close_reason: 'Auto-closed: all children completed'
verify: 'cd /Users/asher/tower && cargo test -p imp-llm -p imp-core 2>&1 | tail -5 | grep -q "test result: ok" && cargo build -p imp-cli 2>&1 | grep -q Finished'
is_archived: true
---

Parent unit for making imp a functional coding agent. Children handle each piece.
