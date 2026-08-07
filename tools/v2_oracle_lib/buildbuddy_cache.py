"""Fail-closed, secret-safe BuildBuddy cache evidence collection."""
from __future__ import annotations

import hashlib
import json
import os
import platform
import re
import secrets
import shutil
import stat
import subprocess
import tempfile
from pathlib import Path
from typing import Any, Callable, Iterable

REPO_ROOT = Path(__file__).resolve().parents[2]
MANIFEST_SHA256 = "3a717cb4b0a1f5cab06d336e69d2382861a9c21af9a1502ea20c54b990adf6d5"
BAZELRC_SHA256 = "e72f4223b6cfffbc96de018849e306ff9cbfdf4ca50248d8fee229a80dc4c805"
VERSION = "slug-buildbuddy-targets-v1"
FAILURES = {"CONFIG_DRIFT", "REMOTE_UNAVAILABLE", "COMMAND_LINE_FAILURE", "TARGET_FAILURE", "CACHE_MISS_OR_MIXED_REPLAY", "EVIDENCE_INCOMPLETE", "SANITIZER_REJECTED"}
COMMAND_FAILURE_CLASSES = {
    "command": {"OPTIONS_PARSE_FAILURE": "COMMAND_OPTIONS_PARSE", "STARLARK_OPTIONS_PARSE_FAILURE": "COMMAND_STARLARK_OPTIONS_PARSE", "ARGUMENTS_NOT_RECOGNIZED": "COMMAND_ARGUMENTS_NOT_RECOGNIZED", "INVOCATION_POLICY_PARSE_FAILURE": "COMMAND_INVOCATION_POLICY", "INVOCATION_POLICY_INVALID": "COMMAND_INVOCATION_POLICY"},
    "remoteOptions": {"REMOTE_DEFAULT_EXEC_PROPERTIES_LOGIC_ERROR": "REMOTE_OPTIONS_CONFIGURATION", "DOWNLOADER_WITHOUT_GRPC_CACHE": "REMOTE_OPTIONS_CONFIGURATION", "EXECUTION_WITH_INVALID_CACHE": "REMOTE_OPTIONS_CONFIGURATION"},
    "remoteExecution": {"CREDENTIALS_INIT_FAILURE": "REMOTE_EXECUTION_CONFIGURATION", "CACHE_INIT_FAILURE": "REMOTE_EXECUTION_CONFIGURATION", "RPC_LOG_FAILURE": "REMOTE_EXECUTION_CONFIGURATION", "EXEC_CHANNEL_INIT_FAILURE": "REMOTE_EXECUTION_CONFIGURATION", "CACHE_CHANNEL_INIT_FAILURE": "REMOTE_EXECUTION_CONFIGURATION", "DOWNLOADER_CHANNEL_INIT_FAILURE": "REMOTE_EXECUTION_CONFIGURATION", "REMOTE_DOWNLOAD_OUTPUTS_MINIMAL_WITHOUT_INMEMORY_DOTD": "REMOTE_EXECUTION_CONFIGURATION", "REMOTE_DOWNLOAD_OUTPUTS_MINIMAL_WITHOUT_INMEMORY_JDEPS": "REMOTE_EXECUTION_CONFIGURATION"},
    "executionOptions": {"INVALID_STRATEGY": "EXECUTION_OPTIONS_CONFIGURATION", "RESTRICTION_UNMATCHED_TO_ACTION_CONTEXT": "EXECUTION_OPTIONS_CONFIGURATION", "REMOTE_FALLBACK_STRATEGY_NOT_ABSTRACT_SPAWN": "EXECUTION_OPTIONS_CONFIGURATION", "STRATEGY_NOT_FOUND": "EXECUTION_OPTIONS_CONFIGURATION", "DYNAMIC_STRATEGY_NOT_SANDBOXED": "EXECUTION_OPTIONS_CONFIGURATION", "MULTIPLE_EXECUTION_LOG_FORMATS": "EXECUTION_OPTIONS_CONFIGURATION"},
    "execution": {"EXECUTION_LOG_INITIALIZATION_FAILURE": "EXECUTION_LOG_CONFIGURATION"},
    "buildConfiguration": {"PLATFORM_MAPPING_EVALUATION_FAILURE": "BUILD_CONFIGURATION", "INVALID_CONFIGURATION": "BUILD_CONFIGURATION", "INVALID_BUILD_OPTIONS": "BUILD_CONFIGURATION", "MULTI_CPU_PREREQ_UNMET": "BUILD_CONFIGURATION", "HEURISTIC_INSTRUMENTATION_FILTER_INVALID": "BUILD_CONFIGURATION", "CYCLE": "BUILD_CONFIGURATION", "CONFLICTING_CONFIGURATIONS": "BUILD_CONFIGURATION", "INVALID_OUTPUT_DIRECTORY_MNEMONIC": "BUILD_CONFIGURATION", "CONFIGURATION_DISCARDED_ANALYSIS_CACHE": "BUILD_CONFIGURATION", "INVALID_PROJECT": "BUILD_CONFIGURATION"},
}


class GateError(Exception):
    def __init__(self, classification: str):
        self.classification = classification if classification in FAILURES else "SANITIZER_REJECTED"


def load_manifest(path: Path) -> tuple[str, tuple[str, ...]]:
    data = path.read_bytes()
    if hashlib.sha256(data).hexdigest() != MANIFEST_SHA256:
        raise GateError("CONFIG_DRIFT")
    lines = data.decode("ascii").splitlines()
    if not data.endswith(b"\n") or len(lines) != 45 or lines[0] != VERSION:
        raise GateError("CONFIG_DRIFT")
    kinds = [line.split("\t", 1) for line in lines[1:]]
    if any(len(item) != 2 or not item[1].startswith("//") for item in kinds):
        raise GateError("CONFIG_DRIFT")
    builds = [label for kind, label in kinds if kind == "build"]
    tests = [label for kind, label in kinds if kind == "test"]
    if builds != ["//app/slug_cli_v2:slug"] or len(tests) != 43 or tests != sorted(tests):
        raise GateError("CONFIG_DRIFT")
    return builds[0], tuple(tests)


def json_sequence(data: bytes) -> Iterable[dict[str, Any]]:
    try:
        text = data.decode("utf-8")
        decoder = json.JSONDecoder()
        position = 0
        while True:
            while position < len(text) and text[position].isspace():
                position += 1
            if position == len(text):
                return
            value, position = decoder.raw_decode(text, position)
            if not isinstance(value, dict):
                raise ValueError
            yield value
    except (UnicodeDecodeError, json.JSONDecodeError, ValueError):
        raise GateError("EVIDENCE_INCOMPLETE") from None


def _field(item: dict[str, Any], *names: str, default: Any = None) -> Any:
    for name in names:
        if name in item:
            return item[name]
    return default


def _runner(value: Any) -> str:
    return {"remote cache hit": "remote_cache_hit", "local": "local", "worker": "local", "linux-sandbox": "local", "disk cache hit": "disk_cache_hit", "remote": "remote_execution"}.get(value, "unknown")


def _boolean(item: dict[str, Any], *names: str) -> bool:
    value = _field(item, *names)
    if value is None:
        return False
    if not isinstance(value, bool):
        raise GateError("EVIDENCE_INCOMPLETE")
    return value


def _count(value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise GateError("EVIDENCE_INCOMPLETE")
    return value


def _digest(value: Any) -> str:
    if not isinstance(value, dict) or not isinstance(value.get("hash"), str) or not re.fullmatch(r"[0-9a-f]{64}", value["hash"]):
        raise GateError("EVIDENCE_INCOMPLETE")
    size = value.get("sizeBytes")
    if isinstance(size, bool):
        raise GateError("EVIDENCE_INCOMPLETE")
    if isinstance(size, str):
        if not re.fullmatch(r"0|[1-9][0-9]*", size):
            raise GateError("EVIDENCE_INCOMPLETE")
        size = int(size)
    if not isinstance(size, int) or not 0 <= size <= 9223372036854775807:
        raise GateError("EVIDENCE_INCOMPLETE")
    return json.dumps({"hash": value["hash"], "sizeBytes": size}, sort_keys=True, separators=(",", ":"))


def _command_failure_class(value: Any) -> str:
    if not isinstance(value, dict) or set(value) - {"message"} == set() or not isinstance(value.get("message", ""), str):
        return "UNKNOWN_COMMAND_LINE_ERROR"
    categories = set(value) - {"message"}
    if len(categories) != 1:
        return "UNKNOWN_COMMAND_LINE_ERROR"
    category = categories.pop()
    detail = value.get(category)
    if not isinstance(detail, dict) or set(detail) != {"code"} or not isinstance(detail.get("code"), str):
        return "UNKNOWN_COMMAND_LINE_ERROR"
    return COMMAND_FAILURE_CLASSES.get(category, {}).get(detail["code"], "UNKNOWN_COMMAND_LINE_ERROR")


def spawn_summary(entries: Iterable[dict[str, Any]], phase: str) -> dict[str, Any]:
    buckets = {name: 0 for name in ("local", "remote_cache_hit", "disk_cache_hit", "remote_execution", "unknown")}
    digests: list[str] = []
    failures = {"cache_hit": 0, "status": 0, "exit": 0}
    eligible = 0
    for item in entries:
        event = item.get("spawn", item.get("SpawnExec", item))
        if not isinstance(event, dict):
            raise GateError("EVIDENCE_INCOMPLETE")
        remotely_cacheable = _boolean(event, "remote_cacheable", "remoteCacheable")
        cacheable = _boolean(event, "cacheable")
        if not (cacheable and remotely_cacheable):
            continue
        kind = _runner(_field(event, "runner"))
        buckets[kind] += 1
        digest = _digest(_field(event, "action_digest", "actionDigest", "digest"))
        hit = _field(event, "cache_hit", "cacheHit")
        status = _field(event, "status", default="")
        exit_code = _field(event, "exit_code", "exitCode", default=0)
        eligible += 1
        digests.append(digest)
        failures["cache_hit"] += int(not isinstance(hit, bool) or hit != (phase == "replay"))
        failures["status"] += int(status not in ("", None))
        failures["exit"] += int(_count(exit_code) != 0)
        if phase == "prime":
            failures["status"] += int(kind != "local")
        else:
            failures["status"] += int(kind != "remote_cache_hit")
    return {"count": eligible, "digest_multiset_sha256": hashlib.sha256("\n".join(sorted(digests)).encode()).hexdigest(), "cache_hit_failures": failures["cache_hit"], "status_failures": failures["status"], "exit_failures": failures["exit"], **buckets}


def phase_record(bep: bytes, execution: bytes, phase: str, tests: tuple[str, ...], process_exit: int) -> dict[str, Any]:
    build_finished: dict[str, Any] | None = None
    completed: set[str] = set()
    passed: set[str] = set()
    runs: dict[str, int] = {}
    cached: dict[str, int] = {}
    persistent_hits = 0
    aborted_remote = False
    for event in json_sequence(bep):
        ident = event.get("id", {})
        if "buildFinished" in ident:
            build_finished = event.get("finished")
        label = next((v.get("label") for v in ident.values() if isinstance(v, dict) and isinstance(v.get("label"), str)), None)
        if "targetCompleted" in ident and label:
            if event.get("completed", {}).get("success") is True:
                completed.add(label)
        if "testSummary" in ident and label:
            summary = event.get("testSummary", {})
            if summary.get("overallStatus") == "PASSED":
                passed.add(label)
            runs[label] = _count(summary.get("totalRunCount", 0))
            cached[label] = _count(summary.get("totalNumCached", 0))
        metrics = event.get("buildMetrics", {}).get("actionSummary", {}).get("actionCacheStatistics", {})
        if isinstance(metrics, dict):
            persistent_hits += _count(metrics.get("hits", 0))
        aborted = event.get("aborted")
        if isinstance(aborted, dict):
            aborted_remote |= aborted.get("reason") == "REMOTE_ENVIRONMENT_FAILURE"
    if not isinstance(build_finished, dict):
        raise GateError("EVIDENCE_INCOMPLETE")
    exit_data = build_finished.get("exitCode", {})
    if not isinstance(exit_data, dict):
        raise GateError("EVIDENCE_INCOMPLETE")
    summary = spawn_summary(json_sequence(execution), phase)
    name = exit_data.get("name")
    safe_name = name if name in {"SUCCESS", "REMOTE_ERROR", "REMOTE_ENVIRONMENTAL_ERROR", "COMMAND_LINE_ERROR"} else "OTHER"
    if aborted_remote:
        safe_name = "REMOTE_ERROR"
    code = exit_data.get("code")
    code = _count(code)
    completed_tests = completed & set(tests)
    if any(cached.get(test, 0) > runs.get(test, 0) for test in tests):
        raise GateError("EVIDENCE_INCOMPLETE")
    passed_once = {test for test in passed & completed_tests if runs.get(test, 0) == 1}
    command_failure_class = _command_failure_class(build_finished.get("failureDetail")) if safe_name == "COMMAND_LINE_ERROR" else "NONE"
    return {"process_exit_code": process_exit, "build_finished": {"name": safe_name, "code": code}, "command_failure_class": command_failure_class, "build_success_count": int("//app/slug_cli_v2:slug" in completed), "passed_test_count": len(passed_once), "test_run_count": sum(runs.get(test, 0) == 1 for test in tests), "remotely_cached_test_count": sum(cached.get(test, 0) == 1 for test in tests), "persistent_action_cache_hit_count": persistent_hits, "eligible_spawns": summary}


def classify(prime: dict[str, Any], replay: dict[str, Any], tests: tuple[str, ...]) -> str:
    required = len(tests)
    if any(record["build_finished"]["name"] in {"REMOTE_ERROR", "REMOTE_ENVIRONMENTAL_ERROR"} for record in (prime, replay)):
        return "REMOTE_UNAVAILABLE"
    if any(record["process_exit_code"] == 2 and record["build_finished"] == {"name": "COMMAND_LINE_ERROR", "code": 2} and record["command_failure_class"] != "NONE" for record in (prime, replay)):
        return "COMMAND_LINE_FAILURE"
    if any(record["process_exit_code"] != 0 or record["build_finished"] != {"name": "SUCCESS", "code": 0} or record["build_success_count"] != 1 or record["passed_test_count"] != required or record["test_run_count"] != required for record in (prime, replay)):
        return "TARGET_FAILURE"
    if any(record["persistent_action_cache_hit_count"] for record in (prime, replay)) or prime["remotely_cached_test_count"]:
        return "CACHE_MISS_OR_MIXED_REPLAY"
    if replay["remotely_cached_test_count"] != required:
        return "CACHE_MISS_OR_MIXED_REPLAY"
    a, b = prime["eligible_spawns"], replay["eligible_spawns"]
    if not a["count"] or not b["count"]:
        return "EVIDENCE_INCOMPLETE"
    if a["count"] != b["count"] or a["digest_multiset_sha256"] != b["digest_multiset_sha256"]:
        return "CACHE_MISS_OR_MIXED_REPLAY"
    if any(a[key] for key in ("cache_hit_failures", "status_failures", "exit_failures")) or any(b[key] for key in ("cache_hit_failures", "status_failures", "exit_failures", "local", "disk_cache_hit", "remote_execution", "unknown")):
        return "CACHE_MISS_OR_MIXED_REPLAY"
    return "PROVED_CACHE_ONLY"


def command(phase: str, bazel: str, output_base: Path, bep: Path, execution: Path, nonce: str, labels: tuple[str, ...]) -> list[str]:
    common = [bazel, f"--output_base={output_base}", "test", "--config=buildbuddy-cache", "--remote_cache=grpcs://remote.buildbuddy.io", "--remote_instance_name=", "--remote_executor=", "--bes_backend=", "--bes_results_url=", "--disk_cache=", "--spawn_strategy=worker,sandboxed,local", "--test_strategy=local", "--cache_test_results=yes", "--runs_per_test=1", "--test_sharding_strategy=disabled", "--noremote_local_fallback", "--build_event_publish_all_actions", f"--build_event_json_file={bep}", f"--execution_log_json_file={execution}", f"--action_env=SLUG_BUILDBUDDY_CACHE_GATE_NONCE={nonce}", f"--test_env=SLUG_BUILDBUDDY_CACHE_GATE_NONCE={nonce}"]
    extra = ["--noremote_accept_cached", "--remote_upload_local_results", "--noremote_cache_async"] if phase == "prime" else ["--remote_accept_cached", "--noremote_upload_local_results", "--noremote_cache_async"]
    return common + extra + list(labels)


def _preflight(bazel: str, runner: Callable[..., subprocess.CompletedProcess[bytes]]) -> tuple[str, bool]:
    version = runner([bazel, "--ignore_all_rc_files", "version"], cwd=REPO_ROOT, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False)
    try:
        labels = [line for line in version.stdout.decode("utf-8").splitlines() if line.startswith("Build label:")]
    except (AttributeError, UnicodeDecodeError):
        raise GateError("CONFIG_DRIFT") from None
    if version.returncode != 0 or labels != ["Build label: 9.2.0"] or platform.system() != "Linux" or platform.machine() not in {"x86_64", "AMD64"}:
        raise GateError("CONFIG_DRIFT")
    head, clean = _git("rev-parse", "HEAD"), _git("status", "--porcelain") == ""
    if not re.fullmatch(r"[0-9a-f]{40}", head) or not clean:
        raise GateError("CONFIG_DRIFT")
    return head, clean


def run_gate(manifest: Path, bazel: str = "bazel", runner: Callable[..., subprocess.CompletedProcess[bytes]] = subprocess.run) -> dict[str, Any]:
    build, tests = load_manifest(manifest)
    rc_hash = hashlib.sha256((REPO_ROOT / ".bazelrc").read_bytes()).hexdigest()
    if rc_hash != BAZELRC_SHA256:
        raise GateError("CONFIG_DRIFT")
    old_umask = os.umask(0o077)
    root: Path | None = None
    try:
        head, clean = _preflight(bazel, runner)
        root = Path(tempfile.mkdtemp(prefix="slug-buildbuddy-cache-"))
        if stat.S_IMODE(root.stat().st_mode) != 0o700:
            raise GateError("SANITIZER_REJECTED")
        try:
            root.resolve().relative_to(REPO_ROOT.resolve())
        except ValueError:
            pass
        else:
            raise GateError("SANITIZER_REJECTED")
        nonce = secrets.token_hex(32)
        records: dict[str, dict[str, Any]] = {}
        for phase in ("prime", "replay"):
            phase_root = root / phase
            phase_root.mkdir()
            bep, execution = phase_root / "bep.json", phase_root / "execution.json"
            stdout, stderr = phase_root / "stdout", phase_root / "stderr"
            with stdout.open("xb") as stdout_file, stderr.open("xb") as stderr_file:
                result = runner(command(phase, bazel, phase_root / "output", bep, execution, nonce, (build,) + tests), cwd=REPO_ROOT, stdout=stdout_file, stderr=stderr_file, check=False)
            records[phase] = phase_record(bep.read_bytes(), execution.read_bytes(), phase, tests, result.returncode)
        classification = classify(records["prime"], records["replay"], tests)
        return {"schema_version": 1, "classification": classification, "mode": "buildbuddy-cache-only", "bazel_version": "9.2.0", "host_platform": "linux-x86_64", "git_head": head, "git_clean": clean, "manifest_sha256": MANIFEST_SHA256, "bazelrc_sha256": BAZELRC_SHA256, "target_counts": {"build": 1, "test": len(tests)}, "prime": records["prime"], "replay": records["replay"]}
    except GateError:
        raise
    except Exception:
        raise GateError("EVIDENCE_INCOMPLETE") from None
    finally:
        os.umask(old_umask)
        cleanup_failed = False
        if root is not None:
            for phase in ("prime", "replay"):
                try:
                    shutdown = runner([bazel, "--ignore_all_rc_files", f"--output_base={root / phase / 'output'}", "shutdown"], cwd=REPO_ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
                    cleanup_failed |= shutdown.returncode != 0
                except Exception:
                    cleanup_failed = True
            try:
                shutil.rmtree(root)
            except Exception:
                cleanup_failed = True
        if cleanup_failed:
            raise GateError("SANITIZER_REJECTED")


def _git(*args: str) -> str:
    result = subprocess.run(["git", *args], cwd=REPO_ROOT, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False)
    return result.stdout.decode("ascii", "replace").strip()
