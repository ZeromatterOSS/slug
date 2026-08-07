"""Closed, manifest-aware BuildBuddy cache prime/replay evidence."""
from __future__ import annotations

import hashlib
import json
import os
import platform
import re
import secrets
import stat
import subprocess
import tempfile
from pathlib import Path
from typing import Any, Callable, Iterable

REPO_ROOT = Path(__file__).resolve().parents[2]
MANIFEST_SHA256 = "3a717cb4b0a1f5cab06d336e69d2382861a9c21af9a1502ea20c54b990adf6d5"
BAZELRC_SHA256 = "e72f4223b6cfffbc96de018849e306ff9cbfdf4ca50248d8fee229a80dc4c805"
VERSION = "slug-buildbuddy-targets-v1"
MODE = "buildbuddy-cache-only"
BAZEL_VERSION = "9.2.0"
HOST_PLATFORM = "linux-x86_64"
BUILD_LABEL = "//app/slug_cli_v2:slug"
CLASSES = frozenset(("PROVED_CACHE_ONLY", "CONFIG_DRIFT", "REMOTE_UNAVAILABLE", "COMMAND_LINE_FAILURE", "TARGET_FAILURE", "CACHE_MISS_OR_MIXED_REPLAY", "EVIDENCE_INCOMPLETE", "SANITIZER_REJECTED"))
FAILURES = CLASSES - {"PROVED_CACHE_ONLY"}
SPAWN_KEYS = frozenset(("count", "digest_multiset_sha256", "cache_error_count", "status_error_count", "exit_error_count", "local", "worker", "linux_sandbox", "remote_cache_hit", "other"))
PHASE_KEYS = frozenset(("process_success_count", "build_finished_success_count", "build_success_count", "output_count", "test_completion_count", "passed_test_count", "test_run_count", "remotely_cached_test_count", "persistent_action_cache_hit_count", "eligible_spawns"))
MAX_BEP_BYTES = 128 << 20
MAX_EXECUTION_BYTES = 512 << 20


class GateError(Exception):
    def __init__(self, classification: str = "SANITIZER_REJECTED"):
        self.classification = classification if classification in CLASSES else "SANITIZER_REJECTED"


def _manifest_bytes() -> bytes:
    parent_fd = root_fd = tests_fd = oracle_fd = file_fd = None
    try:
        parent_fd = os.open(REPO_ROOT.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        root_fd = os.open(REPO_ROOT.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd)
        tests_fd = os.open("tests", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=root_fd)
        oracle_fd = os.open("v2_oracle", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=tests_fd)
        file_fd = os.open("buildbuddy_cache_targets.txt", os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW, dir_fd=oracle_fd)
        parent = os.fstat(parent_fd); parent_id = parent.st_dev, parent.st_ino
        opened = (os.fstat(root_fd), os.fstat(tests_fd), os.fstat(oracle_fd), os.fstat(file_fd))
        identities = tuple((item.st_dev, item.st_ino) for item in opened)
        current = (os.stat(REPO_ROOT.name, dir_fd=parent_fd, follow_symlinks=False), os.stat("tests", dir_fd=root_fd, follow_symlinks=False), os.stat("v2_oracle", dir_fd=tests_fd, follow_symlinks=False), os.stat("buildbuddy_cache_targets.txt", dir_fd=oracle_fd, follow_symlinks=False))
        current_parent = os.stat(REPO_ROOT.parent, follow_symlinks=False)
        if not stat.S_ISDIR(parent.st_mode) or (current_parent.st_dev, current_parent.st_ino) != parent_id or any(not stat.S_ISDIR(item.st_mode) for item in opened[:3]) or not stat.S_ISREG(opened[3].st_mode) or opened[3].st_nlink != 1 or opened[3].st_size > 16 << 10 or tuple((item.st_dev, item.st_ino) for item in current) != identities: raise OSError
        chunks: list[bytes] = []; size = 0
        while chunk := os.read(file_fd, min(4096, (16 << 10) + 1 - size)):
            chunks.append(chunk); size += len(chunk)
            if size > 16 << 10: raise OSError
        after = os.fstat(file_fd)
        current = (os.stat(REPO_ROOT.name, dir_fd=parent_fd, follow_symlinks=False), os.stat("tests", dir_fd=root_fd, follow_symlinks=False), os.stat("v2_oracle", dir_fd=tests_fd, follow_symlinks=False), os.stat("buildbuddy_cache_targets.txt", dir_fd=oracle_fd, follow_symlinks=False))
        current_parent = os.stat(REPO_ROOT.parent, follow_symlinks=False)
        if (current_parent.st_dev, current_parent.st_ino) != parent_id or (after.st_dev, after.st_ino) != identities[3] or after.st_size != size or tuple((item.st_dev, item.st_ino) for item in current) != identities: raise OSError
        return b"".join(chunks)
    except OSError:
        raise GateError("CONFIG_DRIFT") from None
    finally:
        for fd in (file_fd, oracle_fd, tests_fd, root_fd, parent_fd):
            if fd is not None: os.close(fd)


def load_manifest() -> tuple[str, tuple[str, ...]]:
    data = _manifest_bytes()
    if hashlib.sha256(data).hexdigest() != MANIFEST_SHA256: raise GateError("CONFIG_DRIFT")
    try: lines = data.decode("ascii").splitlines()
    except UnicodeDecodeError: raise GateError("CONFIG_DRIFT") from None
    if not data.endswith(b"\n") or len(lines) != 45 or lines[0] != VERSION: raise GateError("CONFIG_DRIFT")
    kinds = [line.split("\t", 1) for line in lines[1:]]
    if any(len(item) != 2 or not item[1].startswith("//") for item in kinds): raise GateError("CONFIG_DRIFT")
    builds = [label for kind, label in kinds if kind == "build"]
    tests = [label for kind, label in kinds if kind == "test"]
    if builds != [BUILD_LABEL] or len(tests) != 43 or tests != sorted(tests): raise GateError("CONFIG_DRIFT")
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


def command(phase: str, bazel: str, output_base: Path, bep: Path, execution: Path, nonce: str, labels: tuple[str, ...]) -> list[str]:
    common = [bazel, f"--output_base={output_base}", "test", "--config=buildbuddy-cache", "--@rules_rust//rust/toolchain/channel=nightly", "--remote_cache=grpcs://remote.buildbuddy.io", "--remote_instance_name=", "--remote_executor=", "--bes_backend=", "--bes_results_url=", "--disk_cache=", "--spawn_strategy=worker,sandboxed,local", "--test_strategy=local", "--cache_test_results=yes", "--runs_per_test=1", "--test_sharding_strategy=disabled", "--noremote_local_fallback", "--build_event_publish_all_actions", f"--build_event_json_file={bep}", f"--execution_log_json_file={execution}", f"--action_env=SLUG_BUILDBUDDY_CACHE_GATE_NONCE={nonce}", f"--test_env=SLUG_BUILDBUDDY_CACHE_GATE_NONCE={nonce}"]
    extra = ["--noremote_accept_cached", "--remote_upload_local_results", "--noremote_cache_async"] if phase == "prime" else ["--remote_accept_cached", "--noremote_upload_local_results", "--noremote_cache_async"]
    return common + extra + list(labels)


def full_command(phase: str, bazel: str, output: Path, bep: Path, execution: Path, nonce: str, labels: tuple[str, ...]) -> list[str]:
    if type(phase) is not str or phase not in ("prime", "replay") or not isinstance(nonce, str) or not re.fullmatch(r"[0-9a-f]{64}", nonce): raise GateError("SANITIZER_REJECTED")
    if type(labels) is not tuple or len(labels) != 44 or labels[0] != BUILD_LABEL or tuple(sorted(labels[1:])) != labels[1:] or len(set(labels)) != 44: raise GateError("CONFIG_DRIFT")
    return [bazel, f"--output_base={output}", "test", "--config=buildbuddy-cache", "--noremote_accept_cached" if phase == "prime" else "--remote_accept_cached", "--@rules_rust//rust/toolchain/channel=nightly", "--remote_executor=", "--bes_backend=", "--bes_results_url=", "--disk_cache=", "--noremote_local_fallback", "--cache_test_results=yes", "--runs_per_test=1", "--test_sharding_strategy=disabled", f"--action_env=SLUG_BUILDBUDDY_CACHE_GATE_NONCE={nonce}", f"--test_env=SLUG_BUILDBUDDY_CACHE_GATE_NONCE={nonce}", f"--build_event_json_file={bep}", f"--execution_log_json_file={execution}", *labels]


def spawn_summary(entries: Iterable[dict[str, Any]], phase: str) -> dict[str, Any]:
    if type(phase) is not str or phase not in ("prime", "replay"): raise GateError()
    runners = {key: 0 for key in ("local", "worker", "linux_sandbox", "remote_cache_hit", "other")}
    digests: list[str] = []; errors = {key: 0 for key in ("cache", "status", "exit")}
    for item in entries:
        event = item.get("spawn", item.get("SpawnExec", item))
        if not isinstance(event, dict): raise GateError("EVIDENCE_INCOMPLETE")
        if not (_boolean(event, "cacheable") and _boolean(event, "remote_cacheable", "remoteCacheable")): continue
        runners[{"local": "local", "worker": "worker", "linux-sandbox": "linux_sandbox", "remote cache hit": "remote_cache_hit"}.get(_field(event, "runner"), "other")] += 1
        digests.append(_digest(_field(event, "action_digest", "actionDigest", "digest")))
        hit = _field(event, "cache_hit", "cacheHit")
        errors["cache"] += int(not isinstance(hit, bool) or hit != (phase == "replay"))
        errors["status"] += int(_field(event, "status", default="") not in ("", None))
        errors["exit"] += int(_count(_field(event, "exit_code", "exitCode", default=0)) != 0)
    return {"count": len(digests), "digest_multiset_sha256": hashlib.sha256("\n".join(sorted(digests)).encode()).hexdigest(), "cache_error_count": errors["cache"], "status_error_count": errors["status"], "exit_error_count": errors["exit"], **runners}


def phase_record(bep: bytes, execution: bytes, phase: str, tests: tuple[str, ...], process_exit: int) -> dict[str, Any]:
    if type(tests) is not tuple or len(tests) != 43 or tests != tuple(sorted(tests)): raise GateError("CONFIG_DRIFT")
    finished: list[Any] = []; completions: dict[str, list[bool]] = {BUILD_LABEL: []}; summaries: dict[str, list[dict[str, Any]]] = {test: [] for test in tests}
    completions.update({test: [] for test in tests}); persistent_hits = 0; remote_failure = False
    for event in json_sequence(bep):
        ident = event.get("id")
        if not isinstance(ident, dict): raise GateError("EVIDENCE_INCOMPLETE")
        if "buildFinished" in ident: finished.append(event.get("finished"))
        target = ident.get("targetCompleted")
        if "targetCompleted" in ident:
            if not isinstance(target, dict) or not isinstance(target.get("label"), str) or target["label"] not in completions: raise GateError("EVIDENCE_INCOMPLETE")
            completions[target["label"]].append(event.get("completed", {}).get("success") is True)
        summary_id = ident.get("testSummary")
        if "testSummary" in ident:
            if not isinstance(summary_id, dict) or not isinstance(summary_id.get("label"), str) or summary_id["label"] not in summaries or not isinstance(event.get("testSummary"), dict): raise GateError("EVIDENCE_INCOMPLETE")
            summaries[summary_id["label"]].append(event["testSummary"])
        metrics = event.get("buildMetrics", {}).get("actionSummary", {}).get("actionCacheStatistics", {})
        if isinstance(metrics, dict): persistent_hits += _count(metrics.get("hits", 0))
        remote_failure |= isinstance(event.get("aborted"), dict) and event["aborted"].get("reason") == "REMOTE_ENVIRONMENT_FAILURE"
    if len(finished) != 1 or not isinstance(finished[0], dict) or any(len(items) != 1 for items in completions.values()) or any(len(items) != 1 for items in summaries.values()): raise GateError("EVIDENCE_INCOMPLETE")
    exit_data = finished[0].get("exitCode")
    if not isinstance(exit_data, dict): raise GateError("EVIDENCE_INCOMPLETE")
    name, code = exit_data.get("name"), _count(exit_data.get("code", 0))
    success = name == "SUCCESS" and code == 0
    outcome = "remote" if remote_failure or name in {"REMOTE_ERROR", "REMOTE_ENVIRONMENTAL_ERROR"} else "command" if name == "COMMAND_LINE_ERROR" and code == 2 else "success" if success else "target"
    runs, cached, passed = {}, {}, set()
    for test, items in summaries.items():
        item = items[0]; runs[test] = _count(item.get("totalRunCount")); cached[test] = _count(item.get("totalNumCached", 0 if phase == "prime" else None))
        if cached[test] > runs[test]: raise GateError("EVIDENCE_INCOMPLETE")
        if item.get("overallStatus") == "PASSED": passed.add(test)
    return {"process_success_count": int(_count(process_exit) == 0), "build_finished_success_count": int(success), "build_success_count": int(completions[BUILD_LABEL] == [True]), "output_count": 0, "test_completion_count": sum(completions[test] == [True] for test in tests), "passed_test_count": len(passed), "test_run_count": sum(runs[test] == 1 for test in tests), "remotely_cached_test_count": sum(cached[test] == 1 for test in tests), "persistent_action_cache_hit_count": persistent_hits, "eligible_spawns": spawn_summary(json_sequence(execution), phase), "_outcome": outcome}


def classify(prime: dict[str, Any], replay: dict[str, Any]) -> str:
    records = (prime, replay)
    if any(item.get("_outcome") == "remote" for item in records): return "REMOTE_UNAVAILABLE"
    if any(item.get("_outcome") == "command" for item in records): return "COMMAND_LINE_FAILURE"
    required = {"process_success_count": 1, "build_finished_success_count": 1, "build_success_count": 1, "output_count": 1, "test_completion_count": 43, "passed_test_count": 43, "test_run_count": 43}
    if any(item.get("_outcome") != "success" or any(item[key] != value for key, value in required.items()) for item in records): return "TARGET_FAILURE"
    if prime["remotely_cached_test_count"] != 0 or replay["remotely_cached_test_count"] != 43 or any(item["persistent_action_cache_hit_count"] for item in records): return "CACHE_MISS_OR_MIXED_REPLAY"
    a, b = prime["eligible_spawns"], replay["eligible_spawns"]
    if not a["count"] or not b["count"]: return "EVIDENCE_INCOMPLETE"
    if a["count"] != b["count"] or a["digest_multiset_sha256"] != b["digest_multiset_sha256"]: return "CACHE_MISS_OR_MIXED_REPLAY"
    if any(a[key] or b[key] for key in ("cache_error_count", "status_error_count", "exit_error_count")): return "CACHE_MISS_OR_MIXED_REPLAY"
    if any(a[key] for key in ("remote_cache_hit", "other")) or a["local"] + a["worker"] + a["linux_sandbox"] != a["count"]: return "CACHE_MISS_OR_MIXED_REPLAY"
    if b["remote_cache_hit"] != b["count"] or any(b[key] for key in ("local", "worker", "linux_sandbox", "other")): return "CACHE_MISS_OR_MIXED_REPLAY"
    return "PROVED_CACHE_ONLY"


def _empty_phase() -> dict[str, Any]:
    spawns = {key: 0 for key in SPAWN_KEYS}; spawns["digest_multiset_sha256"] = hashlib.sha256(b"").hexdigest()
    return {key: (spawns if key == "eligible_spawns" else 0) for key in PHASE_KEYS}


def record(classification: str = "SANITIZER_REJECTED", prime: dict[str, Any] | None = None, replay: dict[str, Any] | None = None, git_head: str = "0" * 40, git_clean: bool = False) -> dict[str, Any]:
    if type(classification) is not str or classification not in CLASSES: classification = "SANITIZER_REJECTED"
    def public(item: dict[str, Any] | None) -> dict[str, Any]:
        source = _empty_phase() if item is None else item
        return {key: source[key] for key in PHASE_KEYS}
    return {"schema_version": 1, "mode": MODE, "classification": classification, "bazel_version": BAZEL_VERSION, "host_platform": HOST_PLATFORM, "git_head": git_head, "git_clean": git_clean, "manifest_version": VERSION, "manifest_sha256": MANIFEST_SHA256, "bazelrc_sha256": BAZELRC_SHA256, "target_counts": {"build": 1, "test": 43}, "prime": public(prime), "replay": public(replay)}


def normalize(value: object) -> dict[str, Any]:
    try:
        if type(value) is not dict or set(value) != set(record()) or any(type(value[key]) is not str for key in ("mode", "classification", "bazel_version", "host_platform", "git_head", "manifest_version", "manifest_sha256", "bazelrc_sha256")): raise GateError()
        if type(value["schema_version"]) is not int or value["schema_version"] != 1 or type(value["git_clean"]) is not bool or type(value["target_counts"]) is not dict: raise GateError()
        fixed = (value["mode"], value["bazel_version"], value["host_platform"], value["manifest_version"], value["manifest_sha256"], value["bazelrc_sha256"], value["target_counts"])
        if fixed != (MODE, BAZEL_VERSION, HOST_PLATFORM, VERSION, MANIFEST_SHA256, BAZELRC_SHA256, {"build": 1, "test": 43}) or value["classification"] not in CLASSES or not re.fullmatch(r"[0-9a-f]{40}", value["git_head"]): raise GateError()
        def phase(source: object) -> dict[str, Any]:
            if type(source) is not dict or set(source) != PHASE_KEYS or type(source["eligible_spawns"]) is not dict or set(source["eligible_spawns"]) != SPAWN_KEYS: raise GateError()
            result = {key: _count(source[key]) for key in PHASE_KEYS - {"eligible_spawns"}}; raw = source["eligible_spawns"]
            digest = raw["digest_multiset_sha256"]
            if type(digest) is not str or not re.fullmatch(r"[0-9a-f]{64}", digest): raise GateError()
            spawns = {key: _count(raw[key]) for key in SPAWN_KEYS - {"digest_multiset_sha256"}}
            if sum(spawns[key] for key in ("local", "worker", "linux_sandbox", "remote_cache_hit", "other")) != spawns["count"] or any(spawns[key] > spawns["count"] for key in ("cache_error_count", "status_error_count", "exit_error_count")): raise GateError()
            spawns["digest_multiset_sha256"] = digest; result["eligible_spawns"] = {key: spawns[key] for key in SPAWN_KEYS}
            return {key: result[key] for key in PHASE_KEYS}
        prime, replay = phase(value["prime"]), phase(value["replay"])
        if value["classification"] == "PROVED_CACHE_ONLY":
            if not value["git_clean"] or value["git_head"] == "0" * 40 or classify({**prime, "_outcome": "success"}, {**replay, "_outcome": "success"}) != "PROVED_CACHE_ONLY": raise GateError()
        result = record(value["classification"], prime, replay, value["git_head"], value["git_clean"])
        return result if result == value else record()
    except Exception:
        return record()


def _hardened() -> Any:
    from tools.v2_oracle_lib import buildbuddy_build_cache
    return buildbuddy_build_cache


def _clean() -> bool: return _hardened()._clean()
def _anchored(*args: Any) -> bool: return _hardened()._anchored(*args)
def _outputs(path: Path) -> int: return _hardened()._outputs(path)
def _shutdown(bazel: str, output: Path, runner: Callable[..., subprocess.CompletedProcess[bytes]]) -> bool: return _hardened()._shutdown(bazel, output, runner)
def _remove_original(parent_fd: int, root_fd: int, identity: tuple[int, int]) -> bool: return _hardened()._remove_original(parent_fd, root_fd, identity)


def _remove_reserved(root: Path, parent_fd: int) -> bool:
    reserved_fd = None
    try:
        temp = Path(tempfile.gettempdir()).resolve()
        parent = os.fstat(parent_fd); expected = os.stat(temp, follow_symlinks=False)
        if not root.name.startswith("slug-buildbuddy-cache-") or root.parent.resolve() != temp or not stat.S_ISDIR(parent.st_mode) or (parent.st_dev, parent.st_ino) != (expected.st_dev, expected.st_ino): return False
        try: item = os.stat(root.name, dir_fd=parent_fd, follow_symlinks=False)
        except FileNotFoundError: return True
        if not stat.S_ISDIR(item.st_mode):
            os.unlink(root.name, dir_fd=parent_fd); return False
        reserved_fd = os.open(root.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd); opened = os.fstat(reserved_fd); identity = opened.st_dev, opened.st_ino
        if (item.st_dev, item.st_ino) != identity: return False
        return _remove_original(parent_fd, reserved_fd, identity)
    except Exception: return False
    finally:
        if reserved_fd is not None: os.close(reserved_fd)


def _root_bytes(name: str, limit: int) -> bytes:
    parent_fd = root_fd = file_fd = None
    try:
        parent_fd = os.open(REPO_ROOT.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW); root_fd = os.open(REPO_ROOT.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd)
        root = os.fstat(root_fd); root_id = root.st_dev, root.st_ino; current_root = os.stat(REPO_ROOT.name, dir_fd=parent_fd, follow_symlinks=False)
        if not stat.S_ISDIR(root.st_mode) or (current_root.st_dev, current_root.st_ino) != root_id: raise OSError
        file_fd = os.open(name, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW, dir_fd=root_fd); before = os.fstat(file_fd); identity = before.st_dev, before.st_ino
        current = os.stat(name, dir_fd=root_fd, follow_symlinks=False)
        if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1 or before.st_size > limit or (current.st_dev, current.st_ino) != identity: raise OSError
        chunks: list[bytes] = []; size = 0
        while chunk := os.read(file_fd, min(4096, limit + 1 - size)):
            chunks.append(chunk); size += len(chunk)
            if size > limit: raise OSError
        after = os.fstat(file_fd); current = os.stat(name, dir_fd=root_fd, follow_symlinks=False); current_root = os.stat(REPO_ROOT.name, dir_fd=parent_fd, follow_symlinks=False)
        if (after.st_dev, after.st_ino) != identity or after.st_size != size or (current.st_dev, current.st_ino) != identity or (current_root.st_dev, current_root.st_ino) != root_id: raise OSError
        return b"".join(chunks)
    finally:
        if file_fd is not None: os.close(file_fd)
        if root_fd is not None: os.close(root_fd)
        if parent_fd is not None: os.close(parent_fd)


def _preflight() -> str:
    try:
        okay = _clean() and platform.system() == "Linux" and platform.machine() in {"x86_64", "AMD64"}
        okay &= _root_bytes(".bazelversion", 64) == b"9.2.0\n" and hashlib.sha256(_root_bytes(".bazelrc", 1 << 20)).hexdigest() == BAZELRC_SHA256
        head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=REPO_ROOT, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False)
        text = head.stdout.decode("ascii").strip(); okay &= head.returncode == 0 and bool(re.fullmatch(r"[0-9a-f]{40}", text))
    except Exception: okay, text = False, ""
    if not okay: raise GateError("CONFIG_DRIFT")
    return text


def _artifact_bytes(directory_fd: int, name: str, limit: int, identity: tuple[int, int] | None = None) -> bytes:
    fd = None
    try:
        fd = os.open(name, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW, dir_fd=directory_fd); before = os.fstat(fd); current = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
        actual = before.st_dev, before.st_ino
        if not stat.S_ISREG(before.st_mode) or stat.S_IMODE(before.st_mode) != 0o600 or before.st_nlink != 1 or before.st_size > limit or (current.st_dev, current.st_ino) != actual or identity not in (None, actual): raise OSError
        chunks: list[bytes] = []; size = 0
        while chunk := os.read(fd, min(1 << 20, limit + 1 - size)):
            chunks.append(chunk); size += len(chunk)
            if size > limit: raise OSError
        after = os.fstat(fd); current = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
        if (after.st_dev, after.st_ino) != actual or after.st_size != size or (current.st_dev, current.st_ino) != actual: raise OSError
        return b"".join(chunks)
    except OSError: raise GateError("EVIDENCE_INCOMPLETE") from None
    finally:
        if fd is not None: os.close(fd)


def run_gate(bazel: str = "bazel", runner: Callable[..., subprocess.CompletedProcess[bytes]] = subprocess.run) -> dict[str, Any]:
    old, root, parent_fd, root_fd = os.umask(0o077), None, None, None
    result, root_id, phases, started, head = record(), None, {}, set(), "0" * 40
    try:
        build, tests = load_manifest(); labels = (build,) + tests; head = _preflight()
        root = Path(tempfile.mkdtemp(prefix="slug-buildbuddy-cache-"))
        if stat.S_IMODE(root.stat().st_mode) != 0o700: raise GateError()
        try: root.resolve().relative_to(REPO_ROOT.resolve()); raise GateError()
        except ValueError: pass
        parent_fd = os.open(root.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW); root_fd = os.open(root.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd)
        item = os.fstat(root_fd); root_id = item.st_dev, item.st_ino; nonce = secrets.token_hex(32); records = {}
        for phase in ("prime", "replay"):
            phase_root = root / phase; phase_root.mkdir(); output = phase_root / "output"; output.mkdir()
            phase_fd = os.open(phase, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=root_fd); phase_item = os.fstat(phase_fd); phase_id = phase_item.st_dev, phase_item.st_ino
            output_fd = os.open("output", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=phase_fd); output_item = os.fstat(output_fd); output_id = output_item.st_dev, output_item.st_ino
            phases[phase] = (phase_fd, phase_id, output_fd, output_id)
            bep, execution, stdout, stderr = (phase_root / name for name in ("bep.json", "execution.json", "stdout", "stderr"))
            for path in (bep, execution, stdout, stderr): _hardened().cleanup._private_file(path)
            identities = {path.name: (path.lstat().st_dev, path.lstat().st_ino) for path in (bep, execution)}; started.add(phase)
            with stdout.open("ab") as out, stderr.open("ab") as err: done = runner(full_command(phase, bazel, output, bep, execution, nonce, labels), cwd=REPO_ROOT, stdout=out, stderr=err, check=False)
            if not _anchored(root, root_id, root_fd, phase, phase_id, output_fd, output_id): raise GateError("EVIDENCE_INCOMPLETE")
            public = phase_record(_artifact_bytes(phase_fd, bep.name, MAX_BEP_BYTES, identities[bep.name]), _artifact_bytes(phase_fd, execution.name, MAX_EXECUTION_BYTES), phase, tests, _count(done.returncode))
            public["output_count"] = _outputs(output)
            if not _anchored(root, root_id, root_fd, phase, phase_id, output_fd, output_id): raise GateError("EVIDENCE_INCOMPLETE")
            records[phase] = public
        result = record(classify(records["prime"], records["replay"]), records["prime"], records["replay"], head, True)
    except GateError as error: result = record(error.classification, git_head=head, git_clean=head != "0" * 40)
    except Exception: result = record("EVIDENCE_INCOMPLETE", git_head=head, git_clean=head != "0" * 40)
    finally:
        os.umask(old); okay = True
        if root is not None and root_fd is not None and root_id is not None:
            for phase, (phase_fd, phase_id, output_fd, output_id) in phases.items():
                if phase in started and _anchored(root, root_id, root_fd, phase, phase_id, output_fd, output_id): okay &= _shutdown(bazel, root / phase / "output", runner) and _anchored(root, root_id, root_fd, phase, phase_id, output_fd, output_id)
                elif phase in started: okay = False
            for phase_fd, _, output_fd, _ in phases.values(): os.close(output_fd); os.close(phase_fd)
            if parent_fd is not None: okay &= _remove_original(parent_fd, root_fd, root_id)
            else: okay = False
            os.close(root_fd); root_fd = None
        elif root_fd is not None: os.close(root_fd); root_fd = None; okay = False
        if root is not None and parent_fd is not None: okay &= _remove_reserved(root, parent_fd)
        elif root is not None: okay = False
        if parent_fd is not None: os.close(parent_fd)
        if root is not None and not _clean(): okay = False
        if not okay: result = record()
    return normalize(result)
