---
id: '23'
title: 'bug: mana status uses check_blocked without archive — shows archived deps as blocking'
slug: bug-mana-status-uses-checkblocked-without-archive
status: open
priority: 2
created_at: '2026-03-23T09:43:44.401287Z'
updated_at: '2026-03-23T09:43:44.401287Z'
labels:
- bug
- mana
verify: cd /Users/asher/tower && cargo test -p mana-core status_archived_dep 2>&1 | grep -E "[1-9][0-9]* passed"
fail_first: true
---

ops/status.rs line 50 calls check_blocked(entry, &index) which does NOT check the archive. ops/run.rs line 297 correctly calls check_blocked_with_archive(entry, &index, Some(&archive)). This means mana status shows units as blocked on archived deps even though mana run would correctly schedule them.

Fix: In ops/status.rs status_summary(), load the archive index and call check_blocked_with_archive instead of check_blocked. Same pattern as ops/run.rs build_queue().

Files: mana/crates/mana-core/src/ops/status.rs (line 50), mana/crates/mana-core/src/blocking.rs

Test: Create a unit with a dep on an archived unit, verify status_summary does not list it as blocked.
