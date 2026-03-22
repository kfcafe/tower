# Tower Prompt Evaluation Suite

Systematic evaluation of agent prompts using [promptfoo](https://promptfoo.dev).

## Setup

```bash
# API key must be set
export ANTHROPIC_API_KEY=your-key-here

# Or use an env file
echo "ANTHROPIC_API_KEY=your-key" > evals/.env
```

## Running evals

```bash
cd evals/

# Review retry prompts (compare v1 vs v2 formatting)
npx promptfoo eval

# Task completion prompts
npx promptfoo eval -c task-completion-config.yaml

# Run just one test case (for iteration)
npx promptfoo eval --filter-first-n 1

# Skip cache (force fresh API calls)
npx promptfoo eval --no-cache

# View results in browser
npx promptfoo view
```

## What's being tested

### Review retry (`promptfooconfig.yaml`)
When an agent's work passes verify but gets rejected in human review,
does the retry prompt cause the agent to:
- Address every review comment specifically?
- Preserve working code from prior attempts?
- Make surgical changes (not over-correct)?
- Handle vague feedback constructively?

Two prompt formats are compared:
- **v1 (minimal)** — review comments listed simply
- **v2 (structured)** — numbered comments with explicit rules

### Task completion (`task-completion-config.yaml`)
Does the agent build what the unit describes? Does it:
- Use project facts when provided?
- Learn from prior failed attempts?
- Reference completed dependencies?

## Adding test cases

Test cases live in `tests/`. Each is a YAML file with test scenarios.
Add cases based on real review failures — every time you find an agent
that doesn't address review feedback properly, add the scenario here.

## Structure

```
evals/
  promptfooconfig.yaml          # review retry eval (default)
  task-completion-config.yaml   # task completion eval
  prompts/
    review-retry-v1.txt         # minimal review feedback format
    review-retry-v2.txt         # structured review feedback format
    task-work-v1.txt            # standard task prompt
  tests/
    review-retry.yaml           # review retry test cases
    task-completion.yaml        # task completion test cases
```
