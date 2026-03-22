# Wizard — Canvas-Native Interface for Mana

Status: Draft 0.3  
Owner: Wizard project (new workspace)  
Temporary placement: `wizard/` lives inside the current mana repo for now. See `UMBRELLA.md` for the plan to move all four projects (`mana/`, `imp/`, `wizard/`, `familiar/`) under a shared `tower/` monorepo root.  
Relationship to existing vision: replaces TUI-first `wizard` with a canvas-first desktop client while preserving `wiz` CLI and headless orchestration.

## 1. Summary

Wizard is the primary local interface for navigating and operating mana. Instead of centering the experience on a terminal UI, Wizard uses an infinite project canvas with semantic zoom, typed cards, and live agent presence.

Wizard is the "bigger IDE" for the tower ecosystem: the primary unit of interest is not a file but a cluster of work made up of a unit, the agent bound to it, and the evidence around it.

The canvas is not a whiteboard. It is a visual operating surface over real mana state:
- units
- dependencies
- facts
- attempts
- verify results
- agent activity
- pinned evidence

The core architectural split remains the same:
- `mana` is the source of truth for project work state
- `imp` is the intentionally minimal worker engine
- `wizard-orch` dispatches, supervises, and streams runtime events
- Wizard desktop is the primary visual client and agent command center
- `wiz` CLI remains available for remote use, scripting, and fast ops

## 2. Product Positioning

### Problem

Mana already models work as a graph, but the dominant interaction model is still list and terminal oriented. That is strong for execution, but weak for:
- spatial memory
- returning to a project after time away
- understanding why something is blocked
- seeing how failures relate to work structure
- tracking multiple agents at once
- preserving shared evidence across attempts

### Thesis

Agentic coding is graph-native and evidence-heavy. The primary interface should reflect that.

The right mental model is not "an editor with some agent panels attached." It is an agent command center where units, agents, verify state, and artifacts are first-class and files are a secondary working surface.

A canvas is better than a TUI for:
- project overview
- decomposition
- dependency reasoning
- live multi-agent monitoring
- review and retry workflows
- long-lived working context

A TUI remains useful for:
- SSH
- low-latency text operations
- fallback when desktop UI is unavailable
- scripts and automation

### Decision

Wizard will be:
- **canvas-first** for primary local use
- **daemon-backed** for orchestration and shared runtime state
- **CLI-capable** for headless and remote workflows
- **local-first** with `.mana/` as shared project truth and `.wizard/` as personal view state

## 3. Goals

1. Make the mana graph understandable at a glance.
2. Let a human start, stop, retry, inspect, and decompose work without leaving the canvas.
3. Give agents and humans a shared working set of persistent artifacts.
4. Preserve spatial memory across sessions.
5. Keep operations keyboard-first even inside a visual interface.
6. Keep orchestration separate from presentation.
7. Support both solo and future collaborative use without changing the mana core model.
8. Make workflow state visible — spec, plan, execution, review, and verify should be inspectable as durable artifacts, not buried in chat.

## 4. Non-Goals

1. Replace all external editors entirely or require every workflow to happen inside Wizard.
2. Store all UI layout in git by default.
3. Turn the canvas into a freeform whiteboard with weak semantics.
4. Require a network service or cloud backend for single-user local operation.
5. Couple Wizard to one specific coding agent implementation.

## 5. Primary User Jobs

### Solo builder
- "What should run next?"
- "Why is this blocked?"
- "What did the agent try already?"
- "Where are the risky parts of the graph?"
- "How do I get back into this project after two days away?"

### Operator / supervisor
- "Which agents are running right now?"
- "Which unit is stuck or expensive?"
- "Can I retry this with context from the previous attempt?"
- "What changed and did verify pass?"

### Planner
- "How should this epic decompose?"
- "What are the contracts between child units?"
- "Which files and facts belong with this work area?"

## 6. Design Principles

1. **Typed, not freeform.** Every object on the canvas should have meaning.
2. **Shared truth, personal layout.** Work graph is shared; arrangement is mostly local.
3. **Semantic zoom.** Zoom level changes the representation, not just the scale.
4. **Evidence over chat.** Important discoveries should become persistent artifacts.
5. **Keyboard-first.** Mouse helps with spatial work; keyboard drives operations.
6. **Ops in context.** Run, retry, stop, inspect, and review should happen where the work lives.
7. **Local-first durability.** If the UI dies, state survives.

## 7. Core Objects

The canvas is composed of typed cards and edges.

| Object | Backing source | Purpose |
|---|---|---|
| Unit card | `.mana` unit | Primary work item |
| Dependency edge | `.mana` dependency / produces-requires relation | Blocking and flow |
| Feature card | `.mana` feature | Product-level grouping |
| Fact card | `.mana` fact | Verified project knowledge |
| Attempt card | unit history / notes | Prior failure or retry context |
| Verify card | verify history | Command result, last run, pass/fail |
| Agent card | runtime state from `wizard-orch` | Live worker presence |
| File card | pinned artifact | Relevant file or excerpt |
| Diff card | runtime/review artifact | Proposed or completed changes |
| Query card | search artifact | Saved search or semantic query result |
| Workflow card | shared artifact (spec, plan, review, playbook step) | Make engineering process visible and durable |
| Portal card | derived UI object | Entry into a focused sub-canvas |

### 7.1 Unit card fields

Every unit card must be able to show, depending on zoom and focus state:
- id
- title
- status
- priority
- attempt count
- verify state
- parent/child state
- dependency warnings
- active agent badge
- stale fact or stale context markers

### 7.2 Agent card fields

- agent id
- bound unit id
- model
- runtime duration
- current phase (`planning`, `reading`, `editing`, `verifying`, `idle`, `waiting`)
- latest tool call summary
- token/cost counters
- stop / retry / escalate actions

### 7.3 Artifact cards

Artifacts are first-class and persist when valuable. They may be:
- human-pinned
- agent-emitted
- system-generated

Artifact examples:
- file excerpt
- failing test output
- semantic search result cluster
- approved spec snippet
- implementation plan
- proposed diff
- verify transcript snippet
- review finding or checklist
- promoted note

## 8. Main Surfaces

Wizard uses one rendering system with multiple named views.

### 8.1 Project Home

Default entry point for a project.

Shows:
- major features / parent units
- ready frontier
- running work
- blocked clusters
- critical path highlight
- recent failures
- top facts

Purpose:
- answer "what is happening in this project right now?"

### 8.2 Focus Room

A scoped sub-canvas for one parent unit, epic, or selected cluster.

Shows:
- target unit and nearby dependencies
- child units
- acceptance criteria
- linked workflow artifacts (spec, plan, review notes)
- verify command and latest result
- attempt history
- pinned files, diffs, and facts
- live agent stream for selected unit

Purpose:
- answer "what do I need to understand and do for this area?"

### 8.3 Runtime View

Can be entered from any canvas selection.

Shows:
- active agents
- last event per agent
- current tool activity
- runtime timers
- token/cost usage
- queue and concurrency state
- budget alerts

Purpose:
- answer "are my imps healthy and making progress?"

### 8.4 Review View

Focused on completed or failed work.

Shows:
- changed files
- diffs
- verify results
- notes from the agent
- compare attempts
- compare implementation against approved plan/spec when present
- human review checklist

Purpose:
- answer "what changed and should I trust it?"

### 8.5 Knowledge Map

Persistent knowledge layer.

Shows:
- facts
- architecture decisions
- setup gotchas
- recurring failure modes
- linked files and units

Purpose:
- answer "what is true about this project and what keeps biting us?"

## 9. Semantic Zoom

Semantic zoom is required. The interface must change what is shown based on scale.

### Zoom level 1 — Strategic
Shows:
- features / parent units only
- aggregate status rings
- critical path
- running count badges
- blocked cluster heat

### Zoom level 2 — Tactical
Shows:
- executable units
- dependency edges
- attempt counts
- agent presence
- verify pass/fail badges

### Zoom level 3 — Operational
Shows:
- acceptance summary
- last note
- verify snippet
- attached facts and files
- current agent phase

### Zoom level 4 — Inspectable
Shows:
- full unit details in inspector
- transcript snippets
- tool call timeline
- diff previews
- exact verify output

## 10. Layout Rules

Infinite canvas only works if structure prevents chaos.

### Required defaults

1. Dependency flow defaults left → right.
2. Parent/child hierarchy defaults top → bottom.
3. Artifacts default below the unit they support.
4. Active agents sit adjacent to their unit, not in a separate universe.
5. Facts may be shown in a dedicated layer or near related units.

### Behavior

- Auto-layout is on by default.
- Users may reposition objects in local view state.
- Reset layout is always available.
- Saved views capture filters and camera state.
- The same project can have multiple named views.

## 11. Interaction Model

### Core actions

From a selected unit, the user can:
- open
- focus
- run
- run children
- retry
- stop active agent
- claim / unclaim
- verify
- add note
- add dependency
- create child
- promote note to fact
- pin file / diff / query result
- open in editor

### Drag interactions

Allowed:
- drag to reposition card locally
- drag from unit to unit to propose dependency
- drag artifact onto unit to attach evidence
- drag selection into portal to create a focus room

Not allowed:
- freeform edge drawing with no semantic meaning
- arbitrary shapes as first-class work objects in MVP

### Keyboard interactions

Must support:
- command palette
- directional navigation between nearby cards
- search
- filter toggles
- zoom in/out
- focus selection
- run / retry / stop
- open inspector
- toggle layers (facts, agents, verify, diffs)

Suggested defaults:
- `Space` command palette
- `Enter` open / inspect
- `F` focus room
- `R` run selected
- `T` retry selected
- `.` open unit actions
- `G` toggle graph emphasis
- `A` toggle agents layer
- `K` toggle knowledge layer
- `V` toggle verify layer
- `/` search
- `Esc` back / clear selection

## 12. Shared vs Local State

### Shared project state — `.mana/`

Remains source of truth for:
- units
- features
- dependencies
- facts
- attempts and notes
- verify history
- review outcomes
- shared artifacts with durable meaning

### Local user state — `.wizard/`

Wizard stores personal, non-git-critical state separately:
- card positions
- viewport and zoom
- open panels
- saved local views
- hidden layers
- selection history
- per-user preferences

Default path:

```text
wizard/.wizard/
  state.json
  views/
  cache/
```

### Shareable views

Not in MVP.

Later, allow explicit export/import of named views. Shared views must be opt-in to avoid constant git churn from layout movement.

## 13. System Architecture

Wizard becomes a new workspace rooted at `wizard/`.

### Proposed structure

```text
wizard/
  SPEC.md
  .mana/
  .wizard/
  Cargo.toml
  crates/
    wizard-orch/      # daemon: dispatch, supervision, projection, event stream
    wizard-proto/     # commands, events, snapshot types
    wizard-store/     # local view state and cache
  app/
    desktop/          # Photon shell + canvas client (TypeScript + Zig)
  docs/
```

### 13.1 `wizard-orch`

Responsibilities:
- watch `.mana/`
- compute derived graph state
- dispatch and supervise imps
- manage runtime queues, budgets, retries, hooks
- publish snapshots and event streams
- provide mutation commands to the UI and CLI

### 13.2 `wizard-proto`

Shared types for:
- graph snapshots
- runtime snapshots
- commands
- event payloads
- artifact envelopes
- review records

### 13.3 `wizard-store`

Stores:
- local layout state
- saved views
- cached projections for fast startup

### 13.4 `desktop`

Desktop app responsibilities:
- render infinite canvas via Photon
- show inspector and command palette
- subscribe to runtime events from `wizard-orch` daemon via socket
- issue commands through `wizard-proto`
- integrate built-in editor, terminal, and browser panels
- manage Photon window lifecycle, native panel compositing

The desktop shell is a Photon app — a Zig binary with Bun backend, using Photon's custom rendering engine instead of a system webview. SolidJS runs on JavaScriptCore (via Bun) against Photon's DOM. The backend daemon (`wizard-orch`) remains a separate Rust process; the desktop shell connects to it over a socket using `wizard-proto`.

## 13.5 Rendering Architecture

The desktop app runs on **Photon** — a custom rendering engine written in Zig, developed as part of the bun-shell project (`~/projects/bun-shell`). Photon replaces the system webview with a purpose-built renderer: custom DOM, CSS parser, layout engine, and GPU paint pipeline. JavaScript execution is provided by JavaScriptCore through Bun.

Wizard is Photon's flagship application — a real, complex, demanding desktop tool that proves the engine works.

```text
┌─ Wizard Desktop (Photon) ─────────────────────────────────────────┐
│                                                                    │
│  ┌─ Layer 1: Canvas + UI + Editor (Photon renderer) ──────────┐   │
│  │  Photon: custom DOM + CSS + layout + GPU paint (Zig)        │   │
│  │  JavaScript: JavaScriptCore via Bun                         │   │
│  │  SolidJS shell + canvas renderer + CodeMirror 6 editor      │   │
│  │  All typed cards, edges, semantic zoom, inspector,          │   │
│  │  command palette, status bar, editor panes                  │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                    │
│  ┌─ Layer 2: Terminal panels (libghostty) ─────────────────────┐   │
│  │  Zig-native integration — no FFI boundary                   │   │
│  │  Real PTY, GPU-accelerated, per-room persistent sessions    │   │
│  │  Composited directly by Photon's window manager             │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                    │
│  ┌─ Layer 3: Browser panels (Photon rendering) ────────────────┐   │
│  │  Same engine renders inline URL content                     │   │
│  │  Progressive: app-level HTML/CSS now, full web later        │   │
│  │  Fallback: open in external browser always available         │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                    │
│  ┌─ Daemon connection ─────────────────────────────────────────┐   │
│  │  WebSocket or Unix socket to wizard-orch (Rust daemon)      │   │
│  │  wizard-proto: commands, events, snapshots                  │   │
│  │  Same protocol used by `wiz` CLI                            │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                    │
└────────────────────────────────────────────────────────────────────┘
```

### Why Photon instead of Tauri + system webview

1. **We own the renderer.** Wizard's canvas needs (semantic zoom, hundreds of cards, animated transitions, dense graph layout) push any system webview toward its limits. On Photon, we can add canvas-specific fast paths — batch-render card outlines, skip layout for off-screen nodes, damage-track only the visible zoom level. On a system webview, we hope Apple or Microsoft optimize for our use case.

2. **Terminal integration is Zig-native.** libghostty is Zig. Photon is Zig. No FFI boundary, no Tauri native view compositing dance, no `wizard-terminal` wrapper crate translating between Rust and Zig. Terminal panels are first-class native views composited directly by the same process.

3. **Browser panels converge naturally.** Photon's roadmap (bun-shell Phase 3) grows into a browser engine. Wizard's browser panels — PR preview, docs, dev server — evolve from limited app-level rendering to full web rendering as Photon matures. No separate engine bolted on.

4. **Dogfooding drives quality.** Wizard is a complex, long-lived, performance-sensitive desktop application. Building it on Photon means every Wizard need becomes a Photon priority. Real usage surfaces real bugs faster than synthetic benchmarks.

5. **Full stack ownership.** The entire tower ecosystem — mana, imp, wizard, and now the rendering substrate — is code we control. No external framework gates our progress.

### Layer 1 — Canvas UI + Editor (Photon renderer)

The primary surface. Renders the entire canvas, all typed cards, edges, inspector, command palette, status bar, and the built-in editor panes. Photon provides the DOM, CSS resolution, layout, and GPU paint pipeline. JavaScript runs on JavaScriptCore through Bun.

**Framework decision:** use **SolidJS** for the shell and UI bindings. Solid compiles to direct DOM API calls (`createElement`, `appendChild`, `setAttribute`) — no virtual DOM, no abstraction layer. These standard DOM APIs are exactly what Photon implements via its JS DOM bindings. Solid's signal-based reactivity is a natural fit for Wizard's fine-grained state: selection, zoom, card expansion, agent presence, runtime updates.

**Built-in editor decision:** use **CodeMirror 6** inside the same rendering layer. CodeMirror is DOM-native and needs precise text measurement, fast DOM mutation, and editor-grade input handling (IME, clipboard, selection). Photon's Phase 2 roadmap targets exactly these capabilities for the VS Code benchmark. Keep a path for an optional Neovim-backed power-user mode later.

The canvas renderer benefits from Photon's architecture:
- **GPU-accelerated rendering** via Metal (macOS), Vulkan (Linux), DirectX (Windows) — smooth zoom and pan at scale
- **Damage tracking** — only repaint regions that changed (a hovered card doesn't repaint the entire graph)
- **Display list pipeline** — Photon's paint system can batch hundreds of card backgrounds, borders, and text runs efficiently
- **Virtualized rendering** — semantic zoom naturally maps to Photon's layout: different zoom levels render different DOM subtrees

Unlike a system webview where DOM-first rendering is a starting point that may need to graduate to canvas/WebGL, Photon IS the GPU renderer. The DOM is rendered directly to the GPU paint pipeline. There is no graduation step.

**Future option — WASM DOM for the graph hot path:**

Photon's WASM DOM bindings allow any language compiled to WASM to manipulate the DOM directly, without JavaScript. This opens an architecturally unique option: the performance-critical canvas layer (graph layout, edge drawing, semantic zoom transitions) could be written in Rust, compiled to WASM, and drive Photon's DOM directly. SolidJS handles the UI chrome (inspector, command palette, panels); Rust WASM handles the rendering hot path.

This is an optimization path, not a requirement. SolidJS-only on Photon works for MVP.

### Layer 2 — Terminal panels (libghostty)

libghostty is the terminal library extracted from Ghostty. It provides a real, PTY-backed, GPU-accelerated terminal emulator designed for embedding.

**Why libghostty:**
- Real PTY — not a web simulation of a terminal
- GPU-rendered text — matches or exceeds standalone terminal performance
- Proper Unicode, ligatures, true color, sixel — developers expect these
- **Zig-native compositing** — Photon and libghostty are both Zig. The terminal panel is composited directly by Photon's window manager with zero FFI overhead. No wrapper crate, no C ABI translation layer.

**Terminal panel types:**

| Panel type | Purpose | Lifecycle |
|---|---|---|
| Room terminal | Persistent shell tied to a focus room | Lives as long as the room is open |
| Agent terminal | Live PTY stream from a running imp | Created on agent spawn, kept after exit for review |
| Verify terminal | Runs a verify command on demand | Created on `V` keypress, dismissed after review |
| Quick terminal | Ad-hoc shell from command palette | User-managed, like VS Code's integrated terminal |

**Behavior:**
- Terminal panels can be docked (bottom, right) or floating
- Each focus room remembers its terminal state (cwd, scroll position, session)
- Agent terminals show the imp's actual stdout/stderr as a real terminal stream, not parsed log cards
- When an agent exits, its terminal becomes read-only and scrollable for review
- `Ctrl+\`` or a configurable key toggles the terminal panel

**Integration with canvas:**
- Selecting a unit and pressing a terminal shortcut opens a shell pre-cd'd to the project root
- Selecting a running agent card and pressing a terminal shortcut attaches to that agent's output stream
- Verify terminal results are capturable as verify cards on the canvas

### Layer 3 — Browser panels (Photon rendering)

For viewing URLs inline without leaving the app. Unlike the previous Tauri architecture that bolted on secondary system webviews, Wizard uses Photon itself to render URL content.

**Progressive capability:**

- **Now (Photon Phase 2):** Photon renders modern HTML/CSS — sufficient for localhost dev servers, simple docs pages, and structured content. Complex sites fall back to external browser.
- **Later (Photon Phase 3):** Photon becomes a browser engine with full networking, expanded CSS, and JavaScript Web APIs. Browser panels render GitHub PRs, library docs, and most modern websites natively.
- **Always available:** "Open in external browser" is one click away for anything Photon cannot yet render.

**Browser panel types:**

| Panel type | Purpose | Triggered by |
|---|---|---|
| PR preview | Show a GitHub PR from a completed unit | Unit card action or review view |
| Dev server | Show localhost for a running service | Command palette or room config |
| Docs | Show library docs or project wiki | Fact card link or command palette |
| URL preview | Show any URL referenced in a unit or fact | Link click in inspector |

**Behavior:**
- Browser panels render inline, not in a separate browser chrome
- No address bar, no bookmarks, no navigation UI by default — just the rendered page
- A minimal toolbar appears on hover: back, forward, reload, open in external browser, close
- Browser panels can be docked or floating, same as terminal panels
- URLs opened from unit cards or fact cards remember their association — reopening the card reopens the URL

### Layer coordination

All layers run in the same Photon process. The Zig binary manages:
- Window lifecycle and native platform integration
- Photon renderer: DOM, layout, paint, GPU compositing
- libghostty PTY lifecycle: spawn, resize, destroy, compose into window
- Panel layout and docking state (stored in `.wizard/`)
- Bun runtime: hosts SolidJS app code, provides JSC for DOM scripting

The UI communicates with the `wizard-orch` daemon over a socket using `wizard-proto`. This is the same protocol the `wiz` CLI uses — the desktop app is just another client of the daemon. The daemon manages orchestration, graph projection, and imp supervision; the desktop app manages rendering, input, and local view state.

### IPC architecture

```text
┌─ Photon Desktop ─────────────────────────────┐
│  SolidJS (JSC) ←→ Bun backend (TypeScript)   │
│                    │                          │
│                    │ WebSocket / Unix socket   │
│                    ▼                          │
│              wizard-proto client              │
└───────────────┬───────────────────────────────┘
                │
                ▼
┌─ wizard-orch daemon (Rust) ───────────────────┐
│  .mana/ watcher                               │
│  graph projection                             │
│  imp dispatch + supervision                   │
│  snapshot + event publishing                  │
│  wizard-proto server                          │
└───────────────────────────────────────────────┘
```

The Bun backend in the Photon process acts as the bridge: it receives user intent from SolidJS via Photon's internal bridge, translates it into `wizard-proto` commands, and forwards them to the daemon. Incoming events and snapshots from the daemon flow back through Bun into the SolidJS reactive state.

This separation means:
- The daemon survives UI closure (headless orchestration continues)
- Multiple clients can connect (`wiz` CLI, desktop app, future web client)
- The desktop shell is a replaceable client, not the whole system

## 14. Runtime Model

Wizard is backed by a daemon, even for local desktop use.

### Why a daemon

It allows:
- continuous orchestration while UI is closed
- clean separation between runtime and rendering
- multiple clients in the future (`wiz`, desktop app, maybe web)
- better supervision and logging

### CLI remains

`wiz` stays as a thin ops surface:

```text
wiz             # open desktop app or attach to running daemon
wiz daemon      # run orchestration headless
wiz status      # text summary from current project
wiz logs 1.2    # stream a unit's logs
wiz open 1.4    # focus a specific unit in the app
```

## 15. Event Model

Wizard requires both snapshots and incremental events.

### 15.1 Snapshot types

- `ProjectSnapshot`
- `GraphSnapshot`
- `RuntimeSnapshot`
- `SelectionContext`
- `ReviewSnapshot`

### 15.2 Event types

Minimum event set:
- `ProjectLoaded`
- `GraphChanged`
- `UnitCreated`
- `UnitUpdated`
- `UnitStatusChanged`
- `DependencyAdded`
- `DependencyRemoved`
- `FactCreated`
- `FactStale`
- `AttemptRecorded`
- `VerifyStarted`
- `VerifyFinished`
- `AgentSpawned`
- `AgentUpdated`
- `AgentExited`
- `BudgetAlert`
- `ArtifactPinned`
- `ViewStateChanged`

### 15.3 Artifact envelope

Artifacts should share a common shape:
- id
- kind
- source (`human`, `agent`, `system`)
- related unit ids
- summary
- payload reference
- created_at
- staleness / superseded markers

## 16. Agent-Human Shared Working Set

This is the key product bet.

Important findings should not disappear into chat logs. Wizard should make it easy for an agent or human to create durable, source-backed artifacts such as:
- "this function is the auth choke point"
- "this exact test failure blocks unit 2.3"
- "these three files define the payment flow"
- "this verify failure has already been tried twice"
- "this is the approved plan for this unit"
- "this review note is why the previous attempt was rejected"

This is also where workflow methodology becomes durable. Practices such as brainstorming, planning, debugging, TDD, and review should show up as visible artifacts and checkpoints, not only as hidden prompt text inside one agent session.

### MVP rule

In MVP, agents do not need fully automatic artifact creation from every tool call. It is enough to support:
- manual pin from logs or inspector
- manual pin from search results
- manual pin from file preview
- system-generated verify and attempt artifacts

### Later

Add richer agent-emitted artifacts from tool events.

## 17. File, Editor, Terminal, and Browser Integration

Wizard coordinates code work. The canvas remains the primary surface, but Wizard now includes an integrated editor, terminal, and browser where they directly serve the coordination workflow.

### Editor integration (CodeMirror 6)

Wizard includes a built-in editor for focused code work.

Required actions:
- open files directly from unit cards, file cards, diffs, and search results
- edit and save files without leaving the app
- open file at path and line from cards, logs, or verify output
- preview file excerpts in inspector (read-only, syntax highlighted)
- preview and navigate diffs without leaving the app
- compare before/after state for agent-edited files
- split editor panes within a focus room when needed

The built-in editor is for:
- quick and medium-sized edits
- reviewing and refining agent output
- guided changes while watching agent/runtime context nearby
- diff review and patch application

The built-in editor is not trying to fully replace a standalone IDE. External editor handoff still matters.

Required external-editor actions:
- open file in default editor (`$EDITOR` or configured IDE)
- open file at path and line when available (deep link into VS Code, Cursor, Neovim, etc.)
- "open selection in external editor" from the canvas or built-in editor

Future path:
- optional Neovim-backed power mode for users who want a real editor core with modal editing and existing config

### Terminal integration (libghostty — Zig-native)

The canvas includes real terminal panels for workflows that need a shell:
- run verify commands manually and see live output
- watch agent output as a real terminal stream
- quick shell access without leaving the app
- persistent per-room terminals that remember state

Because Photon and libghostty are both Zig, terminal panels are composited directly by the same process with no FFI boundary. See §13.5 Layer 2 for full terminal architecture.

### Browser integration (Photon rendering)

The canvas includes inline URL panels for context that lives on the web:
- PR previews from completed units
- dev server output at localhost
- library docs linked from facts
- GitHub views for repos and issues

Browser panels are rendered by Photon itself. Initially limited to modern HTML/CSS (sufficient for localhost and structured docs). As Photon matures toward Phase 3 (browser engine), these panels gain full web rendering. External browser fallback is always available. See §13.5 Layer 3 for full browser architecture.

### What wizard explicitly does not do

- Full embedded IDE with code editing, LSP, and debugging
- Full browser with tabs, extensions, and developer tools
- Full terminal multiplexer replacing tmux or screen
- Any of these as the primary surface — the canvas is always primary

## 18. Review and Retry Workflow

### Failed verify flow

When verify fails:
1. unit stays open
2. verify card turns red with summary
3. attempt card is recorded under the unit
4. related logs and diff become reviewable
5. user may retry, decompose, or annotate

### Successful verify flow

When verify passes:
1. unit card turns success state
2. changed files are still reviewable
3. parent progress updates immediately
4. feature readiness updates if relevant

### Retry requirements

Every retry must surface:
- prior attempts
- latest verify failure
- last note
- changed files from last run if available

## 19. Visual Language

The interface should feel like a tool for builders, not a consumer productivity app.

Desired qualities:
- dense but calm
- graph legibility first
- restrained color with status accents
- animation only when it improves state awareness
- readable in dark mode first, but themeable

Status colors should communicate:
- ready
- running
- blocked
- failed
- passed
- stale knowledge

Motion should communicate:
- active agent heartbeat
- new event arrival
- verify running
- dependency unblock

## 20. MVP Scope

### MVP 1 — Read-only graph client on Photon
- Photon renders Wizard's canvas UI (SolidJS on JSC)
- socket connection to `wizard-orch` daemon for graph snapshots
- render mana graph on canvas (Photon DOM + GPU paint)
- semantic zoom (4 levels)
- filters and inspector panel
- saved local views in `.wizard/`
- keyboard navigation and command palette

This is a simpler rendering target than Photon's VS Code benchmark — mostly flexbox cards with text, status badges, and CSS transforms for zoom. Validates the Photon + SolidJS + daemon architecture end-to-end.

### MVP 2 — Operational controls + terminal + editor
- run / retry / stop / verify from canvas
- live runtime bar with agent status
- active agent cards
- libghostty integration: Zig-native terminal compositing, verify terminal and quick terminal
- agent output streaming to terminal panel
- built-in CodeMirror 6 editor: open, edit, save, jump-to-line (depends on Photon editor-grade input handling)
- editor panes attached to focus rooms and file cards

### MVP 3 — Review and evidence
- attempt cards
- verify cards with captured output
- diff preview in inspector and editor
- file pinning and query pinning
- browser panels for localhost dev servers and structured docs (Photon rendering)

### MVP 4 — Focus rooms + room terminals
- portal into parent unit or cluster
- local layouts per room
- persistent per-room terminal sessions
- per-room browser panels (dev server, docs)
- fast navigation between project home and focus rooms

### MVP 5 — Richer agent artifacts + full browser panels
- pin from tool events
- better clustering of evidence around units
- agent terminal review (scroll back through completed agent sessions)
- browser panels gain full web rendering as Photon Phase 3 matures (PR preview, GitHub, library docs)
- WASM DOM optimization for canvas hot path if needed

## 21. Success Criteria

Wizard is successful when a user can:
1. answer what is ready, running, and blocked in under 10 seconds
2. restart context on a project after 48 hours without reading raw unit files first
3. inspect a failed agent attempt and decide next action in under 30 seconds
4. launch, retry, or stop work in 1–2 actions from the selected unit
5. trust that closing the app does not lose orchestration state

## 22. Risks

### Canvas chaos
Mitigation:
- strong auto-layout
- typed objects only
- focus rooms
- saved views
- semantic zoom

### Too much UI state in git
Mitigation:
- keep local layout state in `.wizard/`
- only explicit shared views become versioned later

### Runtime / UI overcoupling
Mitigation:
- daemon boundary
- shared protocol crate
- keep orchestration headless-capable

### Agent artifact noise
Mitigation:
- manual pinning first
- aggressive summarization
- promote only high-value artifacts to persistent state

### Photon coupling
Wizard depends on Photon for rendering. If Photon hits a hard wall on a feature Wizard needs, both projects stall.
Mitigation:
- we own both projects — Wizard's needs become Photon's priorities
- Wizard's initial rendering needs (flexbox cards, text, transforms, scroll, events) are simpler than Photon's VS Code benchmark target
- the two projects co-evolve: Wizard surfaces gaps, Photon fills them
- daemon architecture means the backend is independent of the rendering layer — if Photon is ever abandoned, the desktop shell can be rebuilt on another substrate without touching wizard-orch, wizard-proto, or mana

## 23. Decisions and Remaining Open Questions

### Resolved decisions

1. ~~Should shared views eventually live in `.mana/views/` or a dedicated export format?~~ **Decision: personal views live in `.wizard/views/`; explicitly shared views export into `.mana/views/`.**
2. ~~Do we want the desktop canvas implemented with Tauri + web canvas immediately, or stage with a lighter Rust-native prototype?~~ **Decision (revised): Photon rendering engine + libghostty for terminals + Photon browser panels. No Tauri, no system webview, no Gecko, no Chromium, no xterm.js. Wizard is Photon's flagship application.**
3. ~~How much low-level tool activity should be exposed by default vs on demand?~~ **Decision: three levels — default summarized runtime, expandable per-agent detail, and raw debug/event stream on demand.**
4. ~~Should facts be visually separate from work, or embedded directly into work clusters?~~ **Decision: hybrid — separate knowledge map at overview level, embedded facts inside focus rooms.**
5. ~~Should `wiz` launch the desktop app by default, or remain CLI-first with `wiz open` for GUI?~~ **Decision: open or attach to the desktop app on local GUI sessions; fall back to CLI behavior in headless, SSH, or no-GUI environments.**
6. ~~Which web framework for the canvas UI layer? React (ecosystem), Solid (performance), Svelte (simplicity), or Leptos (Rust-native via WASM)?~~ **Decision: SolidJS for the shell and UI bindings. Solid compiles to direct DOM API calls, which map cleanly onto Photon's JS DOM bindings.**
7. ~~libghostty integration path — wrapper crate, Tauri native views, or direct Zig integration?~~ **Decision (revised): Zig-native integration. Photon and libghostty are both Zig — terminal panels are composited directly by Photon's window manager with zero FFI overhead. No wrapper crate needed.**
8. ~~Should agent terminal output be a real PTY passthrough from the imp process, or a reconstructed stream from structured events?~~ **Decision: both — live PTY passthrough for rich runtime viewing, plus structured capture for persistence, search, and artifact creation.**
9. ~~Where should shared Wizard project config live without polluting `.wizard/` local state?~~ **Decision: shared project config lives in `<project>/.wizard.toml`; `.wizard/` stays local-only and user defaults live in `~/.config/wizard/config.toml`.**
10. ~~How does the desktop shell communicate with the wizard-orch daemon?~~ **Decision: WebSocket or Unix socket using wizard-proto. Same protocol the `wiz` CLI uses. The Bun backend in the Photon process acts as the bridge between SolidJS and the Rust daemon.**
11. ~~Should browser panels use a separate rendering engine (system webview, Gecko, Chromium)?~~ **Decision: No. Browser panels use Photon itself. Progressive capability — app-level HTML/CSS now, full web rendering as Photon Phase 3 matures. External browser fallback always available.**

### Remaining open questions

1. Should the optional power-user editor mode be Neovim-backed, or is CodeMirror 6 plus strong keyboard workflows enough for v1 and v2?
2. Should docked panels (editor, terminal, browser, inspector) share one unified layout manager, or should the canvas own its own layout and treat panels as secondary surfaces?
3. Should the canvas graph hot path use Rust → WASM → Photon DOM for performance, or is SolidJS-only sufficient? Evaluate after MVP 1.
4. What is the minimum Photon feature set needed for Wizard MVP 1? (Flexbox, text, transforms, scroll, mouse/keyboard events — likely already covered by current Photon Phase 2 progress.)
5. Should the Bun↔daemon socket use WebSocket (simpler, HTTP-compatible) or Unix domain socket (faster, local-only)?

## 24. Initial Mana Breakdown

If this folder becomes its own mana project, seed it like this:
- `1` Feature: Canvas-native Wizard for mana
- `1.1` Write concrete product and technical spec
- `1.2` Define snapshot + event protocol for wizard-orch
- `1.3` Build read-only canvas with semantic zoom
- `1.4` Add operational controls (run, retry, stop, verify)
- `1.5` Add review artifacts (attempts, verify, diffs, pins)
- `1.6` Add focus rooms and saved local views

## 25. Placement Decision

For now:
- create `wizard/` inside the current repo
- keep `SPEC.md` as the canonical spec
- initialize a fresh local `.mana/` under `wizard/`
- let the root feature be unit `1`

Later:
- move `wizard/` to the `tower/` monorepo root alongside `mana/`, `imp/`, and `familiar/` (see `UMBRELLA.md`)
- keep the same internal structure when it graduates — the wizard spec is already written against the target layout
