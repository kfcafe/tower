# Vision

## The Problem

AI agents today work in isolation. They receive a prompt, execute tools, produce output, and forget everything. When agents need to coordinate — decompose work, track dependencies, share knowledge, retry intelligently — they either can't, or they hack it through unstructured text in conversation history.

The missing piece isn't a better LLM or more tools. It's a **coordination substrate** — a shared, structured, verified medium that agents think through, plan with, and communicate over.

## The Metaphor

**Wizards orchestrate mana. Imps do the work.**

- **Mana** — the raw material. The work graph: verified units, dependencies, facts, memory.
- **Imp** — the worker. An agent that reads code, writes code, runs tests, follows instructions.
- **Wizard** — the orchestrator. Sees the full graph, dispatches imps, reacts to results, manages the plan.

You are the wizard. The terminal is your tower. You manage your mana pool, summon imps to work on tasks, and watch results flow back into the graph.

## The Architecture

Four projects, one ecosystem. All local-first, all Rust except the platform layer.

```
┌─────────────────────────────────────────────────────┐
│  wizard         Your interface + orchestrator        │
│                 `wiz` — TUI or headless daemon       │
│                 Manages mana, dispatches imps,       │
│                 hooks, scheduler, backpressure.      │
│                 Rust (ratatui + tokio)                │
├─────────────────────────────────────────────────────┤
│  imp            The agent engine                     │
│                 LLM streaming, tool execution,       │
│                 context management, sessions.         │
│                 Headless (dispatched by wizard)       │
│                 or interactive (summoned by wizard).  │
│                 Rust (uses rig-core for LLM)          │
├─────────────────────────────────────────────────────┤
│  mana           Coordination substrate               │
│                 Library-first. Work graph, deps,     │
│                 verification, facts, decomposition.  │
│                 Rust                                  │
├─────────────────────────────────────────────────────┤
│  .mana/         Filesystem (the universal interface) │
│                 Human-readable, git-friendly,        │
│                 survives any component dying.         │
└─────────────────────────────────────────────────────┘

  Familiar        Platform layer (future)
                  GitHub, Slack, dashboard, teams.
                  Phoenix/Elixir — where OTP shines.
```

### mana (Rust workspace: mana-core + mana CLI)

The coordination substrate. Everything agents plan, track, verify, and remember flows through mana.

A mana unit is a node in a verified work graph:
- **Title and description** — what needs to happen
- **Verify gate** — a shell command that must exit 0 to prove completion
- **Dependencies** (produces/requires) — contracts between units of work
- **History** — structured record of every attempt (timing, agent, tokens, cost, output)
- **Notes** — episodic memory that survives across agent sessions
- **Facts** — verified project knowledge with staleness detection
- **Parent/child hierarchy** — decomposition is first-class

Agents are both producers and consumers of mana. They create units, decompose them, wire dependencies, work on them, fail with useful notes, and close them when verify passes. The mana graph is how agents think about work, coordinate with each other, and remember what happened.

**Library-first.** The core is a Rust crate (`mana-core`) with no CLI dependencies. The CLI (`mana`) is a thin wrapper. Other consumers — imp (native Rust), wizard (native Rust), future tools — link the library directly.

**The `.mana/` directory is the universal interface.** YAML files with markdown frontmatter. Human-readable, human-editable, git-friendly. Every component reads and writes the same files. If any layer crashes, the state survives on disk.

### imp (Rust workspace: imp-core + imp binary)

The agent engine. Takes a task (or interactive prompt), connects to an LLM, executes tools, manages context, produces results.

- **Agent loop** — ReAct cycle (reason → act → observe → repeat)
- **Tool execution** — file ops, shell, search, code intelligence, MCP
- **Context management** — observation masking, compaction, token tracking
- **Session persistence** — conversation history, resume across sessions
- **Mana-native** — links mana-core directly. Creating, reading, and updating mana units are function calls, not CLI invocations.
- **LLM client** — via rig-core (20+ providers, streaming, tool calling)

Two modes:
- **Headless** — dispatched by wizard. Reads its brief from mana state, does its work, writes results back, runs verify, exits.
- **Interactive** — summoned through wizard's TUI. Conversational agent for working through a task with the human.

### wizard (Rust workspace: wizard-tui + wizard-orch + wiz binary)

The wizard's tower. Your interface to the entire system, plus the automated orchestrator that keeps imps working while you're away.

**wizard-tui** (ratatui):
- Mana graph visualization — what's ready, running, blocked, done
- Agent monitoring — stream output from running imps
- Interactive work — enter a mana unit, summon an imp to work on it with you
- Orchestration controls — dispatch, stop, retry, escalate
- Skills, prompt templates, slash commands

**wizard-orch** (tokio):
- **Dispatch** — computes ready units (deps satisfied, not claimed), spawns imp binaries
- **Supervision** — monitors agent processes, reacts to completion or failure
- **Backpressure** — concurrency limits, agents pull work at capacity
- **Hooks (orchestration-level)** — event-driven reactions across agents. "When a unit fails 3 times, escalate." "When all children pass, verify parent."
- **Scheduler** — pulse (periodic awareness) and cron (time-based dispatch)
- **Budget** — cost tracking, circuit breaker (N consecutive failures → stop dispatching)

**wiz binary**:
```
$ wiz                    Launch the TUI (starts daemon if not running)
$ wiz daemon             Headless orchestration only
$ wiz status             Quick status from any terminal
$ wiz dispatch           Kick off a dispatch wave manually
$ wiz stop               Halt all running imps
$ wiz logs 225.3.1       Stream logs from a specific imp
```

The TUI checks if the daemon is running on launch and starts it automatically. One command to enter your tower.

### Familiar (Phoenix app) — future

Platform layer for teams. GitHub integration, Slack, dashboard, isolated environments, action broker, permissions. Built on mana + wizard concepts. Elixir/OTP is the right choice here — hundreds of concurrent agents across teams, real-time dashboard over WebSocket, distributed state. Not in scope until the foundation is solid.

## Key Dependencies

- **rig-core** — Rust LLM client library. 20+ providers, streaming, tool calling. Eliminates building an LLM client from scratch.
- **ratatui** — Rust TUI framework for wizard-tui.
- **tokio** — Async runtime for wizard-orch (process supervision, scheduling, file watching).

## Design Principles

**Agents are both producers and consumers.** They create mana units, decompose them, work on them, and close them. The system is recursive — agents use mana to coordinate work that includes creating more mana.

**Library-first.** The CLI is a convenience, not the primary interface. The library is what agents, orchestrators, and platforms link against.

**Filesystem as the universal interface.** `.mana/` on disk is the source of truth. Components communicate through file state. This is durable (survives crashes), observable (git diff shows what changed), and language-agnostic.

**Verified, not trusted.** Work isn't done when an agent says it's done. It's done when the verify gate passes. This is the fundamental difference from every agent that relies on LLM self-assessment.

**Failure is data.** Every failed attempt is recorded with full context — what was tried, what went wrong, what the agent learned. The next agent reads this. Mistakes compound into knowledge, not wasted tokens.

**Right tool for each layer.** Rust for everything local (mana, imp, wizard) — instant startup, single binaries, native linking between crates. Elixir for the platform layer (Familiar) — where OTP's concurrent supervision of hundreds of agents across teams genuinely shines.

**Agent-agnostic orchestration.** Wizard doesn't know or care what's inside an imp. It manages the mana graph, spawns processes, monitors results. The agent could be imp, could be a shell script, could be a human running `mana close` manually. The protocol is the `.mana/` directory.

## Hooks: Two Layers

**imp-core hooks** — in-process, during agent execution. "After writing a file, run the formatter." "Before editing, check the file was read first." Fast, synchronous, scoped to a single agent session.

**wizard-orch hooks** — orchestration-level, across agents. "When a unit fails 3 times, bump priority." "When all children close, verify the parent." "Every 30 minutes, check on stale units." Reactive, asynchronous, scoped to the work graph.

Different triggers, different scope, no overlap.

## Configuration

**mana config** (`.mana/config.yaml`, per-project) — work graph settings. Max attempts, default priority, fact definitions, orchestration hooks. Checked into git.

**imp config** (`~/.config/imp/config.toml` + `.imp/config.toml`) — agent settings. Default model, provider keys, thinking level, tool preferences, skills, in-process hooks. Personal + per-project.

**wizard config** (`~/.config/wizard/config.toml`) — orchestration settings. Concurrency limits, dispatch strategy, scheduler (pulse/cron), budget caps, orchestration hooks.

They compose but don't merge. Mana doesn't care which model runs. Imp doesn't care about scheduling rules. Wizard doesn't care about tool preferences.

## Facts vs Skills

**Facts** (mana) — verified project knowledge stored in the work graph. "This project uses JWT for auth." "The test suite requires Docker running." Facts have staleness detection and are injected into agent context automatically when relevant. They live in `.mana/` and are shared across all agents working on the project.

**Skills** (imp-core) — agent instructions for how to do a type of work. "When writing Rust, follow these conventions." "When debugging, use this systematic approach." Skills are loaded by the agent runtime based on task matching. They live in the imp configuration.

Facts are *what's true*. Skills are *how to work*.

## Order of Work

1. **mana** — rename units, extract library, workspace split. Unblocks everything.
2. **imp** — clean room spec from pi, implement in Rust with rig-core. The agent engine.
3. **wizard** — TUI + orchestrator. The interface and automation layer.
4. **Familiar** — platform layer. When the foundation is solid.

## Prior Art and Influences

- **units** (this codebase) — the coordination concepts: verify gates, dependency graphs, failure accumulation, facts, decomposition. All carry forward into mana.
- **imp** (`~/imp`) — the Elixir agent engine. The orchestration patterns (GenStage pipeline, supervision, hooks, scheduler, budget controls) inform wizard. The agent design (effects-as-data, strategies, roles) informs imp.
- **pi** — the extensible coding agent. The UX patterns (skills, tools, prompt templates, extension model) inform imp and wizard-tui. Clean room spec, not a port.
- **rig** — Rust LLM library. The LLM plumbing layer for imp.
- **Familiar** (`~/familiar`) — the platform vision. Informs the long-term direction but not the immediate work.
- **Stripe Minions** — proved unattended agents at scale (1000+ PRs/week). The target operational model.
