# Wizard Fullstack Architecture

Status: Draft 0.1  
Scope: End-to-end architecture across UI, native backend, daemon, mana, imp, and local project state  
Related: `SPEC.md`, `FRONTEND_ARCHITECTURE.md`, `BACKEND_ARCHITECTURE.md`, `UMBRELLA.md`

## 1. Purpose

This document explains how Wizard works as a full system.

Wizard is not just a frontend and not just a daemon. It is the combination of:
- a desktop shell
- a canvas UI
- a built-in editor
- native terminal and browser panels
- a daemon/orchestration backend
- mana as canonical project state
- imp as the worker engine

This document focuses on how these layers connect and where responsibilities stop.

## 2. System Overview

```text
┌─ User ─────────────────────────────────────────────────────────────┐
│  sees canvas, edits files, runs units, watches agents            │
└───────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─ Wizard Desktop (Photon) ────────────────────────────────────────┐
│                                                                   │
│  Photon renderer (Zig)                                            │
│  ├── custom DOM + CSS + layout + GPU paint                        │
│  ├── SolidJS app (on JSC via Bun)                                 │
│  │   ├── canvas                                                   │
│  │   ├── inspector                                                │
│  │   ├── CodeMirror editor panes                                  │
│  │   ├── command palette                                          │
│  │   └── runtime strip                                            │
│  ├── libghostty terminal panels (Zig-native compositing)          │
│  └── browser panels (Photon rendering)                            │
│                                                                   │
│  Bun backend (TypeScript)                                         │
│  └── wizard-proto client → daemon connection                      │
└───────────────────────────────────────────────────────────────────┘
                               │
                               │ WebSocket / Unix socket
                               ▼
┌─ Wizard Daemon ───────────────────────────────────────────────────┐
│                                                                   │
│  wizard-orch      daemon / projection / orchestration             │
│  wizard-store     local `.wizard/` state                          │
│  wizard-proto     shared commands, events, snapshots              │
└───────────────────────────────────────────────────────────────────┘
                               │
                 ┌─────────────┴─────────────┐
                 ▼                           ▼
┌─ `.mana/` project state ──────────┐   ┌─ imp worker processes ────┐
│ units, facts, attempts, verify     │   │ read/write code, run tools │
│ history, deps, features            │   │ update `.mana/`, verify    │
└────────────────────────────────────┘   └────────────────────────────┘
```

## 3. Responsibility Boundaries

## 3.1 Desktop UI
Owns:
- rendering
- keyboard and mouse interactions
- local selection and panel state
- embedded editing experience

Does not own:
- canonical work state
- orchestration truth
- process supervision

## 3.2 Backend/daemon
Owns:
- snapshots
- runtime projections
- agent dispatch and supervision
- PTY and browser native integrations
- local layout persistence

Does not own:
- long-term truth outside `.mana/`
- editor-specific UI behavior

## 3.3 `.mana/`
Owns:
- work graph truth
- dependency truth
- facts
- attempts
- verify truth
- shared project memory

Does not own:
- card positions
- room layout
- local UI state

## 3.4 imp
Owns:
- agent reasoning loop
- tool execution
- context management
- code changes
- verify execution when delegated

Does not own:
- project graph visualization
- orchestration policy across many units
- user-facing desktop interaction model

## 4. Main Data Flows

## 4.1 Project open flow

1. User launches Wizard.
2. Desktop app finds current project root.
3. Backend loads `.mana/`.
4. Backend builds snapshots and derived projections.
5. Frontend receives initial snapshot.
6. Frontend restores `.wizard/` local state.
7. Canvas opens on the project home view.

## 4.2 Run unit flow

1. User selects a unit card.
2. User runs `RunUnit` via keyboard or card action.
3. Frontend sends typed command to backend.
4. Backend validates current state and dispatch eligibility.
5. Backend spawns imp process.
6. Backend emits `AgentSpawned` and runtime updates.
7. Frontend updates agent card and runtime strip.
8. Agent works on the codebase and updates `.mana/`.
9. Backend watches `.mana/` changes and refreshes projections.
10. Frontend receives updated graph/runtime state.

## 4.3 Edit file flow

1. User opens a file from a unit card or artifact.
2. Frontend opens CodeMirror editor pane.
3. Backend provides file contents and metadata.
4. User edits locally.
5. Save command sends buffer back to backend.
6. Backend writes file to disk.
7. Any relevant diffs or project changes are reflected back into projections.

## 4.4 Verify flow

1. User runs verify from selected unit.
2. Backend creates verify terminal session.
3. Verify command runs in PTY.
4. Terminal panel shows live output via libghostty.
5. Backend captures structured verify result.
6. `.mana/` verify history updates.
7. Frontend updates verify cards, badges, and runtime summaries.

## 4.5 Review flow

1. Agent exits or verify finishes.
2. Backend captures attempt metadata, changed files, and runtime result.
3. Frontend shows review view with:
   - diff preview
   - verify output
   - attempt summary
   - related artifacts
4. User decides to accept, retry, annotate, or decompose.

## 5. Fullstack State Model

## 5.1 Shared state (`.mana/`)
- units
- dependencies
- features
- facts
- attempts
- verify history
- shared artifacts with durable meaning

## 5.2 Wizard config
- user config: `~/.config/wizard/config.toml`
- shared project config: `<project>/.wizard.toml`
- override order: built-in defaults < user config < project config < environment overrides < CLI or in-session overrides

Wizard config exists so shared orchestration and repo-specific behavior can be explicit without turning `.wizard/` into a git-churn directory.

## 5.3 Local state (`.wizard/`)
- room layout
- panel layout
- camera position
- saved views
- hidden layers
- local editor tab state
- local browser/terminal restoration metadata

## 5.4 Ephemeral runtime state
- active websocket/socket connection
- temporary command in flight
- hover state
- selection state
- currently attached terminal session

## 6. Fullstack Technology Choices

| Layer | Technology | Why |
|---|---|---|
| Rendering engine | Photon (Zig) | custom DOM/CSS/layout/GPU paint — we own the renderer, canvas-specific optimizations possible |
| Desktop shell | Photon + Bun | Zig binary with Bun backend, no system webview |
| UI framework | SolidJS (on JSC) | fine-grained reactivity, compiles to standard DOM APIs that map to Photon's JS bindings |
| Editor | CodeMirror 6 (on Photon) | embeddable, flexible, sufficient for v1 focused editing |
| Terminal | libghostty (Zig-native) | real PTY, native quality, composited directly by Photon — no FFI boundary |
| Browser panels | Photon rendering | same engine renders URL content, progressive capability, no separate engine |
| Daemon | Rust (wizard-orch) | orchestration, graph projection, imp supervision |
| Daemon IPC | WebSocket or Unix socket | wizard-proto commands and events, same protocol as `wiz` CLI |
| Canonical state | `.mana/` | durable, human-readable, language-agnostic |
| Worker engine | imp | specialized agent engine |

## 7. Why This Shape Instead of Alternatives

## 7.1 Why not Tauri + system webview
Because we own Photon and Wizard benefits from controlling the rendering substrate. Canvas-specific GPU optimizations, Zig-native libghostty compositing, and progressive browser panel rendering are all natural on Photon but would require workarounds on Tauri. The daemon boundary means the rendering layer is replaceable if needed.

## 7.2 Why not full native Rust UI
Because the canvas/editor/panel experimentation is faster with SolidJS + DOM APIs, and the frontend needs rich editor and rendering ecosystems. Photon provides the native-quality rendering while SolidJS provides the rapid UI iteration.

## 7.3 Why not bundle Gecko or Chromium
Because Wizard is not a browser. Photon renders what Wizard needs. As Photon matures toward Phase 3, browser panels gain full web rendering without a separate engine.

## 7.4 Why not use Neovim/Helix/Zed as the default editor
Because Wizard needs an integrated editor surface that is easy to compose with the canvas and panel system. CodeMirror 6 is the lower-risk default. Keep a path for optional Neovim later.

## 8. Room-Centric UX Model

A focus room is the fullstack coordination unit.

A room may include:
- scoped graph projection
- selected unit
- built-in editor pane(s)
- room terminal
- room docs/dev-server browser panels
- room-local camera position
- room-local filters and overlays

This is the crucial idea that ties frontend and backend together:
**the room is the durable local workspace for a part of the graph.**

## 9. Runtime Modes

## 9.1 Desktop attached mode
- user has Wizard open
- daemon streams live events to UI
- terminal/browser/editor all available

## 9.2 Desktop detached mode
- UI closed
- daemon continues orchestrating
- user later reattaches and gets fresh state

## 9.3 CLI mode
- `wiz` talks to the same backend/runtime
- suitable for remote ops, SSH, scripting, and quick status

## 10. End-to-End Failure Handling

### UI crash
- daemon survives
- `.mana/` survives
- `.wizard/` local state mostly survives
- user relaunches and reconnects

### Daemon crash
- canonical truth still survives in `.mana/`
- local state survives in `.wizard/`
- daemon can rebuild projections on restart

### Agent crash
- attempt is recorded
- unit remains open
- retry path remains available
- live terminal session closes into review mode

### Native terminal/browser integration failure
- canvas still works
- panel feature degrades gracefully
- open externally remains available

## 11. Suggested Cross-Layer Module Map

```text
wizard/
  SPEC.md
  FRONTEND_ARCHITECTURE.md
  BACKEND_ARCHITECTURE.md
  FULLSTACK_ARCHITECTURE.md
  crates/
    wizard-proto/
    wizard-orch/
    wizard-store/
    wizard-terminal/
    wizard-browser/
  app/
    desktop/
      src/
        app/
        canvas/
        editor/
        panels/
        state/
        projection/
        ipc/
```

## 12. Build Order Across the Stack

### Phase 1
- `wizard-proto` with socket transport (WebSocket or Unix socket)
- `wizard-orch` snapshot loader + event publisher
- Photon + SolidJS shell with daemon connection
- read-only graph canvas on Photon

### Phase 2
- command palette
- room state persistence
- CodeMirror editor integration (on Photon)
- verify terminal via libghostty (Zig-native compositing)

### Phase 3
- agent supervision and runtime strip
- browser panels (Photon rendering for localhost/docs)
- diff review
- attempt/review surfaces

### Phase 4
- focus rooms with full panel restoration
- richer artifacts
- daemon detach/reattach workflow

### Phase 5
- optional power-user editor path
- WASM DOM optimization for canvas hot path if needed
- browser panels gain full web rendering (Photon Phase 3)
- collaboration-friendly exports and shared views

## 13. Design Tensions to Watch

1. **Canvas vs panels** — panels must not overpower the canvas.
2. **Editor ambition vs orchestration focus** — build enough editor to make Wizard real, not enough to derail the product.
3. **Live PTY richness vs structured persistence** — keep both, do not collapse one into the other.
4. **Speed vs architecture purity** — early embedded mode is okay, but preserve daemon boundaries.
5. **Personal layout vs shared meaning** — `.wizard/` stays local by default.

## 14. Decision Summary

| Question | Decision |
|---|---|
| Primary surface | Canvas |
| Rendering engine | Photon (Zig) |
| Built-in editor | Yes, CodeMirror 6 (on Photon) |
| Power-user editor path | Maybe later, likely Neovim-backed |
| Terminal | libghostty (Zig-native compositing) |
| Browser panels | Photon rendering (progressive capability) |
| Desktop shell | Photon + Bun |
| Daemon | wizard-orch (Rust) via socket |
| Daemon IPC | WebSocket or Unix socket, wizard-proto |
| Canonical project truth | `.mana/` |
| Local UX truth | `.wizard/` |
| Worker engine | imp |
