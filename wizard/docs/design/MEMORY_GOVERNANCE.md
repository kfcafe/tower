# Memory Governance — System-Wide Agent Resource Management

Status: Design  
Owner: wizard-orch  
Related: `mana run` memory gate (implemented), wizard-proto `ProcessMetrics`/`AgentInfo`

## Problem

Agent processes are memory-hungry and unpredictable. A single `imp` or `pi` process with a large context window, loaded tools, and tree-sitter parsers can easily consume 200–500MB RSS. Running 8 agents on a 16GB machine while also compiling Rust in parallel causes swap thrashing, OS instability, and agent timeouts.

Today, `mana run` has a static `max_concurrent` cap (default 4) and a basic memory reserve gate that checks system-available memory before each spawn. This prevents the worst OOM scenarios but is blind to:
- Per-agent memory consumption over time
- Memory growth during execution (context accumulation)
- Cross-project coordination (two `mana run` instances don't know about each other)
- Proactive memory pressure response (can only refuse to spawn, not shed load)

## Current State (v1 — shipped in mana)

`mana run` checks available system memory before spawning each agent:
- Config: `memory_reserve_mb` in `.mana/config.yaml` (default 0 = disabled)
- When enabled, the ready queue calls `sysinfo::System::available_memory()` before each spawn
- If available memory is below the reserve, dispatch pauses until a running agent finishes
- If no agents are running and memory is still tight, dispatch exits with a clear message
- `max_concurrent` remains as a hard ceiling on top

This uses the OS as the shared state — if two `mana run` instances are going, they both see the same available memory. Simple, no coordination needed.

## Future Vision — wizard-orch as Memory Governor

When wizard-orch becomes the active runtime supervisor, it should own system-wide memory governance. The design below describes what that looks like.

### Architecture

```
┌─────────────────────────────────────────────┐
│                 wizard-orch                   │
│                                               │
│  ┌───────────┐  ┌────────────┐  ┌─────────┐ │
│  │  Memory    │  │  Process   │  │  Budget  │ │
│  │  Monitor   │  │  Tracker   │  │  Policy  │ │
│  └─────┬─────┘  └─────┬──────┘  └────┬────┘ │
│        │              │               │       │
│        └──────┬───────┘               │       │
│               ▼                       │       │
│     ┌─────────────────┐              │       │
│     │   Admission     │◄─────────────┘       │
│     │   Controller    │                       │
│     └────────┬────────┘                       │
│              │                                │
└──────────────┼────────────────────────────────┘
               │
    ┌──────────┼──────────┐
    ▼          ▼          ▼
 mana run   mana run   mana run
 (project A) (project B) (ad-hoc)
```

### Components

#### Memory Monitor
Polls system memory at a configurable interval (default 2s). Tracks:
- Total system memory
- Available memory (Linux `MemAvailable`, macOS `total - used` fallback)
- Swap usage and swap-in rate
- Memory pressure indicators (PSI on Linux, `memory_pressure` on macOS)

Emits `Event::MemoryPressure { level, available_mb, timestamp }` when thresholds are crossed.

#### Process Tracker
Tracks all agent child processes by PID. For each:
- RSS (resident set size) — actual physical memory used
- VMS (virtual memory size) — total address space
- CPU usage percentage
- Process tree (agent → child builds, test runners, etc.)

Data source: `/proc/<pid>/stat` on Linux, `proc_pidinfo` on macOS, or `sysinfo::Process`.

Updates `AgentInfo.memory_usage` and `AgentInfo.cpu_usage` with real values (replacing current mocks).

#### Budget Policy
Configurable memory budget that answers "how much can agents use total?"

Modes:
- **Auto** (default): Reserve a percentage of total RAM for the OS + user apps. Agents share the rest. Example: on a 16GB machine with 25% reserve, agents get up to 12GB total.
- **Fixed**: User specifies a hard budget in MB. Example: `agent_memory_budget_mb: 8192`.
- **Unlimited**: No budget enforcement (equivalent to today's `memory_reserve_mb: 0`).

The budget is global — shared across all projects on the machine.

#### Admission Controller
The decision point. When `mana run` wants to spawn an agent, it consults the admission controller:

```
Request: "I want to spawn agent for unit 5.3, estimated 300MB"
         ↓
Check 1: Is system available memory above reserve? (current v1 logic)
Check 2: Is total agent RSS below budget?
Check 3: Are any agents trending toward OOM? (RSS growth rate)
         ↓
Response: Admit / Wait / Deny { reason }
```

The controller can also provide back-pressure hints: "you can spawn, but use a smaller context window" or "switch to a lighter model."

### Communication Protocol

wizard-orch exposes the admission controller via a Unix socket at `~/.wizard/orch.sock`:

```
→  { "type": "admit", "project": "tower", "unit_id": "5.3", "estimated_mb": 300 }
←  { "type": "admitted", "budget_remaining_mb": 4200 }

→  { "type": "register", "project": "tower", "unit_id": "5.3", "pid": 12345 }
←  { "type": "ok" }

→  { "type": "release", "project": "tower", "unit_id": "5.3" }
←  { "type": "ok" }
```

If the socket doesn't exist (wizard-orch not running), `mana run` falls back to the v1 local memory check. No hard dependency.

### Pressure Response Levels

| Level | Trigger | Action |
|-------|---------|--------|
| **Normal** | Available > 50% of budget | Spawn freely up to max_concurrent |
| **Cautious** | Available 25–50% of budget | Spawn only high-priority units, log warning |
| **Pressure** | Available 10–25% of budget | Don't spawn new agents, wait for completions |
| **Critical** | Available < 10% of budget or swap-in detected | Consider killing lowest-priority agent |

The "kill lowest-priority agent" at Critical is a last resort. Agents don't checkpoint, so killing one loses all work. But it's better than the OS OOM-killing random processes.

### Per-Agent Memory Estimation

Over time, wizard-orch can build a model of agent memory usage:
- Track RSS at spawn, 1min, 5min, completion for each agent
- Record model used, context window size, tools loaded
- Build per-model memory profiles: "Sonnet agents average 350MB, Haiku agents average 150MB"
- Use these profiles for admission estimates instead of fixed 300MB

This data lives in `~/.wizard/agent_profiles.json` and improves over time.

### Cross-Project Coordination

Since wizard-orch is a per-machine daemon, it naturally coordinates across projects:
- Project A's `mana run` registers 3 agents
- Project B's `mana run` asks to spawn 2 more
- Admission controller sees 3 already running, checks budget, decides

No distributed system needed. The Unix socket is the coordination point.

### Integration with wizard UI

The wizard desktop canvas should surface memory governance visually:
- System memory bar in the runtime panel (already scaffolded in `main.ts`)
- Per-agent memory usage next to each running agent card (types already in `wizard-proto`)
- Color coding: green/yellow/red based on pressure level
- "Memory constrained" indicator when dispatch is being throttled

### Migration Path

1. **v1 (shipped)**: `memory_reserve_mb` in mana config. OS-level check. No daemon.
2. **v2**: wizard-orch starts collecting real per-process metrics (replace mocks in `collect_process_metrics`). Agents still use v1 gate.
3. **v3**: wizard-orch exposes admission controller socket. `mana run` tries socket first, falls back to v1.
4. **v4**: Full budget policy, memory profiling, pressure response levels.

### Open Questions

- Should wizard-orch manage CPU scheduling too, or just memory? (Memory is the scarier resource — OOM kills are catastrophic, high CPU just slows things down.)
- Should the memory budget be per-user or per-machine? (Per-machine for single-user; revisit for familiar/multi-user.)
- Should agents be able to request a memory increase mid-execution? ("I need to load a large file into context" → admission controller approves or denies.)
- How does this interact with container/cgroup limits if someone runs mana inside Docker?

### Non-Goals

- Remote agent management (that's familiar's job)
- GPU memory management (future concern for local model inference)
- Network bandwidth management
