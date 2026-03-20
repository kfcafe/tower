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
- **Tauri 2** desktop shell
- system webview for primary UI rendering

## UI framework
- **SolidJS**

### Why SolidJS

Wizard is a long-lived desktop tool with dense, fine-grained state:
- selection
- hover state
- zoom state
- card expansion
- runtime status changes
- per-room layout state
- panel docking state
- editor state
- agent presence

Solid's signal-based reactivity fits this better than React's rerender-centered model. The code stays more local, more explicit, and easier to reason about in an agent-authored codebase.

## Editor
- **CodeMirror 6** as the default integrated editor

### Why CodeMirror 6
- easy to embed in the same webview layer as the canvas
- strong extension model
- good enough for real editing, diff review, and patching
- simpler and less risky than embedding Neovim, Helix, or Zed as the default editor engine

## Graph rendering
- **DOM-first** for MVP
- **hybrid DOM + canvas/WebGL** when performance requires it

## Browser panels
- Tauri secondary webviews

## Terminal panels
- native panel managed by backend via libghostty

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
CodeMirror wrapper and editor-specific behaviors.

Suggested modules:
- `EditorPane.tsx`
- `EditorTabs.tsx`
- `DiffEditorPane.tsx`
- `editorCommands.ts`
- `codeMirrorExtensions.ts`
- `externalEditorBridge.ts`

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

## 8.1 MVP rendering

Start with DOM-based rendering for cards and overlays.

Use CSS transforms for:
- pan
- zoom
- selection framing
- transitions

### Why start simple
- easier to implement
- easier to debug
- easier for agents to modify
- likely enough for small and medium graphs

## 8.2 Graduation path

Move to hybrid rendering when metrics say it is time.

Use canvas/WebGL for:
- edge rendering
- large node fields
- animated overview layers

Keep DOM/HTML for:
- card content
- inspector
- command palette
- editor panes
- panel chrome

## 8.3 Trigger to upgrade

Graduate from DOM-first when one or more becomes true:
- frame rate drops below target on medium-sized graphs
- node count makes DOM interactions sluggish
- animated transitions become visibly inconsistent

## 9. Editor Architecture

## 9.1 Default mode
- CodeMirror 6
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

## 9.4 Future path
- optional Neovim-backed power-user mode
- richer code intelligence via language server integration

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

1. App shell in Tauri + SolidJS
2. Connection to backend snapshots/events
3. Read-only graph canvas
4. Inspector and command palette
5. Panel manager
6. CodeMirror editor pane
7. Terminal panel host
8. Browser panel host
9. Focus rooms
10. Performance tuning and hybrid rendering if needed

## 15. Decision Summary

| Area | Decision |
|---|---|
| Shell | Tauri 2 |
| UI framework | SolidJS |
| Default editor | CodeMirror 6 |
| Terminal | libghostty via native panel |
| Browser | Tauri secondary webviews |
| Graph rendering | DOM-first, hybrid later |
| State model | normalized graph store + explicit projections |
| Panel model | docked/floating, scope-aware, room-restorable |
