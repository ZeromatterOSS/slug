from __future__ import annotations

import json
from pathlib import Path
from typing import Any

FORBIDDEN_WHAT_RAN = {"Local", "LocalWorker", "Worker", "WorkerInit"}


def validate_evidence(path: Path) -> list[str]:
    failures: list[str] = []
    if not path.is_file():
        return [f"evidence file does not exist: {path}"]

    for line_number, raw_line in enumerate(path.read_text(encoding="utf-8-sig").splitlines(), start=1):
        if not raw_line.strip():
            continue
        try:
            row = json.loads(raw_line)
        except json.JSONDecodeError as exc:
            failures.append(f"line {line_number}: invalid JSON: {exc}")
            continue
        if not isinstance(row, dict):
            failures.append(f"line {line_number}: evidence row must be an object")
            continue
        failures.extend(_validate_row(line_number, row))

    return failures


def _validate_row(line_number: int, row: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    prefix = f"line {line_number}"

    if row.get("executor_boundary") != "reapi":
        failures.append(f'{prefix}: executor_boundary must be "reapi"')
    if _as_int(row.get("reapi_actions")) <= 0:
        failures.append(f"{prefix}: reapi_actions must be positive")
    if _as_int(row.get("direct_local_actions")) != 0:
        failures.append(f"{prefix}: direct_local_actions must be 0")
    if not row.get("backend"):
        failures.append(f"{prefix}: backend must be nonempty")

    action_digests = row.get("action_digests")
    if action_digests is None:
        action_digests = [row.get("action_digest")] if row.get("action_digest") else []
    if not _nonempty_string_list(action_digests):
        failures.append(f"{prefix}: action digests must be nonempty")

    if not _nonempty_list(row.get("uploaded_digests")):
        failures.append(f"{prefix}: uploaded_digests must be nonempty")
    if not _nonempty_list(row.get("materialized_outputs")):
        failures.append(f"{prefix}: materialized_outputs must be nonempty")

    what_ran = row.get("what_ran", [])
    if isinstance(what_ran, list):
        forbidden = sorted(FORBIDDEN_WHAT_RAN.intersection(str(item) for item in what_ran))
        if forbidden:
            failures.append(f"{prefix}: forbidden what_ran entries: {', '.join(forbidden)}")
    else:
        failures.append(f"{prefix}: what_ran must be a list when present")

    return failures


def _as_int(value: Any) -> int:
    return value if isinstance(value, int) else -1


def _nonempty_list(value: Any) -> bool:
    return isinstance(value, list) and bool(value)


def _nonempty_string_list(value: Any) -> bool:
    return isinstance(value, list) and bool(value) and all(isinstance(item, str) and item for item in value)
