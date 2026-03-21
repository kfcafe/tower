# Tower Rules

## Scope
- Root `.mana/` is for cross-project and ecosystem-level work only
- Project-local work belongs in the project's own `.mana/`

## Ownership
- `mana/` owns durable work-state concepts
- `imp/` owns worker execution
- `wizard/` owns supervision and interface
- `familiar/` owns team/platform concerns

## Working style
- Prefer small, focused changes
- Preserve boundaries between projects
- When a change spans projects, explain the contract between them
- Treat `~/tower` as the primary working root for agents

## Verification
- Prefer workspace-level verification only when the change is truly cross-project
- Otherwise use the smallest meaningful project or crate-level check
- Use `cargo check -p <crate>` not `cargo check` for verify gates
- Use pre-built binaries (e.g. `../target/debug/imp`) instead of `cargo run -p`
- Never run parallel cargo commands against the same workspace — they deadlock on the build lock

## Build
- sccache is enabled via `.cargo/config.toml` — do not override `rustc-wrapper`
- Keep crate files under 300 lines; split into modules when they grow past that
- Keep `wizard-proto` types-only — changes cascade to every wizard consumer
- Avoid adding heavy dependencies without justification — each one adds compile time

## Documentation
- Update root docs when cross-project behavior or ownership changes
- Update project docs when the change is local to one project
