# familiar

familiar is the future team platform in the Tower ecosystem.

It extends the local-first mana/imp/wizard model into:
- approvals
- shared operations
- integrations
- remote orchestration
- team-facing dashboards and workflows

## Key docs
- `plan.md`
- `ESTIMATES.md`
- `../VISION.md`
- `../UMBRELLA.md`

## Current state

Familiar is still plan-heavy. This folder is included in Tower so agents can understand the full ecosystem from one root even before the platform code exists.

## Configuration model

Familiar should be config-centric, but its configuration is different from the local-first repo tools because it is a multi-tenant platform.

The layers are:
- **Operator/runtime config** — Elixir-native app config for deployment, infrastructure, feature flags, and external service wiring
- **Team-managed settings** — model preferences, cost caps, permissions, connected repos, and tool/service settings stored by the app
- **Per-user preferences** — notification and dashboard preferences stored per user
- **Secrets and credentials** — environment variables, secret managers, or the encrypted gateway store; never committed repo config

The key rule is that team data is not the same thing as deploy-time app config. Familiar should keep operator config explicit, keep team settings first-class in the product, and keep credentials out of committed files.
