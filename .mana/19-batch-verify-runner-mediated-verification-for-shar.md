---
id: '19'
title: 'Batch verify: runner-mediated verification for shared commands'
slug: batch-verify-runner-mediated-verification-for-shar
status: open
priority: 1
created_at: '2026-03-22T17:05:31.396246Z'
updated_at: '2026-03-22T17:05:31.396246Z'
labels:
- feature
- mana
- performance
verify: cargo check -p mana-core -p mana-cli && cargo check -p imp-cli
---

## Summary

When multiple agents run in parallel and share the same verify command (e.g. cargo build), each agent independently runs verify multiple times — causing cargo lock contention, redundant builds, and wasted wall-clock time.

This feature introduces **runner-mediated batch verification**: agents signal completion without running verify themselves, and the runner batches shared verify commands so each unique command runs exactly once.

## Design

### Two-phase close

- **Agent context** (MANA_BATCH_VERIFY=1 env var): mana close marks the unit as AwaitingVerify instead of running verify inline. Agent exits immediately.
- **Runner context**: After agents complete, runner collects AwaitingVerify units, groups by verify command string, runs each unique command once, and applies pass/fail to all units sharing that command. Passing units get finalized as Closed. Failing units get re-opened for re-dispatch.
- **Manual context** (human runs mana close): Unchanged — verify runs inline as today.

### Opt-in via config

New config field batch_verify: bool (default: false). When enabled, mana run sets MANA_BATCH_VERIFY=1 on spawned agent processes, and runs batch verification after agent completion.

### Agent feedback

Agents still see the verify command in their prompt (for understanding success criteria) and are encouraged to use scoped checks during work. The full verify is deferred to the runner.

### imp integration

When MANA_BATCH_VERIFY=1 is set, imp run_headless_mode skips its post-agent verify and exits 0 after the agent loop completes.
