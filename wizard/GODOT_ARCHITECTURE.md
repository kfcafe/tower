# Wizard — Godot Architecture

Wizard is the agent command center for the Tower ecosystem. A desktop application where one human sees, dispatches, and monitors hundreds of AI coding agents working across projects — organized around verified tasks with dependency edges, not chat conversations.

Built in Godot 4 with godot-rust (gdext) for the Rust integration layer.

---

## Why Godot

Every developer tool on the market is either a web app or an Electron wrapper. They feel like dashboards. Wizard should feel like a **command center** — a place you inhabit, where work is visible, alive, and spatial.

Game engines build experiences. Web frameworks build applications. Wizard is an experience.

Concretely, Godot provides:

- **GraphEdit/GraphNode** — built-in node graph editor with connections, ports, pan, zoom, drag. This IS Wizard's hero feature.
- **Shaders and animation** — nodes pulse when agents think, glow when verify passes, crack on failure. No CSS animation library competes.
- **Sound design** — optional ambient audio. A chime when work completes. A hum when agents are active. Makes the command center feel alive.
- **60fps custom rendering** — smooth camera, no jank, GPU-accelerated. The graph feels physical.
- **~30-50MB native binary** — instant startup, no Chromium, no runtime. Real software.
- **Cross-platform** — macOS, Windows, Linux from the same project.
- **godot-rust (gdext)** — direct Rust integration via GDExtension. wizard-orch links directly, no HTTP/WebSocket serialization layer.

---

## System Architecture

```
┌────────────────────────────────────────────────────────────┐
│                     Wizard (Godot App)                       │
│                                                              │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                   Godot Scenes (GDScript)            │    │
│  │                                                     │    │
│  │  DAG View · Agent Panel · Task Inspector            │    │
│  │  Command Bar · Settings · Project Picker            │    │
│  │                                                     │    │
│  │  Rendering, animation, input, sound, UI             │    │
│  └────────────────────────┬────────────────────────────┘    │
│                           │ signals / method calls           │
│  ┌────────────────────────▼────────────────────────────┐    │
│  │              GDExtension (Rust — wizard-bridge)      │    │
│  │                                                     │    │
│  │  Connection management to wizard-orch               │    │
│  │  Event stream → Godot signals                       │    │
│  │  Commands (dispatch, stop, create) → wizard-orch    │    │
│  │  State cache (current snapshot of mana graph)       │    │
│  │  mana-core types exposed as Godot resources         │    │
│  └────────────────────────┬────────────────────────────┘    │
└───────────────────────────┼──────────────────────────────────┘
                            │ Unix socket / local TCP
┌───────────────────────────▼──────────────────────────────────┐
│                    wizard-orch (Rust daemon)                   │
│                                                              │
│  Watches .mana/ directories                                  │
│  Dispatches imp agents                                       │
│  Manages concurrency, scheduling, budget                     │
│  Streams events to connected clients                         │
│  Runs independently — survives Wizard restarts               │
└──────────────────────────────────────────────────────────────┘
                            │ spawns
                    ┌───────▼───────┐
                    │  imp agents   │
                    │  (Rust binary) │
                    └───────────────┘
```

### Separation of concerns

**Godot app** — Purely visual. Renders the graph, plays animations, handles input, shows agent output. All state comes from the bridge. The Godot layer never reads `.mana/` directly or spawns agents.

**wizard-bridge (GDExtension)** — The Rust integration crate. Maintains a connection to wizard-orch. Translates orch events into Godot signals. Translates user actions into orch commands. Caches the current mana graph state for the UI to query.

**wizard-orch (daemon)** — The brain. Watches `.mana/`, resolves dependencies, dispatches agents, monitors results, enforces budget/concurrency. Runs as a separate process. Multiple clients can connect (Wizard desktop, `wiz` CLI, future web dashboard).

### Why this split matters

- wizard-orch continues working if Wizard crashes or closes
- wizard-orch can be started independently (`wiz daemon`)
- The GDExtension is a thin translation layer, not business logic
- The same wizard-orch serves the Godot app, the CLI, and eventually Familiar

---

## Project Structure

```
wizard/
├── godot/                          # Godot project root
│   ├── project.godot               # Godot project config
│   ├── export_presets.cfg          # macOS, Windows, Linux export configs
│   │
│   ├── scenes/
│   │   ├── main.tscn              # Root scene — workspace container
│   │   ├── dag_view.tscn          # The hero — GraphEdit-based DAG
│   │   ├── unit_node.tscn         # Custom GraphNode for a mana unit
│   │   ├── agent_panel.tscn       # Agent output streaming panel
│   │   ├── task_inspector.tscn    # Selected task detail/edit
│   │   ├── command_bar.tscn       # Quick actions, search, dispatch
│   │   ├── project_picker.tscn   # Open/switch project
│   │   ├── settings.tscn         # Configuration
│   │   └── components/
│   │       ├── log_viewer.tscn    # Scrollable rich text log
│   │       ├── tool_call.tscn     # Expandable tool call display
│   │       ├── status_badge.tscn  # Status indicator (open/running/passed/failed)
│   │       └── cost_display.tscn  # Token/cost readout
│   │
│   ├── scripts/
│   │   ├── main.gd                # Root workspace logic
│   │   ├── dag_view.gd            # DAG rendering, layout, interaction
│   │   ├── unit_node.gd           # Unit node behavior, animations, state
│   │   ├── agent_panel.gd         # Agent stream handling
│   │   ├── task_inspector.gd      # Task detail and editing
│   │   ├── command_bar.gd         # Command palette logic
│   │   ├── project_picker.gd      # Project selection
│   │   ├── settings.gd            # Settings management
│   │   ├── theme_manager.gd       # Dark/light theme, color constants
│   │   └── sound_manager.gd       # Audio cues
│   │
│   ├── shaders/
│   │   ├── node_pulse.gdshader    # Running state — soft pulse
│   │   ├── node_glow.gdshader     # Passed state — green glow
│   │   ├── node_crack.gdshader    # Failed state — crack/shake
│   │   ├── edge_flow.gdshader     # Dependency edge — data flow animation
│   │   └── node_blocked.gdshader  # Blocked state — dimmed
│   │
│   ├── themes/
│   │   ├── dark.tres              # Dark theme (default)
│   │   └── colors.gd              # Status color constants
│   │
│   ├── audio/
│   │   ├── task_complete.wav      # Subtle chime on verify pass
│   │   ├── task_failed.wav        # Low tone on failure
│   │   ├── dispatch.wav           # Whoosh on agent dispatch
│   │   └── ambient_working.wav    # Optional low ambient when agents active
│   │
│   └── fonts/
│       ├── mono.ttf               # Monospace for code/logs
│       └── ui.ttf                 # UI font
│
├── crates/
│   └── wizard-bridge/             # GDExtension Rust crate
│       ├── Cargo.toml
│       ├── src/
│       │   ├── lib.rs             # GDExtension entry point
│       │   ├── bridge.rs          # Connection to wizard-orch
│       │   ├── events.rs          # Event types, signal emission
│       │   ├── commands.rs        # Command types sent to orch
│       │   ├── state.rs           # Cached mana graph state
│       │   └── resources.rs       # Godot Resource wrappers for mana types
│       └── wizard_bridge.gdextension  # GDExtension config
│
├── crates/
│   ├── wizard-orch/               # Existing orchestration daemon
│   ├── wizard-proto/              # Shared types (events, commands, snapshots)
│   ├── wizard-store/              # Local view state and cache
│   ├── wizard-terminal/           # Terminal wrapper (future)
│   └── wizard-browser/            # Browser panel (future)
│
├── SPEC.md                        # Product spec
├── GODOT_ARCHITECTURE.md          # This document
└── README.md
```

---

## Core Scenes

### Main Scene (`main.tscn`)

The root workspace. A container that manages the layout of all panels.

```
Main (Control — full screen)
├── TopBar (HBoxContainer)
│   ├── ProjectName (Label)
│   ├── AgentCount (Label — "4 running")
│   ├── CostDisplay (Label — "$0.34 this session")
│   └── SettingsButton (Button)
├── HSplitContainer
│   ├── DAGView (GraphEdit — main area, ~70% width)
│   └── VSplitContainer (right panel, ~30% width)
│       ├── TaskInspector (top — selected task detail)
│       └── AgentPanel (bottom — agent output stream)
└── CommandBar (hidden until invoked — Ctrl+K or /)
```

The split is resizable. The DAG view is always visible. The right panels show contextual detail for whatever is selected.

### DAG View (`dag_view.tscn`)

The hero scene. A `GraphEdit` node containing `UnitNode` instances connected by dependency edges.

```
DAGView (GraphEdit)
├── UnitNode_1 (custom GraphNode)
├── UnitNode_2 (custom GraphNode)
├── UnitNode_3 (custom GraphNode)
└── ... (dynamically created from mana state)
```

GraphEdit provides: pan, zoom, selection, minimap, snapping, connection drawing. We customize: node appearance, edge styling, layout algorithm, real-time updates.

### Unit Node (`unit_node.tscn`)

A custom `GraphNode` representing one mana unit.

```
UnitNode (GraphNode)
├── StatusIndicator (ColorRect with shader)
├── TitleLabel (Label — task title, truncated)
├── StatusBadge (Label — "running", "passed", "failed")
├── AgentIndicator (TextureRect — shows if agent is active)
├── CostLabel (Label — "$0.05" — hidden if zero)
└── AttemptDots (HBoxContainer — dots showing attempt history)
```

Visual states driven by shaders and AnimationPlayer:

| State | Visual treatment |
|-------|-----------------|
| **Open** | Neutral, slightly dimmed. Ready to be worked on. |
| **Blocked** | Grayed out, dependency edges highlighted. Can't start yet. |
| **Running** | Pulsing shader (soft breathing glow). Agent indicator visible. |
| **Passed** | Green glow shader, brief particle burst on transition. Solid. |
| **Failed** | Red tint, subtle shake animation. Attempt dot turns red. |
| **Closed (manually)** | Same as passed but dimmer. |

### Agent Panel (`agent_panel.tscn`)

Shows streaming output from one or more running agents.

```
AgentPanel (VBoxContainer)
├── TabBar (tabs for each active agent, or "All")
├── LogViewer (RichTextLabel — scrollable, BBCode formatted)
│   ├── Tool calls (expandable — click to see input/output)
│   ├── Agent thinking (dimmed text)
│   ├── Agent output (normal text)
│   └── Verify result (green/red highlighted)
└── BottomBar (HBoxContainer)
    ├── TokenCount (Label)
    ├── CostDisplay (Label)
    └── KillButton (Button — stops this agent)
```

### Task Inspector (`task_inspector.tscn`)

Detail view for the selected unit. Shows when you click a node in the DAG.

```
TaskInspector (VBoxContainer)
├── Title (LineEdit — editable)
├── Status (StatusBadge)
├── VerifyCommand (CodeEdit — editable, monospace)
├── Description (TextEdit — editable, markdown)
├── Dependencies (list of produces/requires)
├── AttemptHistory (VBoxContainer — collapsible per attempt)
├── Actions (HBoxContainer)
│   ├── DispatchButton ("Run")
│   ├── RetryButton ("Retry")
│   └── EditButton ("Edit")
└── Notes (TextEdit — agent notes from previous attempts)
```

### Command Bar (`command_bar.tscn`)

A floating command palette (like Ctrl+K in VS Code). Hidden by default.

```
CommandBar (PanelContainer — centered, floating)
├── SearchInput (LineEdit — type to filter)
└── ResultList (ItemList — filtered commands)
```

Commands include:
- `Dispatch all ready` — dispatch all open units with resolved dependencies
- `Stop all agents` — kill switch
- `Create unit` — new task
- `Open project...` — switch project
- `Focus on unit...` — search and zoom to a unit
- `Fit graph` — zoom to fit all nodes
- `Toggle sound` — enable/disable audio cues
- Settings, help, etc.

---

## DAG Visualization Deep Dive

### Layout algorithm

Mana units form a DAG through produces/requires dependencies. The layout algorithm arranges nodes by dependency depth:

```
Level 0 (no dependencies):     [Unit A]  [Unit B]  [Unit C]
                                    │         │
Level 1 (depends on level 0):  [Unit D]  [Unit E]
                                    │
Level 2:                        [Unit F]
```

Implementation:
1. Topological sort of units by dependency edges
2. Assign depth level to each unit
3. Horizontal spread within each level (minimize edge crossings)
4. Vertical spacing between levels
5. Animate positions when units are added/removed/rearranged

Auto-layout runs on initial load and when the graph structure changes. Users can drag nodes to override positions (stored in `.wizard/` local state).

### Edge rendering

Dependency edges are `GraphEdit` connections with custom styling:

| Edge state | Visual |
|------------|--------|
| Normal dependency | Thin gray line, subtle arrow |
| Active (data flowing) | Animated shader — particles or dashes moving along the edge |
| Satisfied (dependency met) | Solid line, slightly brighter |
| Blocking (dependency not met) | Dashed line, dimmed |

The `edge_flow.gdshader` creates the "data flowing" animation — a gradient or particle effect that moves from producer to consumer when a unit completes.

### Camera and navigation

- **Pan** — Middle mouse drag, or Space + left drag
- **Zoom** — Scroll wheel, or pinch on trackpad
- **Fit to graph** — Double-click empty space, or `F` key
- **Focus on node** — Double-click a node, or search in command bar
- **Minimap** — GraphEdit has a built-in minimap (bottom-right corner)

Camera transitions are smoothly animated (Tween), not instant jumps.

### Real-time updates

The GDExtension (wizard-bridge) emits Godot signals when wizard-orch sends events:

```gdscript
# dag_view.gd
func _ready():
    WizardBridge.unit_status_changed.connect(_on_unit_status_changed)
    WizardBridge.unit_created.connect(_on_unit_created)
    WizardBridge.unit_removed.connect(_on_unit_removed)
    WizardBridge.agent_started.connect(_on_agent_started)
    WizardBridge.agent_output.connect(_on_agent_output)
    WizardBridge.agent_completed.connect(_on_agent_completed)

func _on_unit_status_changed(unit_id: String, old_status: String, new_status: String):
    var node = _unit_nodes.get(unit_id)
    if node:
        node.set_status(new_status)
        if new_status == "passed":
            _play_completion_effect(node)
            SoundManager.play("task_complete")
```

---

## Agent Monitoring Deep Dive

### Log viewer

A `RichTextLabel` with BBCode formatting. Agent output is streamed line-by-line from wizard-orch events.

Formatting:
- **Tool calls** — highlighted header with tool name, collapsible body
- **Agent thinking** — dimmed italic text
- **Code blocks** — monospace with background tint
- **Verify output** — green background on pass, red on fail
- **Errors** — red text
- **Timestamps** — dimmed, right-aligned

```bbcode
[color=#888]12:04:32[/color] [b]Reading[/b] src/api/users.rs
[color=#888]12:04:33[/color] [b]Editing[/b] src/api/users.rs
[color=#666][i]Adding pagination parameters to the query...[/i][/color]
[color=#888]12:04:35[/color] [b]Running verify[/b] cargo test api::users::pagination
[bgcolor=#1a3a1a][color=#4a4]✅ All 4 tests passed[/color][/bgcolor]
```

### Tool call visualization

Tool calls expand on click to show input and output:

```
▶ edit src/api/users.rs                      [$0.02]
  (click to expand)

▼ bash cargo test api::users::pagination     [$0.01]
  │ Running 4 tests...
  │ test api::users::pagination::test_first_page ... ok
  │ test api::users::pagination::test_next_page ... ok
  │ test api::users::pagination::test_empty ... ok
  │ test api::users::pagination::test_invalid_cursor ... ok
  │
  │ test result: ok. 4 passed; 0 failed
```

### Multiple agent streams

When multiple agents run simultaneously, the panel has tabs:

```
[ All ] [ Unit 5.1 - pagination ] [ Unit 5.2 - auth ] [ Unit 5.3 - tests ]
```

"All" shows interleaved output from all agents with unit labels. Individual tabs show one agent's stream.

---

## GDExtension Integration Layer

### wizard-bridge crate

```toml
# wizard-bridge/Cargo.toml
[package]
name = "wizard-bridge"
version.workspace = true

[lib]
crate-type = ["cdylib"]

[dependencies]
godot = "0.2"          # gdext — Rust bindings for Godot 4
wizard-proto.workspace = true
mana-core.workspace = true
tokio = { version = "1", features = ["rt", "net", "sync"] }
serde_json = "1"
```

### Bridge class

Exposed to Godot as a singleton autoload:

```rust
// bridge.rs
use godot::prelude::*;

#[derive(GodotClass)]
#[class(base=Node)]
pub struct WizardBridge {
    connection: Option<OrchestratorConnection>,
    state: ManaGraphState,
    base: Base<Node>,
}

#[godot_api]
impl WizardBridge {
    // Signals emitted to Godot
    #[signal] fn unit_status_changed(unit_id: GString, old_status: GString, new_status: GString);
    #[signal] fn unit_created(unit_id: GString);
    #[signal] fn unit_removed(unit_id: GString);
    #[signal] fn agent_started(unit_id: GString, model: GString);
    #[signal] fn agent_output(unit_id: GString, line: GString, line_type: GString);
    #[signal] fn agent_completed(unit_id: GString, success: bool, cost: f64);
    #[signal] fn connection_status_changed(connected: bool);

    // Commands called from Godot
    #[func] fn connect_to_orch(&mut self, socket_path: GString);
    #[func] fn disconnect(&mut self);
    #[func] fn dispatch_unit(&self, unit_id: GString);
    #[func] fn dispatch_all_ready(&self);
    #[func] fn stop_unit(&self, unit_id: GString);
    #[func] fn stop_all(&self);
    #[func] fn create_unit(&self, title: GString, verify: GString, description: GString);
    #[func] fn get_units(&self) -> Array<Dictionary>;
    #[func] fn get_unit(&self, unit_id: GString) -> Dictionary;
    #[func] fn get_edges(&self) -> Array<Dictionary>;
    #[func] fn open_project(&mut self, path: GString);
}
```

### Event flow

```
wizard-orch → Unix socket → wizard-bridge (Rust) → Godot signal → GDScript handler → UI update
```

Events are JSON-newline delimited over the socket. The bridge parses them in a background tokio task and queues signal emissions on the Godot main thread via `call_deferred`.

### Command flow

```
User clicks "Dispatch" → GDScript calls WizardBridge.dispatch_unit("5.1") → Rust sends command over socket → wizard-orch dispatches agent
```

Commands are fire-and-forget with optional response events.

---

## Communication Protocol

### wizard-orch ↔ wizard-bridge

**Transport:** Unix domain socket (macOS/Linux) or named pipe (Windows). Path: `~/.wizard/orch.sock` or configured.

**Format:** Newline-delimited JSON. Simple, debuggable, no binary protocol complexity.

**Events (orch → bridge):**

```json
{"type": "unit_status", "unit_id": "5.1", "old": "open", "new": "running", "ts": "..."}
{"type": "agent_output", "unit_id": "5.1", "line": "Reading src/main.rs", "kind": "tool", "ts": "..."}
{"type": "agent_completed", "unit_id": "5.1", "success": true, "cost": 0.12, "duration_s": 45, "ts": "..."}
{"type": "graph_snapshot", "units": [...], "edges": [...], "ts": "..."}
```

**Commands (bridge → orch):**

```json
{"cmd": "dispatch", "unit_id": "5.1"}
{"cmd": "dispatch_ready"}
{"cmd": "stop", "unit_id": "5.1"}
{"cmd": "stop_all"}
{"cmd": "create", "title": "...", "verify": "...", "description": "..."}
{"cmd": "open_project", "path": "/Users/asher/myproject"}
{"cmd": "subscribe"}
```

**Connection lifecycle:**
1. Bridge connects to socket
2. Sends `subscribe` command
3. Orch responds with `graph_snapshot` (full current state)
4. Orch streams events as they happen
5. Bridge sends commands as user interacts
6. Reconnection with exponential backoff if socket drops

---

## UX Flow

### First launch

1. Wizard opens with a project picker: "Open a project directory"
2. User selects a directory containing `.mana/`
3. Wizard checks if wizard-orch is running — starts it if not
4. Bridge connects, receives graph snapshot
5. DAG renders with all units positioned by dependency depth
6. Recent projects remembered for quick switching

### Main workflow

```
See the graph → Spot open/ready units → Dispatch → Watch agents work → Verify passes → Done
```

1. **See** — The DAG shows everything. Open units are neutral. Blocked units are dimmed. Running units pulse.
2. **Dispatch** — Click a unit and press "Run", or use command bar: "Dispatch all ready"
3. **Watch** — Running nodes pulse. Agent panel shows streaming output. Tool calls expand.
4. **Verify** — When a verify gate passes, the node glows green. Brief particle effect. Chime.
5. **React** — Failed nodes shake red. Click to see what went wrong. Retry or edit.

### Keyboard shortcuts

| Key | Action |
|-----|--------|
| `/` or `Ctrl+K` | Open command bar |
| `D` | Dispatch selected unit |
| `Shift+D` | Dispatch all ready units |
| `Escape` | Deselect / close panel |
| `F` | Fit graph to view |
| `R` | Retry selected (failed) unit |
| `X` | Stop selected (running) unit |
| `Shift+X` | Stop all running units |
| `N` | Create new unit |
| `Tab` | Switch focus: DAG ↔ Inspector ↔ Agent Panel |
| `1-9` | Switch agent panel tabs |

### Sound design

All sounds are optional (toggle in settings). They serve a purpose: you can monitor work without watching the screen.

| Event | Sound | Purpose |
|-------|-------|---------|
| Agent dispatched | Subtle whoosh | Confirmation of action |
| Verify passed | Soft chime (pleasant, brief) | Know work completed without looking |
| Verify failed | Low tone | Know something needs attention |
| All tasks complete | Distinct completion tone | Session finished |
| Agents working (ambient) | Very quiet hum/texture | Awareness that work is happening |

Design rule: every sound should be something you'd leave on for 8 hours. If it would annoy you in an hour, it's wrong.

---

## Visual Design Direction

### Color language

```
Background:     #1a1a2e (deep dark blue-black)
Surface:        #16213e (slightly lighter)
Border:         #0f3460 (subtle blue border)

Open:           #888888 (neutral gray)
Blocked:        #555555 (dimmed)
Running:        #e94560 (warm coral-red — active, alive)
Passed:         #53d769 (green — success)
Failed:         #ff6b6b (red — error)

Text primary:   #e0e0e0
Text secondary: #888888
Text code:      #a8d8ea (light blue for code/monospace)

Edges:          #333355 (default)
Edges active:   #e94560 (same as running — data flowing)
Edges satisfied:#53d769 (same as passed)
```

### Animation philosophy

- **Purposeful** — Every animation communicates state. Nothing moves for decoration.
- **Subtle** — Pulses are gentle breathing, not disco lights. Glows fade in, not flash.
- **Satisfying** — The moment a verify gate passes should feel GOOD. Brief particle burst, clean color transition, pleasant chime. This is the payoff for the whole system.
- **Fast** — Transitions complete in 200-400ms. Nothing blocks interaction.

### Typography

- **UI text** — Clean sans-serif (Inter, SF Pro, or system default), 13-14px
- **Code/logs** — Monospace (JetBrains Mono, Fira Code, or similar), 12-13px
- **Node titles** — UI font, bold, 12px (must fit in compact node)
- **Status badges** — UI font, uppercase, 10px

---

## MVP Scope

### Phase 1 — The graph that works (4-6 weeks)

This is the shippable $30 product.

- [ ] Godot project setup with GDExtension scaffold
- [ ] wizard-bridge crate connecting to wizard-orch via Unix socket
- [ ] DAG view rendering mana units as GraphNodes with dependency edges
- [ ] Auto-layout by dependency depth
- [ ] Unit status visualization (open, blocked, running, passed, failed)
- [ ] Running state shader (pulse animation)
- [ ] Passed state shader (green glow)
- [ ] Failed state visual (red tint + shake)
- [ ] Click node → Task Inspector shows detail
- [ ] Dispatch button (single unit or all ready)
- [ ] Stop button (single unit or all)
- [ ] Agent panel with streaming output (RichTextLabel)
- [ ] Tool calls as expandable sections
- [ ] Command bar (Ctrl+K) with basic commands
- [ ] Project picker (open .mana/ directory)
- [ ] Keyboard shortcuts (dispatch, stop, focus, fit)
- [ ] Sound effects (task complete, failed, dispatch)
- [ ] Dark theme
- [ ] macOS export (signed binary)
- [ ] Windows export
- [ ] Linux export

### Phase 2 — Polish and power (weeks 7-10)

- [ ] Edge flow animation (shader showing data movement)
- [ ] Particle effects on verify pass
- [ ] Create unit from within Wizard
- [ ] Edit unit description and verify command inline
- [ ] Attempt history display in task inspector
- [ ] Multiple agent tabs in agent panel
- [ ] Token/cost display per agent and total
- [ ] Minimap toggle
- [ ] Settings screen (sound, theme, orch connection)
- [ ] Recent projects list
- [ ] Auto-start wizard-orch on launch
- [ ] Auto-update mechanism
- [ ] License key activation

### Phase 3 — Command center (weeks 11-16)

- [ ] Multi-project support (switch between projects)
- [ ] Budget controls (max concurrent agents, cost cap)
- [ ] Scheduling controls (dispatch rules, auto-retry)
- [ ] Agent history (past sessions, cost rollup)
- [ ] Unit creation wizard (title → verify → description → deps)
- [ ] Drag-to-connect dependencies
- [ ] Graph filtering (show only running, show only failed, etc.)
- [ ] Export graph as image
- [ ] Light theme option

### Future

- [ ] Embedded terminal (run commands without leaving Wizard)
- [ ] Embedded code viewer (read files from the project)
- [ ] Familiar cloud connection (cloud agents alongside local)
- [ ] Collaborative mode (multiple Wizards connected to same orch)
- [ ] Mobile companion (view status from phone)
- [ ] Plugin system (custom node types, custom panels)

---

## Build & Distribution

### Godot export

Godot 4 exports to native binaries via export templates:

- **macOS** — `.app` bundle, code-signed and notarized for Gatekeeper
- **Windows** — `.exe` with optional installer (NSIS or similar)
- **Linux** — AppImage or `.tar.gz`

The GDExtension (wizard-bridge) compiles as a dynamic library (`.dylib`, `.dll`, `.so`) that ships inside the Godot export.

### Build pipeline

```
1. cargo build --release -p wizard-bridge    → libwizard_bridge.dylib
2. Copy to godot/addons/wizard_bridge/
3. Open Godot project → export for each platform
4. Code-sign macOS build
5. Package for distribution
```

Automate with CI (GitHub Actions):
- Build wizard-bridge for all targets (macOS arm64/x86, Windows, Linux)
- Run Godot export in headless mode
- Sign and notarize macOS binary
- Upload artifacts to release

### License and activation

The $30 buy-once model needs a simple activation system:

- **Purchase** — Gumroad, Paddle, or Stripe checkout
- **License key** — emailed after purchase
- **Activation** — enter key in Wizard settings, verified against a simple API
- **Offline grace** — works offline for 30 days between checks
- **No DRM** — honor system with gentle reminders. Piracy is marketing.

### Auto-update

Wizard checks for updates on launch (optional). If an update is available:
- Shows notification in the top bar
- User clicks to download
- Replaces binary and restarts

Implementation: GitHub releases API → download latest → replace binary. Simple, no framework needed.

---

## Relationship to Other Tower Components

### wizard-orch

Already exists as a Rust crate in the Tower workspace. Wizard's Godot app is a client of wizard-orch, not a replacement. The orch daemon is the source of truth for agent dispatch, scheduling, and state management.

wizard-orch needs to be extended with:
- Unix socket server (accept connections from Wizard and `wiz` CLI)
- Event streaming protocol (JSON-newline)
- Command protocol (dispatch, stop, create, etc.)

### wiz CLI

The `wiz` command-line tool is another client of wizard-orch. It provides quick status, dispatch, and monitoring from any terminal. Same protocol as Wizard's bridge — just text output instead of visual rendering.

### mana

Wizard reads mana state through wizard-orch, not directly. The orch daemon watches `.mana/` and streams state changes. This means Wizard doesn't need mana-core as a dependency — the bridge only needs wizard-proto types.

### imp

Wizard never talks to imp directly. wizard-orch spawns and monitors imp agents. Wizard sees agent activity through orch events.

### Familiar

When Familiar exists as a cloud platform, Wizard gains a "cloud mode" — connecting to a Familiar instance instead of (or alongside) a local wizard-orch. The bridge supports both connection types. The Godot UI doesn't change.
