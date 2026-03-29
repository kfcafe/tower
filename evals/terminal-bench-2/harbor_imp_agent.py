#!/usr/bin/env python3
"""Custom Harbor agent adapter for running imp on Harbor / Terminal-Bench 2.0.

Usage from Harbor:
  harbor run \
    --dataset terminal-bench@2.0 \
    --agent-import-path evals.terminal_bench_2.harbor_imp_agent:ImpAgent \
    --model anthropic/claude-opus-4-6

The Harbor model string is preserved for reporting, but this adapter translates
known Harbor model names into imp-compatible provider/model flags.
"""

from __future__ import annotations

import json
import os
import shlex
from pathlib import Path, PurePosixPath
from typing import Any

from harbor.agents.installed.base import BaseInstalledAgent, CliFlag, with_prompt_template
from harbor.environments.base import BaseEnvironment
from harbor.models.agent.context import AgentContext
from harbor.models.trial.paths import EnvironmentPaths


class ImpAgent(BaseInstalledAgent):
    """Harbor adapter for the imp CLI."""

    SUPPORTS_ATIF: bool = True
    _OUTPUT_FILENAME = "imp-jsonl.txt"

    CLI_FLAGS = [
        CliFlag(
            "thinking",
            cli="--thinking",
            type="enum",
            choices=["off", "minimal", "low", "medium", "high", "xhigh"],
            env_fallback="IMP_THINKING",
        ),
        CliFlag(
            "max_turns",
            cli="--max-turns",
            type="int",
            env_fallback="IMP_MAX_TURNS",
        ),
        CliFlag(
            "provider",
            cli="--provider",
            type="str",
            env_fallback="IMP_PROVIDER",
        ),
    ]

    @staticmethod
    def name() -> str:
        return "imp"

    @property
    def _trajectory_path(self) -> PurePosixPath:
        return PurePosixPath(EnvironmentPaths.agent_dir / "trajectory.json")

    def get_version_command(self) -> str | None:
        return "imp --version"

    def parse_version(self, stdout: str) -> str:
        return stdout.strip().splitlines()[0].strip()

    def _should_install_from_source(self) -> bool:
        model = (self.model_name or "").lower()
        install_mode = os.environ.get("IMP_INSTALL_MODE", "").lower()
        release_channel = os.environ.get("IMP_RELEASE_CHANNEL", "").lower()
        if install_mode == "source":
            return True
        if self._container_mounted_binary_path():
            return False
        if install_mode in {"release", "binary"}:
            return False
        if release_channel == "edge":
            return False
        if model.startswith("gpt-5") or "/gpt-5" in model:
            return True
        return False

    async def install(self, environment: BaseEnvironment) -> None:
        await self.exec_as_root(
            environment,
            command=(
                "if command -v apk >/dev/null 2>&1; then "
                "apk add --no-cache curl bash ca-certificates python3 jq git build-base pkgconf openssl-dev; "
                "elif command -v apt-get >/dev/null 2>&1; then "
                "apt-get update && apt-get install -y curl ca-certificates python3 jq git build-essential pkg-config libssl-dev; "
                "elif command -v yum >/dev/null 2>&1; then "
                "yum install -y curl ca-certificates python3 jq git gcc gcc-c++ make pkgconfig openssl-devel; "
                "else "
                "echo 'Warning: unknown package manager; assuming curl/python3 are available' >&2; "
                "fi"
            ),
            env={"DEBIAN_FRONTEND": "noninteractive"},
        )

        version = self._version or os.environ.get("IMP_VERSION") or "0.1.0"
        mounted_binary = self._container_mounted_binary_path()
        install_script = (
            self._build_mounted_binary_install_command(mounted_binary)
            if mounted_binary
            else self._build_source_install_command()
            if self._should_install_from_source()
            else self._build_install_command(version)
        )
        await self.exec_as_agent(environment, command=install_script)

    @staticmethod
    def _container_mounted_binary_path() -> str | None:
        value = os.environ.get("IMP_MOUNTED_BINARY_PATH", "").strip()
        return value or None

    def _build_mounted_binary_install_command(self, binary_path: str) -> str:
        quoted = shlex.quote(binary_path)
        return (
            "set -euo pipefail; "
            'mkdir -p "$HOME/.local/bin"; '
            f"cp {quoted} \"$HOME/.local/bin/imp\"; "
            'chmod +x "$HOME/.local/bin/imp"; '
            'export PATH="$HOME/.local/bin:$PATH"; '
            'imp --version'
        )

    def _build_source_install_command(self) -> str:
        git_url = shlex.quote(os.environ.get("IMP_GIT_URL", "https://github.com/kfcafe/imp.git"))
        git_ref = shlex.quote(os.environ.get("IMP_GIT_REF", "HEAD"))
        source_path = shlex.quote(os.environ.get("IMP_SOURCE_PATH", "/tmp/imp-src"))
        tower_root = shlex.quote(os.environ.get("IMP_TOWER_ROOT_PATH", "/tmp/tower-root"))
        return (
            "set -euo pipefail; "
            'export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"; '
            "if ! command -v cargo >/dev/null 2>&1; then "
            "  curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable; "
            "fi; "
            'export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"; '
            "BUILD_DIR=/tmp/imp-build; rm -rf \"$BUILD_DIR\"; mkdir -p \"$BUILD_DIR\"; "
            f"if [ -d {source_path} ]; then "
            f"  cp -R {source_path}/. \"$BUILD_DIR\"; "
            "else "
            f"  git clone {git_url} \"$BUILD_DIR\"; "
            f"  if [ {git_ref} != HEAD ]; then (cd \"$BUILD_DIR\" && git checkout {git_ref}); fi; "
            "fi; "
            f"if [ -d {tower_root}/mana ]; then ln -sfn {tower_root}/mana /tmp/mana; fi; "
            'cd "$BUILD_DIR"; '
            'if [ -f Cargo.workspace.toml ] && [ ! -f Cargo.toml ]; then cp Cargo.workspace.toml Cargo.toml; fi; '
            'cargo install --path crates/imp-cli --locked; '
            'imp --version'
        )

    def _build_install_command(self, version: str) -> str:
        quoted_version = shlex.quote(version)
        return (
            "set -euo pipefail; "
            "ARCH=$(uname -m); "
            "case \"$ARCH\" in "
            "  x86_64|amd64) TARGET=x86_64-unknown-linux-gnu ;; "
            "  aarch64|arm64) TARGET=aarch64-unknown-linux-gnu ;; "
            '  *) echo "Unsupported architecture: $ARCH" >&2; exit 1 ;; '
            "esac; "
            'URL="${IMP_BINARY_URL:-}"; '
            'CHANNEL="${IMP_RELEASE_CHANNEL:-release}"; '
            f"VERSION={quoted_version}; "
            'if [ -z "$URL" ]; then '
            '  if [ "$CHANNEL" = "edge" ]; then '
            '    FILE="imp-edge-${TARGET}.tar.gz"; '
            '    URL="https://github.com/kfcafe/imp/releases/download/edge/${FILE}"; '
            '  else '
            '    FILE="imp-${VERSION}-${TARGET}.tar.gz"; '
            '    URL="https://github.com/kfcafe/imp/releases/download/v${VERSION}/${FILE}"; '
            '  fi; '
            'fi; '
            "mkdir -p \"$HOME/.local/bin\" /tmp/imp-install; "
            'curl -fL "$URL" -o /tmp/imp.tar.gz; '
            'tar xzf /tmp/imp.tar.gz -C /tmp/imp-install; '
            'BIN_PATH=$(find /tmp/imp-install -type f -name imp | head -1); '
            'test -n "$BIN_PATH"; '
            'cp "$BIN_PATH" "$HOME/.local/bin/imp"; '
            'chmod +x "$HOME/.local/bin/imp"; '
            'export PATH="$HOME/.local/bin:$PATH"; '
            'imp --version'
        )

    @staticmethod
    def _extract_text_from_content(content: Any) -> str:
        if isinstance(content, str):
            return content
        if not isinstance(content, list):
            return ""

        parts: list[str] = []
        for block in content:
            if not isinstance(block, dict):
                continue
            if block.get("type") == "text":
                text = block.get("text")
                if isinstance(text, str) and text:
                    parts.append(text)
            elif block.get("type") == "thinking":
                text = block.get("text")
                if isinstance(text, str) and text:
                    parts.append(text)
        return "\n".join(parts).strip()

    @staticmethod
    def _iso_from_timestamp(value: Any) -> str | None:
        if isinstance(value, str):
            return value
        if isinstance(value, (int, float)):
            from datetime import datetime, timezone

            return datetime.fromtimestamp(value, tz=timezone.utc).isoformat().replace(
                "+00:00", "Z"
            )
        return None

    def _map_model_for_imp(self) -> tuple[str | None, str | None]:
        """Translate Harbor model names into imp CLI provider/model flags.

        Harbor names are often provider-qualified preview names like
        anthropic/claude-opus-4-1. imp's built-in registry has slightly different
        canonical names, but also supports custom model IDs when a provider is
        supplied.
        """
        if not self.model_name:
            return (None, None)

        raw = self.model_name.strip()
        provider = None
        model = raw
        if "/" in raw:
            provider, model = raw.split("/", 1)

        known = {
            "anthropic/claude-opus-4": ("anthropic", "claude-opus-4-6"),
            "anthropic/claude-opus-4-6": ("anthropic", "claude-opus-4-6"),
            "anthropic/claude-sonnet-4": ("anthropic", "claude-sonnet-4-6"),
            "anthropic/claude-sonnet-4-6": ("anthropic", "claude-sonnet-4-6"),
            "anthropic/claude-haiku-4": ("anthropic", "haiku"),
        }
        if raw in known:
            return known[raw]

        if provider:
            # For OpenAI-family models, let imp auto-select between API-key OpenAI
            # and subscription-backed openai-codex based on available credentials.
            if provider == "openai":
                return (None, model)
            return (provider, model)

        lower = raw.lower()
        if lower.startswith("claude"):
            return ("anthropic", raw)
        if (
            lower.startswith("gpt")
            or lower.startswith("o1")
            or lower.startswith("o3")
            or lower.startswith("o4")
            or lower.startswith("chatgpt")
        ):
            return (None, raw)
        if lower.startswith("gemini"):
            return ("google", raw)

        return (None, raw)

    @staticmethod
    def _build_env(provider: str | None) -> dict[str, str]:
        env: dict[str, str] = {
            "PATH": os.environ.get("PATH", ""),
        }
        pass_through = [
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "GOOGLE_API_KEY",
            "DEEPSEEK_API_KEY",
            "GROQ_API_KEY",
            "CEREBRAS_API_KEY",
            "XAI_API_KEY",
            "MISTRAL_API_KEY",
            "TOGETHER_API_KEY",
            "OPENROUTER_API_KEY",
            "FIREWORKS_API_KEY",
            "IMP_MODEL",
            "IMP_MODE",
            "IMP_THINKING",
            "IMP_MAX_TURNS",
            "IMP_PROVIDER",
            "IMP_INSTALL_MODE",
            "IMP_GIT_URL",
            "IMP_GIT_REF",
            "IMP_SOURCE_PATH",
            "IMP_TOWER_ROOT_PATH",
            "IMP_BINARY_URL",
            "IMP_RELEASE_CHANNEL",
            "IMP_MOUNTED_BINARY_PATH",
            "IMP_MOUNTED_AUTH_PATH",
            "IMP_USE_HOST_AUTH",
        ]
        for key in pass_through:
            value = os.environ.get(key)
            if value:
                env[key] = value

        if provider == "anthropic" and "ANTHROPIC_API_KEY" not in env:
            token = os.environ.get("ANTHROPIC_AUTH_TOKEN")
            if token:
                env["ANTHROPIC_API_KEY"] = token

        return env

    @staticmethod
    def _container_mounted_auth_path() -> str:
        return os.environ.get("IMP_MOUNTED_AUTH_PATH", "/tmp/imp-host-auth.json")

    @staticmethod
    def _host_imp_auth_path() -> Path | None:
        path = Path.home() / ".config" / "imp" / "auth.json"
        return path if path.exists() else None

    @with_prompt_template
    async def run(
        self, instruction: str, environment: BaseEnvironment, context: AgentContext
    ) -> None:
        provider_override = self._resolved_flags.get("provider")
        max_turns = self._resolved_flags.get("max_turns")
        thinking = self._resolved_flags.get("thinking")

        provider, model = self._map_model_for_imp()
        if provider_override:
            provider = provider_override

        env = self._build_env(provider)
        env["PATH"] = f"{os.environ.get('HOME', '$HOME')}/.local/bin:" + env["PATH"]

        host_auth = self._host_imp_auth_path() if os.environ.get("IMP_USE_HOST_AUTH", "1") != "0" else None
        mounted_auth = self._container_mounted_auth_path()
        if mounted_auth:
            setup_auth = (
                'if [ -f ' + shlex.quote(mounted_auth) + ' ]; then '
                'mkdir -p "$HOME/.config/imp" && '
                'cp ' + shlex.quote(mounted_auth) + ' "$HOME/.config/imp/auth.json"; '
                'fi'
            )
            await self.exec_as_agent(environment, command=setup_auth, env=env)
        elif host_auth is not None:
            auth_json = host_auth.read_text(encoding="utf-8")
            setup_auth = (
                'mkdir -p "$HOME/.config/imp" && '
                f"cat > \"$HOME/.config/imp/auth.json\" <<'EOF_IMP_AUTH'\n{auth_json}\nEOF_IMP_AUTH"
            )
            await self.exec_as_agent(environment, command=setup_auth, env=env)

        output_path = PurePosixPath(EnvironmentPaths.agent_dir / self._OUTPUT_FILENAME)
        payload = json.dumps({"type": "prompt", "content": instruction}, ensure_ascii=False)
        command_parts = [
            "imp --no-session --mode json",
        ]
        if provider:
            command_parts.append(f"--provider {shlex.quote(provider)}")
        if model:
            command_parts.append(f"--model {shlex.quote(model)}")
        if thinking:
            command_parts.append(f"--thinking {shlex.quote(str(thinking))}")
        if max_turns is not None:
            command_parts.append(f"--max-turns {int(max_turns)}")
        imp_command = " ".join(command_parts)
        command = (
            f'export PATH="$HOME/.local/bin:$PATH"; '
            f"printf '%s\\n' {shlex.quote(payload)} | "
            f"{imp_command} 2>&1 | tee {shlex.quote(output_path.as_posix())}"
        )

        await self.exec_as_agent(environment, command=command, env=env)

    def populate_context_post_run(self, context: AgentContext) -> None:
        output_path = self.logs_dir / self._OUTPUT_FILENAME
        if not output_path.exists():
            print(f"imp output log missing: {output_path}")
            return

        try:
            trajectory, summary = self._convert_imp_jsonl_to_trajectory(output_path)
        except Exception as exc:
            print(f"Failed to convert imp output to trajectory: {exc}")
            return

        if trajectory is not None:
            trajectory_path = self.logs_dir / "trajectory.json"
            try:
                with open(trajectory_path, "w", encoding="utf-8") as handle:
                    json.dump(
                        trajectory,
                        handle,
                        indent=2,
                        ensure_ascii=False,
                    )
            except OSError as exc:
                print(f"Failed to write trajectory file {trajectory_path}: {exc}")

        if summary:
            context.n_input_tokens = summary.get("input_tokens")
            context.n_output_tokens = summary.get("output_tokens")
            context.n_cache_tokens = summary.get("cached_tokens")
            context.cost_usd = summary.get("cost_usd")
            context.metadata = {
                "imp": {
                    key: value
                    for key, value in summary.items()
                    if key
                    not in {"input_tokens", "output_tokens", "cached_tokens", "cost_usd"}
                }
            }

    def _convert_imp_jsonl_to_trajectory(
        self, output_path: Path
    ) -> tuple[dict[str, Any] | None, dict[str, Any]]:
        steps: list[dict[str, Any]] = []
        session_id = output_path.stem
        agent_version = self.version() or "unknown"
        default_model = self.model_name
        total_prompt_tokens = None
        total_completion_tokens = None
        total_cached_tokens = None
        total_cost_usd = None
        errors: list[str] = []

        pending_tool_calls: dict[str, dict[str, Any]] = {}
        current_turn = 0

        with open(output_path, "r", encoding="utf-8") as handle:
            for raw_line in handle:
                line = raw_line.strip()
                if not line:
                    continue
                try:
                    event = json.loads(line)
                except json.JSONDecodeError:
                    continue

                event_type = event.get("type")
                timestamp = self._iso_from_timestamp(event.get("timestamp"))

                if event_type == "agent_start":
                    default_model = event.get("model") or default_model
                    session_id = event.get("timestamp") or session_id
                    continue

                if event_type == "turn_start":
                    current_turn = int(event.get("index") or current_turn or 0)
                    continue

                if event_type == "turn_end":
                    message = event.get("message") or {}
                    content = self._extract_text_from_content(message.get("content"))
                    reasoning = None
                    if isinstance(message.get("content"), list):
                        reasoning_parts: list[str] = []
                        for block in message["content"]:
                            if isinstance(block, dict) and block.get("type") == "thinking":
                                text = block.get("text")
                                if isinstance(text, str) and text:
                                    reasoning_parts.append(text)
                        reasoning = "\n".join(reasoning_parts).strip() or None
                    metrics = None
                    usage = message.get("usage")
                    if isinstance(usage, dict):
                        metrics = {
                            "prompt_tokens": usage.get("input_tokens"),
                            "completion_tokens": usage.get("output_tokens"),
                            "cached_tokens": usage.get("cache_read_tokens"),
                        }
                    step: dict[str, Any] = {
                        "step_id": len(steps) + 1,
                        "timestamp": self._iso_from_timestamp(message.get("timestamp")) or timestamp,
                        "source": "agent",
                        "message": content or "",
                    }
                    if default_model:
                        step["model_name"] = default_model
                    if reasoning:
                        step["reasoning_content"] = reasoning
                    if metrics and any(v is not None for v in metrics.values()):
                        step["metrics"] = {k: v for k, v in metrics.items() if v is not None}
                    steps.append(step)
                    continue

                if event_type == "tool_call":
                    call_id = event.get("id") or f"toolcall-{len(pending_tool_calls) + 1}"
                    pending_tool_calls[call_id] = {
                        "tool_call_id": call_id,
                        "function_name": event.get("name") or "unknown",
                        "arguments": event.get("arguments")
                        if isinstance(event.get("arguments"), dict)
                        else {"value": event.get("arguments")},
                        "timestamp": timestamp,
                    }
                    continue

                if event_type == "tool_execution_start":
                    call_id = event.get("tool_call_id") or f"tool-{len(pending_tool_calls) + 1}"
                    pending_tool_calls[call_id] = {
                        "tool_call_id": call_id,
                        "function_name": event.get("tool_name") or "unknown",
                        "arguments": event.get("args")
                        if isinstance(event.get("args"), dict)
                        else {},
                        "timestamp": timestamp,
                    }
                    continue

                if event_type == "tool_execution_end":
                    call_id = event.get("tool_call_id") or f"tool-{len(pending_tool_calls) + 1}"
                    call = pending_tool_calls.pop(
                        call_id,
                        {
                            "tool_call_id": call_id,
                            "function_name": event.get("tool_name") or "unknown",
                            "arguments": {},
                            "timestamp": timestamp,
                        },
                    )
                    content = self._extract_text_from_content(event.get("content"))
                    details = event.get("details") if isinstance(event.get("details"), dict) else None
                    extra = {}
                    if event.get("is_error") is not None:
                        extra["is_error"] = event.get("is_error")
                    if details:
                        extra["details"] = details

                    step: dict[str, Any] = {
                        "step_id": len(steps) + 1,
                        "timestamp": call.get("timestamp") or timestamp,
                        "source": "agent",
                        "message": f"Executed {call['function_name']}",
                        "tool_calls": [
                            {
                                "tool_call_id": call["tool_call_id"],
                                "function_name": call["function_name"],
                                "arguments": call.get("arguments") or {},
                            }
                        ],
                        "observation": {
                            "results": [
                                {
                                    "source_call_id": call["tool_call_id"],
                                    "content": content or None,
                                }
                            ]
                        },
                    }
                    if default_model:
                        step["model_name"] = default_model
                    if extra:
                        step["extra"] = extra
                    steps.append(step)
                    continue

                if event_type == "error":
                    error = event.get("error")
                    if isinstance(error, str) and error:
                        errors.append(error)
                        steps.append(
                            {
                                "step_id": len(steps) + 1,
                                "timestamp": timestamp,
                                "source": "system",
                                "message": f"imp error: {error}",
                            }
                        )
                    continue

                if event_type == "agent_end":
                    total_prompt_tokens = event.get("input_tokens")
                    total_completion_tokens = event.get("output_tokens")
                    total_cached_tokens = event.get("cache_read_tokens")
                    total_cost_usd = event.get("cost_total")
                    continue

        if not steps:
            return None, {
                "input_tokens": total_prompt_tokens,
                "output_tokens": total_completion_tokens,
                "cached_tokens": total_cached_tokens,
                "cost_usd": total_cost_usd,
                "errors": errors,
            }

        trajectory: dict[str, Any] = {
            "schema_version": "ATIF-v1.6",
            "session_id": str(session_id),
            "agent": {
                "name": self.name(),
                "version": agent_version,
                "model_name": default_model,
            },
            "steps": steps,
            "final_metrics": {
                "total_prompt_tokens": total_prompt_tokens,
                "total_completion_tokens": total_completion_tokens,
                "total_cached_tokens": total_cached_tokens,
                "total_cost_usd": total_cost_usd,
                "total_steps": len(steps),
            },
        }
        if errors:
            trajectory["extra"] = {"errors": errors}

        summary = {
            "input_tokens": total_prompt_tokens,
            "output_tokens": total_completion_tokens,
            "cached_tokens": total_cached_tokens,
            "cost_usd": total_cost_usd,
            "steps": len(steps),
            "errors": errors,
        }
        return trajectory, summary
