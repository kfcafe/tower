# Wizard Backend Architecture

Status: Draft 0.1  
Scope: Rust backend, daemon, IPC, projection, and native integrations for Wizard  
Related: `SPEC.md`, `FRONTEND_ARCHITECTURE.md`, `FULLSTACK_ARCHITECTURE.md`

## 1. Purpose

This document defines the backend architecture for Wizard.

The backend is responsible for:
- orchestrating work and supervising agents
- reading and projecting `.mana/` state
- exposing commands and event streams to the desktop client
- managing native integrations (terminal lifecycle coordination with the Photon desktop shell)
- persisting local Wizard state where appropriate

The backend is split into **daemon responsibilities** and **desktop-local native responsibilities**. They should be separate even when they run in the same process during early development.

## 2. Architectural Principles

1. **`.mana/` is canonical.** Backend caches and projections are derived, never primary.
2. **Daemon first.** Orchestration must survive the UI closing.
3. **Typed protocols.** UI/backend communication uses typed commands and events, not stringly JSON blobs.
4. **Agent-agnostic orchestration.** Wizard supervises workers through `.mana/`, process contracts, and runtime events. It should not assume one specific internal imp implementation.
5. **Native integrations behind clean interfaces.** PTY, sockets, and OS-specific behaviors live behind focused crates/modules. libghostty and browser panels are now managed by the Photon desktop shell (Zig-native), not the Rust backend.
6. **Config-centric policy.** Shared runtime policy comes from explicit config, not from `.wizard/` local state or scattered constants.

## 3. Proposed Crates

```text
wizard/
  crates/
    wizard-orch/        # daemon, orchestration, watchers, projections, IPC server
    wizard-proto/       # shared commands, events, snapshot types
    wizard-store/       # local state persistence and cache
```

**Note:** `wizard-terminal` and `wizard-browser` are no longer needed as separate Rust crates. Terminal panels (libghostty) are composited Zig-natively by the Photon desktop shell. Browser panels are rendered by Photon directly. The Rust daemon focuses on orchestration, projection, and the wizard-proto IPC server.

## 4. Crate Responsibilities

## 4.1 `wizard-proto`
Shared types only.

Should define:
- command enums
- event enums
- snapshot structs
- panel descriptors
- room identifiers
- artifact envelope types
- serialization helpers

This crate must remain lightweight and dependency-minimal.

## 4.2 `wizard-orch`
The daemon and orchestration runtime.

Responsibilities:
- watch `.mana/`
- load project snapshots
- derive graph/runtime projections
- compute ready units
- dispatch and supervise imp processes
- emit runtime events
- expose IPC server for desktop and CLI clients
- integrate with `mana-core`

## 4.3 `wizard-store`
Persistence for local Wizard state.

Responsibilities:
- load and save `.wizard/` files
- persist room state
- persist panel layout
- cache last-known projections for faster startup
- version local state schemas

## 4.4 Terminal integration (Photon-side)

Terminal panels are managed by the Photon desktop shell, not the Rust daemon.

libghostty and Photon are both Zig — terminal panels are composited directly by Photon's window manager with zero FFI overhead. The Rust daemon's role is limited to:
- emitting agent spawn/exit events (so the desktop shell knows when to create/destroy agent terminal panels)
- providing structured transcript capture for persistence and artifact creation
- exposing terminal session metadata through wizard-proto

The Photon desktop shell owns PTY lifecycle, resize, scrollback, and native compositing.

## 4.5 Browser panel integration (Photon-side)

Browser panels are rendered by Photon directly, not by a separate Rust-managed webview.

The Rust daemon's role is limited to:
- persisting URL/panel associations by scope (room, unit, fact) via wizard-store
- exposing panel metadata through wizard-proto

The Photon desktop shell owns panel creation, navigation, rendering, and toolbar actions.

## 5. Process Model

## 5.1 Long-term target

```text
wiz / desktop app
    │
    ├── connects to wizard-orch daemon
    │
    └── local native hosts (terminal/browser views)

wizard-orch daemon
    ├── .mana/ watcher
    ├── projection engine
    ├── dispatch scheduler
    ├── imp process supervisor
    └── IPC server
```

## 5.2 Early-stage simplification

It is acceptable in early development for the desktop app to start an embedded orchestration runtime. But the code structure must preserve a future split into a standalone daemon.

Rule:
- **one code path for embedded mode**
- **same code path for daemon mode**

No second orchestration implementation.

## 6. Core Runtime Services

## 6.1 Project loader
Loads `.mana/` state and assembles a `ProjectSnapshot`.

Responsibilities:
- discover project root
- load units, facts, attempts, verify history
- normalize project state
- detect staleness / changed files

## 6.2 Watch service
Watches `.mana/` and related project files.

Responsibilities:
- detect unit changes
- detect fact updates
- detect verify state changes
- trigger projection recompute
- debounce bursty file events

## 6.3 Projection engine
Transforms raw mana state into frontend-ready projections.

Outputs:
- graph snapshot
- runtime snapshot
- room projection
- review snapshot
- knowledge projection

The frontend should not need to derive everything from raw unit files.

## 6.4 Dispatch service
Coordinates agent work.

Responsibilities:
- compute ready units
- enforce concurrency limits
- spawn imp processes
- track claims and releases
- react to exit codes and verify results
- issue retries according to policy

## 6.5 Runtime event service
Publishes structured events to UI and CLI clients.

Examples:
- `ProjectLoaded`
- `GraphChanged`
- `UnitStatusChanged`
- `AgentSpawned`
- `AgentUpdated`
- `VerifyStarted`
- `VerifyFinished`
- `ArtifactPinned`

## 7. IPC Model

Use typed IPC between frontend and backend.

## 7.1 Command channel
Frontend sends commands such as:
- `OpenProject`
- `RunUnit`
- `RetryUnit`
- `StopAgent`
- `VerifyUnit`
- `OpenEditorFile`
- `SaveEditorBuffer`
- `OpenTerminalPanel`
- `OpenBrowserPanel`
- `PersistRoomState`

## 7.2 Event channel
Backend streams events and snapshot deltas to the frontend.

Important property:
- frontend must be able to reconnect and receive a fresh snapshot
- events are incremental, snapshots are authoritative

## 7.3 Transport

The Photon desktop shell connects to the wizard-orch daemon over a **WebSocket or Unix socket** using wizard-proto. This is the same protocol the `wiz` CLI uses — one transport, multiple clients.

The Bun backend in the Photon process acts as the bridge: it connects to the daemon socket, serializes/deserializes wizard-proto messages, and forwards them between SolidJS and the daemon.

The transport is socket-based from day one. No framework-specific IPC (Tauri invoke/listen) to abstract away later.

## 8. Terminal Backend Design

## 8.1 Session kinds

```rust
enum TerminalSessionKind {
    Room { room_id: String },
    Agent { agent_id: String, unit_id: String },
    Verify { unit_id: String },
    Quick,
}
```

## 8.2 Required capabilities
- spawn PTY
- attach libghostty surface
- resize on panel changes
- capture stdout/stderr stream
- expose session metadata
- persist transcript for later review

## 8.3 Live vs persisted output

Use a dual model:
- **live PTY passthrough** for faithful runtime viewing
- **structured capture** for persistence, search, and artifact extraction

The backend owns both.

## 9. Editor Backend Design

The backend should provide file operations for the built-in editor.

Required commands:
- open file
- save file
- revert file
- load diff against disk or git state
- apply patch
- report dirty conflicts when file changed externally

The backend should remain editor-agnostic.
It does not know about CodeMirror directly. It only speaks in file buffers, diffs, and edit commands.

## 10. Browser Backend Design

The backend owns browser panel identity and lifecycle.

Required responsibilities:
- create browser panel
- associate panel with room/unit/fact
- remember last URL per panel scope
- restore browser panel on room reopen
- proxy minimal toolbar actions to the webview host

The backend should not become a browser automation engine. That belongs elsewhere.

## 11. Orchestration and Agent Supervision

## 11.1 Spawn contract
Wizard supervises imp through:
- command invocation
- runtime metadata
- stdout/stderr capture
- exit code
- `.mana/` side effects

It should not depend on fragile transcript parsing for core orchestration correctness.

## 11.2 Supervision responsibilities
- start process
- monitor health
- detect idle timeout
- kill stuck process when necessary
- record attempt metadata
- emit runtime events
- trigger verify flow

## 11.3 Budget and backpressure
The daemon owns:
- concurrency limits
- cost caps
- retry caps
- escalation behavior
- stale-unit awareness

## 12. Persistence Model

## Shared project state
- `.mana/` only

## Local Wizard state
- `.wizard/`

Suggested backend persistence:

```text
.wizard/
  state.json
  rooms/
  views/
  panels/
  cache/
    project-snapshot.json
    runtime-snapshot.json
```

## 13. Failure Modes

### 13.1 UI disconnects while daemon runs
Mitigation:
- daemon continues
- reconnect gets fresh snapshot

### 13.2 File watcher event storms
Mitigation:
- debounce
- coalesced projection updates

### 13.3 Terminal session leaks
Mitigation:
- session registry with owner/scope
- explicit cleanup on panel close and process exit

### 13.4 Projection drift from `.mana/`
Mitigation:
- snapshots rebuilt from canonical state
- no mutable truth in cache

### 13.5 Native integration sprawl
Mitigation:
- wrapper crates (`wizard-terminal`, `wizard-browser`)
- no direct platform glue scattered through orchestration code

## 14. Suggested Rust Module Shape

```text
wizard-orch/
  src/
    lib.rs
    daemon.rs
    ipc.rs
    watch.rs
    loader.rs
    project.rs
    projection/
      mod.rs
      graph.rs
      runtime.rs
      review.rs
      knowledge.rs
    dispatch/
      mod.rs
      queue.rs
      supervisor.rs
      retry.rs
    terminal_bridge.rs
    browser_bridge.rs
```

## 15. Near-Term Build Order

1. `wizard-proto`
2. `wizard-orch` snapshot loader + watch service
3. `wizard-orch` event stream + run/verify commands
4. `wizard-store`
5. `wizard-terminal`
6. `wizard-browser`
7. embedded mode in desktop app
8. standalone daemon mode

## 16. Decision Summary

| Area | Decision |
|---|---|
| Canonical state | `.mana/` |
| Local state | `.wizard/` |
| IPC | typed commands/events via `wizard-proto` |
| Daemon | `wizard-orch` |
| Terminal integration | Photon-side (Zig-native libghostty compositing) |
| Browser panels | Photon-side (Photon rendering, progressive capability) |
| Editor backend | file/diff operations, editor-agnostic |
| Live terminal output | real PTY passthrough |
| Persisted terminal output | structured capture |
