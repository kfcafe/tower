# Fail-Then-Pass Verification

## Problem

Agents can write "cheating tests" that don't verify anything:

```python
def test_something():
    assert True  # always passes
```

## Solution

**Fail-first is the default.** Any unit with `--verify` has the verify command run at creation time — it must FAIL:

```
mana quick "fix unicode urls" --verify "pytest test_unicode.py"
```

1. Run verify command → must FAIL (proves it tests something real)
2. Unit created
3. Agent does work
4. `mana close` → verify must PASS

Use `--pass-ok` / `-p` to skip this check for refactoring or build tasks:

```
mana quick "extract helper" --verify "cargo test" -p
```

## CLI Changes

```rust
// In cli.rs, Quick and Create commands:
/// Skip fail-first check (allow verify to already pass)
#[arg(long, short = 'p')]
pass_ok: bool,
```

## Implementation

```rust
// In commands/quick.rs and commands/create.rs, before creating the unit:

if !args.pass_ok {
    if let Some(verify_cmd) = args.verify.as_ref() {
        let project_root = mana_dir.parent()
            .ok_or_else(|| anyhow!("Cannot determine project root"))?;
        
        println!("Running verify (must fail): {}", verify_cmd);
        
        let status = std::process::Command::new("sh")
            .args(["-c", verify_cmd])
            .current_dir(project_root)
            .status()?;
        
        if status.success() {
            anyhow::bail!(
                "Cannot create unit: verify command already passes!\n\
                 \n\
                 The test must FAIL on current code to prove it tests something real.\n\
                 Either:\n\
                 - The test doesn't actually test the new behavior\n\
                 - The feature is already implemented\n\
                 - The test is a no-op (assert True)\n\n\
                 Use --pass-ok / -p to skip this check."
            );
        }
        
        println!("✓ Verify failed as expected - test is real");
    }
}
```

## Example Flow

```bash
# Cheating attempt - test already passes (rejected by default)
$ mana quick "fix unicode" --verify "python -c 'assert True'"
Running verify (must fail): python -c 'assert True'
error: Cannot create unit: verify command already passes!

The test must FAIL on current code to prove it tests something real.

# Real test - fails on current code (accepted)
$ mana quick "fix unicode" --verify "pytest tests/test_unicode.py::test_fetch"
Running verify (must fail): pytest tests/test_unicode.py::test_fetch
FAILED tests/test_unicode.py::test_fetch - URLError: ...
✓ Verify failed as expected - test is real
Created and claimed unit 5: fix unicode (by pi-agent)

# After implementing...
$ mana close 5
Running verify: pytest tests/test_unicode.py::test_fetch
PASSED tests/test_unicode.py::test_fetch
✓ Verify passed for unit 5
Closed unit 5

# Refactoring - verify should already pass, use --pass-ok
$ mana quick "extract helper" --verify "cargo test" --pass-ok
Created and claimed unit 6: extract helper (by pi-agent)
```

## Design History

1. **v1: Opt-in `--fail-first`** — Required explicit flag. Too easy to forget.
2. **v2: Default on, opt-out `--pass-ok` / `-p`** — Fail-first is now the default for all units with `--verify`. The `fail_first` field in unit metadata records that verification was enforced.

## Unit Metadata

The `fail_first: true` field is stored in unit metadata when fail-first was enforced (i.e., verify was provided and `--pass-ok` was not used). This provides an audit trail.

## Integration with deli/spro

Checkpoints already exist for parallel agent work. The fail-first check runs at unit creation time, before any agent claims the work.
