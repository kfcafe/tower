---
id: '17'
title: 'feature: auto-commit on mana close'
slug: feature-auto-commit-on-mana-close
status: open
priority: 2
created_at: '2026-03-21T17:18:34.214939Z'
updated_at: '2026-03-21T17:18:34.214939Z'
labels:
- mana
- feature
- git
verify: mana --help 2>&1 | rg -q "auto.commit" || rg -q "auto_commit" mana/crates/mana-core/src/config.rs
---

## Problem
When imp agents close units via mana close, the changes are not committed to git. This means a session of agent work produces a dirty tree that requires manual git add && git commit.

## Proposal
Add an auto_commit config option to .mana/config.yaml (already exists in the Config struct but may not be fully wired). When enabled, mana close should:
1. Stage all changes in the working tree
2. Create a commit with message "Close unit {id}: {title}"
3. Skip if in worktree mode (worktree already commits)

This is especially valuable for dogfooding imp as a runner where multiple units close in sequence.

## Context
The mana Config struct already has an auto_commit: bool field. The mana/.mana/config.yaml for the mana project already sets auto_commit: true. This unit should verify the feature works end-to-end and document it.

## Files
- mana/crates/mana-core/src/config.rs (already has the field)
- mana/crates/mana-core/src/ops/close.rs (needs to implement the commit)
- mana/README.md (document the option)
