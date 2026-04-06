---
id: '1'
title: 'Implement Ghosttyfox as a clean open-source Firefox terminal-tab project'
slug: implement-ghosttyfox-open-source-firefox-terminal-tab
status: open
priority: 1
created_at: '2026-04-19T21:20:00Z'
updated_at: '2026-04-19T21:32:00Z'
labels:
- project
- firefox
- terminal
- ghostty
- open-source
- release
kind: epic
verify: "test -f /Users/asher/tower/ghosttyfox/.mana/1.1-extension-ui-and-bundling.md && test -f /Users/asher/tower/ghosttyfox/.mana/1.2-rust-native-host-and-protocol.md && test -f /Users/asher/tower/ghosttyfox/.mana/1.3-installation-docs-and-integration-polish.md"
---
Goal: deliver Ghosttyfox as a polished standalone repo at `/Users/asher/tower/ghosttyfox`.

Current state:
- `package.json` exists and dependencies are installed.
- `ghostty-web` is present in `node_modules`.
- directories exist but implementation files are still missing.
- a draft `README.md` exists but should be replaced with finished public-facing docs.

Intent:
- this repo should be publishable as open source.
- code should feel deliberate, compact, and cohesive.
- avoid generated-looking abstractions and unnecessary layers.

Architecture:
```text
Firefox extension tab
  ├── ghostty-web renderer (WASM)
  ├── terminal page JS bridge
  └── native messaging port
            ↓
Rust native host
  ├── framed stdin/stdout JSON
  ├── PTY management
  ├── shell process
  └── resize + output relay
```

Implementation decisions already settled:
1. Firefox Manifest V2.
2. `ghostty-web` in the extension page, not direct libghostty embedding into Firefox.
3. Rust native host with `portable-pty`.
4. One native host process per terminal tab.
5. macOS-only installer for now.

Important confirmed research:
- `ghostty-web` exports `Ghostty`, `Terminal`, and `FitAddon`.
- `Ghostty.load(wasmPath)` is the reliable initialization path.
- `FitAddon.observeResize()` exists.
- Firefox native messaging uses 4-byte little-endian length prefix + UTF-8 JSON.

Child units:
- `1.1` — extension UI and bundling
- `1.2` — Rust native host and protocol
- `1.3` — installation, docs, and integration polish

Execution order:
1. complete `1.1`
2. complete `1.2`
3. complete `1.3`

Repo-wide standards:
- consistent naming: `Ghosttyfox` / `ghosttyfox`
- no stale `ghostty-firefox` names
- minimal comments, only where protocol or browser behavior is non-obvious
- no framework, no TS migration, no speculative config layer
- no TODO placeholders left behind

Out of scope:
- Linux/Windows installer support
- settings UI
- tab/session persistence
- AMO signing/publishing automation

Do not:
- add the project to the Tower workspace
- switch to MV3
- add a frontend framework
- over-abstract the JS or Rust code
- leave placeholder files once the unit is done
