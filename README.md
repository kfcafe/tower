# Tower

Tower is the umbrella development root for the mana ecosystem.

It exists for one reason:

> give humans and agents one coherent place to understand, plan, and evolve the entire system.

## Projects

- `mana/` — coordination substrate and CLI
- `imp/` — worker/agent engine
- `wizard/` — canvas-native command center, desktop client, and orchestrator
- `familiar/` — future team platform

## Why this root exists

Before Tower, the ecosystem lived in separate directories:
- `~/mana`
- `~/imp`
- `~/familiar`
- `~/mana/wizard`

That made it harder for agents to:
- discover the whole architecture from one root
- follow cross-project relationships
- plan work that spans multiple projects
- search code and specs across the ecosystem
- understand which project owns what

Tower fixes that by giving agents a single working root with:
- root architecture docs
- a root `.mana/` for cross-project planning
- sibling project folders
- a shared Rust workspace for `mana`, `imp`, and the initial `wizard` crates

## Architectural split

The system has a deliberate four-part split:

### `mana/`
Owns durable work state:
- units
- dependencies
- facts
- attempts
- verify history
- artifacts

### `imp/`
Owns execution:
- agent loop
- tools
- context management
- sessions
- LLM integration

### `wizard/`
Owns supervision and interface:
- canvas-native UI
- orchestration
- runtime monitoring
- editor/terminal/browser integration
- agent command center workflows

### `familiar/`
Owns team/platform concerns:
- approvals
- shared operations
- remote orchestration
- dashboard and integrations

## Canonical root docs

Start here when working in Tower:
- `VISION.md` — ecosystem vision
- `UMBRELLA.md` — migration and structure spec
- `AGENTS.md` — root working instructions for agents
- `wizard/SPEC.md` — product spec for Wizard
- `wizard/FRONTEND_ARCHITECTURE.md`
- `wizard/BACKEND_ARCHITECTURE.md`
- `wizard/FULLSTACK_ARCHITECTURE.md`

## Planning

Tower has two planning layers:

### Root `.mana/`
Use for:
- cross-project work
- migration tasks
- interface contracts
- ecosystem milestones

### Per-project `.mana/`
Use for:
- project-local bugs and features
- internal refactors
- project-scoped docs and tests

## Build and verification

### Rust workspace
From the Tower root:

```bash
cargo metadata
cargo check -p mana-cli -p imp-core
cargo test -p mana-cli
```

### Familiar
Familiar stays a separate Elixir project inside the same root.

```bash
cd familiar && mix compile
```

## Current state

Tower is currently a **copy-first umbrella root**.

That means:
- this directory is now the best place for agents to work from
- the old project directories still exist outside Tower as safety/rollback
- migration is partially complete, not fully retired everywhere yet

## Working rule

If work spans multiple projects, do it from the Tower root.

If work is local to one project, work inside that project folder — but keep Tower as the main mental model.
