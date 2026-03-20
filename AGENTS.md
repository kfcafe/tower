# AGENTS.md — Tower Root

Tower is the umbrella development root for the mana ecosystem.

This file exists to help agents understand the entire system from one place.

## Read first

Before making structural or cross-project changes, read:
1. `README.md`
2. `VISION.md`
3. `UMBRELLA.md`
4. project-specific specs when relevant (`wizard/SPEC.md`, `wizard/*ARCHITECTURE*.md`, etc.)

## Project map

### `mana/`
The coordination substrate.

Owns:
- unit model
- dependency graph
- facts
- verify gates
- planning and orchestration substrate

When in doubt, `mana/` is the source of truth for how work is represented.

### `imp/`
The worker engine.

Owns:
- agent loop
- tools
- sessions
- context assembly
- model/provider integration

When in doubt, `imp/` is the source of truth for how one worker executes one task.

### `wizard/`
The command center.

Owns:
- canvas UI
- orchestration daemon
- runtime monitoring
- built-in editor, terminal, and browser integration
- room/focus workflows

When in doubt, `wizard/` is the source of truth for how humans supervise and navigate agentic work.

### `familiar/`
The future team platform.

Owns:
- multi-user workflows
- approvals
- integrations
- dashboard/platform concerns

When in doubt, `familiar/` is where the local-first single-user system grows into a team system.

## Cross-project relationships

Read this as the core dependency map:

- `mana` defines the work graph and durable project memory
- `imp` consumes mana state to execute work
- `wizard` visualizes and orchestrates work that lives in mana and is executed by imp
- `familiar` extends the same model to team workflows and remote operations

### Simplified stack

```text
familiar   → team platform
wizard     → command center / bigger IDE
imp        → worker / agent runtime
mana       → coordination substrate / source of truth
```

## Planning rules

### Use root `.mana/` for:
- migration tasks
- cross-project features
- interface contracts between projects
- ecosystem architecture work

### Use project-local `.mana/` for:
- project-specific bugs
- project-specific features
- internal refactors
- local cleanup

## Navigation rules for agents

1. Start from the Tower root when the task mentions more than one project.
2. Use root docs to understand ownership before changing code.
3. Prefer sibling awareness over local optimization — ask "which project should own this?"
4. If a change affects shared contracts, inspect both producer and consumer projects.
5. If work spans `mana` and `imp`, or `mana` and `wizard`, consider creating or updating a root `.mana/` unit.

## Rust workspace

The Tower root `Cargo.toml` is the umbrella workspace.

Current workspace members:
- `mana/`
- `mana/crates/mana-core`
- `imp/crates/imp-llm`
- `imp/crates/imp-core`
- `imp/crates/imp-lua`
- `imp/crates/imp-tui`
- `imp/crates/imp-cli`
- `wizard/crates/wizard-proto`
- `wizard/crates/wizard-store`
- `wizard/crates/wizard-terminal`
- `wizard/crates/wizard-browser`
- `wizard/crates/wizard-orch`

`wizard/` now has initial crate scaffolding, but the desktop app itself is still architecture-first and early.

## Current migration status

Tower is implemented as a **copy-first umbrella root**.

That means:
- treat `~/tower` as the primary working root for agents
- old directories outside Tower still exist as safety
- do not assume every legacy path has been retired yet

## Local-first rule

- `.mana/` = canonical shared work state
- `.wizard/` = local Wizard UI state
- docs explain intent; code and `.mana/` define reality

## Default behavior

When uncertain:
- prefer small, structural changes
- preserve project boundaries
- document cross-project reasoning in the root `.mana/`
- explain why a change belongs in one project and not another
