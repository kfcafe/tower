#!/usr/bin/env python3
"""Canonical Harbor agent adapter module for imp.

This file mirrors the adapter documented in evals/terminal-bench-2/README.md and
is the importable Python module used by Harbor:

    evals.terminal_bench_2.harbor_imp_agent:ImpAgent
"""

from pathlib import Path

# Keep the implementation in the hyphenated eval directory so docs, scripts,
# and adapter source stay colocated. This package file loads and re-exports it.
_source = Path(__file__).resolve().parents[1] / "terminal-bench-2" / "harbor_imp_agent.py"
_namespace: dict[str, object] = {}
exec(_source.read_text(encoding="utf-8"), _namespace)
ImpAgent = _namespace["ImpAgent"]
