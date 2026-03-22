# Superagent Audit: Pi × Mana Integration

> Full gap analysis of the current pi ↔ units agent system against published research
> from Anthropic ("Building Effective Agents", "Context Engineering", "Advanced Tool Use")
> and production multi-agent orchestration patterns.
>
> Date: 2026-02-25

---

## Executive Summary

The pi + units system is already well-architected. The ready-queue scheduler, dependency
graph, verify gates, and claim/release lifecycle are solid foundations. The biggest
wins are in **what the agent sees when it starts working** (prompt engineering + context
assembly) and **what happens when it fails** (retry intelligence). These are high-impact,
moderate-effort improvements.

### Priority Stack (impact × effort)

| Priority | Area | Impact | Effort | Section |
|----------|------|--------|--------|---------|
| **P0** | Agent prompt engineering | 🔴 High | 🟢 Low | [§1](#1-agent-prompt-engineering) |
| **P1** | Failure recovery & retry | 🔴 High | 🟡 Medium | [§2](#2-failure-recovery--retry-intelligence) |
| **P2** | Context assembly upgrades | 🟠 Medium-High | 🟡 Medium | [§3](#3-smarter-context-assembly) |
| **P3** | Cross-agent intelligence | 🟡 Medium | 🟠 Medium-High | [§4](#4-cross-agent-intelligence) |
| **P4** | Observability & evaluation | 🟡 Medium | 🟡 Medium | [§5](#5-observability--evaluation) |
| **P5** | Advanced orchestration | 🟢 Lower | 🔴 High | [§6](#6-advanced-orchestration) |

---

## Current Architecture (What's Working Well)

Before listing gaps, credit where it's due:

**✓ Ready-queue scheduler** — `scheduler.ts` computes waves via topological sort on
`produces`/`requires` and explicit `dependencies`. Mana become ready the moment their
deps complete, not at wave boundaries. This is the Orchestrator-Workers pattern done right.

**✓ Verify gates** — Every unit has a shell command that must exit 0 to close. This is the
machine-checkable proof that Anthropic's research emphasizes over agent self-assessment.

**✓ Claim/release lifecycle** — Atomic claiming prevents two agents from working the same unit.
Failed agents release claims automatically. Clean state transitions.

**✓ Token-based sizing** — `mana run` measures unit context tokens and routes to Plan vs.
Implement action. This is the Routing pattern from Anthropic's taxonomy.

**✓ Progress TUI** — `progress.ts` shows live per-agent status with spinners, tool activity,
token counts, and costs. Good observability during execution.

**✓ Notes as cross-attempt memory** — Failed agents leave notes, next agent reads them.
This is structured note-taking, one of Anthropic's three long-horizon techniques.

---

## 1. Agent Prompt Engineering

**Current state:** `prompt.ts` → `buildAgentPrompt()` is ~40 lines. It builds:

```
1. RULES.md (if exists)
2. "You are implementing unit {id}: {title}"
3. "When complete, run: mana close {id}"
4. "If stuck, run: mana update {id} --note 'Stuck: ...'"
```

User message: `"implement this unit and run mana close {id} when done"`

The unit file is passed as `@{path}` — pi reads it as a file reference.

**What research says:** This is the "too vague" end of Anthropic's altitude spectrum.
The agent gets the unit description (good) but zero guidance on *how* to work effectively.
Anthropic found they spent more time optimizing tool guidance than overall prompts.

### Gaps

#### 1a. No structured task sections

The agent sees RULES.md + the unit body. There's no framework for *how to approach the work*.

**Recommended:** Add a structured preamble to the system prompt:

```markdown
# Your Assignment
You are implementing unit {id}: {title}

# Approach
1. Read the unit description carefully — it IS your spec
2. Understand the acceptance criteria before writing code
3. Read referenced files to understand existing patterns
4. Implement changes file by file
5. Run the verify command to check your work: {verify}
6. If verify passes, run: mana close {id}
7. If verify fails, fix the issue and try again
8. If stuck after 3 attempts, run: mana update {id} --note "Stuck: <explanation>"

# Verify Gate
Your verify command is: {verify}
This MUST exit 0 for the unit to close. Test it before declaring done.

# Constraints
- Only modify files mentioned in the description unless clearly necessary
- Don't add dependencies without justification
- Preserve existing tests — don't delete or skip them
- Run the project's test/build commands before closing
```

**Impact:** Agents will follow a consistent workflow instead of ad-hoc exploration.
The verify command visibility alone should improve first-attempt success — currently
agents have to discover it by reading the unit file.

#### 1b. No parent context injection

When unit `3.2` runs, it only sees its own description. Parent `3` may contain
critical architectural context (design decisions, shared types, security constraints).

**Recommended:** In `buildAgentPrompt`, walk up the parent chain and inject
parent descriptions as context sections:

```markdown
# Parent Context (unit 3: Authentication System)
[parent description here — architecture decisions, shared types, constraints]

# Your Task (unit 3.2: Token Refresh)
[this unit's description]
```

The parser already has `UnitEntry.parent` — just need `readUnit(unitsDir, parent)`
and include the body.

**Token budget:** Cap parent context at ~2K tokens. If parent description is longer,
summarize or truncate to the first section.

#### 1c. No acceptance criteria emphasis

The acceptance criteria are buried in the unit body. Agents often skip them or misunderstand
what "done" means.

**Recommended:** Extract the `acceptance` field from YAML frontmatter and surface it
prominently in the prompt:

```markdown
# Acceptance Criteria (must ALL be true)
- POST /auth/refresh accepts { refresh_token }
- Returns { access_token, expires_in }
- Returns 401 if token expired
- Returns 401 if signature invalid

# Verify Command (must exit 0)
npm test -- --grep "refresh"
```

#### 1d. No previous attempt context

If this is attempt 2+, the agent should know what the previous agent tried and why it failed.
Currently, notes exist but aren't surfaced in the prompt.

**Recommended:** If `unit.meta.attempts > 0`, include the notes section:

```markdown
# Previous Attempts ({attempts} so far)
{notes}

IMPORTANT: Do NOT repeat the same approach. The above explains what was already tried.
```

**Cross-reference:** This connects to §2 (Failure Recovery) — the quality of notes
determines how useful this is.

#### 1e. No tool guidance

Agents in pi have access to Read, Write, Edit, Bash, probe_search, ast_grep, etc.
but get zero guidance on which tools are best for which situations.

**Recommended:** Add a brief tool strategy section:

```markdown
# Tool Strategy
- Use `probe_search` to find code by meaning (semantic), `Bash(rg ...)` for exact text
- Use `Read` to examine files before editing — never edit blind
- Use `Edit` for surgical changes (old text must match exactly)
- Use `Bash` to run tests, builds, and the verify command
- Read referenced files from the description FIRST, before exploring
```

### Implementation

All of the above changes are in `prompt.ts` → `buildAgentPrompt()`. The function
currently takes `UnitFull` and `PromptOptions`. Changes needed:

1. Read parent unit (requires `readUnit` from `parser.ts` — already available)
2. Extract verify command from YAML frontmatter (need to parse frontmatter from `UnitFull`)
3. Extract acceptance criteria (same — parse from frontmatter)
4. Check `unit.meta.attempts` and include notes if > 0
5. Build structured prompt with all sections

**Estimated effort:** 2-4 hours. All in one file. No new dependencies.

---

## 2. Failure Recovery & Retry Intelligence

**Current state:** When an agent fails:

```typescript
// spawner.ts line ~230
await releaseUnit(exec, unit.meta.id);
await updateUnitNote(exec, unit.meta.id, `Agent failed: ${agent.error}`);
```

The note is just the error string (e.g., "Exit code 1", "Idle timeout (5m)").
No analysis of *why* it failed. The next agent gets the same prompt with this
thin note appended.

**What research says:** "Never retry with identical instructions. Always add what
went wrong." (Mana BEST_PRACTICES.md says this too — but the code doesn't enforce it.)

### Gaps

#### 2a. No failure log analysis

The agent's stdout/stderr is streamed and tracked for tool counts and tokens, but
when it fails, none of that execution history is analyzed.

**Recommended:** On failure, capture the last N tool calls and any error output,
then generate a structured failure summary:

```typescript
// In spawner.ts, on failure:
const failureSummary = buildFailureSummary(agent, logs);
await updateUnitNote(exec, unit.meta.id, failureSummary);
```

Where `buildFailureSummary` produces:

```
## Attempt 2 Failed (2026-02-25, 47s, 23k tokens)

### What was tried
- Read src/auth/token.rs, src/auth/mod.rs
- Modified src/auth/token.rs (added refresh_token function)
- Wrote tests in src/auth/tests/refresh_test.rs
- Ran verify: npm test -- --grep "refresh"

### Why it failed
- Test `test_expired_token` failed: "expected 401, got 500"
- Error in refresh_token(): unwrap() on None at line 47

### Files modified
- src/auth/token.rs
- src/auth/tests/refresh_test.rs

### Suggestion for next attempt
- The refresh_token function doesn't handle the case where the token
  has no expiry field. Check for None before unwrapping.
```

**How:** The agent event stream already tracks tool names, file paths, and errors.
Capture the last 10-20 tool events before failure. Parse the verify command output
(it's in stderr or the last few log lines) for specific test failure messages.

#### 2b. No progressive hints

First attempt: agent gets the unit description only.
Second attempt: same description + thin error note.
Third attempt: same thing.

**Recommended:** Escalating context based on attempt number:

| Attempt | Extra context |
|---------|--------------|
| 1 | Standard prompt (§1 improvements) |
| 2 | + Previous attempt failure summary + "Don't repeat: ..." |
| 3 | + Expanded file contents pre-loaded + specific code snippets |
| 4+ | + Flag for human review ("This unit has failed {n} times") |

In `buildAgentPrompt`:

```typescript
if (unit.meta.attempts >= 3) {
  systemParts.push(`# ⚠️ Multiple Failures (${unit.meta.attempts} attempts)
This unit has failed multiple times. Before starting:
1. Read ALL previous attempt notes carefully
2. Identify the SPECIFIC error, not the symptom
3. Consider whether the approach needs to change entirely
4. If the verify command itself is wrong, note that and stop`);
}
```

#### 2c. No "failure pattern" learning across units

If unit 3.1 fails because of a database connection issue, and unit 3.2 is about to
run and will likely hit the same issue, there's no cross-unit learning.

**Recommended (medium effort):** After a failure, check if the error matches common
patterns and add a warning to sibling units' context:

```
# Warning from sibling unit 3.1
Unit 3.1 failed with: "database connection timeout". If your work
involves database access, verify connection pool settings first.
```

This requires reading sibling unit statuses during prompt assembly.

#### 2d. No verify command pre-flight

Agents sometimes spend 5+ minutes implementing, only to discover the verify command
is broken, unreachable, or tests a different module entirely.

**Recommended:** Add to the prompt preamble:

```markdown
# Pre-flight Check
Before implementing anything, run the verify command to see its CURRENT state:
  {verify}
It should FAIL (that's expected — the feature isn't built yet).
If it errors for infrastructure reasons (missing deps, wrong path), fix that first.
```

This is "fail-first" verification — already a units design principle, but not enforced
in the agent prompt.

### Implementation

- `spawner.ts`: Capture last N events, build structured failure summary
- `prompt.ts`: Read attempts + notes, inject progressive context
- New: `failure.ts` — failure summary builder (extracts patterns from logs)

**Estimated effort:** 4-8 hours. Mostly new code in failure.ts, modifications to
spawner.ts and prompt.ts.

---

## 3. Smarter Context Assembly

**Current state:** Two separate context assembly paths:

1. **Rust (`ctx_assembler.rs`):** Used by `mana run` direct mode. Extracts file paths
   via regex from description, reads files, wraps in code blocks. Passed via
   `--append-system-prompt`.

2. **TypeScript (`prompt.ts`):** Used by pi's `unit_run` tool. Passes unit file as
   `@{path}` reference. Loads RULES.md. Does NOT load referenced files from description
   (that's left to the Rust side or the agent to discover).

The pi extension's prompt.ts does NOT do file context assembly — it relies on the
`@{unit_path}` file reference to give pi the unit description, and the agent discovers
files by reading them.

**What research says:** Anthropic's context engineering paper advocates a hybrid approach:
pre-load critical context, let the agent discover the rest. "Just-in-time" retrieval
is better than dumping everything upfront (context rot). But some pre-loading is
essential to avoid wasted exploration.

### Gaps

#### 3a. No file content pre-loading in pi extension

When `unit_run` spawns an agent, the agent gets:
- System prompt: RULES.md + unit assignment instructions
- User message: `@{unit_file}` + "implement and mana close"

The agent must then discover and read all referenced files itself. This wastes
tokens and introduces exploration risk (wrong files, missed context).

**Recommended:** In `prompt.ts`, extract file paths from the unit description
(port the regex from `ctx_assembler.rs` or call `mana context {id}`), and include
the most critical files in the system prompt:

```typescript
// In buildAgentPrompt:
const paths = extractPaths(unit.body);
const workspace = path.resolve(options.unitsDir, '..');
const fileContext = assembleFileContext(paths, workspace, TOKEN_BUDGET);
if (fileContext) {
  systemParts.push(`# Referenced Files\n\n${fileContext}`);
}
```

**Token budget strategy:**
- Budget ~8K tokens for file context (out of ~60K total)
- Include files mentioned in "Files to modify" or "## Files" sections first
- Include type definition files (types.rs, types.ts, interfaces) second
- Truncate large files to just the relevant functions/types
- Skip test files (agent will write/read those itself)

#### 3b. No relevance scoring

Current regex extraction gets all paths mentioned anywhere in the description.
A file mentioned in "see also" is weighted the same as "modify this file".

**Recommended:** Score paths by context:
- "create" / "add" / "modify" → HIGH (must read to understand current state)
- "see" / "reference" / "pattern" → MEDIUM (useful but not critical)
- "don't modify" / "ignore" → SKIP (agent shouldn't see these)

The Rust `relevance.rs` file already exists in the units codebase — could be
a reference for scoring logic.

#### 3c. No type/signature extraction

When a unit says "Add RefreshToken variant to GrantType enum", the agent needs to
see the current GrantType definition. Currently it must find and read the file.

**Recommended:** For files with known extensions (.rs, .ts, .py), use tree-sitter
(via `scan extract`) to pull just the relevant type definitions and function
signatures, rather than including entire files:

```markdown
# Key Types (from src/auth/grants.rs)
```rust
pub enum GrantType {
    Password,
    ClientCredentials,
    AuthorizationCode,
}
```

This is dramatically more token-efficient than including the full file.

### Implementation

- `prompt.ts`: Add `extractPaths()` (port from ctx_assembler.rs regex)
- `prompt.ts`: Add `assembleFileContext()` with token budgeting
- Optional: Call `scan extract` for type extraction (requires scan tool on PATH)
- Alternative: Shell out to `mana context {id}` which already does this in Rust

**Estimated effort:** 4-6 hours. Could start simple (just pre-load referenced files)
and add sophistication later.

---

## 4. Cross-Agent Intelligence

**Current state:** Each agent runs in complete isolation. Agent A's work is only
visible to Agent B through:
1. File changes on disk (if not using worktrees)
2. Unit status changes (closed/failed)
3. Notes left on units

There's no mechanism for sharing *discoveries* — if Agent A finds that the codebase
uses a specific pattern, Agent B must rediscover this independently.

### Gaps

#### 4a. No shared discovery context

When multiple agents work on sibling units under a parent, they often need the same
background understanding (architecture patterns, naming conventions, error handling style).

**Recommended:** After each successful unit completion, extract a brief "discovery
summary" and store it on the parent unit as a note:

```
## Discoveries from unit 3.1
- Auth tokens use RS256 signing (not HS256 as assumed)
- Test fixtures are in tests/fixtures/auth.rs, not inline
- Database uses connection pooling via `sqlx::Pool<Postgres>`
```

Sibling units' prompts can then include discoveries from completed siblings:

```markdown
# Context from completed siblings
{parent.notes filtered to discovery summaries}
```

**How:** This requires the agent to output a structured summary on success.
Add to the close instructions:

```
When you successfully close this unit, ALSO run:
mana update {id} --note "Discoveries: <brief notes about patterns, conventions, or 
gotchas you found that might help sibling units>"
```

#### 4b. No file conflict awareness

If units 3.1 and 3.2 both modify `src/auth/mod.rs`, and they run in parallel,
the second agent may overwrite the first agent's changes.

**Current mitigation:** The Rust `mana run` has git worktree support (each agent
gets an isolated worktree). The pi extension does NOT use worktrees — agents
run in the same working directory.

**Recommended (short-term):** During scheduling, detect file overlaps between
parallel units and either:
1. Serialize them (add implicit dependency)
2. Warn in the prompt: "Note: unit 3.1 may also be modifying src/auth/mod.rs.
   Be careful with merge conflicts."

**Recommended (long-term):** Port worktree isolation to the pi extension.

#### 4c. No artifact passing for produces/requires

`produces`/`requires` currently only control scheduling order. The actual artifact
content isn't passed between units. If unit A "produces AuthTypes" by creating
`src/auth/types.rs`, unit B must discover this file on its own.

**Recommended:** When a unit with `produces: ["AuthTypes"]` closes, capture what
was created/modified and include it in the dependent unit's prompt:

```markdown
# Artifacts from dependencies
## AuthTypes (produced by unit 3.1)
Unit 3.1 created src/auth/types.rs:
```rust
pub struct AuthToken { ... }
pub enum GrantType { ... }
```
```

**How:** On successful close, diff the working tree to find what changed,
then store the diff summary as metadata. When building prompts for units that
`require` those artifacts, include the summary.

### Implementation

- 4a: Modify close instructions in `prompt.ts`, add discovery extraction
- 4b: Add file overlap detection in `scheduler.ts`
- 4c: Requires new metadata storage + diff capture on close (bigger lift)

**Estimated effort:** 4a is 2 hours, 4b is 4 hours, 4c is 8+ hours.

---

## 5. Observability & Evaluation

**Current state:**
- `progress.ts` shows live per-agent status (tools, tokens, cost) — good
- `unit_logs` captures tool activity — good
- `unit_status` shows aggregate run stats — good
- No persistent evaluation data
- No success rate tracking
- No cost-per-unit analytics

### Gaps

#### 5a. No success rate tracking

There's no way to answer: "What percentage of units succeed on first attempt?"
or "Which types of units fail most often?"

**Recommended:** On each agent completion (success or fail), append to a
persistent log file (e.g., `.mana/agent_history.jsonl`):

```json
{"unit_id":"3.2","attempt":1,"success":true,"duration_secs":47,"tokens":23400,"cost":0.12,"model":"sonnet-4","timestamp":"2026-02-25T00:30:00Z"}
```

Add a `mana stats agents` command to analyze this data:
```
First-attempt success rate: 72% (18/25)
Average tokens per success: 28k
Average tokens per failure: 41k
Most-failed units: type=refactor (3/5 failed)
```

#### 5b. No cost aggregation across runs

The progress widget shows per-agent costs, but there's no aggregate view across
all runs in a session or project.

**Recommended:** Track cumulative cost in the run state and surface it in
`unit_status`:

```
Total session cost: $2.47 (18 agents, 412k tokens)
Average cost per unit: $0.14
```

#### 5c. No golden set / regression testing

There's no way to validate that prompt changes improve agent performance.

**Recommended (future):** Create a `mana eval` command that:
1. Runs a set of "golden" units with known solutions
2. Measures success rate, token usage, and time
3. Compares against baseline
4. Reports regression/improvement

This is the "continuous evaluation" pattern from production orchestration research.

### Implementation

- 5a: Add JSONL logging in `spawner.ts` on completion
- 5b: Aggregate in `unit_status` tool response
- 5c: Future work — needs golden set creation tooling

**Estimated effort:** 5a+5b is 3 hours. 5c is a larger project.

---

## 6. Advanced Orchestration

Lower priority because the current system works well. These are "nice to have"
improvements for scale.

### 6a. Model routing by task type

Currently all agents use the same model (or the override from `--model`).
Some units are simple (rename a variable) and could use a cheaper model.
Others are complex (design a new subsystem) and need the strongest model.

**Recommended:** Add model hints to unit metadata:

```yaml
labels:
  - model:haiku    # Simple task, use cheaper model
  - model:opus     # Complex reasoning needed
```

Or auto-route based on token size and complexity signals.

### 6b. Two-phase actions for risky units

For units that modify critical paths (auth, payments, database schemas),
add a Plan → Validate → Execute pattern:

1. Agent proposes a plan (structured diff/change set)
2. Validation runs (linting, type checking, security scan)
3. Only then: agent executes

Could be implemented as a unit label `requires-plan: true` that triggers
plan-first mode in the spawner.

### 6c. Agent specialization

Some units are "add tests", some are "refactor", some are "implement feature".
Different system prompts could optimize for each type.

**Recommended:** Detect unit type from title/labels and load specialized
prompt sections:

- `test:` units → emphasize test patterns, coverage, edge cases
- `refactor:` units → emphasize preserving behavior, running existing tests
- `docs:` units → emphasize accuracy, completeness, examples
- `bug:` units → emphasize reproduction, root cause analysis, regression tests

### Implementation

All of §6 is future work. The prompt improvements in §1 deliver 80% of the value.

---

## Recommended Implementation Order

```
Phase 1 (Quick wins — 1-2 days)
├── §1a: Structured task preamble in prompt.ts
├── §1b: Parent context injection
├── §1c: Acceptance criteria + verify command surfacing
├── §1d: Previous attempt context injection
└── §1e: Tool guidance section

Phase 2 (High-value — 2-3 days)
├── §2a: Failure log analysis + structured summaries
├── §2b: Progressive hints based on attempt count
├── §2d: Verify pre-flight check in prompt
└── §5a: Agent success rate tracking (JSONL)

Phase 3 (Context quality — 2-3 days)
├── §3a: File content pre-loading in prompt.ts
├── §3b: Relevance scoring for file paths
├── §4a: Cross-agent discovery sharing
└── §4b: File conflict detection

Phase 4 (Advanced — when needed)
├── §3c: Type/signature extraction
├── §4c: Artifact passing for produces/requires
├── §5c: Golden set evaluation
├── §6a: Model routing
├── §6b: Two-phase actions
└── §6c: Agent specialization
```

Phase 1 is almost entirely changes to `prompt.ts` — one file, huge impact.
Phase 2 adds `failure.ts` and modifies `spawner.ts`.
Phase 3 expands context assembly and scheduling.
Phase 4 is for when the system is mature and you want to optimize further.

---

## Files That Would Change

| File | Phase | Changes |
|------|-------|---------|
| `extensions/units/prompt.ts` | 1, 2, 3 | Major expansion — from 40 to ~200 lines |
| `extensions/units/spawner.ts` | 2 | Failure summary capture + JSONL logging |
| `extensions/units/parser.ts` | 1 | Add frontmatter field extraction (verify, acceptance, notes, attempts) |
| `extensions/units/scheduler.ts` | 3 | File overlap detection |
| `extensions/units/failure.ts` | 2 | New file — failure analysis + summary builder |
| `extensions/units/index.ts` | 2, 3 | Pass new data through to spawner |

---

## Key Metrics to Track

After implementing, measure:

1. **First-attempt success rate** — should increase from ~60-70% to ~80-85%
2. **Tokens per successful unit** — should decrease (less exploration waste)
3. **Retry success rate** — should increase dramatically (better failure context)
4. **Time to first tool call** — should decrease (pre-loaded context reduces exploration)
5. **Agent idle timeouts** — should decrease (better guidance reduces confusion)
