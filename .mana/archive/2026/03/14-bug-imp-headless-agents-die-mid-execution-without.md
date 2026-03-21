---
id: '14'
title: 'bug: imp headless agents die mid-execution without error output'
slug: bug-imp-headless-agents-die-mid-execution-without
status: closed
priority: 0
created_at: '2026-03-21T17:17:49.528407Z'
updated_at: '2026-03-21T20:12:12.497300Z'
labels:
- imp
- bug
- headless
- reliability
closed_at: '2026-03-21T20:12:12.497300Z'
close_reason: Fixed via HTTP client timeouts + retry logic + template timeout enforcement
verify: cd wizard && ../target/debug/imp --system-prompt "" run 1.6 2>&1 | tail -5 | rg -q "agent_end"
is_archived: true
---

## Problem
Headless imp agents (imp run <unit-id>) consistently die after ~5 tool calls on larger Wizard units. No error output, no agent_end event, no clean exit — the process just vanishes.

## Reproduction
From wizard/:
  ../target/debug/imp --system-prompt "" run 1.6
Watch the JSON-lines output. After ~5 turns the process dies silently.

## Evidence
- Units 1.1-1.5 (small scope) completed successfully
- Unit 1.8 (runtime monitoring, medium scope) completed on second try
- Units 1.6, 1.3, 1.5 died repeatedly during parallel runs (Cargo lock contention)
- Unit 1.6 dies even when run alone after 1.8 landed

## Likely causes
1. wizard-orch now has background threads (RuntimeSupervisor, notify watcher) that may interfere with process lifecycle
2. Cargo build lock contention when multiple agents compile simultaneously
3. Possible silent panic or signal handling issue in imp headless mode
4. No attempt/note logging back to mana on agent death

## Files
- imp/crates/imp-cli/src/main.rs (headless entry point)
- imp/crates/imp-core/src/agent.rs (agent loop)
- wizard/crates/wizard-orch/src/lib.rs (background threads)
