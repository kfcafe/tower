---
id: '24'
title: 'bug: mana run fails to claim units after previous agent timeout'
slug: bug-mana-run-fails-to-claim-units-after-previous-a
status: open
priority: 2
created_at: '2026-03-23T18:42:41.530486Z'
updated_at: '2026-03-23T18:42:41.530486Z'
labels:
- bug
- mana
verify: cd /Users/asher/tower && cargo test -p mana-core claim 2>&1 | grep -v "0 passed" | grep "passed"
---

When a unit's agent times out or fails, the unit stays in in_progress status. Subsequent mana run dispatches fail with "Failed to claim unit (already claimed or verify passed)" even though verify does not pass.

Observed during imp-core hardening (units 16.4, 16.7, 16.8):
1. First run: agents idle-timed out or hit timeout, unit status left as in_progress
2. Second run: new batch tried to claim units, got "Failed to claim unit"
3. Had to manually run mana update <id> --status open to unblock

Expected: When an agent times out or fails, the unit should be automatically reset to open so subsequent runs can claim it without manual intervention.

Root cause is likely in mana_run failure handling — it records the failure in notes but does not reset the unit status from in_progress back to open.
