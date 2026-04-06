---
id: '43'
title: Fix mana orchestration desync bugs
slug: fix-mana-orchestration-desync-bugs
status: closed
priority: 2
created_at: '2026-04-06T13:50:09.795502Z'
updated_at: '2026-04-06T14:34:19.132800Z'
labels:
- feature
- bug
closed_at: '2026-04-06T14:34:19.132800Z'
close_reason: 'Auto-closed: all children completed'
is_archived: true
kind: epic
---

## Overview

During real mana orchestration sessions, five related bugs cause unit index/file desync, lost run state, and unreliable addressing. This feature tracks all five fixes.

See: mana/docs/bugs/native-run-state-and-index-desync.md

## Root causes
1. No index locking in create/update/close — LockedIndex infra exists but is unused
2. Native run state in-memory only — lost on restart
3. get_unit() uses file glob not index — fails after archive
4. Feature/parent units surfaced as ready in plan_dispatch
5. Archive race during close — file moved before index rebuilt

## Acceptance
- Concurrent create/update/close operations don't lose index entries
- Run IDs survive session restarts for at least 1 hour
- `show`/`close` work on recently-archived units
- `mana run` / `next` don't surface parent feature units
- `mana doctor` detects and repairs index/file mismatches
