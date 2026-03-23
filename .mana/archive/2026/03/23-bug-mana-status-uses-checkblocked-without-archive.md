---
id: '23'
title: 'bug: mana status uses check_blocked without archive — shows archived deps as blocking'
slug: bug-mana-status-uses-checkblocked-without-archive
status: closed
priority: 2
created_at: '2026-03-23T09:43:44.401287Z'
updated_at: '2026-03-23T15:38:28.575955Z'
labels:
- bug
- mana
closed_at: '2026-03-23T15:38:28.575955Z'
verify: cd /Users/asher/tower && cargo test -p mana-core status_archived_dep 2>&1 | grep -E "[1-9][0-9]* passed"
fail_first: true
checkpoint: '976b39c612858ccaa220a598aa3fddb417299153'
claimed_by: pi-agent
claimed_at: '2026-03-23T15:23:34.434158Z'
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-23T15:38:11.734059Z'
  finished_at: '2026-03-23T15:38:28.488784Z'
  duration_secs: 16.754
  result: pass
  exit_code: 0
outputs:
  text: 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 670 filtered out; finished in 0.01s'
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-23T15:23:34.434158Z'
  finished_at: '2026-03-23T15:38:28.575955Z'
---

ops/status.rs line 50 calls check_blocked(entry, &index) which does NOT check the archive. ops/run.rs line 297 correctly calls check_blocked_with_archive(entry, &index, Some(&archive)). This means mana status shows units as blocked on archived deps even though mana run would correctly schedule them.

Fix: In ops/status.rs status_summary(), load the archive index and call check_blocked_with_archive instead of check_blocked. Same pattern as ops/run.rs build_queue().

Files: mana/crates/mana-core/src/ops/status.rs (line 50), mana/crates/mana-core/src/blocking.rs

Test: Create a unit with a dep on an archived unit, verify status_summary does not list it as blocked.
