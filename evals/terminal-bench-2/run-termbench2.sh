#!/usr/bin/env bash
set -euo pipefail

# Run imp on Terminal-Bench 2.0 through Harbor.
#
# Examples:
#   evals/terminal-bench-2/run-termbench2.sh
#   evals/terminal-bench-2/run-termbench2.sh --n-concurrent 1 --include-task-name '*async*'
#   MODEL=anthropic/claude-opus-4-6 evals/terminal-bench-2/run-termbench2.sh

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
EVAL_DIR="$ROOT_DIR/evals/terminal-bench-2"

if ! command -v harbor >/dev/null 2>&1; then
  echo "error: harbor is not installed. Install with: uv tool install harbor" >&2
  exit 1
fi

if ! command -v docker >/dev/null 2>&1; then
  echo "error: docker is required for Terminal-Bench 2.0" >&2
  exit 1
fi

MODEL="${MODEL:-anthropic/claude-opus-4-6}"
N_CONCURRENT="${N_CONCURRENT:-1}"
JOBS_DIR="${JOBS_DIR:-$ROOT_DIR/evals/terminal-bench-2/jobs}"
PYTHONPATH_PREFIX="$ROOT_DIR"
IMP_RELEASE_CHANNEL="${IMP_RELEASE_CHANNEL:-edge}"

if [[ -z "${ANTHROPIC_API_KEY:-}" && "$MODEL" == anthropic/* ]]; then
  echo "warning: ANTHROPIC_API_KEY is not set; Harbor/imp will likely fail auth" >&2
fi

mkdir -p "$JOBS_DIR"

export PYTHONPATH="$PYTHONPATH_PREFIX${PYTHONPATH:+:$PYTHONPATH}"
export IMP_RELEASE_CHANNEL

exec harbor run \
  --dataset terminal-bench@2.0 \
  --agent-import-path evals.terminal_bench_2.harbor_imp_agent:ImpAgent \
  --model "$MODEL" \
  --n-concurrent "$N_CONCURRENT" \
  --jobs-dir "$JOBS_DIR" \
  "$@"
