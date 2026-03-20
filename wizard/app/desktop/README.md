# wizard desktop

This folder is the future Tauri 2 + SolidJS desktop app for Wizard.

Planned shape:
- `src/` — SolidJS application shell
- `src-tauri/` — Tauri host and native integration glue

The crate scaffolding now exists under `wizard/crates/` so the next implementation step is wiring the desktop app to `wizard-proto`, `wizard-store`, and `wizard-orch`.
