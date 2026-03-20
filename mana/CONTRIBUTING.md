# Contributing to mana

## Development Setup

```bash
git clone https://github.com/kfcafe/mana.git
cd mana
cargo build
cargo test
```

Requires Rust stable. No additional system dependencies.

## Adding a Command

Every command follows a 4-file pattern:

1. **Add a variant to the `Command` enum** in `src/cli.rs`
   Define the subcommand with clap attributes, help text, and arguments.

2. **Create `src/commands/yourcommand.rs`** with:
   ```rust
   pub fn cmd_yourcommand(beans_dir: &Path, ...) -> Result<()>
   ```
   The function takes `beans_dir` as its first argument and returns `anyhow::Result<()>`.

3. **Register the module** in `src/commands/mod.rs`
   Add `pub mod yourcommand;` and a `pub use yourcommand::cmd_yourcommand;` re-export.

4. **Add a match arm** in `src/main.rs`
   Wire the new `Command::YourCommand` variant to your function inside the `match cli.command` block (line ~88).

5. **Add tests** as a `#[cfg(test)] mod tests` block at the bottom of your command file. See `src/commands/adopt.rs` or `src/commands/edit.rs` for examples.

## Conventions

### Error handling
Use `anyhow::Result` everywhere. Add context with `.context()`:
```rust
fs::read_to_string(&path)
    .context("Pre-create hook execution failed")?;
```

### Testing
Inline `#[cfg(test)]` modules in the same file as the code under test. No separate test files.

### Unit IDs
Always validate user-supplied IDs with `util::validate_bean_id()` before use.

### Sorting
Use `util::natural_cmp` when sorting unit IDs so that `2` sorts before `10`.

### Naming
- Command functions: `cmd_yourcommand`
- Files: `src/commands/yourcommand.rs` (lowercase, no hyphens)

## Running Tests

```bash
cargo test                    # all tests
cargo test commands::close    # specific module
cargo test -- --nocapture     # see println output
```

## Project Structure

```
src/
  main.rs              # CLI entry point and command dispatch
  cli.rs               # clap argument definitions
  lib.rs               # library root
  commands/
    mod.rs             # module declarations and re-exports
    create.rs          # one file per command
    close.rs
    ...
  index.rs             # unit index (in-memory + YAML persistence)
  unit.rs              # unit data model
  util.rs              # shared helpers (ID validation, sorting, etc.)
```

## License

By contributing, you agree that your contributions will be licensed under the Apache-2.0 license.
