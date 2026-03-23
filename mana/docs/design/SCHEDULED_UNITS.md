# Scheduled Units — Design Specification

Status: Draft  
Owners: mana (storage + scheduling model), wizard-orch (execution)  
Inspired by: Hermes Agent cron system, adapted for Tower's architecture

---

## 1. Problem

Tower has no way to run work on a cadence. There's no "every morning, verify
all facts" or "every Friday, audit dependency versions." Mana facts already
have `stale_after` TTLs, but nothing actually checks them. Hermes solves this
with a built-in cron scheduler in its gateway daemon. Tower needs the
equivalent, split correctly across its architecture.

## 2. Design Principle: Separation of Concerns

Hermes bundles scheduling, execution, and delivery into one daemon. Tower
splits them:

| Concern | Hermes | Tower |
|---------|--------|-------|
| Schedule definition | `jobs.json` in `~/.hermes/cron/` | **Mana unit** with `schedule` field in `.mana/` |
| Schedule storage | JSON file, ephemeral | **Markdown + YAML frontmatter**, version controlled |
| Tick / scheduler loop | Gateway daemon (60s tick) | **wizard-orch daemon** (or standalone `mana tick`) |
| Task execution | In-process `AIAgent` | **Spawns imp** in headless mode |
| Result delivery | Platform adapters (Telegram, etc.) | **Notify config** — shell command templates |
| Cron recursion guard | Disables cron tools inside cron runs | **Agent mode** (Worker can't create scheduled units) |

The key insight: **a scheduled unit is just a unit with a `schedule` field.**
It lives in `.mana/`, is version-controlled, has a verify gate, and can be
inspected with `mana show`. The scheduler is just "find units with schedules
that are due and run them."

## 3. Unit Model Changes

### New fields on Unit

```rust
/// Cron/interval schedule expression. When set, this unit is recurring —
/// it re-opens automatically after closing and runs again at the next
/// scheduled time.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub schedule: Option<String>,

/// When this unit is next due for execution. Computed from `schedule`
/// after each run. The scheduler picks up units where `next_run_at <= now`.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub next_run_at: Option<DateTime<Utc>>,

/// Where to deliver results. Overrides config-level notify settings.
/// Values: "local" (default), "command:<template>", "webhook:<url>".
#[serde(default, skip_serializing_if = "Option::is_none")]
pub deliver_to: Option<String>,

/// Maximum number of scheduled runs. None = unlimited.
/// Decremented after each run. Unit auto-closes when 0.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub repeat: Option<u32>,

/// Whether this scheduled unit is paused. Paused units are skipped
/// by the scheduler but retain their schedule and next_run_at.
#[serde(default, skip_serializing_if = "is_false")]
pub paused: bool,
```

### New unit_type: "scheduled"

```
unit_type: scheduled
```

### Example unit file

```yaml
---
id: '30'
title: Verify all project facts
unit_type: scheduled
status: open
priority: 3
schedule: "0 9 * * 1-5"
next_run_at: "2026-03-24T09:00:00Z"
verify: "mana fact verify --json"
labels:
  - scheduled
  - maintenance
created_at: "2026-03-23T00:00:00Z"
updated_at: "2026-03-23T00:00:00Z"
---

Check all project facts for staleness. Re-run verify commands on any
that have passed their TTL. Report any that fail.
```

## 4. Schedule Format

### Cron expressions (recurring)
```
0 9 * * *       → Daily at 9:00 AM
0 9 * * 1-5     → Weekdays at 9:00 AM
0 */6 * * *     → Every 6 hours
30 8 1 * *      → First of every month at 8:30 AM
```

### Interval expressions (recurring)
```
every 30m       → Every 30 minutes
every 2h        → Every 2 hours
every 1d        → Every day
every 7d        → Every week
```

### Relative delays (one-shot)
```
30m             → Run once in 30 minutes
2h              → Run once in 2 hours
1d              → Run once in 1 day
```

### ISO timestamps (one-shot)
```
2026-03-25T09:00:00Z   → Run once at this time
```

### Parsing

New module: `mana-core/src/schedule.rs`

```rust
pub enum Schedule {
    Cron(CronExpr),
    Interval(Duration),
    Delay(Duration),
    At(DateTime<Utc>),
}

impl Schedule {
    pub fn parse(s: &str) -> Result<Self>;
    pub fn next_after(&self, after: DateTime<Utc>) -> Option<DateTime<Utc>>;
    pub fn is_one_shot(&self) -> bool;
}
```

Use the `cron` crate for cron expression parsing.

## 5. Notification Config

### Config additions

New `notify` section in `.mana/config.yaml`:

```yaml
# .mana/config.yaml

notify:
  # Shell command run when a unit closes successfully.
  # Template vars: {id}, {title}, {status}, {verify_output}
  on_close: "ntfy publish tower '{title}: passed ✓'"

  # Shell command run when a unit's verify fails.
  # Template vars: {id}, {title}, {attempt}, {output}, {max_attempts}
  on_fail: "ntfy publish tower-alerts '⚠️ {title} failed (attempt {attempt}/{max_attempts})'"

  # Shell command run when a scheduled unit completes (pass or fail).
  # Template vars: {id}, {title}, {status}, {schedule}, {next_run_at}
  on_scheduled_complete: "ntfy publish tower-cron '{title}: {status}'"
```

All notify commands are **fire-and-forget** — spawned async, failures logged
but never block the operation. Same pattern as existing `on_close` and
`on_fail` config hooks.

### Relationship to existing on_close / on_fail

The existing `on_close` and `on_fail` config fields are **workflow hooks** —
they run shell commands for automation (git commit, deploy, etc.).

The new `notify` section is specifically for **human notification**. They
coexist:

```yaml
# Workflow automation (existing)
on_close: "git add -A && git commit -m 'feat(unit-{id}): {title}'"
on_fail: "echo 'Unit {id} failed' >> .mana/failures.log"

# Human notification (new)
notify:
  on_close: "terminal-notifier -title 'mana' -message '{title}: done'"
  on_fail: "terminal-notifier -title 'mana' -message '⚠️ {title} failed'"
```

### Rust struct

```rust
/// Notification configuration for human-facing alerts.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Default)]
pub struct NotifyConfig {
    /// Command template run when a unit closes successfully.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_close: Option<String>,

    /// Command template run when a unit's verify fails.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_fail: Option<String>,

    /// Command template run when a scheduled unit completes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_scheduled_complete: Option<String>,
}
```

Added to Config:
```rust
/// Notification settings for human-facing alerts.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub notify: Option<NotifyConfig>,
```

### Per-unit override

The `deliver_to` field on a unit overrides the config-level notify:

```yaml
# Unit frontmatter
deliver_to: "command:curl -X POST https://hooks.slack.com/... -d '{\"text\": \"{title}: {status}\"}'"
```

### Example notification backends

| Backend | Config example |
|---------|---------------|
| **ntfy** (self-hosted push) | `ntfy publish mytopic '{title}: {status}'` |
| **macOS native** | `terminal-notifier -title 'mana' -message '{title}: {status}'` |
| **Pushover** | `curl -s -F 'token=...' -F 'user=...' -F 'message={title}: {status}' https://api.pushover.net/1/messages.json` |
| **Slack webhook** | `curl -X POST -H 'Content-type: application/json' -d '{"text":"{title}: {status}"}' https://hooks.slack.com/...` |
| **Discord webhook** | `curl -X POST -H 'Content-type: application/json' -d '{"content":"{title}: {status}"}' https://discord.com/api/webhooks/...` |
| **Desktop notify (Linux)** | `notify-send 'mana' '{title}: {status}'` |
| **Log file** | `echo '[{status}] {title}' >> ~/mana-notifications.log` |
| **Email via msmtp** | `echo '{title}: {status}' \| msmtp user@example.com` |

The user picks their own backend. Mana just runs `sh -c` with template
interpolation. Zero platform-specific code.

## 6. Lifecycle

### Creation

```bash
mana schedule "Verify all facts" \
  --schedule "0 9 * * 1-5" \
  --verify "mana fact verify --json"
```

Or via `mana create --schedule`:

```bash
mana create "Audit dependencies" \
  --verify "cargo audit" \
  --schedule "every 7d"
```

### Tick

```bash
mana tick [--quiet] [--dry-run] [--json]
```

1. Find all units where `unit_type == "scheduled"`, `status == open`,
   `paused == false`, `next_run_at <= now`
2. For each due unit: claim → execute → record result → recompute next_run_at
3. Fire notify commands

**wizard-orch** calls `mana tick` on a configurable interval (default: 60s).
Without wizard, add to system crontab:

```bash
* * * * * cd /path/to/project && mana tick --quiet
```

### Pause / Resume / Manual Run

```bash
mana schedule pause 30
mana schedule resume 30
mana schedule run 30      # Execute immediately
mana schedule list
```

### Re-opening after completion

Recurring scheduled units **don't archive** after verify passes. They:
1. Stay `open`
2. Get `next_run_at` recomputed from `schedule`
3. Get a `RunRecord` appended to `history`
4. `repeat` decremented if set; auto-close when 0

One-shot units close and archive normally.

## 7. Recursion Guard

Scheduled units run with **Worker** agent mode. Workers can't create units,
preventing a scheduled task from spawning more scheduled tasks.

## 8. Fact Auto-Verification

With scheduled units, the stale fact problem is solved:

```bash
mana schedule "Re-verify stale facts" \
  --schedule "0 9 * * *" \
  --verify "mana fact verify --json 2>&1 | grep -q '\"failing_count\": 0'"
```

Or as a built-in `mana tick` behavior:

```yaml
tick:
  verify_stale_facts: true
  stale_fact_schedule: "0 9 * * *"
```

## 9. Index Changes

Add to `IndexEntry`:

```rust
pub schedule: Option<String>,
pub next_run_at: Option<DateTime<Utc>>,
pub paused: bool,
```

## 10. New Dependencies

| Crate | Purpose | Size |
|-------|---------|------|
| `cron` | Parse cron expressions, compute next occurrence | ~50KB |

## 11. Implementation Order

1. **NotifyConfig + Config changes** — Add notify section to config, wire execution
2. **Schedule parser** (`schedule.rs`) — Parse all four formats, compute next_after
3. **Unit model changes** — Add schedule fields to Unit
4. **Index changes** — Mirror in IndexEntry
5. **`mana schedule create`** — Create scheduled units
6. **`mana tick`** — Find due units, execute, recompute, notify
7. **`mana schedule list/pause/resume/run`** — Management commands
8. **wizard-orch integration** — Periodic tick

Step 1 is immediately useful even without scheduling — you get notifications
on every `mana close` and `mana run` failure today.

## 12. What This Enables

- **Notifications on any close/fail** — know when work finishes without watching
- **Automated fact verification** — stale facts re-checked on a cadence
- **Recurring maintenance** — dependency audits, test health, code quality
- **Scheduled reports** — weekly summaries, daily status digests
- **Watchdog tasks** — "every hour, check if staging responds"
- **Learning loop maintenance** — "every week, consolidate agent memory"

All stored as regular mana units. Inspectable. Version controlled. Visible
on the wizard canvas.
