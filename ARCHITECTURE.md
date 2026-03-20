# Tower Architecture Map

This is a short orientation map for the full ecosystem.

## Core idea

The ecosystem is split into layers with clear responsibilities.

```text
familiar   → team platform
wizard     → command center / bigger IDE
imp        → worker runtime
mana       → coordination substrate
```

## What each layer owns

### mana
Durable shared truth:
- units
- dependencies
- facts
- verify history
- attempts
- artifacts

### imp
Single-worker execution:
- agent loop
- tool use
- context and session handling
- code changes
- verify execution

### wizard
Human supervision and orchestration:
- canvas-native UI
- orchestration daemon
- runtime monitoring
- editor/terminal/browser surfaces
- focus rooms and spatial memory

### familiar
Team-level expansion:
- approvals
- integrations
- remote workflows
- dashboard/platform features

## Main data flow

```text
mana (.mana/)  ← canonical project work state
   ↑
   ├── imp reads/writes it while executing work
   ├── wizard reads/projects it for navigation and orchestration
   └── familiar uses the same model at team/platform scope
```

## Key principle

The primary object is not the file.
The primary object is the **work graph**.

Files, diffs, terminals, browser panels, and agent sessions all attach to that graph.

## Root docs

For more detail:
- `VISION.md`
- `UMBRELLA.md`
- `AGENTS.md`
- `wizard/SPEC.md`
- `wizard/FULLSTACK_ARCHITECTURE.md`
