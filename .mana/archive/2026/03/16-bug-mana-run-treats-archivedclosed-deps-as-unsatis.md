---
id: '16'
title: 'bug: mana run treats archived/closed deps as unsatisfied'
slug: bug-mana-run-treats-archivedclosed-deps-as-unsatis
status: closed
priority: 1
created_at: '2026-03-21T17:18:20.083853Z'
updated_at: '2026-03-21T19:55:16.985755Z'
notes: |2

  ## Attempt 1 — 2026-03-21T19:55:10Z
  Exit code: 1

  ```

  ```
labels:
- mana
- bug
- deps
- archive
closed_at: '2026-03-21T19:55:16.985755Z'
verify: |-
  cd /tmp && rm -rf mana-dep-test && mkdir -p mana-dep-test/.mana/archive/2026/03 && echo "project: test
  next_id: 3" > mana-dep-test/.mana/config.yaml && printf -- "---\nid: \"1\"\ntitle: Parent\nstatus: closed\n---\n" > mana-dep-test/.mana/archive/2026/03/1-parent.md && printf -- "---\nid: \"2\"\ntitle: Child\nstatus: open\ndependencies:\n- \"1\"\nverify: \"true\"\n---\n" > mana-dep-test/.mana/2-child.md && cd mana-dep-test && mana sync 2>/dev/null && mana run 2 2>&1 | rg -q "Wave 1"
attempts: 1
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-21T19:55:10.823890Z'
  finished_at: '2026-03-21T19:55:10.904873Z'
  duration_secs: 0.08
  result: fail
  exit_code: 1
---

## Problem
When a dependency unit is closed and archived (moved to .mana/archive/), mana run still treats it as an unsatisfied dependency. This blocks child units that should be ready.

## Reproduction
1. Create parent unit, close it (gets archived to .mana/archive/)
2. Create child unit with deps on the parent
3. mana run <child-id> reports "waiting on <parent-id>"

## Evidence
Wizard units 1.3, 1.4, 1.5 were blocked on 1.2 even after 1.2 was closed and archived.
Workaround was manually removing the dependency lines from unit frontmatter.

## Expected behavior
mana run should check both active units and archived units when resolving dependency satisfaction. A closed/archived dependency is by definition satisfied.

## Files
- mana/src/commands/run/ (dependency resolution in dispatch)
- mana/crates/mana-core/src/index.rs (archived unit lookup)
