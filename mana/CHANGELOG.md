# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-03-18

### Added
- Accept `P0`–`P4` format for `--priority` flag (in addition to numeric)

### Fixed
- Verify-on-claim ignoring `--pass-ok` when `fail_first=false`

### Changed
- Improved `mana run` progress output and updated docs terminology
- Tightened `chrono` and `regex` version floors above known CVEs
- README examples diversified beyond auth domain
- Replaced comparison chart with Spec Kit, GSD, and Ralph loop sections

## [0.2.0] - 2026-03-01

### Added
- File locking to prevent concurrent agent writes
- Atomic file writes for crash safety
- CONTRIBUTING.md
- **Agent orchestration** — `mana run` dispatches ready units to agents with ready-queue scheduling
- **Loop mode** — `mana run --loop-mode` continuously dispatches until no work remains
- **Auto-planning** — `mana run --auto-plan` decomposes large units before dispatch
- **Adversarial review** — `mana run --review` spawns a second agent to verify correctness
- **Agent monitoring** — `mana agents` and `mana logs` for observing running agents
- **Memory system** — `mana fact` for verified project knowledge with TTL and staleness detection
- **Memory context** — `mana context` (no args) outputs stale facts, in-progress units, recent completions
- **MCP server** — `mana mcp serve` for IDE integration (Cursor, Windsurf, Claude Desktop, Cline)
- **Library API** — `lib.rs` module with core type re-exports for use as a Rust crate
- **Interactive wizard** — `mana create` with no args launches step-by-step prompts (fuzzy parent search, smart verify suggestions, `$EDITOR` for descriptions)
- **Sequential chaining** — `mana create next` auto-depends on the most recently created unit
- **Trace command** — `mana trace` walks unit lineage, dependencies, artifacts, and attempt history
- **Recall command** — `mana recall` searches units by keyword across open and archived units
- **Pipe-friendly output** — `--json`, `--ids`, `--format` on list/show/verify/context commands
- **Stdin input** — `--description -`, `--notes -`, `--stdin` for batch operations
- **Batch close** — `mana close --stdin` reads IDs from stdin
- **Failure escalation** — `--on-fail "retry:3"` and `--on-fail "escalate:P0"` for verify failures
- **Config inheritance** — `extends` field for shared config across projects
- **Shell completions** — `mana completions` for bash, zsh, fish, and PowerShell
- **Agent presets** — `mana init --agent` with presets for Claude, pi, and aider
- **File context extraction** — `mana context <id>` extracts files referenced in unit descriptions
- **Structure-only context** — `mana context --structure-only` for signatures and imports only
- **Unarchive** — `mana unarchive` restores archived units
- **Lock management** — `mana locks` views and clears file locks
- **Quick create** — `mana quick` creates and claims a unit in one step
- **Status overview** — `mana status` shows claimed, ready, and blocked units
- **Context command** — `mana context` assembles file context from unit descriptions
- **Edit command** — `mana edit` opens units in `$EDITOR` with schema validation and backup/rollback
- **Hook system** — pre-close hooks with `mana trust` for managing hook execution
- **Smart selectors** — `@latest`, `@blocked`, `@parent`, `@me` resolve to unit IDs dynamically
- **Verify-as-spec** — units without a verify command are treated as goals, not tasks
- **Auto-suggest verify** — detects project type (Cargo.toml, package.json) and suggests verify commands
- **Fail-first enforcement** — verify must fail on create (on by default), `--pass-ok` to skip
- **Agent liveness** — `mana status` shows whether claimed units have active agents
- **Better failure feedback** — verify failures show actionable output
- **Acceptance criteria** — `--acceptance` field for human-readable done conditions
- **Core CLI** — `mana init`, `mana create`, `mana show`, `mana list`, `mana close`
- **Verification gates** — every unit has a verify command that must pass to close
- **Hierarchical tasks** — dot notation (`3.1` is a child of `3`), `mana tree` for visualization
- **Smart dependencies** — `produces`/`requires` fields with auto-inference and cycle detection
- **Dependency graph** — `mana graph` with ASCII, Mermaid, and DOT output
- **Task lifecycle** — `mana claim`, `mana close`, `mana reopen`, `mana delete`
- **Failure tracking** — attempts counter, failure output appended to unit notes
- **Ready/blocked queries** — `mana ready` and `mana blocked` filter by dependency state
- **Dependency management** — `mana dep add/remove/list/tree/cycles`
- **Index engine** — cached index with `mana sync` for rebuild and `mana doctor` for health checks
- **Project stats** — `mana stats` for unit counts and status breakdown
- **Tidy command** — `mana tidy` archives closed units, releases stale claims, rebuilds index
- **Markdown format** — units stored as `.md` files with YAML frontmatter
- **Slug-based filenames** — `{id}-{slug}.md` naming convention
- **Archive system** — closed units auto-archive to `.mana/archive/YYYY/MM/`
- **Git-native** — all state in `.mana/` directory, clean diffs, works offline

### Changed
- Improved robustness for parallel agent workflows
- Package renamed from `bn` to `mana-cli` for crates.io publication
- Improved help text and README for all current commands
- Improved `mana show` rendering with better formatting
- README rewritten with table of contents and consolidated documentation

### Removed
- `mana ready` — use `mana status` (shows ready units in the Ready section, `--json` for scripting)
- `mana blocked` — use `mana status` (shows blocked units in the Blocked section)
- `mana dep tree` — use `mana graph` (richer output with ASCII, Mermaid, DOT formats)
- `mana dep cycles` — use `mana doctor` (runs cycle detection among other health checks)

### Fixed
- `mana context` crash on corrupt archive YAML
- Missing `rules_file` and `memory` fields in test struct literals
- Shell escaping in verify commands
- File extension preservation during archiving
- `.md` format support in dep and verify commands

[Unreleased]: https://github.com/kfcafe/mana/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/kfcafe/mana/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/kfcafe/mana/releases/tag/v0.2.0
