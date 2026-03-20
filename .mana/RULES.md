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

## Documentation
- Update root docs when cross-project behavior or ownership changes
- Update project docs when the change is local to one project
