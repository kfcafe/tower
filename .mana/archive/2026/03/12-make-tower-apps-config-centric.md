---
id: '12'
title: Make Tower apps config-centric
slug: make-tower-apps-config-centric
status: closed
priority: 1
created_at: '2026-03-21T07:50:48.585398Z'
updated_at: '2026-03-21T08:18:21.814175Z'
labels:
- tower
- config
- architecture
- cross-project
closed_at: '2026-03-21T08:18:21.814175Z'
close_reason: 'Auto-closed: all children completed'
verify: rg -n "Configuration philosophy|config-centric|wizard config|familiar config" README.md VISION.md UMBRELLA.md >/dev/null && cargo check -p mana-cli -p imp-core -p wizard-orch
is_archived: true
produces:
- tower-config-philosophy
---

## Goal
Make configuration the primary control surface across Tower apps so shared behavior lives in explicit config, not scattered code defaults.

## Scope
- normalize config layering across mana, imp, wizard, and familiar
- keep shared project behavior in repo config
- keep personal defaults in user config
- keep secrets out of committed config
- make env vars and CLI flags override config rather than replace it
- document the contract in root docs and app docs

## Current state
- `mana` already has `.mana/config.yaml`
- `imp` already has `~/.config/imp/config.toml` plus `.imp/config.toml`
- `wizard` has `.wizard/` local state but no first-class config implementation yet
- `familiar` is still plan-heavy and needs its config model defined in the same spirit

## Acceptance criteria
- Tower root docs clearly define the config-centric rule
- each app has a documented config surface and override order
- wizard gains a first-class configuration model distinct from `.wizard/` local view state
- familiar adopts the same philosophy in its Elixir-native config surface
- no app requires code edits for ordinary policy changes that belong in config
