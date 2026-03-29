# imp Tool Plan

Status: draft  
Scope: `imp` tool surface, tool UX, and how imp should adapt the Manus/CLI-interface lessons without regressing on Tower's architecture.

---

## 1. Thesis

imp should **not** collapse into a single `run(command="...")` tool.

But imp **should** adopt the strongest lessons from that approach:

1. reduce tool-choice entropy
2. make tool discovery progressive
3. make errors navigational
4. standardize result presentation
5. preserve raw execution semantics while shaping outputs for the model
6. keep skills about **method**, not command syntax

The right direction for imp is:

- keep **broad native typed tools**
- keep **bash** as an escape hatch and composition layer
- keep **mana** native, not CLI-mediated
- improve the **LLM-facing ergonomics** of the existing toolset

In short:

> Tower should keep structured native tools, but make them feel more like one coherent tool system.

---

## 2. Position on mana

## 2.1 Native mana tool is the right evolution

`mana` should remain a native imp tool, not something primarily used through `bash` plus a big skill.

Why native is better:

- fewer turns: no CLI syntax recall step
- less prompt weight: no large mana-command skill needed just to operate the substrate
- better validation: action enums and typed args beat shell strings
- better errors: imp can return agent-oriented guidance instead of raw CLI usage output
- better permissions: mode gating can allow/disallow mana actions directly
- better structured results: concise text for the model, structured `details` for runtime/UI
- less indirection: no subprocess/parsing layer for common mana operations

## 2.2 What the mana skill should become

A mana skill is still useful, but it should become **small and methodological**.

It should teach things like:

- when to create child units
- how to write good verify gates
- when to `update` vs `close`
- how to use notes and attempts well
- when to claim/release
- how to think in terms of dependencies and graph hygiene

It should **not** primarily teach:

- CLI syntax
- subcommand spellings
- argument formats
- how to parse mana CLI output

That knowledge should live in the native `mana` tool.

## 2.3 Practical implication

The system should move toward:

- **tool = transport + semantics**
- **skill = work method**

That is cleaner than having the same operational knowledge duplicated in both code and prompt prose.

---

## 3. What imp should copy from the Manus post

imp should copy the **insights**, not the literal implementation.

## 3.1 Reduce tool-choice entropy

The real problem is not absolute tool count. The problem is how hard it is for the model to choose correctly.

imp already has a reasonably broad but coherent set of primitives:

- filesystem: `read`, `write`, `edit`, `diff`
- code discovery: `grep`, `find`, `ls`, `scan`
- execution/external: `bash`, `web`
- coordination/human/system: `mana`, `ask`, `memory`, `extend`, `session_search`

The goal is not to delete these. The goal is to make the choice between them more obvious.

## 3.2 Progressive discovery

The agent should be able to learn the tool surface in layers:

1. overview: what tools exist
2. contrast: when to prefer one over another
3. details: parameter help/examples on demand
4. recovery: errors that point to the next likely action

## 3.3 Error messages as navigation

Errors should say:

- what failed
- why it failed
- what to do next

That matters more than adding more tools.

## 3.4 Execution layer vs presentation layer

The core Manus insight here is strong and directly applicable to imp:

- tool execution should preserve real semantics
- the model-facing output should be shaped for context limits and recovery

Implication for imp:

- keep native logic and structured details internally
- standardize how outputs are truncated, annotated, and suggested to the model

## 3.5 Standardized result formatting

The model should repeatedly see a stable pattern for:

- success/failure
- what object/path/id was affected
- whether output was truncated
- where the full output lives
- likely next steps
- duration/cost hints where possible

---

## 4. What imp should **not** copy

## 4.1 Do not collapse everything into bash

This would be a regression.

We would lose or weaken:

- exact file editing
- structured patch application
- AST-aware code extraction
- image-aware reading
- mode-aware permissions
- native mana integration
- better UI/event integration
- safer and clearer argument validation

`bash` should remain:

- the real-world execution tool
- the composition/escape hatch
- the place where shell semantics matter

It should not become the only interface.

## 4.2 Do not split into many more narrow tools

The Manus warning against fragmented tool catalogs is valid.

imp should avoid growing into lots of tiny overlapping tools like:

- `read_text`
- `read_image`
- `search_symbol`
- `search_text`
- `patch_apply`
- `patch_preview`

Broad tools with clear boundaries are better.

## 4.3 Do not push command syntax back into skills

If imp can natively expose semantics, do not reintroduce those semantics as a prompt burden.

---

## 5. Current-state assessment

## 5.1 What imp is already doing well

### Broad tools rather than tiny tools

This is the right direction. The tool set is fairly compact given the capability surface.

### Multi-action tools where it makes sense

`mana`, `web`, and `diff` already reflect the right instinct:

- one conceptual namespace
- multiple explicit actions
- typed arguments

This is a good hybrid between raw CLI and fragmented function catalogs.

### Typed tools where structure matters

`edit`, `read`, `scan`, and `mana` are all substantially better as native tools than as shell commands.

### Readonly parallelism

Parallel execution of readonly tools is a strong runtime optimization and should stay.

### Mode gating

Tool availability and mana-action permissions are part of imp's strength. This is harder to do well with a pure CLI surface.

## 5.2 Current weaknesses / drift to fix

### Tool inventory drift

There is evidence of tool-surface drift:

- `memory` and `session_search` exist in code but are not currently registered in `register_native_tools()`
- README/tool docs imply a broader set than the currently registered native tool list
- mode allow-lists mention internal names like `diff_show`, `diff_apply`, `multi_edit` even though the top-level exposed tools are `diff` and `edit`

This increases confusion for both humans and models.

### Descriptions are too terse

Most tool descriptions state what the tool is, but not:

- when to prefer it
- what nearby tools it differs from
- when not to use it

### Output conventions are inconsistent

Different tools handle truncation, errors, and detail hints differently.

### Errors often stop at validation

Many current errors are correct but not yet maximally helpful. More of them should route the model toward the next step.

---

## 6. Core design principles for the next version

## 6.1 Optimize for tool-choice clarity, not minimum tool count

The right question is:

> can the model quickly choose the right next action?

Not:

> how few top-level tools can we get away with?

## 6.2 Keep tool names stable and broad

Avoid churn in the top-level tool inventory unless there is a clear payoff.

## 6.3 Prefer native typed semantics for Tower-owned domains

Especially for:

- mana operations
- file mutation
- code structure extraction
- human prompts
- persistent memory

## 6.4 Let skills teach method, not mechanics

Skills should encode work style and strategy, not tool syntax that the runtime already knows.

## 6.5 Make outputs explorable

Large outputs should become artifacts with pointers, not context floods.

## 6.6 Use details JSON as the machine layer

`ToolOutput.details` should become the consistent structured layer.

The text content should be optimized for the LLM.

---

## 7. Proposed plan

## Phase 0 — audit and align the current tool surface

Goal: remove confusion before adding new affordances.

### 0.1 Establish a single source of truth for exposed tools

Audit and align:

- `register_native_tools()`
- README tool list
- mode allow-lists
- system prompt tool listing
- tests asserting available tools

### 0.2 Resolve registration drift

Decide explicitly whether these should be exposed:

- `memory`
- `session_search`

If yes, register them and document them.
If no, remove stale references and keep them internal until ready.

### 0.3 Resolve naming drift

Top-level exposed tool names should match:

- what the model sees
- what modes allow
- what docs promise

If `diff` and `edit` are the public tools, then the mode and docs should describe them that way rather than leaking internal sub-tool names.

### 0.4 Add tool inventory tests

Add a small suite that verifies:

- the registered top-level tool names
- which tools are available in each mode
- the README/tool docs are consistent with the runtime

This prevents future drift.

---

## Phase 1 — improve tool descriptions for model choice

Goal: keep the same tool set, but make it easier to pick the right one.

### 1.1 Rewrite every tool description in contrastive form

Each description should answer:

- what it does
- when to prefer it
- what nearby tools it differs from

Examples:

- `read`: read file contents directly; prefer over `bash` for inspecting files; supports offsets and images
- `edit`: make exact in-place replacements; prefer over `write` when changing part of a file
- `diff`: preview/apply unified diffs; prefer when you need a reviewable patch
- `scan`: extract code structure; prefer over `grep` when you need symbols/types/functions
- `mana`: inspect and mutate work graph state; prefer over `bash` for mana operations
- `bash`: run shell commands when native tools are insufficient or when real command execution is needed

### 1.2 Keep descriptions compact

Do not turn descriptions into large manuals.

The target is better selection, not huge prompt inflation.

### 1.3 Optionally add one-line examples for a few ambiguous tools

High-value candidates:

- `grep`
- `scan`
- `diff`
- `mana`
- `web`

Only if token cost stays reasonable.

---

## Phase 2 — standardize result envelopes

Goal: make tool outputs easier for the model to parse and learn from.

### 2.1 Introduce a consistent output contract

Every tool result should try to communicate:

- outcome: success/error
- subject: file/path/url/unit/id/query
- summary: what happened
- truncation/artifact info if applicable
- suggested next steps when relevant
- duration if available

### 2.2 Keep machine-readable details structured

Expand `ToolOutput.details` conventions.

Suggested common keys:

```json
{
  "subject": "src/main.rs",
  "kind": "file",
  "action": "read",
  "ok": true,
  "duration_ms": 18,
  "truncated": false,
  "artifact_path": null,
  "suggestions": []
}
```

Tool-specific details can extend this.

### 2.3 Add central timing augmentation

Rather than asking every tool to measure itself, the agent runtime can augment the result with elapsed time around tool execution.

This should happen centrally in the tool execution path.

### 2.4 Avoid polluting execution semantics

Do not shove metadata into execution internals where it changes behavior.

Metadata belongs in:

- model-facing text formatting
- `details`
- session/UI events

---

## Phase 3 — unify truncation and artifact behavior

Goal: large outputs should remain usable instead of flooding context.

### 3.1 Replace ad hoc truncation notes with one house style

All large-output tools should use the same pattern:

- preview content
- tell the model the output was truncated
- save the full output somewhere stable
- tell the model how to inspect it further

### 3.2 Prefer session-scoped artifacts over anonymous temp files

Current temp-file behavior is useful, but a stronger version would be:

- session-scoped artifact directory
- stable paths visible in logs/UI
- easy re-read with `read`
- optionally indexed in session metadata later

Candidate path shape:

- `.imp/artifacts/<session-id>/tool-<n>.txt`

### 3.3 Add exploration hints

For truncation cases, include hints like:

- `read(path=...)` to inspect the saved artifact
- `grep(path=..., pattern=...)` to search within it
- `tail` via `bash` only when native tools are insufficient

### 3.4 Priority tools for unified truncation

Start with:

- `bash`
- `grep`
- `read`
- `web`
- `mana` (especially logs/tree/list/run output)

---

## Phase 4 — make errors navigational

Goal: cut down on blind retries and tool thrashing.

### 4.1 Upgrade parameter errors

Instead of only:

- `Missing required parameter: path`

Prefer:

- `Missing required parameter: path. Use read(path=...) to inspect a file, or grep(path=..., pattern=...) to search many files.`

### 4.2 Add nearest-tool suggestions in common failure modes

Examples:

- `edit` no match -> suggest reading the file first or using `grep` to locate the text
- `read` binary file -> suggest image handling when applicable or explain that binary inspection may require `bash`
- `web search` missing provider key -> name the missing env var and alternate providers
- `mana` invalid action in current mode -> list allowed actions for that mode
- `bash` command not found -> preserve stderr and, if possible, suggest native tools when the intent is obvious

### 4.3 Keep errors short and actionable

Do not return giant manuals on failure.

Good error shape:

1. what failed
2. why
3. what to try next

---

## Phase 5 — add progressive tool help without bloating the prompt

Goal: make discovery deeper when needed, not upfront all the time.

### 5.1 Add an optional `tool_help` capability

Possible design:

```text
tool_help(name="grep")
tool_help(name="mana", action="update")
```

This should return:

- purpose
- parameter notes
- a few examples
- common pitfalls

### 5.2 Keep the system prompt compact

The default prompt should still list tools briefly.

Detailed help should be on demand.

### 5.3 Use this especially for high-surface tools

Best candidates:

- `grep`
- `scan`
- `mana`
- `web`
- `diff`

---

## Phase 6 — keep mana native and make its role explicit

Goal: reinforce the correct tool-choice boundary.

### 6.1 Strengthen the `mana` description

It should explicitly say:

- prefer this over `bash` for mana operations
- use it for work graph inspection, updates, orchestration, and delegation

### 6.2 Narrow the need for mana skills

Move syntax knowledge fully into the tool.

Keep skill content focused on:

- decomposition discipline
- verify strategy
- update etiquette
- dependency hygiene
- attempt learning

### 6.3 Keep mana action granularity inside one tool

This is already the right shape.

Avoid exploding mana into many separate top-level tools.

---

## Phase 7 — revisit grouping only if evidence demands it

Goal: avoid premature restructuring.

There is a possible future direction where more tools become grouped namespaces, but this should be evidence-driven.

Examples of possible future grouping:

- `fs` -> read/write/edit/diff-like actions
- `code` -> grep/find/scan-like actions
- `knowledge` -> web/session_search/memory-like actions

But this should **not** happen now unless we see clear evidence that the current top-level set is causing tool-choice failures.

Current recommendation:

- keep current top-level tools
- improve their UX first

---

## 8. Tool-specific recommendations

## 8.1 `mana`

Keep as one multi-action tool.

Improve:

- description contrast vs `bash`
- mode-specific error guidance
- truncation/artifact handling for big outputs
- clearer summaries for `list`, `tree`, `run`, `logs`

## 8.2 `bash`

Keep as escape hatch and true execution tool.

Improve:

- preserve stderr clearly
- standardize truncation notes and artifact path
- expose duration centrally
- consider clearer formatting for exit status/timeouts/cancellation

## 8.3 `read`

Keep as native file inspection tool.

Improve:

- better binary guidance
- clearer offset/limit hints on large files
- unify truncation formatting with the rest of the system
- explicitly contrast against `bash cat`

## 8.4 `edit`

Keep exact replacement semantics.

Improve:

- stronger error guidance when no match is found
- explicitly suggest reading first when editing unread files
- document when to prefer `edit` vs `write` vs `diff`

## 8.5 `diff`

Keep as one action-based tool.

Improve:

- description contrast vs `edit`
- naming/docs cleanup around internal `show/apply` behavior
- ensure modes/docs reflect the public surface, not internal helper names

## 8.6 `grep`

Keep broad search + block extraction in one tool.

Improve:

- better guidance for when to use `grep` vs `scan`
- stronger extraction examples/help
- unified truncation behavior

## 8.7 `scan`

Keep as the AST-aware structural tool.

Improve:

- clearer contrast against `grep`
- a couple of canonical examples in optional help

## 8.8 `web`

Keep `search` + `read` in one tool.

Improve:

- missing-provider guidance
- unify output truncation and saved-artifact behavior
- maybe suggest `web read` after relevant search results in some cases

## 8.9 `ask`

Keep as first-class human interruption.

Improve only lightly:

- perhaps stronger error text when no UI is available
- explicitly contrast with guessing

## 8.10 `memory`

Expose it only if the product wants self-managed learning active by default.

If exposed:

- register it consistently
- document it consistently
- keep it out of modes where it should not be used

## 8.11 `session_search`

This is potentially very valuable, but should either be:

- fully registered/documented/tested
- or intentionally hidden until ready

Do not leave it half-visible.

---

## 9. Suggested implementation order

## Step 1 — alignment and cleanup

- audit registered tools
- audit mode allow-lists
- align README/system prompt/docs
- add tests to prevent drift

## Step 2 — rewrite tool descriptions

Low risk, high ROI.

## Step 3 — standardize truncation and artifact notes

High ROI for model quality and context efficiency.

## Step 4 — standardize error guidance

Add next-step hints to the most common failure paths.

## Step 5 — add central timing/detail augmentation

Make tool outputs more uniform without burdening every tool implementation.

## Step 6 — optional `tool_help`

Only after the basics are in place.

## Step 7 — evaluate whether further grouping is needed

Probably not necessary if the previous steps work.

---

## 10. Metrics to watch

To know whether this is helping, track:

- average number of tool calls per completed task
- rate of tool-argument validation failures
- rate of repeated tool-call retries after an error
- number of times `bash` is used where a native tool would have sufficed
- number of times a tool error is followed by a successful corrective tool choice in the next turn
- share of outputs that get truncated
- whether truncated outputs are later revisited via artifact paths

If possible, derive this from session logs and agent events instead of manual observation.

---

## 11. Non-goals

This plan does **not** aim to:

- turn imp into a CLI-only agent
- remove typed tools in favor of shell strings
- make bash the primary interface for mana
- massively increase the top-level tool count
- move tool semantics back into large prompt skills

---

## 12. Final recommendation

The best adaptation from the Manus post is not:

> replace imp's tool system with one shell tool

It is:

> make imp's existing native tools feel like one coherent, navigable, low-friction tool system.

The architectural stance should be:

- **mana stays native**
- **skills shift toward method**
- **bash stays as escape hatch/composition**
- **typed tools remain for structure-heavy operations**
- **tool UX becomes more consistent, contrastive, and recoverable**

If imp does that, it keeps the advantages of Tower's architecture while gaining the strongest execution-layer lessons from the Manus approach.
