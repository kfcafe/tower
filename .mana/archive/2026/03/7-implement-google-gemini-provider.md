---
id: '7'
title: Implement Google Gemini provider
slug: implement-google-gemini-provider
status: closed
priority: 1
created_at: '2026-03-20T17:41:22.753311Z'
updated_at: '2026-03-21T07:36:24.094195Z'
notes: |-
  ---
  2026-03-21T07:26:46.633151+00:00
  Started implementation. Verified gate fails. Reading anthropic/openai provider patterns and Gemini API docs to mirror request building + SSE state handling.
closed_at: '2026-03-21T07:36:24.094195Z'
parent: '2'
verify: 'cd /Users/asher/tower && cargo test -p imp-llm -- providers::google::tests 2>&1 | grep -q "test result: ok" && ! grep -q "TODO" imp/crates/imp-llm/src/providers/google.rs'
fail_first: true
checkpoint: '3418a0cc774ebcb6f18bd6607331f6e6a982501e'
claimed_by: pi-agent
claimed_at: '2026-03-21T07:22:29.072301Z'
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-21T07:36:24.097623Z'
  finished_at: '2026-03-21T07:36:36.532090Z'
  duration_secs: 12.434
  result: pass
  exit_code: 0
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-21T07:22:29.072301Z'
  finished_at: '2026-03-21T07:36:24.094195Z'
---

## Problem
The Google provider in `imp/crates/imp-llm/src/providers/google.rs` returns an empty stream.
It needs full SSE streaming, tool calling, and thinking support.

## What to implement

Follow the Anthropic provider pattern in `providers/anthropic.rs` (1400+ lines, well-tested).

### 1. Wire-format types
Define request/response types matching the Gemini API:
- API endpoint: `https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent?alt=sse&key={api_key}`
- Request body: `contents`, `tools`, `systemInstruction`, `generationConfig`
- Content format: `contents[].parts[].text`, `functionCall`, `functionResponse`
- Thinking: `generationConfig.thinkingConfig.thinkingBudget`

### 2. Request building
- `build_request()`: convert unified Message types to Gemini format
- Map `Message::User` → `role: "user"`, `Message::Assistant` → `role: "model"`
- Map `ToolCall` → `functionCall { name, args }`
- Map `ToolResult` → `functionResponse { name, response }`
- Map `ThinkingLevel` → `thinkingBudget` (similar budget mapping as Anthropic)
- System prompt goes in `systemInstruction.parts[].text`

### 3. SSE streaming
- Send POST with `alt=sse` query param
- Parse SSE events (data: lines with JSON)
- Each event is a complete `GenerateContentResponse` with `candidates[0].content.parts[]`
- Emit `StreamEvent::TextDelta` for text parts
- Emit `StreamEvent::ThinkingDelta` for thought parts
- Emit `StreamEvent::ToolCall` for function_call parts (may need accumulation)
- Track usage from `usageMetadata` in response

### 4. Model registry
Update `builtin_models()` — already has gemini-2.5-pro and gemini-2.5-flash, verify pricing is current.

### 5. Tests
Add snapshot tests for:
- Request serialization (unified types → Gemini wire format)
- SSE event parsing (canned SSE → StreamEvent sequence)
- Tool call handling
- Thinking/reasoning mapping

## Key reference
Study `providers/anthropic.rs` carefully — it has the exact same structure:
- Wire format types at top
- `build_request()` function
- `impl Provider` with `stream()` that does HTTP + SSE parsing
- State machine for accumulating blocks
- Tests at bottom

## Files
- `imp/crates/imp-llm/src/providers/google.rs` — MODIFY: full implementation
- `imp/crates/imp-llm/src/providers/anthropic.rs` — READ: reference implementation
- `imp/crates/imp-llm/src/provider.rs` — READ: Provider trait, types
- `imp/crates/imp-llm/src/message.rs` — READ: unified message types
- `imp/crates/imp-llm/src/stream.rs` — READ: StreamEvent types

## Do NOT
- Do not change the Provider trait or unified message types
- Do not add image support yet — text + tools + thinking first
- Do not add retry logic — the Provider trait handles that at a higher level
