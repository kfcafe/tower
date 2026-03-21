---
id: '4'
title: 'Wire agent loop: hooks and context management'
slug: wire-agent-loop-hooks-and-context-management
status: closed
priority: 0
created_at: '2026-03-20T17:40:22.146765Z'
updated_at: '2026-03-21T07:40:58.919889Z'
closed_at: '2026-03-21T07:40:58.919889Z'
parent: '2'
verify: 'cd /Users/asher/tower && cargo test -p imp-core -- agent::tests::agent_fires_hooks agent::tests::agent_context_masking 2>&1 | grep -q "test result: ok. 2 passed"'
fail_first: true
checkpoint: '3418a0cc774ebcb6f18bd6607331f6e6a982501e'
claimed_by: pi-agent
claimed_at: '2026-03-21T07:22:29.986491Z'
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-21T07:40:58.922292Z'
  finished_at: '2026-03-21T07:40:59.734937Z'
  duration_secs: 0.812
  result: pass
  exit_code: 0
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-21T07:22:29.986491Z'
  finished_at: '2026-03-21T07:40:58.919889Z'
---

## Problem
The Agent struct has a `pub hooks: HookRunner` field, but `run()` never calls it.
The agent loop also never checks context budget or triggers observation masking / compaction.
This means:
- No hooks fire (after_file_write, before_tool_call, etc.)
- Conversations will hit the context window limit and degrade silently
- Observation masking and compaction never trigger

## What to implement

### 1. Fire hooks in the agent loop (`agent.rs`)

In `Agent::run()`, add hook firing at the right points:

**Before each LLM call** (after building context, before streaming):
```rust
self.hooks.fire(&HookEvent::BeforeLlmCall).await;
```

**After each tool execution** (in execute_one_tool, after getting result):
```rust
self.hooks.fire(&HookEvent::AfterToolCall {
    tool_name: tool_name,
    result: &result,
}).await;
```

**After file writes** (detect if tool was write/edit/multi_edit and file was modified):
```rust
if tool_name == "write" || tool_name == "edit" || tool_name == "multi_edit" {
    if let Some(path) = extract_file_path(&args) {
        self.hooks.fire(&HookEvent::AfterFileWrite { file: &path }).await;
    }
}
```

### 2. Context budget checking

In `Agent::run()`, before each LLM call, check context usage and react:

```rust
let usage = crate::context::context_usage(&self.messages, &self.model);

// Stage 1: Observation masking at 60%
if usage.ratio >= 0.6 {
    crate::context::mask_observations(&mut self.messages, 10);
    self.hooks.fire(&HookEvent::OnContextThreshold { ratio: usage.ratio }).await;
}

// Stage 2: Compaction at 80%
if usage.ratio >= 0.8 {
    self.emit(AgentEvent::CompactionStart).await;
    match crate::compaction::compact(&self.messages, &self.model, Default::default(), &self.api_key).await {
        Ok(result) => {
            // Replace old messages with summary
            // Keep messages from result.first_kept_id onward
            self.emit(AgentEvent::CompactionEnd { summary: result.summary.clone() }).await;
        }
        Err(e) => {
            self.emit(AgentEvent::Error { error: format!("Compaction failed: {e}") }).await;
        }
    }
}
```

### 3. Add tests

Add two new tests in agent.rs:

**agent_fires_hooks**: Create an agent with a hook registered (use callback HookAction).
Run agent. Verify the hook was called.

**agent_context_masking**: Create an agent with many messages that exceed 60% of a small
context window model. Verify observation masking runs.

## Key types
```rust
// From hooks.rs
pub struct HookRunner { ... }
impl HookRunner { pub async fn fire(&self, event: &HookEvent) -> Vec<HookResult> }

// From context.rs
pub fn context_usage(messages: &[Message], model: &Model) -> ContextUsage
pub fn mask_observations(messages: &mut [Message], keep_recent_turns: usize)

// From compaction.rs
pub async fn compact(messages, model, options, api_key) -> Result<CompactionResult>
```

## Files
- `imp/crates/imp-core/src/agent.rs` — MODIFY: add hook firing + context budget checks + tests
- `imp/crates/imp-core/src/hooks.rs` — READ: HookRunner, HookEvent, HookResult
- `imp/crates/imp-core/src/context.rs` — READ: context_usage, mask_observations
- `imp/crates/imp-core/src/compaction.rs` — READ: compact

## Do NOT
- Do not change HookRunner, HookEvent, or context management APIs
- Do not change existing tests — add new ones
