# Wizard Frontend Architecture

Status: Draft 0.1  
Scope: Desktop client UI architecture for Wizard  
Related: `SPEC.md`, `BACKEND_ARCHITECTURE.md`, `FULLSTACK_ARCHITECTURE.md`

## 1. Purpose

This document defines the frontend architecture for Wizard: the canvas-native desktop interface for mana.

The frontend is responsible for:
- rendering the project canvas
- managing selection, focus, zoom, and view state
- presenting agent runtime updates
- embedding the built-in editor
- coordinating terminal and browser panels through the backend
- preserving local spatial memory in `.wizard/`

The frontend is **not** the source of truth for project work state. `.mana/` remains canonical. The frontend renders projections of that state and sends user intent to the backend.

## 2. Stack Decisions

## Shell
- **Photon** rendering engine (Zig) — custom DOM, CSS, layout, GPU paint
- **Bun** backend — hosts vanilla TypeScript on JavaScriptCore, manages daemon connection, loads WASM editor module
- No system webview — Photon replaces it entirely

## UI approach
- **Vanilla TypeScript** — no framework

### Why no framework

Wizard's UI is event-driven, not reactive. The daemon pushes typed events over a socket (unit status changed, agent spawned, verify passed). The TypeScript layer finds the relevant DOM nodes and updates them. This is a message loop, not a component tree.

No SolidJS, no React, no framework. Reasons:
- Photon's DOM is a clean subset — no browser quirks to paper over
- Frameworks add ecosystem dependencies that may not work on Photon's DOM
- Event-driven updates are simpler than reactive patterns for this use case
- Vanilla TypeScript on Bun runs natively — no build step, no bundler
- The code stays explicit and easy to reason about in an agent-authored codebase

## Editor
- **Rust→WASM engine** rendered by Photon

### Why Rust→WASM, not CodeMirror or native Zig
- `ropey` (Rust rope crate) provides an efficient text buffer without writing one from scratch
- `tree-sitter` provides incremental syntax highlighting — same engine as Zed, Neovim, Helix
- Rust→WASM is a well-established compilation target with good tooling
- Photon already has WASM DOM bindings — the editor engine talks to the renderer natively
- Zero JavaScript dependencies — no CodeMirror, no Monaco, no JS in the editing hot path
- We control the editor — custom diff views, inline verify output, agent review workflows are easy to add
- Photon's text subsystem (fonts, measurement, selection, GPU rendering) handles all display

## Graph rendering
- **DOM-first** for MVP
- **hybrid DOM + canvas/WebGL** when performance requires it

## Browser panels
- Photon rendering (progressive: app-level now, full web as Phase 3 matures)
- External browser fallback always available

## Terminal panels
- libghostty composited directly by Photon (Zig-native, no FFI)

## 3. Core Frontend Principles

1. **Canvas is primary.** Every other surface is subordinate to the canvas.
2. **Thin UI, thick app model.** Core state and projections live in explicit app-layer modules, not scattered through components.
3. **Fine-grained updates.** A changed unit should update only the cards, counters, and views that depend on it.
4. **State is split cleanly.** `.mana/` state is shared and backend-driven. `.wizard/` state is local and frontend-driven.
5. **Panels are tools, not worlds.** Editor, terminal, browser, and inspector should feel attached to the work, not like separate apps.
6. **Keyboard-first.** Mouse supports spatial work; keyboard drives operations.

## 4. Frontend Responsibilities

### 4.1 Canvas UI
- render graph nodes and edges
- semantic zoom
- focus room transitions
- spatial selection and navigation
- card overlays and badges

### 4.2 Inspector UI
- selected unit details
- acceptance criteria
- verify command and last result
- attempts and notes
- related artifacts
- current agent activity

### 4.3 Editor UI
- file open/save
- jump to file/line
- diff viewing
- patch review
- lightweight multi-pane editing

### 4.4 Panel UI
- panel docking and floating
- room-local panel restoration
- terminal panel containers
- browser panel containers

### 4.5 Runtime UI
- runtime strip
- cost/tokens display
- queue state
- active agent cards
- event summaries

## 5. Recommended Directory Shape

```text
wizard/
  app/
    desktop/
      src/
        main.ts
        app/
          AppShell.tsx
          routes/
          layout/
          panels/
          canvas/
          editor/
          inspector/
          runtime/
          commands/
          state/
          projection/
          ipc/
          styles/
```

## 6. Frontend Module Boundaries

## 6.1 `state/`
Core client-side state.

Suggested modules:
- `projectState.ts` — active project, connection state, snapshots
- `graphStore.ts` — normalized units, facts, agents, artifacts
- `selectionState.ts` — selected node(s), active room, hover state
- `viewState.ts` — zoom, pan, filters, hidden layers
- `panelState.ts` — docked/floating panels, sizes, room associations
- `editorState.ts` — open files, dirty state, cursors, diff sessions
- `runtimeState.ts` — live agent status, queue, cost, event summaries

## 6.2 `projection/`
Derived views and selectors.

Suggested modules:
- `graphProjection.ts` — graph nodes/edges ready to render
- `roomProjection.ts` — a focus room's scoped graph
- `artifactProjection.ts` — attached artifacts grouped by unit
- `runtimeProjection.ts` — compact summaries for the runtime strip
- `knowledgeProjection.ts` — fact clustering and knowledge views

Important rule: derived projections live here, not inside UI components.

## 6.3 `ipc/`
Frontend/backend bridge.

Responsibilities:
- subscribe to snapshots and events
- send commands (`runUnit`, `stopAgent`, `openBrowserPanel`, `saveFile`)
- reconnect handling
- event batching and ordering

## 6.4 `canvas/`
Canvas rendering and interaction.

Suggested modules:
- `CanvasViewport.tsx`
- `CanvasLayer.tsx`
- `EdgeLayer.tsx`
- `NodeLayer.tsx`
- `ViewportController.ts`
- `SemanticZoom.ts`
- `CanvasCommands.ts`

## 6.5 `editor/`
Editor UI wiring and WASM bridge.

Suggested modules:
- `editorPane.ts` — editor surface container, layout, focus
- `editorTabs.ts` — tab bar, dirty indicators, tab management
- `diffPane.ts` — side-by-side and inline diff views
- `editorCommands.ts` — open, save, jump-to-line, close
- `editorWasmBridge.ts` — load WASM module, forward input events, receive render updates
- `externalEditorBridge.ts` — open in `$EDITOR`, deep links to VS Code/Cursor/Neovim

## 6.6 `panels/`
Panel docking and containers.

Suggested modules:
- `PanelManager.tsx`
- `DockLayout.tsx`
- `TerminalPanelHost.tsx`
- `BrowserPanelHost.tsx`
- `InspectorPanel.tsx`

## 7. State Model

Use a normalized graph store.

```ts
interface GraphStore {
  unitsById: Record<string, UnitViewModel>;
  factsById: Record<string, FactViewModel>;
  agentsById: Record<string, AgentViewModel>;
  artifactsById: Record<string, ArtifactViewModel>;
  edgesById: Record<string, EdgeViewModel>;
}
```

### Why normalized
- avoids duplication across rooms and views
- enables fast targeted updates
- makes event application deterministic
- simplifies derived projections

## 7.1 Local state vs shared state

### Shared, backend-driven
- units
- facts
- attempts
- verify state
- agent runtime state
- artifacts with shared meaning

### Local, frontend-driven
- zoom/pan
- card positions
- hidden layers
- panel layout
- editor tab state
- local filters
- room camera states

## 8. Rendering Strategy

## 8.1 Photon rendering

Photon provides the DOM, CSS layout, and GPU paint pipeline. Vanilla TypeScript manipulates Photon's DOM via standard APIs (`createElement`, `appendChild`, `setAttribute`). Photon renders the DOM directly to the GPU — there is no system webview, no browser engine, and no "graduation from DOM to canvas/WebGL" step.

The canvas uses:
- Photon's flexbox/grid layout for card positioning
- CSS transforms for pan and zoom
- Photon's damage tracker to repaint only changed regions
- Photon's display list for batched GPU rendering of card backgrounds, borders, and text

### Why Photon works well for this
- GPU-accelerated rendering from day one — no performance ceiling from a system webview
- Damage tracking means a hovered card doesn't repaint the entire graph
- Display list batching handles hundreds of card outlines efficiently
- We own the renderer — canvas-specific optimizations are possible (skip layout for off-screen zoom levels, batch card rendering)

## 8.2 WASM usage

The editor already uses Rust→WASM via Photon's WASM DOM bindings (ropey + tree-sitter + editing model). If vanilla TypeScript DOM manipulation becomes a bottleneck for the canvas at scale (hundreds of nodes, rapid graph updates), the graph hot path can also move to Rust WASM.

Evaluate canvas WASM after MVP 1 — likely not needed initially.

## 8.3 Rendering targets

Wizard's rendering needs are simpler than Photon's VS Code benchmark:
- Flexbox cards with text, status badges, and colored borders
- CSS transforms for semantic zoom
- Scroll containers for inspector and list views
- Mouse/keyboard events for selection and navigation
- Rust→WASM editor panes (depends on Photon's text subsystem and input handling)

## 9. Editor Architecture

## 9.1 Default mode
- Rust→WASM editor engine (ropey + tree-sitter)
- Photon renders the editor surface (monospace grid, syntax colors, cursor, selections, diff gutters)
- one editor pane can host either text or diff mode
- panes can be attached to rooms or floated

## 9.2 Supported editor workflows
- open file from card
- open file at line from verify output
- save to disk through backend command
- compare working tree vs last known file state
- review agent changes
- manual patching before rerun

## 9.3 Non-goals for v1
- full LSP parity with mature IDEs
- debugger integration
- complex refactoring UX
- extension marketplace
- vim mode (defer to external editor or future Neovim integration)

## 9.4 Future path
- optional Neovim-backed power-user mode
- richer tree-sitter integration: code folding, symbol outline, semantic navigation
- code intelligence via language server integration

## 10. Panel Architecture

Panels are identified by type and scope.

```ts
type PanelKind = "editor" | "terminal" | "browser" | "inspector" | "runtime";

type PanelScope = "global" | { roomId: string } | { unitId: string } | { agentId: string };
```

Each panel has:
- kind
- scope
- docking position
- size
- focus state
- persistence key

### Key rule
A focus room should be able to restore its own panel constellation:
- files open in editor
- room terminal session
- docs browser panel
- selected unit and camera state

## 11. Command System

The command palette should be backed by typed commands, not arbitrary callbacks.

Examples:
- `OpenUnit`
- `FocusRoom`
- `RunUnit`
- `RetryUnit`
- `OpenFile`
- `OpenDiff`
- `OpenVerifyTerminal`
- `OpenBrowserPanel`
- `PromoteNoteToFact`

This keeps keyboard workflows deterministic and testable.

## 12. Persistence

Persist local frontend state in `.wizard/`.

Suggested files:

```text
.wizard/
  state.json
  views/
    home.json
    auth-room.json
  rooms/
    auth-room.json
  panels/
    layout.json
```

Store:
- camera state
- card positions
- saved filters
- room-local panel layout
- editor tabs and dirty warnings metadata

Do not store canonical project truth here.

## 13. Frontend Failure Modes

### 13.1 Event flood
Mitigation:
- event batching
- coalesced updates
- summarized runtime stream by default

### 13.2 Component-state sprawl
Mitigation:
- keep state in app-layer modules
- minimize ad hoc local stores

### 13.3 Rendering collapse on large graphs
Mitigation:
- virtualization
- semantic zoom
- hybrid renderer path

### 13.4 Editor/panel chaos
Mitigation:
- room-scoped panel layouts
- reset layout action
- calm defaults

## 14. Near-Term Build Order

1. App shell in Photon + vanilla TypeScript (Bun backend, daemon socket connection)
2. Connection to backend snapshots/events
3. Read-only graph canvas
4. Inspector and command palette
5. Panel manager
6. WASM editor pane (ropey + tree-sitter)
7. Terminal panel host
8. Browser panel host
9. Focus rooms
10. Performance tuning and hybrid rendering if needed

## 15. Decision Summary

| Area | Decision |
|---|---|
| Shell | Photon (Zig) + Bun backend |
| UI approach | Vanilla TypeScript (on JSC via Bun) — no framework |
| Editor engine | Rust→WASM (ropey + tree-sitter) rendered by Photon |
| Terminal | libghostty (Zig-native compositing) |
| Browser | Photon rendering (progressive capability) |
| Graph rendering | Photon DOM + GPU paint (no graduation step) |
| State model | normalized graph store + explicit projections |
| Panel model | docked/floating, scope-aware, room-restorable |
| Daemon IPC | WebSocket or Unix socket via wizard-proto |
