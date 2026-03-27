# Familiar — Plan

**Domain:** getfamiliar.dev

An open source agent platform that operates in two modes: as the **internal operations engine** for running agent-powered businesses at scale, and as an **external product** that engineering teams buy to manage their own agents.

The coding agent with verify gates is the wedge. The platform underneath supports much more. Once a team trusts Familiar for coding tasks, they connect services: "monitor Sentry for new errors and file tasks," "triage Linear tickets and draft responses," "keep dependencies up to date."

Internally, Familiar coordinates thousands of agents across hundreds of client projects — powering products like Wurk where customers describe what they want and agents deliver it. The customer never sees Familiar. They see results.

---

## Table of Contents

- [Vision & Strategic Position](#vision--strategic-position)
- [Two Modes](#two-modes)
- [The Funnel](#the-funnel)
- [Target Audiences](#target-audiences)
- [How It Works](#how-it-works)
- [Architecture Overview](#architecture-overview)
- [Agency Mode](#agency-mode)
- [Platform Mode](#platform-mode)
- [Task System](#task-system)
- [Agent Runtime](#agent-runtime)
- [Isolated Environments](#isolated-environments)
- [Tool System](#tool-system)
- [Action Broker + Service Gateway](#action-broker--service-gateway)
- [Permission System](#permission-system)
- [GitHub Integration](#github-integration)
- [Slack Integration](#slack-integration)
- [Dashboard](#dashboard)
- [Agent Fleet Economics](#agent-fleet-economics)
- [Team Management](#team-management)
- [LLM Gateway](#llm-gateway)
- [Memory & Context](#memory--context)
- [Security & Threat Model](#security--threat-model)
- [Infrastructure](#infrastructure)
- [Pricing](#pricing)
- [Phased Scope](#phased-scope)
- [Go-to-Market](#go-to-market)
- [The Portfolio Strategy](#the-portfolio-strategy)
- [Revenue Projections](#revenue-projections)
- [Open Source Strategy](#open-source-strategy)
- [Open Questions](#open-questions)
- [Influences](#influences)

---

## Vision & Strategic Position

Stripe built internal coding agents ("Minions") that produce over 1,000 PRs per week — entirely unattended, from Slack message to merged pull request. They needed a dedicated team, months of infrastructure, a fork of Goose, isolated devboxes, 400+ MCP tools, and a "Leverage team" to maintain it all.

Every engineering team that wants this has to rebuild the same stack. There's no off-the-shelf product that does what Stripe built — let alone extends it to service tasks.

Familiar is that product. But it's also something more.

### The insight that changes everything

Most companies building in the agent space are selling tools: "here's an AI coding agent" or "here's a platform to manage agents." They're selling picks and shovels.

Familiar operates at two levels simultaneously:

1. **Sell the infrastructure** — engineering teams buy Familiar to manage their own agents (the platform)
2. **Use the infrastructure** — we run Familiar internally to coordinate thousands of agents that power our own products and services (the agency)

This is the AWS playbook. Amazon built cloud infrastructure for their own retail business, then realized other companies would pay for it. We build Familiar to run Wurk and our agency products, then sell access to teams who want to run their own agents.

The agency generates revenue immediately through outcomes. The platform generates revenue over time through subscriptions. Both improve the same system. The agency stress-tests the platform. The platform subsidizes the agency.

### Why not just one or the other?

**Agency-only** caps out. You're limited by how many clients one human can oversee, even with agents doing the work. Eventually you need to sell the platform to scale beyond what a single operator can manage.

**Platform-only** is slow. Developer tools take years to reach meaningful adoption. You burn runway waiting for product-market fit while the agency model could be generating revenue from day one.

Both together compound. Agency revenue funds platform development. Platform reliability improves agency operations. Platform customers validate the infrastructure. Agency operations provide case studies that sell the platform.

---

## Two Modes

### Agency Mode (internal operations)

One operator (you) manages thousands of agents across hundreds of client projects. Clients interact through product frontends — Wurk for software development, future products for content, operations, support. Clients never see Familiar. They see outcomes.

```
Client → Product Frontend (Wurk, etc.) → Familiar API → Agent Fleet → Results → Client
```

In this mode, Familiar is the nervous system. It handles:
- Multi-tenant project isolation
- Agent dispatch and scheduling across all clients
- Cost tracking and margin analysis per client
- Operator dashboard showing ALL projects at once
- Quality monitoring and escalation

### Platform Mode (external product)

Engineering teams buy Familiar to manage their own agents. They see the full dashboard, configure permissions, connect services, monitor agents. This is the product described in the original plan — Stripe Minions as a service.

```
Team → Familiar Dashboard/Slack → Agent Fleet → PRs/Results → Team
```

In this mode, Familiar is a SaaS product. Each team manages their own:
- Repos and environments
- Agent configuration and dispatch
- Permission policies
- Service connections
- Budget and cost caps

### Shared Infrastructure

Both modes run on the same codebase. The differences are:

| | Agency Mode | Platform Mode |
|---|---|---|
| Who uses the dashboard | Operator (you) | Customer team |
| Who configures agents | Operator | Customer team |
| Who pays for LLM tokens | Built into pricing | Customer (or BYOK) |
| Customer interface | Product frontend (Wurk, etc.) | Familiar dashboard + Slack |
| Revenue model | Outcome-based (credits, project fees) | Subscription |
| Multi-tenant view | Operator sees ALL clients | Team sees only their projects |

---

## The Funnel

Each layer of the Tower ecosystem feeds the next:

```
imp (free, open source)                     → distribution, developer awareness
    ↓ "I want to orchestrate multiple agents visually"
Wizard ($30 buy-once desktop app)           → power users, solo builders
    ↓ "I want this for my team / in the cloud / unattended"
Familiar Team ($299-999/month)              → engineering teams
    ↓ "I need enterprise features / self-hosted gateway"
Familiar Enterprise ($2K-10K/month)         → large organizations
    ↓ "I want to build products on this"
Familiar Platform API (usage-based)         → agent-powered businesses
```

Meanwhile, the agency products (Wurk, etc.) generate revenue immediately and run on Familiar internally — proving the platform, funding development, and creating case studies.

---

## Target Audiences

### Agency Mode customers (via product frontends)

**Non-technical founders and business owners** who want software built, content produced, operations automated, or support handled — without managing agents or understanding the infrastructure.

They interact through product-specific UIs. They pay for outcomes. They don't know Familiar exists.

### Platform Mode customers

**Engineering teams (10-200 engineers)** who:

- Have well-defined, repetitive tasks they want automated
- Already use CI/CD and have tests they trust
- Use Slack for team communication
- Can't justify building dedicated agent infrastructure
- Want agents that prove their work, not just claim it's done
- Need agents that can interact with external services securely

**Solo developers and power users** upgrading from Wizard who want:
- Cloud-hosted agent execution
- GitHub integration with auto-PR
- Managed LLM access
- Session sync across machines

### Ideal early users (Platform Mode)

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
   - Agent describes intent (services.call(...))
   - Familiar checks permissions (Always / Notify / Ask / Never)
   - Gateway executes the action with real credentials
4. Dashboard shows everything the agent did, full audit trail
```

### Agency flow (internal)

```
1. Client describes what they want through a product frontend (e.g. Wurk)
2. Product frontend calls Familiar API to create a project + task tree
3. Familiar dispatches agents into isolated environments
4. Agents work, verify gates pass, results flow back
5. Product frontend delivers results to client
6. Operator dashboard shows all clients, costs, margins, status
```

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
│  │ Create       │  │ Supervise    │  │ Clone repos        │   │
│  │ Schedule     │  │ Dispatch     │  │ Open PRs           │   │
│  │ Verify       │  │ Monitor      │  │ Webhooks           │   │
│  │ Retry        │  │ Kill         │  │ Status checks      │   │
│  └──────────────┘  └──────┬───────┘  └────────────────────┘   │
│                           │                                    │
│         ┌─────────────────▼─────────────────┐                  │
│         │     imp (Rust binary, spawned)     │                  │
│         │                                   │                  │
│         │  Agent Loop · LLM Client          │                  │
│         │  Built-in Tools · MCP Client      │                  │
│         │  Context Management               │                  │
│         └─────────────────┬─────────────────┘                  │
│                           │                                    │
│  ┌──────────────┐  ┌──────▼───────┐  ┌────────────────────┐   │
│  │ Action       │  │ Environments │  │ Service Gateway    │   │
│  │ Broker       │  │              │  │                    │   │
│  │              │  │ Fly Sprites  │  │ Managed (default)  │   │
│  │ Permissions  │  │ Clone repo   │  │ Self-hosted opt    │   │
│  │ Approvals    │  │ Install deps │  │ Credential store   │   │
│  │ Audit log    │  │ Isolated     │  │ OAuth / tokens     │   │
│  │ Rate limit   │  │              │  │ API execution      │   │
│  └──────────────┘  └──────────────┘  └────────────────────┘   │
│                                                                │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐   │
│  │ Product      │  │ Slack App    │  │ LLM Gateway        │   │
│  │ Frontend API │  │              │  │                    │   │
│  │              │  │ Bot invoke   │  │ Route + meter      │   │
│  │ Headless     │  │ Threads      │  │ Cost caps          │   │
│  │ Familiar for │  │ Notify       │  │ BYOK               │   │
│  │ agency       │  │ Approvals    │  │ Multi-provider     │   │
│  │ products     │  │              │  │                    │   │
│  └──────────────┘  └──────────────┘  └────────────────────┘   │
│                                                                │
│  ┌──────────────┐  ┌──────────────┐                            │
│  │ LiveView     │  │ MCP          │                            │
│  │ Dashboard    │  │ Connections  │  PostgreSQL + pgvector     │
│  │              │  │              │                            │
│  │ Tasks        │  │ Team tools   │                            │
│  │ Agent logs   │  │ Any lang     │                            │
│  │ Services     │  │ 3rd party    │                            │
│  │ Operator view│  │              │                            │
│  │ Kill switch  │  │              │                            │
│  └──────────────┘  └──────────────┘                            │
└────────────────────────────────────────────────────────────────┘
```

**Key architectural change from the original plan:** imp is a Rust binary, not an Elixir SDK. Familiar spawns imp processes and communicates through the mana filesystem protocol (`.mana/` state) and process events. The agent brain runs inside imp. Familiar handles orchestration, web UI, integrations, and business logic. Clean separation — Familiar never touches LLM calls or tool execution.

---

## Agency Mode

### Multi-Tenant Operator View

The operator dashboard shows everything across all clients:

```
┌─────────────────────────────────────────────────────┐
│  OPERATOR DASHBOARD                                  │
│                                                      │
│  Active clients: 47    Running agents: 23            │
│  Monthly revenue: $14,200    LLM costs: $3,100       │
│  Margin: 78%                                         │
│                                                      │
│  ┌─────────────────────────────────────────────────┐ │
│  │ Client          │ Status │ Agents │ Spend │ Rev │ │
│  │ Acme Corp       │ 3 running │  5  │ $120 │ $500│ │
│  │ StartupXYZ      │ idle      │  0  │  $45 │ $200│ │
│  │ Jane's Bakery   │ 1 running │  2  │  $30 │ $149│ │
│  │ ...             │           │     │      │     │ │
│  └─────────────────────────────────────────────────┘ │
│                                                      │
│  Recent: ✅ Acme/pagination passed (2m ago)          │
│          ⚠️ StartupXYZ/auth failed attempt 2         │
│          ✅ Jane's/menu-page passed (5m ago)         │
└─────────────────────────────────────────────────────┘
```

### Product Frontend API

A headless API that product frontends (Wurk, etc.) consume. The full Familiar dashboard is NOT exposed to agency clients.

```
POST   /api/v1/projects              Create a project for a client
POST   /api/v1/projects/:id/tasks    Create tasks with verify gates
POST   /api/v1/projects/:id/dispatch Dispatch agents
GET    /api/v1/projects/:id/status   Project status (tasks, progress)
GET    /api/v1/projects/:id/stream   SSE stream of progress events
GET    /api/v1/projects/:id/results  Completed artifacts
DELETE /api/v1/projects/:id          Tear down project
```

Each product frontend authenticates with a service key. Client isolation is enforced at the API level — a Wurk service key can only see Wurk projects.

### Client Isolation

Every client project gets:
- Its own isolated Fly Sprite environment
- Its own `.mana/` state
- Its own agent pool (no cross-contamination)
- Its own cost tracking
- No visibility into other clients

### Margin Tracking

For every agent-hour, Familiar tracks:
- LLM token cost (input + output, per model)
- Compute cost (Fly Sprite time)
- Revenue attributed (from the product frontend's pricing)
- Net margin

The operator dashboard surfaces this as: "This client is generating $500/month in revenue at $120 in costs = 76% margin." This is the data that drives pricing decisions and capacity planning.

---

## Platform Mode

Platform Mode is the product engineering teams buy to manage their own agents. This is the original Familiar plan — largely unchanged.

### What teams get

- **Dashboard** — Full LiveView dashboard with tasks, agent activity, attempt history, services, permissions, audit trail
- **GitHub App** — Connect repos, auto-clone, auto-PR on verify pass
- **Slack Bot** — Create tasks from Slack threads, get notifications, approve actions
- **Isolated Environments** — Per-repo Fly Sprites, cloned and dependency-installed
- **Verify Gates** — Shell commands that prove work is done
- **Failure Accumulation** — Failed attempts inform the next try
- **Action Broker** — Agents act on services without seeing credentials
- **Permission System** — Layered control over what agents can do
- **LLM Gateway** — Cost tracking, caps, model selection, BYOK
- **MCP Tools** — Teams bring their own tools in any language
- **Kill Switch** — Immediate agent termination, always visible

### Self-service onboarding

```
1. Sign up with GitHub OAuth
2. Install the Familiar GitHub App on your org
3. Select repos to connect
4. Familiar creates environments (clone + deps)
5. Create your first task (or let Slack bot create one)
6. Watch the agent work
```

---

## Task System

The mana concepts — verify gates, fail-first, failure accumulation, dependency scheduling — implemented as native Familiar functionality.

### Task structure

```elixir
%Task{
  id: uuid,
  team_id: uuid,
  client_id: uuid | nil,                    # nil for platform mode
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
  attempts: [],
  max_attempts: 5,
  created_by: uuid,
  source: :dashboard | :slack | :api | :agent | :product_frontend,
  created_at: datetime,
  updated_at: datetime
}
```

### Verify gates

Every coding task has a shell command that defines done. The command runs inside the task's isolated environment.

- **Pass (exit 0)** — task closes, PR opens
- **Fail (exit non-zero)** — attempt recorded with output, agent retries or task stays open

Service tasks may have verify gates too (e.g., `curl -s https://api.example.com/health | jq -e '.status == "ok"'`) or may be verified by the agent's judgment + human review.

### Fail-first

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

The agent runtime is **imp** — a Rust binary from the Tower ecosystem. Familiar spawns imp as child processes.

### Integration model

```
Familiar (Elixir/OTP)
  │
  ├── Spawns: imp <UNIT_ID> --project-dir <PATH> --model <MODEL> [OPTIONS]
  │
  ├── Monitors: process exit code, stdout/stderr streaming
  │
  ├── State: reads/writes .mana/ directory (task state, attempts, notes)
  │
  └── Events: imp writes structured events to stdout, Familiar parses and routes
```

This is the same model that mana uses locally to dispatch agents. Familiar adds:
- Supervised OTP process per agent (restart on crash, timeout enforcement)
- Event routing to LiveView dashboard (real-time UI updates)
- Event routing to Slack (thread updates, notifications)
- Cost tracking per agent (LLM token metering)

### What imp provides

- Agent loop (prompt → LLM → tool calls → loop)
- Multi-provider LLM client (Anthropic, OpenAI, Google, 11+ providers)
- Built-in tools (read, write, edit, bash, grep, find, ls, diff, scan, web, ask, memory)
- MCP client (connect to external tool servers)
- Context management (compaction, token tracking)
- Verify gate execution

### What Familiar adds on top

- Task persistence (Postgres, not local files)
- GitHub integration (clone, branch, commit, PR)
- Slack integration (bot invocation, thread context, notifications)
- Isolated environments (Fly Sprites)
- Multi-tenant isolation
- Dispatch scheduling (parallel agents, dependency ordering, capacity management)
- LLM Gateway (cost tracking, caps, BYOK, multi-provider routing)
- Action broker + service gateway (brokered external actions)
- Permission system (layered, per-service, per-action)
- Monitoring, kill switch, audit trail

### Familiar-specific tools

Injected into imp's environment via configuration or MCP:

- `git.commit(message)` — commit current changes
- `git.diff()` — view current changes
- `services.list()` — list connected services and available actions
- `services.call(service, action, params)` — act through the action broker
- `notify(channel, message)` — send a notification

---

## Isolated Environments

Each repo gets an isolated sandbox. The imp process runs inside the sandbox — its bash/execution tools are scoped to that environment.

### Fly Sprites

- **Pre-warmed** — repo cloned, dependencies installed, tools available
- **Isolated** — can't reach production, can't reach other teams, network restricted
- **Persistent disk** — dependencies survive across agent runs
- **Checkpoint/restore** — snapshot clean state, rollback after each run
- **Sleep when idle** — $0 until needed, wake in ~200ms

### Environment lifecycle

```
Team connects repo (or agency product creates project)
  → Familiar creates Fly Sprite
  → Clones repo, installs dependencies, snapshots clean state
  → Sprite sleeps ($0 until needed)

Coding task dispatched
  → Sprite wakes, restores clean snapshot
  → Creates branch: familiar/task-{id}
  → imp agent executes (bash, write, etc. inside the Sprite)
  → Verify passes → commit, push, open PR
  → Sprite sleeps
```

### Parallel agents

Multiple agents can run on the same repo simultaneously. Each gets its own branch on the clean snapshot. Familiar tracks file references and warns on potential collisions.

---

## Tool System

Teams extend the agent with MCP. No Elixir required.

### Built-in tools (from imp)

Always available: read, write, edit, multi_edit, bash, grep, find, ls, diff, scan, web, ask, memory.

### MCP tools (team-provided)

Teams connect MCP servers that expose tools to the agent in any language:

- Python MCP server that searches internal documentation
- Go MCP server that checks deployment status
- TypeScript MCP server that queries the data warehouse
- Ruby MCP server that runs database migrations

### Third-party MCP servers

Teams can connect any compatible MCP server — Sourcegraph, Linear, Sentry, PagerDuty, etc.

---

## Action Broker + Service Gateway

The core security component for service work. **Familiar brokers actions, not credentials.** Agents never see tokens. The gateway handles credential storage and API execution.

### Architecture

```
Agent: services.call("sentry", "get_issue", {issue_id: "PROJ-123"})
  │
  ▼
Action broker:
  1. Authenticate request (team-scoped)
  2. Check permissions → Always / Notify / Ask / Never
  3. If Ask → notify human (Slack + dashboard), wait for response
  4. Forward approved action to gateway
  5. Log action (audit: who/what/when + params + result metadata)
  6. Return result to agent (issue data, never credentials)

Service gateway:
  - Fetch credentials from encrypted store
  - Handle OAuth token refresh if needed
  - Execute the real API call
  - Return sanitized result/error to broker
```

### Deployment modes

**Managed (default)** — Familiar runs the gateway. Teams configure credentials through the dashboard. Credentials encrypted at rest. Zero infrastructure for the customer.

**Self-hosted** — For teams with compliance requirements or internal services unreachable from the internet. The gateway is a deployable component — same code, customer's infrastructure.

**Hybrid** — Managed for SaaS services, self-hosted for internal APIs.

### Supported credential types

- **OAuth 2.0** — full flow with token refresh
- **API keys** — static tokens, encrypted at rest
- **Custom auth** — headers, mTLS, bearer tokens

---

## Permission System

Layered permissions that control what agents can do, with progressive trust.

| Level | Behavior | Default for |
|---|---|---|
| **Always Allow** | Agent acts, no friction | Read-only operations |
| **Allow + Notify** | Agent acts, team gets notification | Promoted as trust builds |
| **Ask Every Time** | Agent pauses, sends approval request | All write/create/delete actions |
| **Never** | Hard block | Destructive actions |

### Defaults

Conservative. Everything starts at **Ask Every Time** except:
- Read-only operations on connected services → **Always Allow**
- Destructive operations → **Never**

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

When an agent hits "Ask Every Time":
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
## Familiar: Add pagination to users endpoint

**Task:** Add cursor-based pagination to the GET /users endpoint
**Verify:** `cargo test api::users::pagination` ✅ (attempt 2)
**Agent:** imp/claude-sonnet-4 | 3m 41s | $0.12

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

Slack is the front door for platform mode. Teams invoke agents from where they already work.

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
2. Creates a task (coding, service, or hybrid)
3. Dispatches an agent
4. Posts updates in the thread

### Notifications

- Task completed → thread reply + link to PR
- Task failed → thread reply with failure summary
- Approval needed → DM to the requesting team member
- Kill switch activated → thread reply confirming agent stopped

---

## Dashboard

LiveView — server-rendered, real-time via WebSocket, no JavaScript framework.

### Platform Mode screens

**Tasks** (home) — All tasks: open, running, passed, failed. Create, filter, bulk operations.

**Task Detail** — Spec, live agent activity, attempt history, subtasks, dependency graph, PR link.

**Agent Activity** — Running agents, live streaming, token usage, cost, kill switch.

**Repos** — Connected repos, environment status, settings.

**Services** — Connected services, permission summary, gateway status.

**Tools** — MCP servers, built-in tool groups, inventory.

**Permissions** — Per-service, per-action permission matrix with history.

**Audit Trail** — Every agent action on external services, filterable, searchable.

**Team** — Members, roles, invites, model preferences, cost caps.

### Agency Mode screens (operator-only)

**Operator Overview** — All clients, aggregate revenue, costs, margins, active agents.

**Client Detail** — Per-client tasks, agent activity, cost breakdown, revenue attribution.

**Fleet Health** — Agent success rates, average verify attempts, cost per task, utilization.

**Margin Analysis** — Revenue vs. cost per client, per product, per task type. Identifies unprofitable work.

**Capacity Planning** — Current agent load, available headroom, projected costs at scale.

---

## Agent Fleet Economics

In agency mode, every agent-hour is a business decision. Familiar tracks:

### Per-agent metrics

- LLM tokens consumed (input + output, broken by model)
- LLM cost (per-model pricing applied)
- Compute time (Fly Sprite seconds)
- Wall-clock duration
- Verify attempts (how many tries to pass)
- Task outcome (passed / failed / timed out)

### Per-client metrics

- Total agent-hours consumed
- Total LLM cost
- Total compute cost
- Revenue attributed (from product frontend pricing)
- Net margin
- Average tasks per month
- Average cost per task

### Fleet-wide metrics

- Agent utilization rate (busy vs. idle)
- First-attempt pass rate (% of tasks that pass verify on attempt 1)
- Average cost per successful task
- Cost per revenue dollar
- Model efficiency (which models produce best results per dollar)

### Alerts

- Client margin drops below threshold
- Agent failure rate spikes
- LLM costs exceed budget
- Agent utilization drops (capacity being wasted)
- Specific model producing poor results

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

### Agency mode: Operator role

The operator has a superuser role across all teams/clients in agency mode. Not exposed to platform mode customers.

---

## LLM Gateway

Proxies LLM requests from imp agents.

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
- **BYOK** — teams bring own API keys (lower cost for them, lower margin for us)
- **Model routing** — route simple tasks to cheaper models, complex tasks to stronger ones

---

## Memory & Context

### Task context (ephemeral)

The current LLM context window during a task run. Dies when the task completes.

### Attempt history (persistent)

Every task attempt recorded with output, files changed, model, duration. Survives across retries. This IS the memory for coding tasks.

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

Stored in Postgres, scoped by team. Loaded into agent context at task start. Editable via dashboard.

### Semantic recall (future)

Embeddings over past task history and agent conversations. Not v0.1 but the pgvector infrastructure supports it.

---

## Security & Threat Model

### Defense layers

```
Layer 1: Environment isolation (Fly Sprite / Firecracker)
  → Agent can't escape sandbox
  → Agent can't reach other teams or clients
  → Network restricted to package registries

Layer 2: Action broker + service gateway
  → No service credentials in agent context
  → Every external action authenticated and logged
  → Rate limited per service per team

Layer 3: Permission system
  → Team defines what agent can do
  → Conservative defaults
  → Progressive trust model

Layer 4: Branch isolation
  → Agent works on a branch, never pushes to main
  → Human must review and merge

Layer 5: Client isolation (agency mode)
  → Each client's projects are fully isolated
  → No cross-client data access
  → Operator sees everything, clients see only their own

Layer 6: Anomaly detection (future)
  → Rate-of-change monitoring
  → Auto-pause on suspicious behavior
```

### What the agent CAN'T do

- Access production systems directly
- Merge its own PRs
- See other teams' repos, tasks, or credentials
- Access the internet (except package registries)
- See service credentials
- Exceed the permission boundary
- Access other clients' data (agency mode)

---

## Infrastructure

### Control plane

- **Phoenix app** on Fly.io
- **PostgreSQL + pgvector** for tasks, teams, clients, attempts, audit trail, embeddings
- **imp agents** spawned as supervised child processes
- **LiveView** for real-time dashboard
- **Oban** for background jobs

### Execution environments

- **Fly Sprites** per repo (Firecracker isolation, persistent disk, checkpoint/restore)
- Sleep when idle ($0), wake in ~200ms

### Service gateway

- Managed mode: gateway on Familiar infrastructure
- Self-hosted mode: Docker image, customer infrastructure

### Scaling

The BEAM handles thousands of concurrent agent supervisions. Each agent is a lightweight OTP-supervised process. Bottleneck is LLM API rate limits and environment capacity, not the control plane.

---

## Pricing

### Platform Mode

| Tier | Price | Who | Includes |
|------|-------|-----|----------|
| **Solo** | $29/month | Individual developers | 1 repo, 2 concurrent agents, managed LLM, 50K tokens/month |
| **Team** | $299/month | Engineering teams (5-20) | 10 repos, 8 concurrent agents, Slack bot, 500K tokens/month |
| **Growth** | $999/month | Larger teams (20-50) | 50 repos, 20 concurrent agents, action broker, 2M tokens/month |
| **Enterprise** | $2K-10K/month | Large orgs (50-200+) | Unlimited repos, self-hosted gateway, SSO, SLA, dedicated support |

Additional LLM usage beyond included tokens: pass-through pricing with margin. BYOK available at all tiers (removes included tokens, reduces price).

### Agency Mode (internal pricing for products)

Not a customer-facing price — this is the cost structure for running Wurk and other agency products:

| Component | Cost |
|-----------|------|
| Fly Sprite per project | ~$1-5/month |
| LLM tokens per task | Variable (~$0.05-2.00 depending on complexity) |
| Compute/monitoring overhead | ~$0.50/project/month |

Agency product pricing (what clients pay through Wurk, etc.) is set per-product to maintain 60-80% margins on average.

---

## Phased Scope

### v0.1 — Coding wedge (Agency Mode first)

Build the minimum to power Wurk and validate the model with real paying clients.

- [ ] Product Frontend API (headless Familiar for Wurk)
- [ ] Task CRUD via API
- [ ] Verify gates (run in environment, pass/fail)
- [ ] Failure accumulation
- [ ] Isolated environment per project (Fly Sprite, clone + deps)
- [ ] Agent dispatch (imp binary, spawned and supervised)
- [ ] Multi-tenant client isolation
- [ ] Operator dashboard (clients, agents, costs)
- [ ] LLM token tracking and cost attribution
- [ ] GitHub integration (clone, branch, commit, PR)
- [ ] Basic capacity management

### v0.2 — Platform wedge

Expose Familiar to external teams as a self-service product.

- [ ] Team auth (GitHub OAuth)
- [ ] Self-service onboarding (install GitHub App, select repos)
- [ ] Full LiveView dashboard (tasks, agent activity, attempt history)
- [ ] Slack bot (create tasks, notifications)
- [ ] Parallel agent dispatch with dependency scheduling
- [ ] Parent-child tasks
- [ ] Fail-first verification
- [ ] BYOK (bring your own API key)
- [ ] Cost caps per team
- [ ] Kill switch

### v0.3 — Service expansion

- [ ] Service gateway with managed credential storage
- [ ] Action broker (permissions, approvals, rate limiting, audit trail)
- [ ] Self-hosted gateway deployment (Docker image)
- [ ] Permission system (four levels, per-service, per-action)
- [ ] Service tasks (non-coding work via gateway)
- [ ] Approval flow via Slack DM
- [ ] Audit trail
- [ ] MCP tool connections

### v0.4 — Agency scale

- [ ] Agent fleet economics dashboard
- [ ] Margin analysis per client
- [ ] Capacity planning and auto-scaling
- [ ] Model routing (cheap models for simple tasks)
- [ ] Team knowledge / memory system
- [ ] Multiple agency product support (beyond Wurk)

### Later

- Adversarial review (second agent reviews work)
- Anomaly detection
- GitLab / Bitbucket support
- Semantic recall (RAG over task history)
- Scheduled/recurring tasks
- Multi-team billing
- Mobile dashboard
- Platform API for third-party agent businesses

---

## Go-to-Market

### Phase 1: Wurk (now)

Wurk is the first agency product. Clients describe software, agents build it. Familiar v0.1 powers the orchestration. Revenue comes from Wurk's credit/project pricing, not from Familiar directly.

This validates:
- Agent orchestration at scale
- Verify gates in production
- Multi-tenant isolation
- Cost economics (are margins real?)

### Phase 2: Wizard users → Familiar Solo

Wizard (Godot desktop app) builds a developer audience. Power users who hit the ceiling of local orchestration — want cloud agents, GitHub integration, managed LLM — upgrade to Familiar Solo at $29/month.

This validates:
- Self-service onboarding
- Developer willingness to pay for cloud agent orchestration
- The funnel from local to cloud

### Phase 3: Team launch

Once Solo works, launch Team and Growth tiers. Source from:
- Solo users who want team features
- Wizard/imp community (developers who already know the ecosystem)
- Content marketing (case studies from Wurk, "how we run 100 agents" posts)
- Direct outreach to teams with good test coverage

### Phase 4: Enterprise + Platform API

Enterprise tier for large organizations. Platform API for businesses that want to build their own agent-powered products on Familiar's infrastructure.

### Phase 5: Agency expansion

Launch additional agency products beyond Wurk — content, operations, support. Each is a thin product frontend on Familiar's infrastructure.

---

## The Portfolio Strategy

Familiar enables a unique business model: the agent-powered holding company.

### How it works

1. **Acquire small SaaS products** ($1K-50K MRR) from burned-out founders at 2-4x ARR
2. **Point Familiar at the codebase** — agents audit, fix bugs, modernize, add features
3. **Run ongoing development through Familiar** — agents handle support, dependency updates, feature requests
4. **Track economics** — revenue vs. agent cost per product. Kill underperformers.

### Why Familiar makes this work

Without Familiar, acquiring and running 50 SaaS products requires hiring 50+ engineers. With Familiar, the same portfolio runs on agents coordinated by one operator through the same dashboard used for everything else.

The operator dashboard shows the entire portfolio:
- Revenue per product
- Agent cost per product
- Net margin per product
- Active tasks, recent completions, failures
- Quality metrics (test pass rate, customer complaints)

### The compounding effect

- More products → more revenue → more capital for acquisitions
- More codebases → more agent experience → better first-attempt pass rates
- Higher volume → better LLM pricing → lower costs → higher margins
- Proven track record → easier acquisitions → faster portfolio growth

---

## Revenue Projections

| Year | Agency Revenue | Platform Revenue | Total |
|------|---------------|-----------------|-------|
| 1 | $50K-200K (Wurk + early products) | $10K-50K (Wizard Cloud → Solo) | $60K-250K |
| 2 | $500K-2M (scale + new products) | $200K-1M (Team + Growth tiers) | $700K-3M |
| 3 | $2M-10M (agency scale + portfolio) | $1M-5M (Enterprise) | $3M-15M |
| 5 | $10M-50M (portfolio + multiple agency products) | $10M-50M (Platform + API) | $20M-100M |
| 7+ | $50M-200M | $50M-200M | $100M-400M |

These projections assume:
- Wurk achieves product-market fit in year 1
- Platform mode launches in year 1, reaches meaningful adoption in year 2
- Portfolio acquisitions begin in year 2-3
- Additional agency products launch in year 2-3
- Platform API opens in year 3-4

---

## Open Source Strategy

Familiar is fully open source. The managed platform is the business.

### Why open source

1. **Trust** — The gateway handles credentials. Teams must audit the code.
2. **Distribution** — Open source gets adoption faster than closed-source in a crowded market.
3. **Self-host option** — Removes "vendor lock-in" from the objection list.
4. **Contributions** — Community builds tools, integrations, fixes.
5. **Ecosystem** — imp, mana, Wizard, and Familiar form a coherent open source stack.

### What's open source

Everything. The entire Familiar codebase — Phoenix app, service gateway, action broker, dashboard, imp integration, Slack bot, GitHub App logic.

### What you pay for

The managed service:
- Hosted infrastructure (Fly Sprites, control plane, database)
- Managed service gateway
- LLM gateway
- SLA, support, onboarding

---

## Configuration

### Operator/runtime config

Elixir-native deployment config: environment endpoints, queue configuration, feature flags, gateway registration.

### Team-managed settings

Product-level settings via dashboard: model preferences, cost caps, permission policies, repo settings, tool/service configuration.

### Per-user preferences

Notification settings, dashboard defaults, personal workflow preferences.

### Secrets and credentials

Never in committed config. Managed mode: encrypted gateway store. Self-hosted: customer's secret systems.

---

## Open Questions

1. **imp integration protocol** — Process spawn + stdout events + `.mana/` state? Or something richer (Unix socket, gRPC)?

2. **Fly Sprites maturity** — Need to verify checkpoint/restore stability, disk persistence, networking, startup latency under load.

3. **Environment setup reliability** — Dependency installation across languages (npm, cargo, pip, bundler, mix, go mod) is fragile. May need per-language templates or team-defined setup scripts.

4. **Verify command reliability** — Flaky tests mean agents retry forever. Strategy: timeout, max attempts, mark as "flaky, human review."

5. **OAuth engine scope** — Building OAuth flows + token refresh for common SaaS APIs is significant surface area. May start with API key support only.

6. **Slack app review** — Can take weeks. Direct install link works initially.

7. **Self-hosted gateway networking** — Outbound-only connectivity, clean handshake protocol needed.

8. **Multi-repo tasks** — Some tasks span multiple repos. Architecture should support this eventually.

9. **Pricing validation** — Projections are estimates. Need conversations with target teams.

10. **Agency mode legal structure** — One entity running code in client environments at scale has liability implications. Terms of service, insurance, and contract structure need legal review.

11. **Wizard ↔ Familiar bridge** — How does the Godot desktop app connect to Familiar's cloud? WebSocket to the Phoenix app? Same protocol as the LiveView dashboard?

---

## Influences

- **Stripe Minions** — Proved the model: unattended agents, one-shot task to PR, 1,000+ PRs/week.
- **AWS** — Built infrastructure for internal use, then sold it. The agency → platform playbook.
- **mana** — Coordination substrate with verify gates, failure accumulation, dependency scheduling.
- **imp** — The agent engine that powers everything.
- **Wizard** — Local command center. The desktop on-ramp to Familiar's cloud platform.
- **Wurk** — First agency product. Proves the model generates revenue.
- **OpenClaw** — Proved massive demand for personal AI agents. Security cautionary tale.
- **Composio** — "Brokered actions" pattern: agent never sees tokens.
- **Supabase / PostHog** — Open source core, managed platform as the business.
- **MCP** — Open standard for connecting tools to agents.
- **Fly Sprites** — Firecracker-based sandboxes with persistent disk, checkpoint/restore.
