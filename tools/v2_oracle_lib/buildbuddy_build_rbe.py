"""Closed, one-label BuildBuddy managed-RBE evidence."""
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

from tools.v2_oracle_lib import buildbuddy_build_cache as cache
from tools.v2_oracle_lib import buildbuddy_cache as parsed
from tools.v2_oracle_lib import buildbuddy_prime_diagnostic as cleanup

REPO_ROOT = cache.REPO_ROOT
MODE = "buildbuddy-build-rbe-only"
VERSION = "9.2.0"
PLATFORM = "linux-x86_64"
REMOTE_PLATFORM = "linux-amd64-managed"
BAZELRC_SHA256 = "e72f4223b6cfffbc96de018849e306ff9cbfdf4ca50248d8fee229a80dc4c805"
CLASSES = frozenset(("PROVED_BUILD_RBE", "CONFIG_DRIFT", "REMOTE_UNAVAILABLE", "COMMAND_LINE_FAILURE", "TARGET_FAILURE", "CACHE_HIT_OR_MIXED_EXECUTION", "EVIDENCE_INCOMPLETE", "SANITIZER_REJECTED"))
SPAWN_KEYS = frozenset(("count", "valid_digest_count", "remotable_error_count", "cache_hit_error_count", "status_error_count", "exit_error_count", "remote_execution", "remote_cache_hit", "local", "worker", "linux_sandbox", "other"))
PHASE_KEYS = frozenset(("process_success_count", "build_finished_success_count", "target_success_count", "output_count", "persistent_action_cache_hit_count", "spawns"))


class GateError(Exception):
    def __init__(self, classification: str = "EVIDENCE_INCOMPLETE"):
        self.classification = classification if classification in CLASSES else "SANITIZER_REJECTED"


def command(bazel: str, output: Path, bep: Path, execution: Path, nonce: str) -> list[str]:
    if type(nonce) is not str or not re.fullmatch(r"[0-9a-f]{64}", nonce): raise GateError("SANITIZER_REJECTED")
    return [bazel, f"--output_base={output}", "build", "--config=buildbuddy-rbe", "--@rules_rust//rust/toolchain/channel=nightly", "--noremote_accept_cached", "--noremote_upload_local_results", "--remote_download_outputs=toplevel", "--remote_timeout=900", "--jobs=4", "--remote_instance_name=", "--bes_backend=", "--bes_results_url=", "--disk_cache=", "--build_event_publish_all_actions", f"--action_env=SLUG_BUILDBUDDY_BUILD_RBE_NONCE={nonce}", f"--build_event_json_file={bep}", f"--execution_log_json_file={execution}", cache.LABEL]


def _empty_spawns() -> dict[str, int]:
    return {key: 0 for key in SPAWN_KEYS}


def _empty_phase() -> dict[str, object]:
    return {"process_success_count": 0, "build_finished_success_count": 0, "target_success_count": 0, "output_count": 0, "persistent_action_cache_hit_count": 0, "spawns": _empty_spawns()}


def record(classification: str = "SANITIZER_REJECTED", phase: dict[str, object] | None = None) -> dict[str, object]:
    if type(classification) is not str or classification not in CLASSES: classification = "SANITIZER_REJECTED"
    return {"schema_version": 1, "mode": MODE, "classification": classification, "bazel_version": VERSION, "host_platform": PLATFORM, "remote_platform": REMOTE_PLATFORM, "bazelrc_sha256": BAZELRC_SHA256, "rbe": _empty_phase() if phase is None else phase}


def normalize(value: object) -> dict[str, object]:
    try:
        if type(value) is not dict or set(value) != set(record()) or type(value["schema_version"]) is not int or value["schema_version"] != 1: raise GateError()
        if any(type(value[key]) is not str for key in ("mode", "classification", "bazel_version", "host_platform", "remote_platform", "bazelrc_sha256")): raise GateError()
        if (value["mode"], value["bazel_version"], value["host_platform"], value["remote_platform"], value["bazelrc_sha256"]) != (MODE, VERSION, PLATFORM, REMOTE_PLATFORM, BAZELRC_SHA256): raise GateError()
        phase = value["rbe"]
        if type(phase) is not dict or set(phase) != PHASE_KEYS or type(phase["spawns"]) is not dict or set(phase["spawns"]) != SPAWN_KEYS: raise GateError()
        public = {key: cache._count(phase[key]) for key in PHASE_KEYS - {"spawns"}}
        public["spawns"] = {key: cache._count(phase["spawns"][key]) for key in SPAWN_KEYS}; spawns = public["spawns"]
        if sum(spawns[key] for key in ("remote_execution", "remote_cache_hit", "local", "worker", "linux_sandbox", "other")) != spawns["count"] or spawns["valid_digest_count"] > spawns["count"] or any(spawns[key] > spawns["count"] for key in ("remotable_error_count", "cache_hit_error_count", "status_error_count", "exit_error_count")): raise GateError()
        if value["classification"] == "PROVED_BUILD_RBE" and classify(public, "success") != "PROVED_BUILD_RBE": raise GateError()
        result = record(value["classification"], public)
        return result if result == value else record()
    except Exception:
        return record()


def spawn_summary(entries: Iterable[dict[str, Any]]) -> dict[str, int]:
    result = _empty_spawns()
    runners = {"remote": "remote_execution", "remote cache hit": "remote_cache_hit", "local": "local", "worker": "worker", "linux-sandbox": "linux_sandbox"}
    for item in entries:
        event = item.get("spawn", item.get("SpawnExec", item))
        if type(event) is not dict: raise GateError()
        result["count"] += 1
        try: parsed._digest(parsed._field(event, "action_digest", "actionDigest", "digest"))
        except Exception: pass
        else: result["valid_digest_count"] += 1
        remotable = parsed._field(event, "remotable")
        result["remotable_error_count"] += int(type(remotable) is not bool or not remotable)
        hit = parsed._field(event, "cache_hit", "cacheHit")
        result["cache_hit_error_count"] += int(type(hit) is not bool or hit)
        status = parsed._field(event, "status")
        result["status_error_count"] += int(type(status) is not str or status != "")
        exit_code = parsed._field(event, "exit_code", "exitCode")
        result["exit_error_count"] += int(type(exit_code) is not int or exit_code != 0)
        result[runners.get(parsed._field(event, "runner"), "other")] += 1
    return result


def phase_record(bep: bytes, execution: bytes, process_exit: int) -> tuple[dict[str, object], str]:
    base = cache.phase_record(bep, b"", process_exit, "prime")
    phase = {key: base[key] for key in PHASE_KEYS - {"spawns", "output_count"}}
    phase["output_count"] = 0
    phase["spawns"] = spawn_summary(parsed.json_sequence(execution))
    return phase, base["_outcome"]


def classify(phase: dict[str, object], outcome: str) -> str:
    if outcome == "remote": return "REMOTE_UNAVAILABLE"
    if outcome == "command": return "COMMAND_LINE_FAILURE"
    if outcome != "success" or any(phase[key] != 1 for key in ("process_success_count", "build_finished_success_count", "target_success_count", "output_count")): return "TARGET_FAILURE"
    spawns = phase["spawns"]
    if not spawns["count"] or spawns["valid_digest_count"] != spawns["count"]: return "EVIDENCE_INCOMPLETE"
    failures = ("persistent_action_cache_hit_count",)
    if any(phase[key] for key in failures) or any(spawns[key] for key in ("remotable_error_count", "cache_hit_error_count", "status_error_count", "exit_error_count", "remote_cache_hit", "local", "worker", "linux_sandbox", "other")) or spawns["remote_execution"] != spawns["count"]: return "CACHE_HIT_OR_MIXED_EXECUTION"
    return "PROVED_BUILD_RBE"


def _clean() -> bool:
    try: git = cleanup._clean_git()
    except Exception: git = False
    try: daemon = cleanup._no_slugd()
    except Exception: daemon = False
    return git and daemon


def _root_bytes(name: str, limit: int) -> bytes:
    parent_fd = root_fd = file_fd = None
    try:
        parent_fd = os.open(REPO_ROOT.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        root_fd = os.open(REPO_ROOT.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd)
        root_item = os.fstat(root_fd); root_id = root_item.st_dev, root_item.st_ino
        current_root = os.stat(REPO_ROOT.name, dir_fd=parent_fd, follow_symlinks=False)
        if not stat.S_ISDIR(root_item.st_mode) or (current_root.st_dev, current_root.st_ino) != root_id: raise OSError
        file_fd = os.open(name, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW, dir_fd=root_fd)
        before = os.fstat(file_fd); current = os.stat(name, dir_fd=root_fd, follow_symlinks=False)
        identity = before.st_dev, before.st_ino
        if not stat.S_ISREG(before.st_mode) or (current.st_dev, current.st_ino) != identity or before.st_size > limit: raise OSError
        chunks, size = [], 0
        while chunk := os.read(file_fd, min(4096, limit + 1 - size)):
            chunks.append(chunk); size += len(chunk)
            if size > limit: raise OSError
        after = os.fstat(file_fd); current = os.stat(name, dir_fd=root_fd, follow_symlinks=False); current_root = os.stat(REPO_ROOT.name, dir_fd=parent_fd, follow_symlinks=False)
        if (after.st_dev, after.st_ino) != identity or (current.st_dev, current.st_ino) != identity or (current_root.st_dev, current_root.st_ino) != root_id or after.st_size != size: raise OSError
        return b"".join(chunks)
    finally:
        if file_fd is not None: os.close(file_fd)
        if root_fd is not None: os.close(root_fd)
        if parent_fd is not None: os.close(parent_fd)


def _preflight() -> None:
    try:
        okay = _clean() and platform.system() == "Linux" and platform.machine() in {"x86_64", "AMD64"}
        okay &= _root_bytes(".bazelversion", 64) == b"9.2.0\n"
        okay &= hashlib.sha256(_root_bytes(".bazelrc", 1 << 20)).hexdigest() == BAZELRC_SHA256
    except Exception: okay = False
    if not okay: raise GateError("CONFIG_DRIFT")


def _remove_reserved(root: Path, parent_fd: int) -> bool:
    root_fd = None
    try:
        item = os.stat(root.name, dir_fd=parent_fd, follow_symlinks=False)
        if not stat.S_ISDIR(item.st_mode):
            os.unlink(root.name, dir_fd=parent_fd)
            return False
        root_fd = os.open(root.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd)
        opened = os.fstat(root_fd); identity = opened.st_dev, opened.st_ino
        return cache._remove_original(parent_fd, root_fd, identity)
    except FileNotFoundError: return True
    except Exception: return False
    finally:
        if root_fd is not None: os.close(root_fd)


def run_gate(bazel: str = "bazel", runner: Callable[..., subprocess.CompletedProcess[bytes]] = subprocess.run) -> dict[str, object]:
    old, root, parent_fd, root_fd, phase_fd, output_fd, root_id, phase_id, output_id = os.umask(0o077), None, None, None, None, None, None, None, None
    result, started = record(), False
    try:
        _preflight()
        root = Path(tempfile.mkdtemp(prefix="slug-buildbuddy-rbe-"))
        if stat.S_IMODE(root.stat().st_mode) != 0o700: raise GateError("SANITIZER_REJECTED")
        try: root.resolve().relative_to(REPO_ROOT.resolve()); raise GateError("SANITIZER_REJECTED")
        except ValueError: pass
        parent_fd = os.open(root.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW); root_fd = os.open(root.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd)
        item = os.fstat(root_fd); root_id = item.st_dev, item.st_ino
        phase = root / "rbe"; phase.mkdir(); phase_fd = os.open("rbe", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=root_fd)
        item = os.fstat(phase_fd); phase_id = item.st_dev, item.st_ino
        output = phase / "output"; output.mkdir(); output_fd = os.open("output", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=phase_fd)
        item = os.fstat(output_fd); output_id = item.st_dev, item.st_ino
        bep, execution, stdout, stderr = (phase / name for name in ("bep.json", "execution.json", "stdout", "stderr"))
        for path in (bep, execution, stdout, stderr): cleanup._private_file(path)
        identities = {path.name: (path.lstat().st_dev, path.lstat().st_ino) for path in (bep, execution)}
        started = True
        with stdout.open("ab") as out, stderr.open("ab") as err:
            done = runner(command(bazel, output, bep, execution, secrets.token_hex(32)), cwd=REPO_ROOT, stdout=out, stderr=err, check=False)
        if not cache._anchored(root, root_id, root_fd, "rbe", phase_id, output_fd, output_id): raise GateError()
        public, outcome = phase_record(cache._private_bytes(phase_fd, bep.name, identities[bep.name]), cache._execution_bytes(phase_fd, execution.name), cache._count(done.returncode))
        public["output_count"] = cache._outputs(output)
        if not cache._anchored(root, root_id, root_fd, "rbe", phase_id, output_fd, output_id): raise GateError()
        result = record(classify(public, outcome), public)
    except GateError as error: result = record(error.classification)
    except Exception: result = record("EVIDENCE_INCOMPLETE")
    finally:
        os.umask(old); okay = True
        if root is not None:
            okay = parent_fd is not None and root_fd is not None and phase_fd is not None and output_fd is not None and all(value is not None for value in (root_id, phase_id, output_id))
        if root is not None and started and okay and cache._anchored(root, root_id, root_fd, "rbe", phase_id, output_fd, output_id): okay = cache._shutdown(bazel, root / "rbe" / "output", runner) and cache._anchored(root, root_id, root_fd, "rbe", phase_id, output_fd, output_id)
        elif started: okay = False
        if output_fd is not None: os.close(output_fd)
        if phase_fd is not None: os.close(phase_fd)
        if root is not None and parent_fd is not None and root_fd is not None and root_id is not None: okay &= cache._remove_original(parent_fd, root_fd, root_id)
        elif root is not None: okay = False
        if root is not None and parent_fd is not None: okay &= _remove_reserved(root, parent_fd)
        if root_fd is not None: os.close(root_fd)
        if parent_fd is not None: os.close(parent_fd)
        if root is not None and not _clean(): okay = False
        if not okay: result = record()
    return normalize(result)
