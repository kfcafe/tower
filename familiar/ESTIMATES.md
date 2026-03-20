# Code Estimates — imp + Familiar

Last updated: 2026-03-04

## Reference Codebases

| Project | Language | Lines of code |
|---|---|---|
| pi monorepo | TypeScript | ~110,000 |
| — pi-ai (LLM client) | | 21,000 |
| — pi-agent (agent loop) | | 1,000 |
| — pi-coding-agent (full agent) | | 29,000 |
| — pi-tui (terminal UI) | | 7,000 |
| — pi-web-ui | | 12,000 |
| — pi-mom + pods | | 5,000 |
| beans | Rust | 30,000 |
| imp (current) | Elixir | 3,400 |

Elixir is roughly 2-3x more concise than TypeScript and Rust.

---

## imp Estimate (all 5 phases)

### imp_llm — 3,000-4,000 lines

| Component | Lines | Notes |
|---|---|---|
| Anthropic provider | 500 | Current is close to this |
| OpenAI provider | 500 | |
| Google provider | 500 | |
| SSE + types + model registry | 700 | |
| OAuth, retry, token tracking | 500 | |
| Additional providers | 500 | Bedrock, Azure, OpenRouter, etc. |

### imp_core — 9,000-12,000 lines

| Component | Lines | Notes |
|---|---|---|
| Agent loop + supervision | 500 | OTP process, selective receive, events |
| Core tools (bash, read, write, edit) | 800 | Edge cases, truncation, streaming |
| Search tools (grep, find, ls) | 400 | Wrappers around rg, fd |
| Code intel (ast_grep, probe, scan) | 600 | |
| Web tools (search, read) | 400 | |
| Browser + docs + testing tools | 1,000 | Phase 3-4 tools |
| Tool system (groups, validation, truncation) | 500 | |
| MCP client (stdio + HTTP, discovery) | 800 | |
| Tasks (CRUD, verify, failures, deps, graph) | 1,500 | The beans concepts |
| Sub-agent dispatch (GenStage, supervision) | 800 | |
| HTTP template + Lua sandbox | 800 | Self-building tools |
| Credential broker | 400 | |
| Permission gate | 200 | Thin in CLI mode |
| Context management + sessions | 900 | Compaction, persistence, branching |
| ETS cache, config, logging, prompts | 700 | |

### imp_cli — 1,000-1,500 lines

| Component | Lines | Notes |
|---|---|---|
| Readline, ANSI, streaming | 500 | No custom TUI framework needed |
| Slash commands, sessions, model cycling | 300 | |
| Login, pipe mode, settings | 300 | |

### Tests — 3,000-5,000 lines

### imp total: ~16,000-22,000 lines

---

## Familiar Estimate (through v0.3)

### Phoenix core — 1,500 lines

| Component | Lines | Notes |
|---|---|---|
| Boilerplate, router, endpoint | 500 | |
| Auth (GitHub OAuth) | 400 | |
| Ecto schemas + migrations | 600 | |

### Task layer — 1,500 lines

| Component | Lines | Notes |
|---|---|---|
| Postgres-backed task storage | 500 | Extends imp's task callbacks |
| Agent dispatch + scheduling | 600 | Wraps imp's GenStage dispatch |
| PR creation on verify pass | 400 | |

### Integrations — 3,000 lines

| Component | Lines | Notes |
|---|---|---|
| GitHub App (OAuth, webhooks, clone, PR) | 1,200 | Fiddly API surface |
| Slack App (bot, threads, notifications) | 1,000 | |
| Fly Sprite management (lifecycle, snapshots) | 800 | |

### Platform services — 2,500 lines

| Component | Lines | Notes |
|---|---|---|
| Credential broker extensions (Nango) | 600 | |
| Permission extensions (4 levels, persistent) | 500 | |
| LLM Gateway (proxy, metering, caps, BYOK) | 600 | |
| Agent Manager (supervise, monitor, kill) | 500 | |
| Audit trail + memory system | 300 | |

### LiveView dashboard — 3,000 lines

| Component | Lines | Notes |
|---|---|---|
| Tasks + task detail | 800 | |
| Agent activity (live streaming) | 400 | |
| Repos + environments | 300 | |
| Services + tools + permissions | 600 | |
| Team + settings + audit trail | 500 | |
| Kill switch + approval flow | 200 | |
| Layout, components, shared | 200 | |

### Background jobs (Oban) — 400 lines

### Tests — 2,000-3,000 lines

### Familiar total: ~14,000-18,000 lines

---

## Summary

| Project | Lines of Elixir | Equivalent TS/Rust |
|---|---|---|
| imp | 16,000-22,000 | ~45,000-65,000 |
| Familiar | 14,000-18,000 | ~40,000-55,000 |
| **Combined** | **30,000-40,000** | **~85,000-120,000** |

For context: pi's monorepo is 110k TypeScript. beans is 30k Rust. The combined imp + Familiar is comparable in functionality to both at roughly a third the line count thanks to Elixir's conciseness and the BEAM providing supervision, concurrency, and LiveView instead of building them from scratch.

imp is the bigger effort despite fewer lines — the LLM client, tool system, MCP client, tasks + sub-agent dispatch, and GenStage pipeline are all load-bearing infrastructure. Familiar is mostly wiring imp to external services (GitHub, Slack, Nango, Fly) and building the dashboard on top.
