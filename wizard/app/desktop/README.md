# wizard desktop

Tauri 2 + SolidJS desktop app for Wizard.

## Current State

This is a **read-only shell** that demonstrates Wizard's core architectural concepts:

- Loading project snapshots from `.mana/` directories  
- Managing local state in `.wizard/` directories
- Separating shared project data from personal UI preferences

The current implementation uses mock data loaders. Real integration with `wizard-proto`, `wizard-store`, and `wizard-orch` will come in future iterations.

## Structure

- `src/` — SolidJS application shell
- `src-tauri/` — Tauri host and native integration glue (planned)

## What This Is Not (Yet)

- ❌ Full canvas interface
- ❌ Agent orchestration UI  
- ❌ Live project editing
- ❌ Real Tauri IPC integration

## What This Demonstrates

- ✅ Non-placeholder entry point
- ✅ Wizard-aware shell structure
- ✅ ProjectSnapshot and WizardLocalState concepts
- ✅ Read-only project inspection
- ✅ Clear separation between .mana/ and .wizard/

## Next Steps

1. Wire to real `wizard-orch` project loading
2. Add Tauri IPC for native integration  
3. Build minimal canvas view
4. Add agent status monitoring