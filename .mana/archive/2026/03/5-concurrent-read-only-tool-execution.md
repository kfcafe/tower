---
id: '5'
title: Concurrent read-only tool execution
slug: concurrent-read-only-tool-execution
status: closed
priority: 1
created_at: '2026-03-20T17:40:38.288881Z'
updated_at: '2026-03-21T07:39:32.768628Z'
notes: |-
  ---
  2026-03-21T07:23:39.580017+00:00
  Verified gate fails as expected. Reading agent/tool code now to implement concurrent read-only execution with result reordering and targeted test coverage.

  ---
  2026-03-21T07:40:56+00:00
  Discoveries: agent.rs already has sibling-unit hook/compaction work in flight, so surgical edits are safest. The new readonly test uses a shared start barrier + timeout so sequential execution deadlocks and concurrent execution passes while still asserting result order for interleaved readonly/mutable calls. Also, this verify path compiles imp-core unit tests broadly enough that a stale Lua integration test import in src/tools/lua.rs can block unrelated agent work.
closed_at: '2026-03-21T07:39:32.768628Z'
parent: '2'
verify: 'cd /Users/asher/tower && cargo test -p imp-core -- agent::tests::agent_concurrent_readonly 2>&1 | grep -q "test result: ok. 1 passed"'
fail_first: true
checkpoint: '3418a0cc774ebcb6f18bd6607331f6e6a982501e'
claimed_by: pi-agent
claimed_at: '2026-03-21T07:22:29.738347Z'
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-21T07:39:32.771079Z'
  finished_at: '2026-03-21T07:39:33.478609Z'
  duration_secs: 0.707
  result: pass
  exit_code: 0
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-21T07:22:29.738347Z'
  finished_at: '2026-03-21T07:39:32.768628Z'
---

## Problem
In `agent.rs` the `execute_tools()` method runs ALL tools sequentially, with a comment:
"Read-only tools run sequentially for now (concurrent TODO)".
The spec says read-only tools should run concurrently for performance.

## What to implement

Change `execute_tools()` in `imp/crates/imp-core/src/agent.rs`:

1. Partition calls into readonly vs mutable (already done)
2. Run readonly tools concurrently using `futures::future::join_all` or `FuturesUnordered`
3. Run mutable tools sequentially after all readonly complete
4. Preserve result ordering to match original call order

```rust
async fn execute_tools(&self, calls: Vec<(String, String, serde_json::Value)>) -> Vec<ToolResultMessage> {
    let (readonly, mutable): (Vec<_>, Vec<_>) = calls.into_iter()
        .partition(|(_, name, _)| self.tools.get(name).is_some_and(|t| t.is_readonly()));

    let mut results = Vec::new();

    // Read-only tools concurrently
    let futures: Vec<_> = readonly.into_iter()
        .map(|(id, name, args)| self.execute_one_tool(&id, &name, args))
        .collect();
    let concurrent_results = futures::future::join_all(futures).await;
    results.extend(concurrent_results);

    // Mutable tools sequentially
    for (id, name, args) in mutable {
        results.push(self.execute_one_tool(&id, &name, args).await);
    }

    results
}
```

Note: `execute_one_tool` takes `&self` not `&mut self`, so concurrent execution is fine.

## Test
Add `agent_concurrent_readonly` test:
- Register 3 read-only echo tools and 1 mutable tool
- Mock provider returns all 4 as tool calls in one message
- Verify all execute and results come back
- Verify mutable tool runs after readonly tools

## Files
- `imp/crates/imp-core/src/agent.rs` — MODIFY: execute_tools + add test

## Do NOT
- Do not change execute_one_tool signature
- Do not add concurrency limits (bounded by LLM batch size)
