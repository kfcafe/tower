# imp on Terminal-Bench 2.0

This directory wires **imp** into **Harbor**, the official harness for **Terminal-Bench 2.0**.

## What this gives you

- a custom Harbor agent adapter for `imp`
- a convenience runner script for TB2.0
- basic trajectory export from `imp --mode json` into Harbor's ATIF format

## Files

- `harbor_imp_agent.py` — custom Harbor installed-agent adapter for `imp`
- `run-termbench2.sh` — convenience wrapper around `harbor run`

## Prereqs

1. Install Harbor

```bash
uv tool install harbor
```

or

```bash
pip install harbor
```

2. Install Docker and ensure it is running.

3. Export provider credentials for the model you want to use.

For Anthropic:

```bash
export ANTHROPIC_API_KEY=...
```

Or, for local testing, you can mount your existing host `imp` OAuth auth file
into the Harbor container instead of using an API key.

## Quick start

From the Tower root:

```bash
bash evals/terminal-bench-2/run-termbench2.sh
```

By default this runs:
- dataset: `terminal-bench@2.0`
- agent: custom `ImpAgent`
- model: `anthropic/claude-opus-4-6`
- concurrency: `1`

The adapter also supports using your host `imp` OAuth credentials by copying
`~/.config/imp/auth.json` into the Harbor container at runtime. This is enabled
by default. To disable that behavior and require pure env-var auth, set:

```bash
export IMP_USE_HOST_AUTH=0
```

For local testing, the easiest way to prefer fresh binaries is:

```bash
export IMP_RELEASE_CHANNEL=edge
```

You can also point directly at a custom binary URL:

```bash
export IMP_BINARY_URL=https://github.com/kfcafe/imp/releases/download/edge/imp-edge-x86_64-unknown-linux-gnu.tar.gz
```

Or mount a local binary directly into the container and tell the adapter where to find it:

```bash
export IMP_MOUNTED_BINARY_PATH=/tmp/imp-host-binary
```


Run a smaller smoke test by filtering tasks:

```bash
bash evals/terminal-bench-2/run-termbench2.sh \
  --include-task-name '*async*' \
  --n-tasks 1
```

Use a different model:

```bash
MODEL=anthropic/claude-sonnet-4-6 \
  bash evals/terminal-bench-2/run-termbench2.sh
```

Increase concurrency:

```bash
N_CONCURRENT=4 bash evals/terminal-bench-2/run-termbench2.sh
```

Pass extra imp-specific options through Harbor agent kwargs:

```bash
bash evals/terminal-bench-2/run-termbench2.sh \
  --agent-kwarg thinking=high \
  --agent-kwarg max_turns=80
```

Override the imp provider explicitly:

```bash
bash evals/terminal-bench-2/run-termbench2.sh \
  --agent-kwarg provider=anthropic
```

## OAuth support

For local testing, the adapter can reuse your host `imp` OAuth credentials.

If `~/.config/imp/auth.json` exists, you can mount it into the Harbor trial
container and the adapter will copy it into place before launching `imp`.
That means Anthropic or OpenAI can work via the same `imp` OAuth credentials you
already use locally, without requiring an API key.

This is intended for local smoke tests, not ideal long-term benchmark hygiene.
For cleaner reproducible runs, an API key is still preferable.

Disable host auth copying with:

```bash
export IMP_USE_HOST_AUTH=0
```


If you prefer not to use the wrapper script:

```bash
PYTHONPATH=$(pwd) harbor run \
  --dataset terminal-bench@2.0 \
  --agent-import-path evals.terminal_bench_2.harbor_imp_agent:ImpAgent \
  --model anthropic/claude-opus-4-6 \
  --n-concurrent 1
```

## How installation works

Inside the Harbor container, the adapter installs `imp` using this precedence order:

1. mounted binary path via `IMP_MOUNTED_BINARY_PATH`
2. explicit binary URL via `IMP_BINARY_URL`
3. edge release channel via `IMP_RELEASE_CHANNEL=edge`
4. versioned release artifacts (`v${IMP_VERSION}`)
5. source build as a last resort

In the normal Harbor path we want to use **fresh prebuilt Linux binaries**, not
compile in the container.

When using the edge channel, the adapter downloads:

- `https://github.com/kfcafe/imp/releases/download/edge/imp-edge-x86_64-unknown-linux-gnu.tar.gz`
- `https://github.com/kfcafe/imp/releases/download/edge/imp-edge-aarch64-unknown-linux-gnu.tar.gz`

Then it:

1. installs basic system dependencies (`curl`, `python3`, `jq` when available)
2. downloads or copies the Linux `imp` binary archive
3. installs it into `$HOME/.local/bin/imp`
4. runs `imp --no-session --mode json ...`
5. stores raw JSON-lines output under the Harbor agent log dir
6. converts that JSON-lines stream into a basic ATIF trajectory for Harbor artifacts

## Model mapping notes

Harbor examples often use provider-qualified names. The adapter maps common
Anthropic names to imp equivalents, including the current models:

- `anthropic/claude-opus-4-6` -> `--provider anthropic --model claude-opus-4-6`
- `anthropic/claude-sonnet-4-6` -> `--provider anthropic --model claude-sonnet-4-6`

If Harbor passes another provider-qualified name, the adapter forwards it to imp with the inferred provider.

## Outputs

Harbor job outputs will include normal Harbor trial/job artifacts. In each trial's agent log directory, the adapter tries to write:

- `imp-jsonl.txt` — raw `imp --mode json` output
- `trajectory.json` — translated ATIF trajectory

## Current limitations

This is a first-pass integration.

- Tool/result trajectory conversion is best-effort, not a perfect semantic reconstruction.
- The cleanest Harbor path is now fresh Linux binaries from the `edge` release channel.
- OAuth reuse works by copying your local `imp` auth file into the container for local tests.
- We have not yet added a Tower CI job that smoke-tests this Harbor integration.
- We have not yet verified a full TB2.0 run in this repo environment.

So this should be considered **set up for local testing**, not yet proven end-to-end here.

## Suggested first smoke test

```bash
bash evals/terminal-bench-2/run-termbench2.sh \
  --include-task-name '*' \
  --n-tasks 1 \
  --n-concurrent 1
```

Then inspect the generated Harbor job directory under:

```bash
evals/terminal-bench-2/jobs/
```

## Troubleshooting

If Harbor cannot import the custom agent:

```bash
export PYTHONPATH=$(pwd)
```

The importable module path is:

```text
evals.terminal_bench_2.harbor_imp_agent:ImpAgent
```

The source file lives under `evals/terminal-bench-2/` for convenience, but Python
imports it through the underscore package path above.

If imp download fails in-container, verify the release asset naming matches the current GitHub release layout.

If auth fails, confirm the provider key is exported in the host shell before starting Harbor.
