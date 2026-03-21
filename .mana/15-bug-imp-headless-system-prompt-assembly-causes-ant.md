---
id: '15'
title: 'bug: imp headless system prompt assembly causes Anthropic 400'
slug: bug-imp-headless-system-prompt-assembly-causes-ant
status: open
priority: 1
created_at: '2026-03-21T17:18:04.072208Z'
updated_at: '2026-03-21T17:18:04.072208Z'
labels:
- imp
- bug
- headless
- anthropic
- llm
verify: cd wizard && ../target/debug/imp run 1.6 2>&1 | head -5 | rg -q "turn_start"
---

## Problem
When imp assembles its full system prompt in headless mode (imp run <unit-id>), the resulting Anthropic API request returns HTTP 400 invalid_request_error with the unhelpful message "Error". The same model and auth works fine in print mode (imp -p "hello").

## Reproduction
From wizard/:
  ../target/debug/imp run 1.2          # fails with 400
  ../target/debug/imp --system-prompt "" run 1.2   # works

## Evidence
- Print mode works (no tools, minimal system prompt)
- Headless with --system-prompt "" works (tools registered, empty prompt)
- Headless with assembled system prompt fails (tools + full prompt)
- The ask tool schema was one suspect but removing it did not fix the issue

## Likely causes
1. System prompt too large or containing characters Anthropic rejects
2. Interaction between large system prompt and tool definitions in the API request
3. Possible issue with AGENTS.md / skill content being assembled into the prompt

## Workaround
Using --system-prompt "" in the wizard mana runner config.

## Files
- imp/crates/imp-core/src/system_prompt.rs
- imp/crates/imp-core/src/resources.rs
- imp/crates/imp-llm/src/providers/anthropic.rs
- imp/crates/imp-cli/src/main.rs (headless prompt assembly)
