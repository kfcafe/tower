# Mana Review

Date: 2026-03-25
Status: external review memo

## Purpose

This document reviews `mana` against the current agent orchestration landscape — especially LangGraph, CrewAI, OpenCastle, Devin-style async engineering systems, GitHub Copilot coding agent, and related tools.

The goal is not to turn `mana` into a hosted product. The goal is to identify:

1. what `mana` already does unusually well,
2. what is already present but not yet expressed clearly enough,
3. what is still genuinely missing,
4. how `mana` can become a stronger open coordination substrate without absorbing Wizard or Familiar responsibilities.

---

## Executive summary

`mana` is significantly more sophisticated than a simple “task CLI.”

It already has the core of a real execution substrate:

- verify gates,
- fail-first semantics,
- dependency scheduling,
- `produces` / `requires`,
- attempt history,
- facts with staleness,
- review infrastructure,
- context and prompt assembly,
- traceability,
- batch verification,
- risk scoring,
- agent monitoring,
- a library and MCP surface.

Most of the opportunity is **not** to bolt on generic workflow ideas. It is to:

1. make the artifact model more explicit,
2. promote review to a more central concept,
3. turn existing verification/history data into clearer receipts,
4. extend contracts from ordering into actual handoff,
5. stabilize the programmatic API and remove duplicated logic,
6. make flaky/infrastructure failures easier to classify.

`mana` already has the bones of a very strong verified work graph. The next step is to make that shape easier to see and easier for other products to build on.

---

## What `mana` already does unusually well

### 1. Verify-gated completion is the right primitive

This remains the clearest differentiator.

Many systems still center on:

- prompts,
- sessions,
- PRs,
- or human trust.

`mana` centers on:

- machine-checkable completion,
- fail-first verification,
- and durable history of attempts.

This is the right substrate-level bet.

### 2. The graph model is strong

`mana` already has more than simple task dependencies.

It includes:

- explicit dependency edges,
- parent/child decomposition,
- `produces` / `requires`,
- ready-queue scheduling,
- wave computation,
- cycle detection,
- blocking analysis,
- trace/graph/status commands.

That is a serious coordination model, not a glorified checklist.

### 3. Context assembly is much better than average

The structured prompt builder is already strong.

It can surface:

- project rules,
- parent context,
- sibling discoveries,
- referenced files,
- acceptance criteria,
- pre-flight verify checks,
- previous attempts,
- approach guidance,
- verify gate,
- constraints,
- tool strategy.

That means `mana` is already doing part of the job most orchestration systems leave to ad-hoc prompts.

### 4. Facts and staleness are strategically important

The fact system is a big differentiator:

- verified facts,
- TTL/staleness,
- project memory injected into context,
- recall/search behavior.

This is far more useful than generic “notes” alone.

### 5. Review is already real

`mana` is further along here than a casual read suggests.

It already has:

- `mana review`,
- AI adversarial review,
- human review paths,
- `mana-review` crate,
- risk scoring,
- diff analysis,
- persisted review state,
- queueing and HTML review rendering.

This is not a speculative feature. It is already part of the system’s shape.

### 6. History and provenance are richer than a pass/fail flag

The unit model already includes:

- `history` / `RunRecord`,
- `attempt_log`,
- `outputs`,
- `checkpoint`,
- `fail_first`,
- `verify_timeout`,
- agent history JSONL.

That is enough raw material to support a much stronger verification and audit story.

---

## What is underexposed today

These are capabilities or shapes that appear to exist already, but are not yet as explicit as they could be.

### A. Artifact thinking is present but implicit

Today the system already stores several artifact-like things:

- attempts,
- reviews,
- diffs,
- facts,
- outputs,
- trace context,
- verify history.

But the system does not yet tell a simple story like:

- “these are the durable artifacts of a unit.”

### B. Review exists, but does not yet feel central

Review is implemented, but it can still feel like an advanced feature rather than a first-class stage in the work lifecycle.

### C. Verification history is structured, but not yet branded as a receipt

The data is present. The concept is not yet as explicit or ergonomic as it could be.

### D. `produces` / `requires` is powerful for ordering, weaker for handoff

The graph can express that one unit depends on another’s output. It is less strong at preserving and passing the useful content of that output.

### E. The API surface still feels less complete than the CLI surface

The library and MCP integration are promising, but parts of the architecture still appear duplicated or incomplete.

---

## Recommendations

## P0 — Highest priority

### 1. Make the artifact model explicit

**Why**

`mana` already has many durable outputs, but they are scattered across different structures and commands.

A clearer artifact model would make the system easier to understand and easier for Wizard/Familiar to consume.

**Suggested direction**

Define a simple artifact envelope for durable outputs associated with a unit or attempt:

- `kind` — plan, verify_receipt, review, diff_summary, fact, produced_artifact, decision
- `unit_id`
- `attempt`
- `source` — human, agent, system
- `summary`
- `created_at`
- `payload_ref` or inline payload
- `supersedes` / `stale` markers

This does **not** require replacing current storage immediately. It can begin as a unifying conceptual layer over what already exists.

**Goal**

Make durable engineering evidence a first-class part of the substrate.

---

### 2. Promote review to a more central lifecycle stage

**Why**

The review machinery is already strong enough to matter strategically.

Right now, the market is converging on “implementation alone is not enough.” Systems that combine implementation, verification, and review are more trustworthy.

`mana` already has the ingredients.

**Suggested direction**

Clarify and standardize lifecycle states or transitions around review:

- open
- in progress
- awaiting verify
- verified
- awaiting review
- changes requested / approved / rejected

This does not need to make review mandatory for every unit. It should make review feel like an intentional, composable stage.

**Goal**

Turn review from an advanced optional flow into a visible part of the coordination model.

---

### 3. Create a first-class “verify receipt” concept

**Why**

The raw data largely exists already, but users and downstream tools benefit from a named, coherent object.

A verify receipt should answer:

- what command ran,
- when it ran,
- against what checkpoint/base,
- how long it took,
- what the exit status was,
- what summary or structured outputs were produced.

**Suggested direction**

Define a receipt shape that can be rendered in CLI, exposed in APIs, and attached to reviews:

- command
- exit code / result
- duration
- stdout/stderr summary
- structured outputs if available
- checkpoint / ref
- runner/agent metadata
- maybe environment metadata later

**Goal**

Make verification legible, portable, and inspectable.

---

### 4. Extend `produces` / `requires` from ordering into real handoff

**Why**

This is the most important missing capability in the graph model.

Right now, `produces` / `requires` is great for scheduling and blocking, but weak for carrying useful output forward.

That means dependent units may know that something exists without being told what was actually produced.

**Suggested direction**

When a unit closes successfully:

- capture a compact produced-artifact summary,
- bind it to each declared produced artifact,
- make that summary available in dependent unit context.

Examples:

- Unit A produces `AuthTypes` → include the actual type summary or file locations.
- Unit B requires `AuthTypes` → inject the summary into B’s context.

**Goal**

Turn contracts into real communication between units, not just ordering constraints.

---

### 5. Finish and stabilize the programmatic API

**Why**

If `mana` is the substrate, other layers must be able to trust its programmatic behavior as much as its CLI behavior.

Right now, some architecture notes and audits suggest:

- incomplete API surfaces,
- duplicated logic between CLI and MCP paths,
- behavioral drift risk.

**Suggested direction**

Aim for one core mutation/query layer that powers:

- CLI,
- MCP,
- library callers,
- future Wizard/Familiar integrations.

Priority areas:

- create/update/close/review/fact operations,
- fail-first enforcement consistency,
- hooks consistency,
- review state consistency,
- checkpoint/diff provenance consistency.

**Goal**

Reduce drift and make the substrate easier to embed.

---

### 6. Add clearer flaky / infrastructure failure classification

**Why**

As `mana` gets used more heavily, “verify failed” will increasingly mean multiple things:

- real implementation bug,
- flaky test,
- broken environment,
- dependency install failure,
- timeout,
- missing external service.

Right now the system has attempts, timeouts, and hooks, but not yet a very strong failure taxonomy.

**Suggested direction**

Classify failures more explicitly:

- verify_failure
- timeout
- infrastructure_failure
- flaky_suspected
- manual_abort

Then let policies and review flows use that information.

**Goal**

Make retries smarter and avoid punishing the wrong layer.

---

## P1 — Strong next differentiators

### 7. Add automatic failure-to-fact promotion

The fact system is already valuable. The next step is to make it learn from repetition.

Examples:

- repeated verify failures due to Docker not running,
- repeated failed attempts because a package requires a setup step,
- repeated “don’t use offset pagination here” style lessons.

This should begin conservatively:

- suggest a fact,
- require approval or explicit promotion,
- avoid auto-polluting project memory.

---

### 8. Add reusable unit templates / playbooks

`mana` already has strong docs, prompt assembly, run/plan/review config, and presets.

The next step is to encode repeatable work shapes more directly:

- bug fix,
- test addition,
- refactor,
- dependency upgrade,
- incident follow-up,
- review-only task,
- migration task.

This helps teams create better units faster and gives Familiar a better future configuration surface.

---

### 9. Add richer graph analytics

The graph engine already computes ready/blocking/waves. Build more explicit summaries on top:

- critical path,
- long-pole units,
- stale units,
- repeated-failure hotspots,
- risky subtrees,
- graph complexity warnings.

This belongs in the substrate so Wizard can visualize it and Familiar can monitor it.

---

### 10. Implement scheduled units carefully

The design work already exists. That is a strength.

The recommendation is not “invent scheduled work.” It is:

- keep the implementation aligned with the work-graph model,
- let scheduling remain a thin layer over units,
- avoid turning `mana` into a monolithic daemon.

Wizard and Familiar can then provide richer scheduling/orchestration surfaces on top.

---

## P2 — Later

### 11. Stronger attestations and auditability

Possible future direction:

- cryptographic receipts,
- signed verify outputs,
- stronger provenance around diff bases and environment identity.

Interesting, but not urgent.

### 12. More formal decision artifacts

Useful later for human-in-the-loop planning and risk-heavy work, but can wait until the core artifact model is clearer.

---

## Recommended boundaries

`mana` should remain:

- the verified work substrate,
- the dependency and unit graph,
- the durable memory and artifact layer,
- the source of truth for coordination state.

`mana` should **not** become:

- a full UI,
- a cloud control plane,
- a chat interface,
- a runtime-specific orchestrator,
- a generic workflow builder detached from software work.

That discipline is part of its strength.

---

## How `mana` should support the rest of the stack

### For `imp`

`mana` should provide:

- better artifact contracts,
- clearer verify receipts,
- stronger retry/review context,
- stable programmatic APIs.

That makes `imp` a better worker without coupling it tightly to one orchestration style.

### For Wizard

`mana` should expose:

- durable artifact shapes,
- graph analytics,
- review state,
- verify receipts,
- blocking reasons,
- traceable dependency and attempt history.

That will make Wizard’s canvas feel semantic rather than decorative.

### For Familiar

`mana` should expose:

- stable queries and mutations,
- explicit receipts and reviews,
- graph scheduling truth,
- flaky/infrastructure classification,
- composable templates/playbooks.

That makes Familiar easier to govern and easier to explain commercially.

---

## Final take

`mana` does **not** need an identity rewrite.

It is already much closer to a serious verified coordination substrate than a simple task runner. The next step is to:

- unify the artifact story,
- elevate review,
- make verify receipts explicit,
- strengthen contract handoff,
- and finish the API surface.

If those pieces are tightened, `mana` becomes easier to understand, easier to build products on top of, and harder to mistake for “just another task tracker.”
