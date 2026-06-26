#!/usr/bin/env python3
"""Validate Plan 34 local-REAPI evidence produced by tests/plan34."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any


REQUIRED_TESTS = {
    "test_native_link_re_config_default_uses_reapi_without_remote_only",
    "test_native_link_bare_remote_executor_supplies_reapi_cache_endpoint",
    "test_native_link_remote_action_cache_hit_uses_reapi_without_local_fallback",
    "test_native_link_platform_exec_properties_use_reapi_without_local_fallback",
    "test_native_link_nested_paramfile_reaches_reapi_input_tree",
    "test_native_link_cargo_runfiles_paramfile_advances_reapi_layer",
    "test_native_link_cc_actions_reapi_executor_smoke",
    "test_native_link_rules_cc_reapi_executor_smoke",
}
EXECUTION_PHASES = {"remote_execution", "remote_execution_seed"}
CACHE_HIT_PHASE = "remote_action_cache_hit"
EXPECTED_RECORDS = {
    (
        "test_native_link_re_config_default_uses_reapi_without_remote_only",
        "remote_execution",
    ): {
        "reapi_actions": 1,
        "upload_records": 1,
        "materialized_outputs": 1,
    },
    (
        "test_native_link_bare_remote_executor_supplies_reapi_cache_endpoint",
        "remote_execution",
    ): {
        "reapi_actions": 1,
        "upload_records": 1,
        "materialized_outputs": 1,
    },
    (
        "test_native_link_remote_action_cache_hit_uses_reapi_without_local_fallback",
        "remote_execution_seed",
    ): {
        "reapi_actions": 1,
        "upload_records": 1,
        "materialized_outputs": 1,
    },
    (
        "test_native_link_remote_action_cache_hit_uses_reapi_without_local_fallback",
        "remote_action_cache_hit",
    ): {
        "cache_query_actions": 1,
        "cache_hit_actions": 1,
        "materialized_outputs": 1,
    },
    (
        "test_native_link_platform_exec_properties_use_reapi_without_local_fallback",
        "remote_execution",
    ): {
        "reapi_actions": 1,
        "upload_records": 1,
        "materialized_outputs": 1,
    },
    (
        "test_native_link_nested_paramfile_reaches_reapi_input_tree",
        "remote_execution",
    ): {
        "reapi_actions": 1,
        "upload_records": 1,
        "materialized_outputs": 1,
    },
    (
        "test_native_link_cargo_runfiles_paramfile_advances_reapi_layer",
        "remote_execution",
    ): {
        "reapi_actions": 2,
        "upload_records": 2,
        "materialized_outputs": 1,
    },
    (
        "test_native_link_cc_actions_reapi_executor_smoke",
        "remote_execution",
    ): {
        "reapi_actions": 3,
        "upload_records": 3,
        "materialized_outputs": 1,
    },
    (
        "test_native_link_rules_cc_reapi_executor_smoke",
        "remote_execution",
    ): {
        "reapi_actions": 2,
        "upload_records": 2,
        "materialized_outputs": 1,
    },
}


class EvidenceError(Exception):
    pass


def _as_int(record: dict[str, Any], key: str, index: int) -> int:
    value = record.get(key)
    if not isinstance(value, int):
        raise EvidenceError(f"record {index}: {key} must be an integer")
    return value


def load_evidence(path: Path) -> list[dict[str, Any]]:
    if not path.is_file():
        raise EvidenceError(f"Plan 34 evidence file is missing: {path}")

    records: list[dict[str, Any]] = []
    for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError as error:
            raise EvidenceError(f"{path}:{line_no}: invalid JSON: {error}") from error
        if not isinstance(record, dict):
            raise EvidenceError(f"{path}:{line_no}: evidence record must be an object")
        records.append(record)
    if not records:
        raise EvidenceError(f"Plan 34 evidence file is empty: {path}")
    return records


def validate_evidence(records: list[dict[str, Any]]) -> dict[str, int]:
    tests = set()
    phases = set()
    seen_records = set()
    totals = {
        "records": len(records),
        "reapi_actions": 0,
        "direct_local_actions": 0,
        "upload_records": 0,
        "cache_query_actions": 0,
        "cache_hit_actions": 0,
        "materialized_outputs": 0,
    }

    for index, record in enumerate(records, 1):
        if record.get("schema") != 1:
            raise EvidenceError(f"record {index}: schema must be 1")
        test_name = record.get("test")
        if not isinstance(test_name, str) or not test_name:
            raise EvidenceError(f"record {index}: test must be a nonempty string")
        tests.add(test_name)

        phase = record.get("phase")
        if phase not in EXECUTION_PHASES | {CACHE_HIT_PHASE}:
            raise EvidenceError(f"record {index}: unexpected phase {phase!r}")
        phases.add(phase)
        expected_key = (test_name, phase)
        expected = EXPECTED_RECORDS.get(expected_key)
        if expected is None:
            raise EvidenceError(
                f"record {index}: unexpected Plan 34 smoke record "
                f"{test_name!r} phase {phase!r}"
            )
        if expected_key in seen_records:
            raise EvidenceError(
                f"record {index}: duplicate Plan 34 smoke record "
                f"{test_name!r} phase {phase!r}"
            )
        seen_records.add(expected_key)

        if record.get("remote_service") != "local_nativelink":
            raise EvidenceError(f"record {index}: remote_service must be local_nativelink")
        if record.get("executor_boundary") != "reapi":
            raise EvidenceError(f"record {index}: executor_boundary must be reapi")

        direct_local_actions = _as_int(record, "direct_local_actions", index)
        if direct_local_actions != 0:
            raise EvidenceError(
                f"record {index}: direct_local_actions must be 0, got {direct_local_actions}"
            )

        for key in totals:
            if key == "records":
                continue
            totals[key] += _as_int(record, key, index)

        if phase in EXECUTION_PHASES:
            if _as_int(record, "reapi_actions", index) <= 0:
                raise EvidenceError(
                    f"record {index}: remote execution must include REAPI actions"
                )
            if _as_int(record, "upload_records", index) <= 0:
                raise EvidenceError(
                    f"record {index}: remote execution must include upload records"
                )
        else:
            if _as_int(record, "cache_query_actions", index) <= 0:
                raise EvidenceError(
                    f"record {index}: AC hit evidence must include cache queries"
                )
            if _as_int(record, "cache_hit_actions", index) <= 0:
                raise EvidenceError(
                    f"record {index}: AC hit evidence must include cache hits"
                )
        for key, expected_value in expected.items():
            actual = _as_int(record, key, index)
            if actual != expected_value:
                raise EvidenceError(
                    f"record {index}: {test_name} {phase} expected "
                    f"{key}={expected_value}, got {actual}"
                )

    missing_tests = sorted(REQUIRED_TESTS - tests)
    if missing_tests:
        raise EvidenceError(
            "Plan 34 evidence is missing required smoke records: "
            + ", ".join(missing_tests)
        )
    if not phases.intersection(EXECUTION_PHASES) or CACHE_HIT_PHASE not in phases:
        raise EvidenceError(
            "Plan 34 evidence must include remote execution and remote action cache hit phases"
        )
    missing_records = sorted(set(EXPECTED_RECORDS) - seen_records)
    if missing_records:
        raise EvidenceError(
            "Plan 34 evidence is missing required smoke phases: "
            + ", ".join(f"{test}:{phase}" for test, phase in missing_records)
        )
    if totals["direct_local_actions"] != 0:
        raise EvidenceError("Plan 34 evidence contains direct-local actions")
    if totals["reapi_actions"] <= 0:
        raise EvidenceError("Plan 34 evidence contains no REAPI actions")
    if totals["upload_records"] < totals["reapi_actions"]:
        raise EvidenceError("Plan 34 evidence has fewer upload records than REAPI actions")
    if totals["cache_query_actions"] <= 0 or totals["cache_hit_actions"] <= 0:
        raise EvidenceError("Plan 34 evidence contains no remote AC hit proof")
    if totals["materialized_outputs"] < len(REQUIRED_TESTS):
        raise EvidenceError("Plan 34 evidence is missing materialized output proof")

    return totals


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(
            "usage: validate_reapi_evidence.py PATH_TO_PLAN34_JSONL",
            file=sys.stderr,
        )
        return 2

    path = Path(argv[1])
    try:
        totals = validate_evidence(load_evidence(path))
    except EvidenceError as error:
        print(f"Plan 34 REAPI evidence invalid: {error}", file=sys.stderr)
        return 1

    print(
        "Plan 34 REAPI evidence OK: "
        f"records={totals['records']} "
        f"reapi_actions={totals['reapi_actions']} "
        f"upload_records={totals['upload_records']} "
        f"cache_query_actions={totals['cache_query_actions']} "
        f"cache_hit_actions={totals['cache_hit_actions']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
