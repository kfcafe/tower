# YC Application — Familiar

**Batch:** Spring 2026 (late application — still accepted)
**Status:** DRAFT — needs Asher's review + personal details

**North star:** Autonomous agents for engineering teams.

---

## Company Description (50 chars max)

```
Autonomous agents for engineering teams
```

---

## What is your company going to make?

```
Familiar gives engineering teams autonomous agents that code, triage, and act
on services — all unattended, all under the team's control.

A team creates a task: "fix the flaky test in payments." An agent clones the
repo into an isolated sandbox, explores the code, implements a fix, and runs
the verify gate — a shell command (usually a test) that defines "done." If it
passes, a PR opens. If it fails, the output is saved and the next attempt sees
what was tried and why it failed. Humans review and merge.

That's the coding wedge. The platform goes further. Agents monitor Sentry for
errors and auto-file coding tasks. They triage Linear tickets. They act on
external and internal services through an action broker where the agent never
sees credentials — the team controls what agents can do per service, per
action, with layered permissions (always/notify/ask/never).

The combined flow: Sentry alert fires → agent reads the error context → creates
a coding task → fixes the code → verify passes → PR opens → team gets notified.
End to end, no human in the loop until code review.

Stripe built this internally ("Minions") — 1,000+ PRs per week. They needed a
dedicated team and months of infrastructure. We're that infrastructure as a
product.
```

---

## Who writes code, or does other technical work on your product?

```
Just me. I've built two codebases that make up the Familiar stack:

- imp: 60,000 lines of Elixir. The agent engine that powers Familiar — agent
  loop, multi-provider LLM client, 20+ built-in tools, MCP client, task system
  with verify gates and failure accumulation, sub-agent dispatch, permission
  gates, context management. Finishing UX polish now.

- beans: 30,000 lines of Rust. Task tracker for AI agents with verify gates,
  failure accumulation, and dependency scheduling. Published on crates.io.

I also build and use AI agents daily — I've written custom tools, skills, and
agent workflows on top of pi (an open-source coding agent). That daily
experience building with agents is what shaped Familiar's design.

Total: ~90,000 lines of code across Rust and Elixir, all authored by me in the
past [TODO: timeframe].
```

---

## Are you looking for a cofounder?

```
[TODO: Asher to answer — are you open to a cofounder? YC prefers teams but
accepts strong solos. Honest answer is best here.]
```

---

## Where do you live now?

```
[TODO: city/country]
```

---

## How far along are you?

```
The agent engine is built. imp is 60,000 lines of Elixir: full agent loop,
multi-provider LLM client, 20+ tools, MCP client, task system with verify
gates and failure accumulation, sub-agent dispatch, and permission gates.
Finishing UX polish now.

The core workflow (task → agent → verify → done) is already proven in beans
(30k LOC Rust, published on crates.io). Verify gates, failure accumulation,
and dependency scheduling all work today.

The product layer (Phoenix web app, GitHub integration, task management,
sandbox orchestration) is what I'm building now. The agent interface is thin —
spawn, stream, kill, callback — so I'm wiring the product against pi's SDK
(an open-source agent framework) first to get a working demo fast, then
swapping in imp as the production engine. The product layer doesn't change
either way.

No external users or revenue yet.
```

---

## How long have you been working on this?

```
[TODO: When did you start building pi/beans/imp? Need the timeline.]
```

---

## Tech Stack

```
Agent engine: imp (Elixir/OTP — agent runtime, supervision, concurrency)
Initial SDK: pi (open-source TypeScript agent framework, for fast prototyping)
Web app: Phoenix + LiveView (server-rendered, real-time via WebSocket)
Database: PostgreSQL + pgvector
Sandboxes: Fly Sprites (Firecracker isolation, persistent disk, checkpoint/restore)
Background jobs: Oban
LLM providers: Anthropic, OpenAI, Google (multi-provider, BYOK support)
Tool protocol: MCP (Model Context Protocol)
```

---

## Are people using your product?

```
Not yet for Familiar itself. beans is published on crates.io and available
publicly. imp (the agent engine) is in daily use by me.
```

---

## Why did you pick this idea to work on?

```
I build software with AI agents every day. I use them for real work — not
demos, not toy projects. Through that I learned exactly what it takes to make
agents work autonomously: isolated environments, verify gates that prove
correctness, failure context that prevents repeated mistakes, secure service
access, and progressive permission controls. So I built it — an agent engine
(imp, 60k LOC Elixir) and a task orchestration system (beans, 30k LOC Rust).

Stripe proved this model works at scale — their internal Minions system
produces 1,000+ PRs per week, entirely unattended. But they built everything
from scratch: a custom agent, isolated devboxes, 400+ MCP tools, feedback
loops, monitoring, and a dedicated "Leverage team" to maintain it all.

Every engineering team that wants autonomous agents has to rebuild the same
stack. There's no product that does what Stripe built — let alone extends it
to service tasks like triage, monitoring, and API integrations.

I've already built the hardest parts. Familiar is the product that packages
them for any engineering team.
```

---

## Who are your competitors?

```
Infrastructure layer (we use them, don't compete with them):
- Terminal Use (YC W26): Deploys agent sandboxes. No task workflow, no verify
  gates, no service integration — it's plumbing.
- Modal, Fly Sprites, E2B: Raw sandbox compute. We run on top of these.

Agent products (closest competitors):
- Factory (YC W23, $100M+ raised): Autonomous coding agents. Black-box — the
  agent decides when it's done, not a verify gate. Coding only, no service
  tasks or action broker.
- Devin (Cognition): Interactive coding agent. Requires babysitting — it's an
  assistant, not an autonomous worker.
- Cursor / Windsurf / Claude Code: IDE and CLI agents. Interactive, human-in-
  the-loop. Great for pairing, wrong model for unattended work.

What none of them have:
1. Verify gates — a shell command contract that proves correctness
2. Failure accumulation — failed attempts inform the next try
3. Action broker — agents act on services without seeing credentials
4. Layered permissions — always/notify/ask/never per service, per action
5. The combined loop — monitoring → triage → code → PR → notify, all autonomous

The only system with all of this is Stripe's internal Minions. Familiar is the
external version — and goes further with service tasks.
```

---

## How do or will you make money?

```
Monthly subscription per team: $500-2,000/month depending on repos, concurrent
agents, and included LLM tokens. Usage-based LLM pricing on top, or BYOK
(bring your own API keys).

Infrastructure cost per team is low:
- Fly Sprite per repo: ~$1-5/month (sleeps when idle)
- Postgres (shared): ~$0.50/team
- LLM tokens: pass-through with margin, or customer-provided

Target customers: engineering teams (10-200 engineers) with repetitive work —
migrations, refactors, dependency updates, test coverage, triage, monitoring
response. If agents produce even 5 PRs per week that would have taken an
engineer a few hours each, the subscription pays for itself many times over.

Expansion revenue comes from service tasks. Once a team trusts Familiar for
coding, they connect Sentry, Linear, PagerDuty, internal APIs. More
connections = more value = higher willingness to pay.
```

---

## Please tell us in one or two sentences about something impressive that each founder has built or achieved.

```
I built an agent engine (60k LOC Elixir) and a task orchestration system (30k
LOC Rust) from scratch — 90,000 lines across two languages — that together
form the infrastructure for autonomous coding agents with verify gates, failure
accumulation, and dependency scheduling. [TODO: Add non-software achievement
if you have one that's more impressive or unexpected. YC loves a wildcard here.]
```

---

## Please tell us about the time you most successfully hacked some (non-computer) system to your advantage.

```
[TODO: This is the wildcard question. YC partners have said this single answer
has gotten people interviews. Think about: negotiating something unusual,
gaming a system, finding a loophole, an unconventional path to a goal. Doesn't
have to be tech-related.]
```

---

## What convinced you to apply to Y Combinator?

```
Terminal Use just launched in YC W26 for the infrastructure layer of this
space — sandboxed agent hosting. That tells me YC already understands the
market. Familiar is the product layer on top: autonomous agents that code,
triage, and act on services, not just sandboxes to run them in.

I need the network more than the money. Selling to engineering teams at
$500-2K/month requires trust and warm introductions. YC alumni running
engineering teams at 10-200 person companies are my exact target customers.
The batch would compress months of cold outreach into weeks of warm intros.
```

---

## If you had any other ideas you considered applying with, please list them.

```
[TODO: Do you have other ideas? Even listing one or two shows breadth of
thinking. Can be related or unrelated to Familiar.]
```

---

## Founder Video (1 minute)

```
[TODO: Record a 1-minute video. Keep it matter-of-fact:

- "I'm Asher. I'm building Familiar — autonomous agents for engineering teams."
- 15 sec: The problem. Stripe built internal agents that produce 1,000 PRs per
  week. Every team that wants this has to rebuild the same infrastructure.
  Nobody sells it as a product.
- 15 sec: What Familiar does. Teams dispatch tasks. Agents code, triage, act
  on services — all unattended, all under the team's control. Verify gates
  prove correctness. Action broker keeps credentials secure.
- 15 sec: What I've built. 90k LOC across Rust and Elixir — an agent engine
  and a task orchestration system. I build with AI agents daily. The
  infrastructure is done, I'm wiring the product now.
- 15 sec: Why now. The infra layer is commoditizing (Terminal Use, Modal, Fly
  Sprites). The product layer — the actual workflow teams need — is wide open.

Don't script word-for-word. Rehearse the beats, talk naturally. iPhone is fine.]
```

---

## Demo Video

```
[TODO: High-leverage. Show imp working. Options:

1. Best: imp takes a real coding task, runs verify, passes, shows the result.
   Even a rough terminal recording.
2. Good: beans dispatching agents, showing verify gates and failure
   accumulation in action.
3. Also good: The combined flow if you can mock it — Slack message → agent
   runs → PR opens. Even partially.
4. Minimum: Walkthrough of imp running — agent loop, tools, task system.]
```

---

## TODO items for Asher

- [ ] Are you looking for a cofounder? (honest answer)
- [ ] Where do you live?
- [ ] Timeline — when did you start building pi, beans, imp?
- [ ] Non-software impressive achievement (for the "impressive thing" question)
- [ ] Hacked a non-computer system (wildcard question — important)
- [ ] Other ideas you considered
- [ ] Record founder video (1 minute, iPhone is fine)
- [ ] Record demo video (imp or beans in action)
- [ ] Any revenue / user traction beyond personal use?
- [ ] Background info: previous jobs, education, anything notable
