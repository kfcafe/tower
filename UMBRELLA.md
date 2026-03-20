# The Tower — Ecosystem Root

Status: Draft 0.2  
Placement: now implemented as a copy-first umbrella root at `~/tower`. This doc defines the target structure and remaining migration work for the shared umbrella.

## 1. What This Is

Four projects form one ecosystem:

| Project | Role | Language | Current location | Status |
|---|---|---|---|---|
| **mana** | Coordination substrate | Rust | `~/tower/mana` | Copied into umbrella root |
| **imp** | Worker engine | Rust | `~/tower/imp` | Copied into umbrella root |
| **wizard** | Agent command center + orchestrator | Rust + web | `~/tower/wizard` | Copied into umbrella root |
| **familiar** | Team platform | Elixir | `~/tower/familiar` | Copied into umbrella root |

They originally lived in separate directories. A copy-first umbrella root now exists at `~/tower` so agents can work against the ecosystem from one place.

This spec defines both the target structure and the remaining work to complete the migration.

### Architectural position

The ecosystem has a deliberate split:

1. **mana** owns durable work state — units, deps, facts, verify history, attempts, artifacts.
2. **imp** owns execution — one worker takes one unit, uses tools, edits code, verifies, and emits events.
3. **wizard** owns supervision — the agent command center or "bigger IDE" where the primary objects are units, agents, and artifacts, not files.
4. **familiar** extends the same system to teams, approvals, remote orchestration, and shared operations.

There is also a cross-cutting **workflow/methodology layer** — planning, debugging, TDD, review, branch-finish policy, and similar engineering practices. That layer should live mostly as skills, playbooks, policies, and durable artifacts on top of the runtime, not as hardcoded behavior inside `imp`.

## 2. Why Consolidate

### Problems with the current layout

1. **No shared planning surface.** Cross-project work ("imp needs mana-core's public API to stabilize") has nowhere to live.
2. **No cross-project dependencies in code.** imp-core needs to link mana-core. wizard-orch needs mana-core. Today these are separate repos with no path linkage.
3. **Scattered specs.** vision.md is in mana. imp_core_plan.md is in imp. wizard/SPEC.md is in mana. familiar/plan.md is in familiar. There is no single place to see the whole picture.
4. **Independent git histories make coordinated changes hard.** A breaking change in mana-core that affects both imp and wizard requires syncing across three repos.
5. **Context switching.** Moving between `cd ~/mana`, `cd ~/imp`, and `cd ~/familiar` loses spatial and mental context.

### What consolidation gives us

1. One root `.mana/` for ecosystem-level planning and cross-project units.
2. Cargo path dependencies between Rust projects during development.
3. One place for shared specs, vision docs, and architecture decisions.
4. Coordinated commits when a change spans projects.
5. Per-project `.mana/` still works for project-scoped work.
6. One directory to open in an editor or canvas.

## 3. The Name

The metaphor from vision.md:

> You are the wizard. The terminal is your tower.

The umbrella root is **the tower** — the place that contains all the instruments.

```
tower/           # umbrella root
  mana/          # coordination substrate
  imp/           # agent engine
  wizard/        # interface + orchestrator
  familiar/      # team platform
```

The name is lightweight. It can change. What matters is the structure.

## 4. Target Directory Layout

```
tower/
├── .mana/                  # Ecosystem-level planning (cross-project units, roadmap)
├── .gitignore
├── Cargo.toml              # Virtual workspace root for all Rust crates
├── VISION.md               # Canonical ecosystem vision (promoted from mana/vision.md)
├── UMBRELLA.md             # This document
├── README.md               # Ecosystem overview
│
├── mana/                   # Coordination substrate
│   ├── .mana/              # Mana-specific units (bugs, features, refactors)
│   ├── Cargo.toml          # NOT a workspace root — member of tower workspace
│   ├── crates/
│   │   └── mana-core/      # Library crate
│   ├── src/                # CLI binary crate
│   ├── docs/
│   ├── tests/
│   └── README.md
│
├── imp/                    # Agent engine
│   ├── .mana/              # Imp-specific units
│   ├── Cargo.toml          # NOT a workspace root
│   ├── crates/
│   │   ├── imp-llm/
│   │   ├── imp-core/       # depends on mana-core via path
│   │   ├── imp-lua/
│   │   ├── imp-tui/
│   │   └── imp-cli/
│   ├── lua/
│   ├── skills/
│   ├── tools/
│   └── README.md
│
├── wizard/                 # Interface + orchestrator
│   ├── .mana/              # Wizard-specific units
│   ├── .wizard/            # Local view state (gitignored)
│   ├── Cargo.toml          # NOT a workspace root
│   ├── crates/
│   │   ├── wizard-orch/      # daemon + `wiz` binary
│   │   ├── wizard-proto/     # shared commands, events, snapshots
│   │   ├── wizard-store/     # local view state and cache
│   │   ├── wizard-terminal/  # terminal wrapper, future libghostty bridge
│   │   └── wizard-browser/   # browser panel lifecycle
│   ├── app/
│   │   └── desktop/          # Tauri app scaffold
│   ├── SPEC.md
│   ├── FRONTEND_ARCHITECTURE.md
│   ├── BACKEND_ARCHITECTURE.md
│   ├── FULLSTACK_ARCHITECTURE.md
│   └── README.md
│
├── familiar/               # Team platform (Elixir — NOT in Cargo workspace)
│   ├── .mana/              # Familiar-specific units
│   ├── mix.exs             # Elixir project root
│   ├── config/
│   ├── lib/
│   ├── priv/
│   ├── test/
│   ├── plan.md
│   └── README.md
│
└── target/                 # Shared Cargo build artifacts
```

## 5. Workspace Strategy

### Rust — One Virtual Workspace

All Rust crates share a single Cargo workspace rooted at `tower/Cargo.toml`.

```toml
[workspace]
resolver = "2"
members = [
    # mana
    "mana",
    "mana/crates/mana-core",

    # imp
    "imp/crates/imp-llm",
    "imp/crates/imp-core",
    "imp/crates/imp-lua",
    "imp/crates/imp-tui",
    "imp/crates/imp-cli",

    # wizard
    "wizard/crates/wizard-proto",
    "wizard/crates/wizard-store",
    "wizard/crates/wizard-terminal",
    "wizard/crates/wizard-browser",
    "wizard/crates/wizard-orch",
]

[workspace.package]
edition = "2021"
license = "Apache-2.0"

[workspace.dependencies]
# Shared across ecosystem
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
anyhow = "1"
thiserror = "2"
chrono = { version = "0.4", features = ["serde"] }

# Internal path dependencies
mana-core = { path = "mana/crates/mana-core" }
imp-llm = { path = "imp/crates/imp-llm" }
imp-core = { path = "imp/crates/imp-core" }
wizard-proto = { path = "wizard/crates/wizard-proto" }
wizard-store = { path = "wizard/crates/wizard-store" }
wizard-terminal = { path = "wizard/crates/wizard-terminal" }
wizard-browser = { path = "wizard/crates/wizard-browser" }
```

### Why a shared workspace

1. **Path dependencies just work.** `imp-core` depends on `mana-core` via path. No publishing required during development.
2. **One `cargo build`.** All crates compile together. Breaking changes surface immediately.
3. **Shared `target/`.** One build cache. No redundant compilation.
4. **Workspace-level `cargo test`.** Run all tests across the ecosystem or scope to one project.
5. **Consistent dependency versions.** No version drift between projects.

### Why not separate workspaces

Separate workspaces would require:
- publishing mana-core to a registry (or git deps) before imp can use it
- manual version synchronization
- separate build caches (slower)
- no cross-project `cargo test`

These costs are not worth it until the projects need truly independent release cycles, which is far out.

### Elixir — Separate Build

Familiar uses Mix, not Cargo. It lives in the same directory tree but has its own build system.

Familiar talks to mana through:
- the `.mana/` filesystem protocol (same as every other consumer)
- eventually a JSON-RPC or gRPC interface to wizard-orch

No Rust-Elixir FFI is needed or desired. The filesystem is the integration point.

## 6. Dependency Graph

### Compile-time (Rust crate dependencies)

```
mana-core
  ↑
  ├── imp-core
  │     ↑
  │     ├── imp-lua
  │     │     ↑
  │     │     └── imp-tui
  │     │           ↑
  │     │           └── imp-cli
  │     └── imp-llm (no mana dependency)
  │
  ├── wizard-orch
  │     ↑
  │     └── wizard-store
  │
  └── wizard-proto (types only, may not need mana-core)
```

### Runtime (process-level)

```
wizard-orch (daemon)
  ├── watches .mana/ (filesystem)
  ├── spawns imp-cli processes
  ├── streams events to wizard desktop (WebSocket)
  └── streams events to wiz CLI (Unix socket / JSON)

imp-cli (agent process)
  ├── reads .mana/ units (via mana-core)
  ├── calls LLM (via imp-llm)
  ├── executes tools
  └── writes results back to .mana/

wizard desktop (Tauri app / agent command center)
  ├── connects to wizard-orch daemon
  ├── reads .mana/ for graph state
  ├── reads .wizard/ for local layout
  └── issues commands through wizard-proto

familiar (Phoenix app)
  ├── reads/writes .mana/ (or talks to wizard-orch API)
  ├── runs imp agents as OTP processes
  ├── manages isolated environments
  └── serves dashboard via LiveView
```

### Integration boundaries

| Boundary | Protocol | Why |
|---|---|---|
| mana ↔ everything | `.mana/` filesystem | Universal, language-agnostic, crash-durable |
| imp ↔ wizard-orch | Process spawn + exit codes + `.mana/` | Worker supervision boundary — wizard controls, observes, and reviews workers without baking methodology into imp |
| wizard-orch ↔ wizard desktop | WebSocket (localhost) | Low-latency streaming for live canvas |
| wizard-orch ↔ wiz CLI | Unix socket or stdout JSON | Lightweight ops surface |
| familiar ↔ mana | `.mana/` filesystem or wizard-orch API | Same protocol, different deployment |
| familiar ↔ imp | Elixir SDK (imp as OTP process) | Native integration on the BEAM |

## 7. Mana Hierarchy

Two levels of `.mana/`:

### Root `.mana/` — Ecosystem planning

Lives at `tower/.mana/`.

For:
- cross-project roadmap units
- interface contracts between projects
- ecosystem-level milestones
- architectural decisions that span projects
- meta-work (CI, releases, documentation)

Example units:
- "Stabilize mana-core public API for imp and wizard consumption"
- "Define wizard-orch event protocol"
- "Implement imp-core mana tool integration"
- "Set up cross-project CI"

### Per-project `.mana/` — Project-scoped work

Lives at `tower/mana/.mana/`, `tower/imp/.mana/`, etc.

For:
- bugs in that project
- features scoped to that project
- internal refactors
- project-specific tests and docs

Example units:
- `mana/.mana/`: "Fix flaky close.rs tests"
- `imp/.mana/`: "Implement Anthropic streaming provider"
- `wizard/.mana/`: "Build read-only canvas renderer"

### Rules

1. If the work touches only one project, it goes in that project's `.mana/`.
2. If the work requires coordination across projects, it goes in root `.mana/`.
3. Cross-project units can reference per-project units as dependencies.
4. Each `.mana/` is independent — no nesting or inheritance between them.
5. `mana status` always operates on the nearest `.mana/` above the current directory.

## 8. Configuration Hierarchy

Each project keeps its own configuration. No merging across projects.

| Config | Location | Scope |
|---|---|---|
| mana project config | `<project>/.mana/config.yaml` | Per-project work graph settings |
| imp agent config | `~/.config/imp/config.toml` + `<project>/.imp/config.toml` | Agent runtime settings |
| wizard orchestration config | `~/.config/wizard/config.toml` | Orchestration settings |
| Root mana config | `tower/.mana/config.yaml` | Ecosystem-level planning settings |

The workspace `Cargo.toml` handles build-time concerns. Runtime configuration stays separated by project.

## 9. Git Strategy

### Recommendation: Monorepo

One git repository for the entire tower.

**Why monorepo:**
- Cross-project changes are atomic commits
- One PR can update mana-core and its consumers together
- Shared CI pipeline
- One clone, one history, one blame
- Cargo workspace requires it (or path deps break)

**Why not polyrepo:**
- Version synchronization pain
- Can't do atomic cross-project changes
- Separate CI configurations
- Path dependencies don't work without git submodules (which are worse)

### Branch strategy

- `main` — always buildable
- Feature branches per unit or cluster
- Cross-project branches when a change spans multiple projects

### Gitignore strategy

Root `.gitignore`:
```
target/
.DS_Store
```

Per-project `.gitignore` additions:
```
# wizard
wizard/.wizard/

# familiar
familiar/_build/
familiar/deps/
```

### What about the existing separate repos?

Migration plan is in §12.

## 10. Build and Dev Workflow

### Building

```bash
# Build everything
cargo build

# Build one project
cargo build -p mana-cli
cargo build -p imp-cli

# Build one crate
cargo build -p mana-core
cargo build -p imp-llm

# Build familiar (separate)
cd familiar && mix compile
```

### Testing

```bash
# Test everything Rust
cargo test

# Test one project
cargo test -p mana-cli
cargo test -p imp-core

# Test familiar
cd familiar && mix test
```

### Running

```bash
# mana CLI (operates on nearest .mana/)
cargo run -p mana-cli -- status

# imp interactive
cargo run -p imp-cli

# wizard daemon
cargo run -p wizard-orch -- daemon

# wizard desktop
cd wizard/app/desktop && cargo tauri dev

# familiar
cd familiar && mix phx.server
```

### Installing binaries

```bash
# Install all Rust binaries
cargo install --path mana
cargo install --path imp/crates/imp-cli
# wizard-orch install TBD
```

### Cross-project development

The main advantage of the shared workspace:

If you change a type in `mana-core`, then `cargo build` immediately tells you if `imp-core` or `wizard-orch` break. No manual checking across repos. No publishing. No version bumps. Just one build.

## 11. Cross-Project Conventions

### Naming

| Crate | Binary | Description |
|---|---|---|
| `mana-core` | — | Library: work graph, units, deps, facts |
| `mana-cli` | `mana` | CLI for mana operations |
| `imp-llm` | — | Library: multi-provider LLM client |
| `imp-core` | — | Library: agent loop, tools, context |
| `imp-lua` | — | Library: Lua extension runtime |
| `imp-tui` | — | Library: terminal UI components |
| `imp-cli` | `imp` | Interactive agent binary |
| `wizard-orch` | `wiz` | Daemon + CLI for orchestration |
| `wizard-proto` | — | Library: shared event/command types |
| `wizard-store` | — | Library: local view state |

### Error handling

- Libraries (`*-core`, `*-proto`): use `thiserror` with typed errors
- Binaries (`*-cli`, `wiz`): use `anyhow` at the boundary
- Across crate boundaries: typed errors, not stringly-typed

### Shared dependencies

Workspace-level `[workspace.dependencies]` for all shared crates. Each project member inherits versions from the workspace. No per-crate version pinning for common deps.

### Documentation

Each project has its own README and docs. The root has:
- `VISION.md` — the ecosystem narrative
- `UMBRELLA.md` — this structural spec
- `README.md` — quick orientation + links

### Code style

Consistent across all Rust projects:
- `cargo fmt` (default rustfmt)
- `cargo clippy` (default lints)
- Self-documenting code
- Comments explain *why*

## 12. Development Before Full Migration

You do **not** need to move everything manually all at once to start developing toward the tower shape.

There are three practical modes:

### Mode A — Spec-first, no filesystem moves yet

Current state:
- keep `mana` at `~/mana`
- keep `imp` at `~/imp`
- keep `wizard` nested temporarily at `~/mana/wizard`
- keep `familiar` at `~/familiar`

Use this mode when:
- architecture is still changing quickly
- Wizard is still mostly specs
- you want to reduce risk and avoid churn

What to do in this mode:
- keep updating `UMBRELLA.md`, `wizard/SPEC.md`, and the architecture docs
- create ecosystem-level planning in a future root `.mana/` design, but do not move repos yet
- avoid premature code-level path dependencies between `mana` and `imp`

### Mode B — Create `~/tower/` as a parent folder, but keep repos independent temporarily

Structure:

```text
~/tower/
  mana/       # existing mana repo copied or moved here
  imp/        # existing imp repo copied or moved here
  wizard/     # initially can still be sourced from mana/wizard
  familiar/   # existing familiar repo copied or moved here
```

This is useful as a **navigation umbrella** before it becomes a true monorepo.

Benefits:
- one parent folder to open in the OS, editor, or future Wizard app
- preserves existing git repos for a while
- lets you get used to the target shape

Limitations:
- still not one Cargo workspace
- no atomic cross-project commits
- path dependencies stay awkward
- root `.mana/` is conceptual until you choose the monorepo cutover

### Mode C — Full monorepo cutover

This is the target state described by the rest of this document:
- one git repo at `~/tower`
- one Cargo workspace for Rust projects
- root `.mana/` for ecosystem planning
- per-project `.mana/` for local planning

### Recommendation

**Do Mode B next, then Mode C once Wizard has real code and mana-core API boundaries are clearer.**

That gives you the developer ergonomics of a shared parent folder now without forcing the whole migration immediately.

## 13. Migration Plan

### Current state

```text
~/mana/            → mana repo (git)
~/mana/wizard/     → wizard spec + architecture docs, nested in mana
~/imp/             → imp repo (git)
~/familiar/        → familiar plans (git)
```

### Target state

```
~/tower/           → monorepo (git)
~/tower/mana/
~/tower/imp/
~/tower/wizard/
~/tower/familiar/
```

### Steps

#### Phase 0 — Prep (no moves yet)

1. Finalize this umbrella spec.
2. Finalize wizard SPEC.md.
3. Ensure mana and imp have clean git state.
4. Decide on the root directory name.

#### Phase 1 — Create the tower

1. `mkdir ~/tower`
2. `cd ~/tower && git init`
3. Create root `Cargo.toml` (virtual workspace, no code).
4. Create root `.gitignore`.
5. Create root `README.md`.
6. Copy `~/mana/vision.md` to `tower/VISION.md`.
7. Copy `~/mana/UMBRELLA.md` to `tower/UMBRELLA.md`.
8. Initialize `tower/.mana/` for ecosystem planning.

Optional transitional variant:
- instead of immediately making `~/tower` the real monorepo, create the folder and place the existing repos under it first
- example:

```text
~/tower/
  mana/       # current mana repo
  imp/        # current imp repo
  familiar/   # current familiar repo
```

Then add `wizard/` either by moving `~/mana/wizard/` out or by treating it as temporary until the monorepo cutover.

#### Phase 2 — Move mana

1. Copy `~/mana/` contents into `~/tower/mana/` (excluding `wizard/`, `vision.md`, `UMBRELLA.md`).
2. Convert `mana/Cargo.toml` from workspace root to workspace member.
3. Move `mana/crates/mana-core` membership to root workspace.
4. Move `mana/` binary crate membership to root workspace.
5. Verify: `cd ~/tower && cargo build -p mana-cli && cargo test -p mana-cli`
6. Preserve mana's `.beans/` or `.mana/` as `mana/.mana/`.

#### Phase 3 — Move imp

1. Copy `~/imp/` contents into `~/tower/imp/`.
2. Convert `imp/Cargo.toml` from workspace root to workspace member list (or remove — members declared in root).
3. Wire `imp-core`'s dependency on `mana-core` as a path dep through the workspace.
4. Verify: `cd ~/tower && cargo build -p imp-core`

#### Phase 4 — Move wizard

1. Move `~/mana/wizard/` to `~/tower/wizard/`.
2. Already structured correctly from the wizard spec.
3. No code to build yet — just spec + `.mana/`.

#### Phase 5 — Move familiar

1. Copy `~/familiar/` contents into `~/tower/familiar/`.
2. No Cargo integration needed — separate Mix project.
3. Verify: `cd ~/tower/familiar && mix compile` (once code exists).

#### Phase 6 — Verify and commit

1. `cd ~/tower && cargo build && cargo test`
2. Review root `.mana/` for ecosystem-level planning units.
3. First commit: "Initialize tower monorepo with mana, imp, wizard, familiar"
4. Optionally archive the old individual repos.

### Risk: Git history

Moving files into a monorepo loses per-file git history from the original repos unless we use `git filter-repo` or similar tools.

Options:
- **Accept the break.** Start fresh. Old repos stay archived for archaeology.
- **Graft history.** Use subtree merges to preserve history. More complex but history survives.

Recommendation: **accept the break** for now. The old repos remain available. History matters less than forward momentum.

## 14. What Each Project Needs Before the Move

| Project | Blockers | Ready? |
|---|---|---|
| mana | Clean `.mana/` (rename from `.beans/`), stable mana-core API | Mostly — rename is tracked |
| imp | None — workspace already structured for this | Yes |
| wizard | None — just spec + `.mana/` | Yes |
| familiar | None — just plans | Yes |

The `.beans/` → `.mana/` rename in the mana project is already tracked as unit `225.4` in mana's backlog. It should happen before or during the move.

## 15. Open Questions

1. **Root directory name.** `tower` fits the metaphor. Alternatives: `arcana`, `grove`, or just the org name. Does the name matter enough to bikeshed?
2. **Git history strategy.** Accept the break or graft histories?
3. **When to move.** Now (while things are early) or after mana stabilizes further?
4. **CI.** One pipeline or per-project? Monorepo CI tools (nx, turborepo for Rust?) or just cargo workspace + make targets?
5. **Familiar's imp dependency.** Familiar uses imp as an Elixir SDK. The Rust imp is a different implementation. Should the Elixir imp code live in `familiar/` or in `imp/` as a separate Mix project? Or is it a completely separate codebase that just shares concepts?
6. **Published crates.** Will `mana-core` or `imp-llm` ever be published to crates.io? If so, the workspace needs version management tooling (cargo-release, etc.).

## 16. Decision Summary

| Decision | Choice | Rationale |
|---|---|---|
| Structure | Monorepo under one root | Cross-project deps, atomic changes, shared build |
| Rust build | Single Cargo workspace | Path deps, shared target, one `cargo test` |
| Elixir build | Separate Mix project in same tree | Different language, different toolchain |
| Planning | Root `.mana/` + per-project `.mana/` | Cross-project roadmap without polluting project backlogs |
| Git | Single repo, accept history break | Simplicity over archaeology |
| Migration | Phased: create root → move mana → move imp → move wizard → move familiar | Incremental, verifiable at each step |
| Naming | `tower` (provisional) | Fits the metaphor, lightweight, can change |
