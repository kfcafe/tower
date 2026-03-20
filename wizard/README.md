# wizard

wizard is the command center for the Tower ecosystem.

It owns:
- the canvas-native interface
- orchestration and supervision
- runtime monitoring
- integrated editor, terminal, and browser surfaces
- focus rooms and spatial navigation

## Key docs
- `SPEC.md`
- `FRONTEND_ARCHITECTURE.md`
- `BACKEND_ARCHITECTURE.md`
- `FULLSTACK_ARCHITECTURE.md`
- `../VISION.md`
- `../UMBRELLA.md`

## Current state

Wizard now has initial Rust workspace scaffolding under `wizard/crates/`:
- `wizard-proto`
- `wizard-store`
- `wizard-terminal`
- `wizard-browser`
- `wizard-orch`

It is still early and mostly architecture/spec-driven, but it is no longer spec-only.
