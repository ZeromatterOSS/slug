from __future__ import annotations

import difflib
import json
import re
from pathlib import Path
from typing import Any

from .fixture import Fixture, FixtureCommand


def load_expected(path: Path) -> dict[str, Any] | None:
    if not path.is_file():
        return None
    return json.loads(path.read_text(encoding="utf-8"))


def write_expected(path: Path, result: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    data = dict(result)
    data["generated"] = True
    data["run_dir"] = "<run_dir>"
    data["workspace"] = "<workspace>"
    data["output_base"] = "<output_base>"
    data["tool_executable"] = "<tool_executable>"
    for command in data.get("commands", []):
        command["executed_argv"] = ["<tool_executable>", *command.get("argv", [])]
        command["cwd"] = "<workspace>"
        command["duration_ms"] = "<duration_ms>"
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _pattern_failures(label: str, patterns: tuple[str, ...], text: str) -> list[str]:
    failures = []
    for pattern in patterns:
        if re.search(pattern, text, flags=re.MULTILINE) is None:
            failures.append(f"{label} did not match /{pattern}/")
    return failures


def _contains_failures(label: str, needles: tuple[str, ...], text: str) -> list[str]:
    return [f"{label} did not contain {needle!r}" for needle in needles if needle not in text]


def _compare_command_shape(command: FixtureCommand, actual: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    if command.expected_exit is not None and actual.get("exit_code") != command.expected_exit:
        failures.append(f"{command.name}: exit code {actual.get('exit_code')} != {command.expected_exit}")
    failures.extend(f"{command.name}: {failure}" for failure in _contains_failures("stdout", command.stdout_contains, actual.get("normalized_stdout", "")))
    failures.extend(f"{command.name}: {failure}" for failure in _contains_failures("stderr", command.stderr_contains, actual.get("normalized_stderr", "")))
    failures.extend(f"{command.name}: {failure}" for failure in _pattern_failures("stdout", command.stdout_patterns, actual.get("normalized_stdout", "")))
    failures.extend(f"{command.name}: {failure}" for failure in _pattern_failures("stderr", command.stderr_patterns, actual.get("normalized_stderr", "")))
    return failures


def _compare_expected_command(command: FixtureCommand, expected: dict[str, Any], actual: dict[str, Any]) -> list[str]:
    failures = _compare_command_shape(command, actual)
    if expected.get("exit_code") != actual.get("exit_code"):
        failures.append(f"{command.name}: exit code {actual.get('exit_code')} != oracle {expected.get('exit_code')}")
    if command.compare == "exact":
        for field in ("normalized_stdout", "normalized_stderr", "manifest"):
            if expected.get(field) != actual.get(field):
                failures.append(f"{command.name}: {field} differs from oracle")
    elif command.compare == "semantic" and expected.get("manifest"):
        if expected.get("manifest") != actual.get("manifest"):
            failures.append(f"{command.name}: manifest differs from oracle")
    return failures


def compare_result(fixture: Fixture, actual: dict[str, Any], expected: dict[str, Any] | None) -> list[str]:
    failures: list[str] = []
    actual_commands = actual.get("commands", [])
    if len(actual_commands) != len(fixture.commands):
        return [f"expected {len(fixture.commands)} command records, got {len(actual_commands)}"]

    expected_commands = expected.get("commands", []) if expected else []
    if expected_commands and len(expected_commands) != len(actual_commands):
        failures.append(f"oracle has {len(expected_commands)} commands, actual has {len(actual_commands)}")
        expected_commands = []

    for index, (command, actual_command) in enumerate(zip(fixture.commands, actual_commands)):
        if expected_commands:
            failures.extend(_compare_expected_command(command, expected_commands[index], actual_command))
        else:
            failures.extend(_compare_command_shape(command, actual_command))
    return failures


def write_failure_artifacts(run_dir: Path, fixture: Fixture, failures: list[str], actual: dict[str, Any], expected: dict[str, Any] | None) -> Path:
    artifact_dir = run_dir / "comparison"
    artifact_dir.mkdir(parents=True, exist_ok=True)
    (artifact_dir / "failures.txt").write_text("\n".join(failures) + "\n", encoding="utf-8")
    (artifact_dir / "actual.json").write_text(json.dumps(actual, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    if expected is not None:
        (artifact_dir / "expected.json").write_text(json.dumps(expected, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        expected_text = json.dumps(expected, indent=2, sort_keys=True).splitlines(keepends=True)
        actual_text = json.dumps(actual, indent=2, sort_keys=True).splitlines(keepends=True)
        diff = difflib.unified_diff(expected_text, actual_text, fromfile="expected.json", tofile="actual.json")
        (artifact_dir / "expected_vs_actual.diff").write_text("".join(diff), encoding="utf-8")
    (artifact_dir / "README.txt").write_text(
        f"Fixture {fixture.name} failed. Re-run the single fixture before broad suites.\n",
        encoding="utf-8",
    )
    return artifact_dir