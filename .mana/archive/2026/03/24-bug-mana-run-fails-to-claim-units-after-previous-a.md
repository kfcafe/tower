---
id: '24'
title: 'bug: mana run fails to claim units after previous agent timeout'
slug: bug-mana-run-fails-to-claim-units-after-previous-a
status: closed
priority: 2
created_at: '2026-03-23T18:42:41.530486Z'
updated_at: '2026-03-23T19:40:51.515618Z'
labels:
- bug
- mana
closed_at: '2026-03-23T19:40:51.515618Z'
verify: cd /Users/asher/tower && cargo test -p mana-core claim 2>&1 | grep -v "0 passed" | grep "passed"
claimed_by: pi-agent
claimed_at: '2026-03-23T19:33:47.316658Z'
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-23T19:40:36.874289Z'
  finished_at: '2026-03-23T19:40:51.470964Z'
  duration_secs: 14.596
  result: pass
  exit_code: 0
outputs:
  text: |-
    test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 647 filtered out; finished in 0.29s
    test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 32 filtered out; finished in 0.25s
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-23T19:33:47.316658Z'
  finished_at: '2026-03-23T19:40:51.515618Z'
---

When a unit's agent times out or fails, the unit stays in in_progress status. Subsequent mana run dispatches fail with "Failed to claim unit (already claimed or verify passed)" even though verify does not pass.

Observed during imp-core hardening (units 16.4, 16.7, 16.8):
1. First run: agents idle-timed out or hit timeout, unit status left as in_progress
2. Second run: new batch tried to claim units, got "Failed to claim unit"
3. Had to manually run mana update <id> --status open to unblock

Expected: When an agent times out or fails, the unit should be automatically reset to open so subsequent runs can claim it without manual intervention.

Root cause is likely in mana_run failure handling — it records the failure in notes but does not reset the unit status from in_progress back to open.
