---
id: '30'
title: 'bug: broken mana-core build — post_plan field removed from Config but references remain'
slug: bug-broken-mana-core-build-postplan-field-removed
status: closed
priority: 1
created_at: '2026-03-24T02:37:27.198043Z'
updated_at: '2026-03-24T02:38:25.239195Z'
labels:
- bug
- mana
closed_at: '2026-03-24T02:38:25.239195Z'
verify: cd /Users/asher/tower && cargo build -p mana-cli 2>&1 | grep -c "error\[" | grep "^0$"
claimed_by: pi-agent
claimed_at: '2026-03-24T02:37:33.064910Z'
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-24T02:38:23.688734Z'
  finished_at: '2026-03-24T02:38:25.187428Z'
  duration_secs: 1.498
  result: pass
  exit_code: 0
outputs: 0
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-24T02:37:33.064910Z'
  finished_at: '2026-03-24T02:38:25.239195Z'
---

## Bug

Another agent partially removed the `post_plan` field from `mana_core::Config` struct but left references in:
- `mana/crates/mana-core/src/config.rs` (Default impl, merge logic)
- `mana/crates/mana-core/src/ops/config_cmd.rs` (get/set handlers)
- `mana/crates/mana-core/src/ops/init.rs` (initial config creation)
- `mana/crates/mana-cli/src/main.rs` (PlanArgs struct usage)
- `mana/crates/mana-cli/src/commands/plan.rs` (references to removed functions)
- `mana/crates/mana-cli/src/commands/run/mod.rs` (auto_plan field)

The changes are currently stashed (`git stash list` — "other agent mana changes") but some leaked through. Running `cargo build -p mana-cli` produces compile errors.

### Fix options
1. **Complete the removal**: remove all remaining `post_plan` references
2. **Revert the removal**: add `post_plan` back to the Config struct
3. **Cherry-pick clean**: `git checkout HEAD -- mana/` to restore to last good state, then re-apply only the valid parts

Option 3 is safest. Start with `git stash drop` for the broken stash, then `git checkout HEAD -- mana/` to get clean state. If mana-cli still doesn't build, trace the compile errors and fix them.

### Files
- `mana/crates/mana-core/src/config.rs`
- `mana/crates/mana-core/src/ops/config_cmd.rs`
- `mana/crates/mana-core/src/ops/init.rs`
- `mana/crates/mana-cli/src/main.rs`
- `mana/crates/mana-cli/src/commands/plan.rs`
- `mana/crates/mana-cli/src/commands/run/mod.rs`
