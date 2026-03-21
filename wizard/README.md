# wizard

wizard is the command center for the Tower ecosystem.

It owns:
- the canvas-native interface
- orchestration and supervision
- runtime monitoring
- integrated editor, terminal, and browser surfaces
- focus rooms and spatial navigation

## Key docs
- `SPEC.md`
- `FRONTEND_ARCHITECTURE.md`
- `BACKEND_ARCHITECTURE.md`
- `FULLSTACK_ARCHITECTURE.md`
- `../VISION.md`
- `../UMBRELLA.md`

## Current state

Wizard now has initial Rust workspace scaffolding under `wizard/crates/`:
- `wizard-proto`
- `wizard-store`
- `wizard-terminal`
- `wizard-browser`
- `wizard-orch`

It is still early and mostly architecture/spec-driven, but it is no longer spec-only.

## Configuration

Wizard is config-centric, but its config is split across distinct surfaces so shared policy does not leak into local UI state:

- **Shared project config:** `<project>/.wizard.toml`
- **User config:** `~/.config/wizard/config.toml`
- **Local Wizard state:** `<project>/.wizard/`
- **Shared work state:** `<project>/.mana/`

Override order:

```text
built-in defaults
  < user config
  < project config
  < environment overrides
  < CLI or in-session command overrides
```

Use project config for shared Wizard behavior that a repo wants everyone to inherit. Use user config for personal defaults across projects. Keep `.wizard/` for local layouts, views, caches, and other personal state that should not become repo policy.
