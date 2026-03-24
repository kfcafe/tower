---
id: '29'
title: 'bug: 3 mana tool tests fail — agent mode restrictions reference wrong action names'
slug: bug-3-mana-tool-tests-fail-agent-mode-restrictions
status: closed
priority: 2
created_at: '2026-03-24T02:37:12.269041Z'
updated_at: '2026-03-24T02:43:14.557018Z'
labels:
- bug
- imp-core
- tests
closed_at: '2026-03-24T02:43:14.557018Z'
close_reason: 'Fixed: test helper run_with_mode() was hardcoding mode to AgentMode::Full instead of parsing the mode_name parameter'
verify: cd /Users/asher/tower && cargo test -p imp-core --lib tools::mana::tests 2>&1 | grep "test result" | grep -v FAILED
claimed_by: pi-agent
claimed_at: '2026-03-24T02:39:32.028936Z'
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-24T02:43:10.842766Z'
  finished_at: '2026-03-24T02:43:14.482717Z'
  duration_secs: 3.639
  result: pass
  exit_code: 0
outputs:
  text: 'test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 400 filtered out; finished in 0.02s'
attempt_log:
- num: 1
  outcome: success
  notes: 'Fixed: test helper run_with_mode() was hardcoding mode to AgentMode::Full instead of parsing the mode_name parameter'
  agent: pi-agent
  started_at: '2026-03-24T02:39:32.028936Z'
  finished_at: '2026-03-24T02:43:14.557018Z'
---

## Bug

Three tests in `imp/crates/imp-core/src/tools/mana.rs` fail:

```
tools::mana::tests::agent_mode_mana_auditor_blocks_update ... FAILED
tools::mana::tests::agent_mode_mana_planner_blocks_close ... FAILED
tools::mana::tests::agent_mode_mana_worker_blocks_create ... FAILED
```

These were added by another agent implementing agent modes (unit 22). The tests likely reference action names or mode restrictions that don't match the current mana tool implementation.

### Files
- `imp/crates/imp-core/src/tools/mana.rs` — test module

### Fix
Run the failing tests, read the error messages, and fix either the tests or the mode restriction logic to match.
