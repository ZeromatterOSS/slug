from __future__ import annotations

import json
import os
import shutil
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .fixture import Fixture, FixtureCommand
from .manifest import collect_manifest_roots
from .normalize import normalize_text, path_replacements

REPO_ROOT = Path(__file__).resolve().parents[2]


@dataclass(frozen=True)
class ToolConfig:
    name: str
    executable: Path


@dataclass(frozen=True)
class RunOptions:
    run_root: Path
    timeout_seconds: int = 120


def default_run_root() -> Path:
    configured = os.environ.get("SLUG_V2_ORACLE_ROOT")
    if configured:
        return Path(configured)
    return REPO_ROOT / "target" / "v2o"


def _copy_workspace(fixture: Fixture, run_dir: Path) -> Path:
    workspace = run_dir / "workspace"
    if workspace.exists():
        raise FileExistsError(f"refusing to reuse run workspace: {workspace}")
    shutil.copytree(fixture.workspace, workspace, symlinks=True)
    return workspace


def _apply_mutations(workspace: Path, command: FixtureCommand) -> list[dict[str, str | None]]:
    applied: list[dict[str, str | None]] = []
    for mutation in command.mutations:
        path = workspace / mutation.path
        if not path.is_file():
            raise FileNotFoundError(f"mutation target does not exist: {mutation.path}")
        if mutation.content is not None:
            old = path.read_text(encoding="utf-8")
            path.write_text(mutation.content, encoding="utf-8", newline="")
            applied.append({"path": mutation.path, "find": None, "replace": None, "old_digest_hint": str(len(old))})
            continue
        old = path.read_text(encoding="utf-8")
        assert mutation.find is not None
        assert mutation.replace is not None
        if mutation.find not in old:
            raise ValueError(f"mutation text not found in {mutation.path}: {mutation.find!r}")
        path.write_text(old.replace(mutation.find, mutation.replace), encoding="utf-8", newline="")
        applied.append({"path": mutation.path, "find": mutation.find, "replace": mutation.replace, "old_digest_hint": str(len(old))})
    return applied


def _argv(tool: ToolConfig, command: FixtureCommand, output_base: Path) -> list[str]:
    if tool.name == "bazel":
        return [str(tool.executable), f"--output_base={output_base}", *command.argv]
    return [str(tool.executable), *command.argv]


def run_fixture(fixture: Fixture, tool: ToolConfig, options: RunOptions) -> dict[str, Any]:
    run_id = time.strftime("%Y%m%d-%H%M%S") + f"-{os.getpid()}-{tool.name}"
    run_dir = options.run_root / "runs" / fixture.name / run_id
    run_dir.mkdir(parents=True, exist_ok=False)
    workspace = _copy_workspace(fixture, run_dir)
    output_base = options.run_root / "ob" / fixture.name / tool.name
    output_base.mkdir(parents=True, exist_ok=True)
    replacements = path_replacements(workspace=workspace, run_dir=run_dir, output_base=output_base)

    records: list[dict[str, Any]] = []
    for command in fixture.commands:
        mutations = _apply_mutations(workspace, command)
        argv = _argv(tool, command, output_base)
        env = os.environ.copy()
        start = time.monotonic()
        completed = subprocess.run(
            argv,
            cwd=workspace,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=options.timeout_seconds,
            check=False,
        )
        duration_ms = int((time.monotonic() - start) * 1000)
        manifest_roots = command.manifest_roots or fixture.manifest_roots
        records.append(
            {
                "name": command.name,
                "argv": command.argv,
                "executed_argv": argv,
                "env_allowlist": {key: env.get(key) for key in command.env_allowlist},
                "cwd": str(workspace),
                "exit_code": completed.returncode,
                "stdout": completed.stdout,
                "stderr": completed.stderr,
                "normalized_stdout": normalize_text(completed.stdout, replacements),
                "normalized_stderr": normalize_text(completed.stderr, replacements),
                "duration_ms": duration_ms,
                "mutations": mutations,
                "manifest": collect_manifest_roots(workspace, manifest_roots),
            }
        )

    result = {
        "schema_version": 1,
        "fixture": fixture.name,
        "tool": tool.name,
        "tool_executable": str(tool.executable),
        "run_dir": str(run_dir),
        "workspace": str(workspace),
        "output_base": str(output_base),
        "commands": records,
    }
    (run_dir / f"{tool.name}.json").write_text(json.dumps(result, indent=2, sort_keys=True), encoding="utf-8")
    return result