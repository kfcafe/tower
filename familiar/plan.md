# Familiar — Plan

**Domain:** getfamiliar.dev
**Engine:** imp (Elixir agent engine, `~/imp`)

An open source agent platform for engineering teams. Agents run unattended — coding tasks with verify gates in isolated environments, service tasks through brokered actions with permission controls. Slack is the front door. The dashboard provides visibility and control. Teams extend the agent with their own tools via MCP in any language.

The coding agent with verify gates is the wedge — concrete, demonstrable, clear ROI. The platform underneath supports much more. Once a team trusts Familiar for coding tasks, they connect services: "monitor Sentry for new errors and file tasks," "triage Linear tickets and draft responses," "keep dependencies up to date."

## Table of Contents

- [Vision & Positioning](#vision--positioning)
- [Target Audience](#target-audience)
- [How It Works](#how-it-works)
- [Architecture Overview](#architecture-overview)
- [Task System](#task-system)
- [Agent Runtime](#agent-runtime)
- [Isolated Environments](#isolated-environments)
- [Tool System](#tool-system)
- [Action Broker + Service Gateway](#action-broker--service-gateway)
- [Permission System](#permission-system)
- [GitHub Integration](#github-integration)
- [Slack Integration](#slack-integration)
- [Dashboard](#dashboard)
- [Team Management](#team-management)
- [LLM Gateway](#llm-gateway)
- [Memory & Context](#memory--context)
- [Security & Threat Model](#security--threat-model)
- [Infrastructure](#infrastructure)
- [Pricing](#pricing)
- [v0.1 Scope](#v01-scope)
- [Go-to-Market](#go-to-market)
- [Open Questions](#open-questions)

---

## Vision & Positioning

Stripe built internal coding agents ("Minions") that produce over 1,000 PRs per week — entirely unattended, from Slack message to merged pull request. They had to build everything: a fork of Goose, isolated devboxes, a central MCP tool server with 400+ tools, feedback loops with linting and CI, monitoring, and a dedicated "Leverage team" to maintain it all.

Every engineering team that wants this has to do the same. There's no off-the-shelf product that does what Stripe built.

Familiar is that product — and more. Not just coding agents, but a full agent platform that integrates with the services teams already use, through a security model that means agents can act on your behalf without you handing them the keys. Open source, self-hostable, or managed — teams choose the deployment model that fits their trust boundary.

| | Stripe Minions (internal) | Familiar |
|---|---|---|
| Who can use it | Stripe engineers | Any engineering team |
| Agent | Fork of Goose, heavily customized | imp (built for this) |
| Scope | Coding tasks only | Coding + service tasks (Slack, APIs, monitoring) |
| Environments | Custom devboxes (10s spin-up) | Fly Sprites (isolated, persistent) |
| Tools | Toolshed (400+ internal MCP tools) | MCP + built-in tools (any language) |
| Task dispatch | Slack bot + internal platforms | Slack + dashboard + API |
| Feedback loop | Local lint + CI (max 2 rounds) | Verify gates + failure accumulation |
| Service access | Internal auth | Action broker + service gateway (managed or self-hosted) |
| Permissions | Internal access controls | Layered permission system (always/notify/ask/never) |
| Monitoring | Internal web UI | LiveView dashboard |
| Source | Proprietary (internal) | Open source |
| Build cost | Dedicated team, months of work | Sign up and connect your repo (or self-host) |

### Key differentiators

1. **Verify gates** — Every coding task has a shell command contract. The agent isn't done when it says it's done — it's done when the test passes.
2. **Failure accumulation** — Failed attempts are preserved. The next agent sees what was tried and why it failed.
3. **Action broker + service gateway** — Agents act on external and internal services without ever seeing tokens. Familiar enforces permissions + audit logging, then executes actions through the gateway. Teams choose managed (Familiar hosts) or self-hosted (customer runs in their own infra).
4. **Permission controls** — Layered permissions (always/notify/ask/never) control what agents can do, per service, per action type. Conservative defaults, teams loosen with trust.
5. **Unattended by design** — Not an interactive coding assistant. Fire and forget. Come back to a PR, not a chat session.
6. **MCP-native tools** — Teams bring their own tools in any language via MCP. No Elixir required.
7. **Slack-first** — Teams invoke agents from where they already work. Tag the bot in a thread, agent does the work.

---

## Target Audience

Engineering teams (10-200 engineers) who:

- Have well-defined, repetitive tasks they want automated (migrations, refactors, test coverage, dependency updates, triage, monitoring)
- Already use CI/CD and have tests they trust
- Use Slack for team communication
- Can't justify building dedicated agent infrastructure
- Want agents that prove their work, not just claim it's done
- Need agents that can interact with external services securely (GitHub, Sentry, Linear, Stripe, internal APIs)

**Ideal early users:**
- Teams already using AI coding agents (Claude Code, Cursor, Aider) who want to scale to unattended work
- Teams with good test coverage (verify gates need tests)
- Teams with enough repetitive work to justify automation

---

## How It Works

### Coding tasks (the wedge)

```
1. Connect your GitHub repo to Familiar
2. Create a task (dashboard, Slack, or API):
     Title: "Add pagination to the users endpoint"
     Verify: "cargo test api::users::pagination"
3. Familiar clones your repo into an isolated environment
4. An imp agent reads the task, explores the codebase, implements the work
5. Agent runs the verify command
   - Pass → Familiar opens a PR on your repo
   - Fail → Failure output is saved, agent retries with that context
6. You review and merge the PR
```

No babysitting. No chat session. No steering. The verify command is the feedback loop.

### Service tasks (the expansion)

```
1. Connect services through the dashboard (OAuth, API keys, or tokens)
2. Define what the agent should do:
     "When a P0 error appears in Sentry, create a task to fix it
      and notify #oncall in Slack"
3. The agent acts through the action broker:
   - Agent describes intent (`services.call(...)`)
   - Familiar checks permissions (Always / Notify / Ask / Never)
   - Gateway executes the action with real credentials
4. Dashboard shows everything the agent did, full audit trail
```

By default, Familiar manages the gateway and credentials. Teams that need credentials to stay in their own infrastructure can self-host the gateway component — same code, same features, different trust boundary.

### Combined flow

The real power is when coding and service tasks compose:

```
Sentry alert: "NullPointerException in PaymentProcessor.java"
  → Agent reads the error context from Sentry (via action broker)
  → Agent creates a coding task: "Fix NPE in PaymentProcessor"
    Verify: "gradle test --tests PaymentProcessorTest"
  → Agent clones repo, fixes the code, verify passes
  → PR opens on GitHub
  → Agent posts in #oncall: "Fixed the NPE, PR #342 ready for review"
  → Team reviews and merges
```

---

## Architecture Overview

```
┌─ Familiar (Phoenix app) ──────────────────────────────────────┐
│                                                                │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐   │
│  │ Task System   │  │ Agent Manager │  │ GitHub App         │   │
│  │              │  │              │  │                    │   │
│  │ • Create     │  │ • Supervise  │  │ • Clone repos      │   │
│  │ • Schedule   │  │ • Dispatch   │  │ • Open PRs         │   │
│  │ • Verify     │  │ • Monitor    │  │ • Webhooks         │   │
│  │ • Retry      │  │ • Kill       │  │ • Status checks    │   │
│  └──────────────┘  └──────┬───────┘  └────────────────────┘   │
│                           │                                    │
│         ┌─────────────────▼─────────────────┐                  │
│         │          imp (SDK)                 │                  │
│         │                                   │                  │
│         │  Agent Loop · LLM Client          │                  │
│         │  Built-in Tools · MCP Client      │                  │
│         │  Tasks Tool · Permission Gate     │                  │
│         └─────────────────┬─────────────────┘                  │
│                           │                                    │
│  ┌──────────────┐  ┌──────▼───────┐  ┌────────────────────┐   │
│  │ Action       │  │ Environments │  │ Service Gateway    │   │
│  │ Broker       │  │              │  │                    │   │
│  │              │  │ • Fly Sprites│  │ • Managed (default)│   │
│  │ • Permissions│  │ • Clone repo │  │ • Self-hosted opt  │   │
│  │ • Approvals  │  │ • Install    │  │ • Credential store │   │
│  │ • Audit log  │  │   deps       │  │ • OAuth / tokens   │   │
│  │ • Rate limit │  │ • Isolated   │  │ • API execution    │   │
│  └──────────────┘  └──────────────┘  └────────────────────┘   │
│                                                                │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐   │
│  │ LiveView     │  │ Slack App    │  │ LLM Gateway        │   │
│  │ Dashboard    │  │              │  │                    │   │
│  │              │  │ • Bot invoke │  │ • Route + meter    │   │
│  │ • Tasks      │  │ • Threads   │  │ • Cost caps        │   │
│  │ • Agent logs │  │ • Notify    │  │ • BYOK             │   │
│  │ • Services   │  │ • Approvals │  │ • Multi-provider   │   │
│  │ • Permissions│  │              │  │                    │   │
│  │ • Kill switch│  └──────────────┘  └────────────────────┘   │
│  └──────────────┘                                              │
│                                                                │
│  ┌──────────────┐                                              │
│  │ MCP          │                                              │
│  │ Connections  │  PostgreSQL + pgvector                       │
│  │              │                                              │
│  │ • Team tools │                                              │
│  │ • Any lang   │                                              │
│  │ • 3rd party  │                                              │
│  └──────────────┘                                              │
└────────────────────────────────────────────────────────────────┘
```

**Open source, single codebase, single language.** imp agents run as OTP processes on the BEAM. Fly Sprites are sandboxed execution environments for the agent's bash/code tools — the agent brain runs on the control plane. The service gateway is a deployable component of Familiar — managed by default, self-hostable for teams that need credentials in their own infrastructure.

---

## Task System

The beans concepts — verify gates, fail-first, failure accumulation, dependency scheduling — reimplemented in Elixir as native Familiar functionality, built on imp's tasks tool.

### Task structure

```elixir
%Task{
  id: uuid,
  team_id: uuid,
  repo_id: uuid | nil,                      # nil for service-only tasks
  title: "Add pagination to users endpoint",
  description: "Implement cursor-based pagination...",
  verify: "cargo test api::users::pagination",
  status: :open | :running | :passed | :failed,
  task_type: :coding | :service | :hybrid,
  priority: :p0 | :p1 | :p2 | :p3,
  parent_id: uuid | nil,
  produces: ["PaginationParams"],
  requires: ["UserSchema"],
  attempts: [],                              # list of attempt records
  max_attempts: 5,
  created_by: uuid,
  source: :dashboard | :slack | :api | :agent,  # where the task came from
  created_at: datetime,
  updated_at: datetime
}
```

### Verify gates

Every coding task has a shell command that defines done. The command runs inside the task's isolated environment.

- **Pass (exit 0)** → task closes, PR opens
- **Fail (exit non-zero)** → attempt recorded with output, agent retries or task stays open

Service tasks may have verify gates too (e.g., `curl -s https://api.example.com/health | jq -e '.status == "ok"'`) or may be verified by the agent's judgment + human review.

### Fail-first (optional, default on for coding tasks)

Before dispatching an agent, run the verify command. If it already passes, reject — the test doesn't test anything new. Disabled by default for service tasks and refactoring work.

### Failure accumulation

When an agent fails, output is appended to the task:

```
## Attempt 1 — 2026-03-04T12:00:00Z
Agent: imp/claude-sonnet-4
Exit code: 1
Duration: 4m 32s
---
FAILED api::users::pagination::test_cursor_next_page
  Expected 20 items, got 0
---
Files changed: src/api/users.rs, src/models/pagination.rs
```

Next agent sees this. No repeated mistakes.

### Dependency scheduling

Tasks declare what they produce and require. Familiar resolves the graph, detects cycles, dispatches in topological order. Tasks with no dependencies run in parallel.

### Parent-child tasks

Large tasks decompose into subtasks. When all children pass, the parent verifies. Decomposition can be manual (team creates subtasks) or agent-assisted (agent proposes, team approves).

---

## Agent Runtime

The agent runtime is **imp** (`~/imp`). See `~/imp/PLAN.md` for the full spec.

Familiar uses imp as an SDK:

```elixir
{:ok, agent} = Imp.start_agent(%{
  model: team.default_model,
  tools: Imp.default_tools() ++ mcp_tools(team) ++ familiar_tools(task),
  system_prompt: build_agent_prompt(task, team),
  on_event: &handle_agent_event(task, &1),
  credential_broker: platform_broker(team),
  permission_gate: platform_permissions(team)
})
```

### What imp provides

- Agent loop (prompt → LLM → tool calls → loop)
- Multi-provider LLM client (Anthropic, OpenAI, Google)
- Built-in tools (read, write, edit, bash, grep, find, ls, etc.)
- Tasks tool (create, show, close, verify, context)
- MCP client (connect to external tool servers)
- Action broker interface (isolated process; agent never sees credentials)
- Permission gate (always/once/never, per-tool patterns)
- Streaming events (text, thinking, tool calls, errors)
- Context management (compaction, token tracking)

### What Familiar adds on top of imp

- Task persistence (Postgres, not local files)
- GitHub integration (clone, branch, commit, PR)
- Slack integration (bot invocation, thread context, notifications)
- Isolated environments (Fly Sprites)
- Team management and auth
- Dashboard (LiveView)
- Dispatch scheduling (parallel agents, dependency ordering)
- LLM Gateway (cost tracking, caps, BYOK, multi-provider routing)
- Permission extensions (Allow + Notify level, persistent storage, push notifications)
- Service gateway (managed or self-hosted) with credential storage and OAuth
- Action broker integration (permissions, approvals, audit trail)
- Agent monitoring, kill switch, audit trail

### Familiar-specific tools

Registered with imp beyond the built-in set:

- `git.commit(message)` — commit current changes (agent works on a branch)
- `git.diff()` — view current changes
- `services.list()` — list connected services and available actions
- `services.call(service, action, params)` — act on a service through the action broker and gateway
- `notify(channel, message)` — send a notification (Slack, dashboard)

---

## Isolated Environments

Each repo gets an isolated sandbox. The agent brain (imp process) runs on the BEAM, but its bash/execution tools target the sandbox.

### Fly Sprites

- **Pre-warmed** — repo cloned, dependencies installed, tools available
- **Isolated** — can't reach production, can't reach other teams, network restricted
- **Persistent disk** — dependencies survive across agent runs
- **Checkpoint/restore** — snapshot clean state, rollback after each run
- **Sleep when idle** — $0 until needed, wake in ~200ms

### Environment lifecycle

```
Team connects repo
  → Familiar creates Fly Sprite
  → Clones repo, installs dependencies, snapshots clean state
  → Sprite sleeps ($0 until needed)

Coding task dispatched
  → Sprite wakes, restores clean snapshot
  → Creates branch: familiar/task-{id}
  → Agent executes (bash, write, etc. target the Sprite)
  → Verify passes → commit, push, open PR
  → Sprite sleeps
```

Service-only tasks don't need an environment — they run through the action broker and service gateway (plus any connected MCP tools).

### Parallel agents

Multiple agents can run on the same repo simultaneously. Each gets its own branch on the clean snapshot. Familiar tracks file references and warns on potential collisions.

---

## Tool System

Teams extend the agent with MCP. No Elixir required.

### Built-in tools (from imp)

Always available: read, write, edit, bash, grep, find, ls, task tools.
On demand: code intelligence, web search, database, testing, etc.

### MCP tools (team-provided)

Teams connect MCP servers that expose tools to the agent in any language:

- Python MCP server that searches internal documentation
- Go MCP server that checks deployment status
- TypeScript MCP server that queries the team's data warehouse
- Ruby MCP server that runs database migrations

**Connecting MCP servers:**
```
Dashboard → Settings → Tools → Add MCP Server
  URL: https://tools.mycompany.com/mcp
  Auth: Bearer token
```

### Third-party MCP servers

Teams can connect any compatible MCP server — Sourcegraph, Linear, Sentry, PagerDuty, etc. The MCP ecosystem is growing fast.

---

## Action Broker + Service Gateway

The core security component for service work. **Familiar brokers actions, not credentials.** Agents never see tokens. The gateway handles credential storage and API execution. Teams choose who runs it.

There are two parts:

- **Action broker** — enforces permissions, handles approvals, rate limits, and writes the audit trail.
- **Service gateway** — stores credentials, executes API calls, handles OAuth token refresh. A deployable component of Familiar with two modes: managed (default) or self-hosted.

### Architecture

```
Agent: services.call("sentry", "get_issue", {issue_id: "PROJ-123"})
  │
  ▼
Action broker:
  1. Authenticate request (team-scoped)
  2. Check permissions → Always / Notify / Ask / Never
  3. If Ask → notify a human (Slack + dashboard), wait for response
  4. Forward the approved action to the gateway
  5. Log action (audit trail: who/what/when + params + result metadata)
  6. Return result to agent (issue data; never credentials)

Service gateway:
  - Fetch credentials from encrypted store
  - Handle OAuth token refresh if needed
  - Execute the real API call
  - Return a sanitized result/error to the broker
```

### Deployment modes

**Managed (default)**

Familiar runs the gateway. Teams configure service credentials through the dashboard — OAuth flows, API keys, tokens. Credentials are encrypted at rest. Zero infrastructure for the customer.

```
Dashboard → Services → Connect Sentry
  → OAuth flow or paste API key
  → Credentials encrypted and stored
  → Agent can now call Sentry through the broker
```

This is the right default for most teams. Fast onboarding, no infra to manage.

**Self-hosted**

For teams with compliance requirements or internal services that can't be reached from the public internet. The gateway is a deployable component of Familiar — same code, same features, runs in the customer's infrastructure.

```
1. Deploy the gateway (Docker image from the Familiar repo)
2. Configure credentials locally (Vault, KMS, env vars)
3. Register the gateway with the Familiar control plane
4. Agent actions route through the customer's gateway instead
```

Self-hosted gateways can reach internal APIs, VPN-only services, and private databases that the managed gateway never could. The action broker still enforces permissions and logs the audit trail — only the execution location changes.

**Hybrid**

Teams can use both. Managed gateway for SaaS services (Sentry, Linear, Slack), self-hosted gateway for internal APIs and services behind the firewall.

### OAuth and credential management

The gateway handles OAuth flows, token storage, and automatic refresh for connected services. In managed mode, Familiar handles this end-to-end. In self-hosted mode, teams can use their existing credential infrastructure (Vault, KMS, etc.) or let the gateway manage OAuth directly.

Supported credential types:
- **OAuth 2.0** — full flow with token refresh (Sentry, Linear, Slack, GitHub, etc.)
- **API keys** — static tokens, encrypted at rest
- **Custom auth** — headers, mTLS, bearer tokens, whatever the service needs

### Why brokered actions matter

Without brokered actions: you hand the agent a Stripe key / GitHub token / internal service token. If the agent is tricked by a prompt injection, it has full access.

With brokered actions: the agent can only request specific actions. The broker enforces org policy, a human can approve high-risk operations, the gateway executes using real credentials the agent never sees, and the broker records a complete audit trail.

### Open source and trust

The gateway is part of the Familiar codebase — fully open source. Teams can audit exactly what handles their credentials, whether they use managed or self-hosted. This is table stakes for security-sensitive infrastructure: you shouldn't have to trust a black box with your API keys.

---

## Permission System

Layered permissions that control what agents can do, with progressive trust.

| Level | Behavior | Default for |
|---|---|---|
| **Always Allow** | Agent acts, no friction | Read-only operations |
| **Allow + Notify** | Agent acts, team gets notification | — (promoted as trust builds) |
| **Ask Every Time** | Agent pauses, sends approval request | All write/create/delete actions |
| **Never** | Hard block | Destructive actions |

### Defaults

Conservative. Everything starts at **Ask Every Time** except:
- Read-only operations on connected services → **Always Allow**
- Destructive operations → **Never**

Teams promote actions as they build trust: "reading Sentry is fine" → Always Allow. "Creating Linear tickets is fine but I want to know" → Allow + Notify.

### Granularity

Per-service, per-action-type:

```
GitHub:
  Read code                    Always Allow
  Create branches              Always Allow
  Open PRs                     Allow + Notify
  Merge PRs                    Never (humans merge)

Sentry:
  Read errors                  Always Allow
  Resolve issues               Ask Every Time

Linear:
  Read tickets                 Always Allow
  Create tickets               Allow + Notify
  Close tickets                Ask Every Time

Slack:
  Read messages                Always Allow
  Post in channels             Always Allow
  DM users                     Ask Every Time
```

### Approval UX

When the agent hits an "Ask Every Time" action:
1. Agent pauses
2. Notification via Slack DM + dashboard (real-time LiveView)
3. Shows: what the agent wants to do, why, specific parameters
4. Team member taps: **Allow Once** / **Always Allow** / **Allow + Notify** / **Deny**
5. Permission optionally promoted for future calls

---

## GitHub Integration

Familiar is a GitHub App. Teams install it on their org, grant access to specific repos.

### Capabilities

- **Clone repos** into isolated environments
- **Create branches** for agent work (`familiar/task-{id}-{slug}`)
- **Push commits** with agent-authored changes
- **Open PRs** with summary, verify output, and attempt history
- **Report status checks** — tasks show as pending/passed/failed
- **Receive webhooks** — sync environments on push to main

### PR format

```markdown
## 🤖 Familiar: Add pagination to users endpoint

**Task:** Add cursor-based pagination to the GET /users endpoint
**Verify:** `cargo test api::users::pagination` ✅ (attempt 2)
**Agent:** claude-sonnet-4 | 3m 41s | $0.12

### What changed
- `src/api/users.rs` — Added PaginationParams and cursor-based query logic
- `src/models/pagination.rs` — New: generic pagination helper
- `tests/api/users_test.rs` — 4 new tests

### Attempt history
- **Attempt 1** (failed): Tried offset-based pagination, existing tests
  expect cursor-based.
- **Attempt 2** (passed): Cursor-based pagination using created_at timestamp.
```

Humans review and merge. Familiar never merges automatically.

---

## Slack Integration

Slack is the front door. Teams invoke agents from where they already work.

### Bot invocation

Tag the Familiar bot in any Slack thread:

```
@familiar fix the flaky test in payments — it's the timezone issue
  from last week's incident

@familiar update all our Python deps and make sure tests still pass

@familiar check if the Sentry errors from #oncall are related to
  yesterday's deploy
```

The bot:
1. Reads the full thread for context
2. Creates a task (coding, service, or hybrid depending on the request)
3. Dispatches an agent
4. Posts updates in the thread (started, working on it, PR opened, failed)

### Thread context

Slack threads often contain rich context — error messages, screenshots, links to PRs, discussion about what the fix should look like. The bot ingests the full thread and includes it in the agent's context.

### Notifications

- Task completed → thread reply + link to PR
- Task failed → thread reply with failure summary
- Approval needed → DM to the requesting team member
- Kill switch activated → thread reply confirming agent stopped

### Channel integration

Teams can configure Familiar to monitor channels:
- Watch #oncall for patterns ("can someone fix...", error reports)
- Watch #deploys for failed deployment notifications
- Post daily summaries of agent activity to a designated channel

---

## Dashboard

LiveView — server-rendered, real-time via WebSocket, no JavaScript framework.

### Screens

**Tasks** (home)
- All tasks: open, running, passed, failed
- Create new tasks (coding or service)
- Filter by repo, status, priority, type, source
- Bulk operations (re-run failed, cancel running)

**Task Detail**
- Spec: title, description, verify command
- Live agent activity (streaming when running)
- Attempt history with full output
- Subtasks and dependency graph
- Link to PR (if passed)
- Re-run, edit, cancel, delete

**Agent Activity**
- All running agents across the team
- Live streaming of current tool calls and output
- Token usage and cost per agent
- Kill switch per agent

**Repos**
- Connected repositories and environment status
- Per-repo settings (default branch, setup commands, env vars)

**Services**
- Connected service gateways + exposed services/actions
- Connection status, last used, permission summary
- Add/remove gateways / connections

**Tools**
- Connected MCP servers and tool inventory
- Add/remove MCP connections
- Built-in tool group configuration

**Permissions**
- Per-service, per-action permission matrix
- Current level + ability to change
- History of permission changes

**Audit Trail**
- Every action the agent took on external services
- Timestamp, service, action, parameters, result
- Filterable and searchable

**Team**
- Members and roles
- Invite links
- Model preferences, cost caps, usage

**Kill Switch**
- Prominent stop button in header (always visible)
- Immediately halts agent, cancels pending broker calls, drops LLM request
- Per-agent and team-wide options

---

## Team Management

### Roles

| Role | Can do |
|---|---|
| **Owner** | Everything, billing, delete team |
| **Admin** | Manage repos, services, tools, permissions, create/run tasks |
| **Member** | Create tasks, view results, re-run failed tasks, approve requests |
| **Viewer** | View tasks and results only |

### Auth

- GitHub OAuth (sign in with GitHub, see your org's repos)
- Team creation linked to GitHub org
- Members added by GitHub username or invite link

---

## LLM Gateway

Sits on the control plane, proxies LLM requests from agents.

```
imp agent → LLM Gateway (Elixir) → Anthropic / OpenAI / Google
```

Handles:
- **API key management** — teams never see provider API keys (unless BYOK)
- **Model selection** — default model per team, overridable per task
- **Token counting** — track input/output tokens per request
- **Cost tracking** — real-time spend per team, stored in Postgres
- **Rate limiting** — prevent runaway agents
- **Cost caps** — team-configurable: "stop at $X/month"
- **BYOK** — teams can bring their own API keys

---

## Memory & Context

### Task context (ephemeral)

The current LLM context window during a task run. Dies when the task completes.

### Attempt history (persistent)

Every task attempt is recorded with output, files changed, model used, duration. Survives across retries. This IS the memory for coding tasks.

### Team knowledge (persistent)

Structured facts about the team's codebase and conventions:

```
codebase.language = "Rust"
codebase.test_framework = "cargo test"
conventions.branch_prefix = "familiar/"
conventions.pr_reviewers = ["@alice", "@bob"]
services.sentry.project = "backend-api"
services.linear.team = "ENG"
```

- Stored in Postgres, scoped by team
- Loaded into agent context at task start
- Editable via dashboard
- Agent can write new facts via a memory tool

### Semantic recall (future)

Embeddings over past task history, agent conversations, and service interactions. "What happened the last time we had a payments timeout?" Not v0.1 but the pgvector infrastructure supports it.

---

## Security & Threat Model

### Defense layers

```
Layer 1: Environment isolation (Fly Sprite / Firecracker)
  → Agent can't escape sandbox
  → Agent can't reach other teams
  → Network restricted to package registries via proxy

Layer 2: Action broker + service gateway
  → No service credentials in the agent context
  → Managed gateway: credentials encrypted at rest, isolated from agent processes
  → Self-hosted gateway: credentials never leave customer infrastructure
  → Every external action authenticated and logged
  → Rate limited per service per team

Layer 3: Permission system
  → Team defines what agent can do
  → Conservative defaults (Ask Every Time for writes)
  → Progressive trust model

Layer 4: Branch isolation
  → Agent works on a branch, never pushes to main
  → Human must review and merge the PR

Layer 5: Anomaly detection (future)
  → Rate-of-change monitoring in broker
  → Unusual patterns flagged (50 tickets created after months of 3/day)
  → Auto-pause on suspicious behavior

Layer 6: Transparency (dashboard)
  → Team sees everything agent does
  → Full audit trail for service actions
  → Kill switch always available
```

### What the agent CAN'T do

- Access production systems directly
- Merge its own PRs
- See other teams' repos, tasks, or credentials
- Access the internet (except package registries)
- See service credentials (OAuth tokens, API keys — isolated in the gateway)
- Exceed the permission boundary the team has set

### Prompt injection defense

1. **Action broker + service gateway** — even if the agent is tricked, it can only request approved actions; credentials are isolated in the gateway, never in agent context
2. **Permission gates** — destructive actions require human approval by default
3. **Environment isolation** — compromised agent can't escape the sandbox
4. **Audit trail** — everything is logged, anomalies are visible
5. **Kill switch** — instant agent termination

---

## Infrastructure

### Control plane

- **Phoenix app** on Fly.io
- **PostgreSQL + pgvector** for tasks, teams, attempts, gateway registrations, audit trail, embeddings
- **imp agents** as supervised OTP processes on the BEAM
- **LiveView** for real-time dashboard
- **Oban** for background jobs (environment sync, scheduled tasks, notifications)

### Execution environments

- **Fly Sprites** per repo (Firecracker isolation, persistent disk, checkpoint/restore)
- Sleep when idle ($0), wake in ~200ms
- Disk persists installed dependencies

### Service gateway

- **Managed mode (default)** — gateway runs on Familiar's infrastructure, credentials encrypted at rest in Postgres
- **Self-hosted mode** — same gateway code deployed in customer infrastructure (Docker image), credentials stay in customer's network
- **OAuth engine** — built-in OAuth flow + token refresh for common SaaS APIs (Sentry, Linear, Slack, GitHub, etc.)
- **Hybrid** — managed for SaaS, self-hosted for internal APIs

### Scaling

The BEAM handles thousands of concurrent agent processes. Each agent is a lightweight OTP process. Bottleneck is LLM API rate limits and environment capacity, not the control plane.

---

## Pricing

### Cost structure per team (estimated)

| Component | Monthly cost |
|---|---|
| Control plane (shared BEAM) | ~$0 per team (amortized) |
| Fly Sprite per repo | ~$1-5/repo/month |
| Persistent disk per repo | ~$0.15/GB/month |
| Service gateway (managed) | Included in base plan |
| PostgreSQL (shared, amortized) | ~$0.50/team |
| LLM tokens | Variable (dominant cost) |

### Pricing model

- **Base plan:** $X/month per team — repos, concurrent agents, dashboard, GitHub + Slack integration, connected services
- **Usage-based LLM:** pass-through pricing with margin, or included allocation
- **BYOK:** teams bring own API keys (lower cost for them, lower margin for us)

Target: $500-2,000/month per team. Must be cheaper than the engineering time to build this themselves.

Exact pricing TBD after validating with early users.

---

## v0.1 Scope

The minimum to put this in one team's hands and learn if it works. The wedge is coding tasks with verify gates.

### Must have

- [ ] GitHub App (OAuth, clone repos, create branches, open PRs)
- [ ] Task CRUD (create, list, show, edit, delete — via dashboard)
- [ ] Verify gates (run shell command in environment, pass/fail)
- [ ] Failure accumulation (attempt history with output)
- [ ] Isolated environment per repo (Fly Sprite, clone + deps)
- [ ] Agent dispatch (one task at a time is fine for v0.1)
- [ ] imp agent with built-in tools (read, write, edit, bash, grep, find, ls)
- [ ] PR creation on verify pass
- [ ] LiveView dashboard (tasks, agent activity, attempt history)
- [ ] Team auth (GitHub OAuth, single team)
- [ ] LLM token tracking (cost per task)
- [ ] Slack bot (create tasks from Slack, status updates in thread)

### v0.2 (coding agent maturity)

- [ ] Parallel agent dispatch
- [ ] Dependency scheduling (produces/requires)
- [ ] Parent-child tasks with agent-assisted decomposition
- [ ] Fail-first verification
- [ ] MCP tool connections
- [ ] Environment auto-sync on push to main
- [ ] BYOK (bring your own API key)
- [ ] Environment variable management
- [ ] Cost caps per team

### v0.3 (service expansion)

- [ ] Service gateway with managed credential storage (OAuth + API keys, encrypted at rest)
- [ ] Action broker (permissions, approvals, rate limiting, audit trail)
- [ ] Self-hosted gateway deployment (Docker image, registration protocol)
- [ ] Permission system (four levels, per-service, per-action)
- [ ] Service tasks (non-coding work via gateway)
- [ ] Connected services dashboard (service cards, permissions matrix)
- [ ] Audit trail for service actions
- [ ] Slack channel monitoring (watch for patterns, auto-create tasks)
- [ ] Approval flow via Slack DM
- [ ] Team knowledge / memory system

### Later

- Adversarial review (second agent reviews the work)
- Anomaly detection
- GitLab / Bitbucket support
- Semantic recall (RAG over task history)
- Agent self-building tools (HTTP template, Lua)
- Scheduled/recurring tasks
- Multi-team billing
- Mobile dashboard

---

## Open Source Strategy

Familiar is fully open source. The managed platform is the business — not the code.

### Why open source

1. **Trust** — The gateway handles service credentials. Teams need to audit the code that touches their Stripe keys and database tokens. Open source is the trust mechanism.
2. **Distribution** — The agent space is crowded. Open source gets adoption when you're competing against Claude Code, Cursor, Goose, Aider, and whatever ships next week.
3. **Self-host option** — Enterprise teams with compliance requirements can run everything themselves. Most won't — but the option existing removes "vendor lock-in" from the objection list.
4. **Contributions** — Good open source projects attract contributors who build tools, fix bugs, add provider integrations, and extend the platform in ways a small team can't.
5. **Ecosystem** — imp (agent engine), beans (task tracker), and Familiar (platform) form a coherent open source ecosystem. Each piece is useful standalone. Together they're the full stack.

### What's open source

Everything. The entire Familiar codebase — Phoenix app, service gateway, action broker, dashboard, imp integration, Slack bot, GitHub App logic. MIT or Apache 2.0.

### What you pay for

The managed service:
- Hosted infrastructure (Fly Sprites, control plane, database)
- Managed service gateway (we handle credentials, OAuth, uptime)
- LLM gateway (API key management, cost tracking, rate limiting)
- SLA, support, onboarding

Self-hosting is free. Managing it yourself is the cost.

---

## Go-to-Market

### Phase 1: beans (now)

beans is already built, published on crates.io, and works today. It validates the core concepts (verify gates, failure accumulation, dependency scheduling) in the open.

**Show HN: beans — Task tracker for AI coding agents with verify gates**

This establishes the ideas, builds an audience of developers who care about verified agent work, and creates a funnel: "I love beans locally, but I want this on my team's codebase in the cloud" → that's Familiar.

### Phase 2: Familiar open source + managed beta (coding wedge)

Open source the full codebase. Launch the managed platform in private beta with 3-5 teams on real coding tasks. Source from:
- beans users who want the cloud version
- HN/Twitter engagement from the beans launch
- Teams posting about AI coding agent workflows
- Direct outreach to teams with good test coverage

Open source launch and managed beta can happen simultaneously — the code being public builds credibility and drives signups.

### Phase 3: Service expansion beta

Once coding tasks work reliably for early users, introduce the service gateway (managed + self-hosted) and service integration. Early users are the best candidates — they already trust the platform for coding, expanding to services is natural.

### Phase 4: Public launch

Public launch with pricing once the product works for 5-10 teams across both coding and service tasks.

---

## Open Questions

1. **imp maturity** — Familiar depends on imp for the agent runtime. imp Phase 1 (agent loop, LLM client, core tools) + Phase 2 (MCP client, tasks tool) are prerequisites for Familiar v0.1.

2. **Fly Sprites** — Relatively new. Need to verify: checkpoint/restore stability, disk persistence, networking, startup latency under load.

3. **Environment setup** — Cloning is easy. Installing dependencies reliably across languages (npm, cargo, pip, bundler, mix, go mod) is hard. May need per-language templates or team-defined setup scripts.

4. **Verify command reliability** — Flaky tests as verify gates mean agents retry forever on something that isn't their fault. Strategy: timeout, max attempts, team can mark "flaky, human review instead."

5. **OAuth engine scope** — Building OAuth flows + token refresh for common SaaS APIs (Sentry, Linear, Slack, GitHub) is a significant surface area. May start with API key support only and add OAuth incrementally per service.

6. **beans rename** — The beans CLI may get a new name. Concepts carry forward regardless.

7. **Slack app review** — Slack's app directory review process can take weeks. Direct install link works initially but limits discoverability.

8. **Self-hosted gateway networking** — Self-hosted gateways need outbound-only connectivity to register with the Familiar control plane. Need a clean handshake protocol and good docs for enterprise network teams.

9. **Multi-repo tasks** — Some tasks span multiple repos (e.g., update a shared library and all its consumers). Architecture should support this eventually.

10. **Pricing validation** — $500-2,000/month is a guess. Need conversations with target teams to validate willingness to pay and what features justify the price.

---

## Influences

- **Stripe Minions** — Proved the model: unattended agents, one-shot from task to PR, isolated environments, 1000+ PRs/week.
- **beans** (`~/beans`) — Task tracker with verify gates, failure accumulation, dependency scheduling. The concepts Familiar's task system is built on.
- **imp** (`~/imp`) — Elixir agent engine. The agent runtime for Familiar.
- **OpenClaw** — Proved massive demand for personal AI agents (180k GitHub stars). Security cautionary tale.
- **Composio** — "Brokered actions" pattern: agent never sees tokens.
- **Supabase / PostHog** — Open source core, managed platform as the business. Proved the model works.
- **MCP** (Model Context Protocol) — Open standard for connecting tools to agents.
- **Fly Sprites** — Firecracker-based sandboxes with persistent disk, checkpoint/restore.
