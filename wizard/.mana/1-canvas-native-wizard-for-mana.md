---
id: '1'
title: Canvas-native Wizard for mana
slug: canvas-native-wizard-for-mana
status: open
priority: 2
created_at: '2026-03-20T06:31:02.579109Z'
updated_at: '2026-03-20T06:31:02.579109Z'
labels:
- wizard
- canvas
- ux
feature: true
---

## Problem
Mana's current interaction model is strong for execution but weak for spatial memory, graph navigation, and shared agent-human evidence.

## Outcome
Wizard becomes a canvas-first local interface for navigating and operating mana. It should support semantic zoom, typed cards, focus rooms, runtime monitoring, review flows, and persistent knowledge artifacts.

## Spec
See SPEC.md for the canonical product and technical specification.

## Initial shape
- Canvas-first desktop client
- Daemon-backed orchestration
- CLI fallback via wiz
- Shared work state from .mana
- Personal layout state from .wizard

## Non-goals
- Replace the code editor
- Store personal layout churn in git by default
- Depend on a cloud backend for local use
