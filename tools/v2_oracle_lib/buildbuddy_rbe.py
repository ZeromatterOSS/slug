"""Closed, manifest-aware BuildBuddy managed-RBE evidence."""
from __future__ import annotations

import os
import re
import secrets
import stat
import subprocess
import tempfile
from pathlib import Path
from typing import Any, Callable, Iterable

from tools.v2_oracle_lib import buildbuddy_build_rbe as one
from tools.v2_oracle_lib import buildbuddy_cache as cache

REPO_ROOT = cache.REPO_ROOT
MODE = "buildbuddy-rbe-only"
REMOTE_PLATFORM = "linux-amd64-managed"
CLASSES = frozenset(("PROVED_RBE", "CONFIG_DRIFT", "REMOTE_UNAVAILABLE", "COMMAND_LINE_FAILURE", "TARGET_FAILURE", "CACHE_HIT_OR_MIXED_EXECUTION", "EVIDENCE_INCOMPLETE", "SANITIZER_REJECTED"))
SPAWN_KEYS = one.SPAWN_KEYS
PHASE_KEYS = frozenset(("process_success_count", "build_finished_success_count", "build_success_count", "output_count", "test_completion_count", "passed_test_count", "test_run_count", "remotely_cached_test_count", "persistent_action_cache_hit_count", "spawns"))


class GateError(Exception):
    def __init__(self, classification: str = "SANITIZER_REJECTED"):
        self.classification = classification if classification in CLASSES else "SANITIZER_REJECTED"


def command(bazel: str, output: Path, bep: Path, execution: Path, nonce: str, labels: tuple[str, ...]) -> list[str]:
    if type(nonce) is not str or not re.fullmatch(r"[0-9a-f]{64}", nonce): raise GateError()
    if type(labels) is not tuple or len(labels) != 44 or labels[0] != cache.BUILD_LABEL or labels[1:] != tuple(sorted(labels[1:])) or len(set(labels)) != 44: raise GateError("CONFIG_DRIFT")
    return [bazel, f"--output_base={output}", "test", "--config=buildbuddy-rbe", "--@rules_rust//rust/toolchain/channel=nightly", "--noremote_accept_cached", "--noremote_upload_local_results", "--remote_download_outputs=toplevel", "--remote_timeout=900", "--jobs=4", "--remote_instance_name=", "--bes_backend=", "--bes_results_url=", "--disk_cache=", "--build_event_publish_all_actions", "--cache_test_results=yes", "--runs_per_test=1", "--test_sharding_strategy=disabled", f"--action_env=SLUG_BUILDBUDDY_RBE_GATE_NONCE={nonce}", f"--test_env=SLUG_BUILDBUDDY_RBE_GATE_NONCE={nonce}", f"--build_event_json_file={bep}", f"--execution_log_json_file={execution}", *labels]


def spawn_summary(entries: Iterable[dict[str, Any]]) -> dict[str, int]:
    try: return one.spawn_summary(entries)
    except Exception: raise GateError("EVIDENCE_INCOMPLETE") from None


def phase_record(bep: bytes, execution: bytes, tests: tuple[str, ...], process_exit: int) -> tuple[dict[str, Any], str]:
    try: base = cache.phase_record(bep, b"", "prime", tests, process_exit)
    except Exception as error:
        classification = error.classification if isinstance(error, cache.GateError) else "EVIDENCE_INCOMPLETE"
        raise GateError(classification) from None
    phase = {key: base[key] for key in PHASE_KEYS - {"spawns", "output_count"}}
    phase["output_count"] = 0; phase["spawns"] = spawn_summary(cache.json_sequence(execution))
    return phase, base["_outcome"]


def classify(phase: dict[str, Any], outcome: str) -> str:
    if outcome == "remote": return "REMOTE_UNAVAILABLE"
    if outcome == "command": return "COMMAND_LINE_FAILURE"
    required = {"process_success_count": 1, "build_finished_success_count": 1, "build_success_count": 1, "output_count": 1, "test_completion_count": 43, "passed_test_count": 43, "test_run_count": 43}
    if outcome != "success" or any(phase[key] != value for key, value in required.items()): return "TARGET_FAILURE"
    spawns = phase["spawns"]
    if not spawns["count"] or spawns["valid_digest_count"] != spawns["count"]: return "EVIDENCE_INCOMPLETE"
    if phase["remotely_cached_test_count"] or phase["persistent_action_cache_hit_count"] or any(spawns[key] for key in ("remotable_error_count", "cache_hit_error_count", "status_error_count", "exit_error_count", "remote_cache_hit", "local", "worker", "linux_sandbox", "other")) or spawns["remote_execution"] != spawns["count"]: return "CACHE_HIT_OR_MIXED_EXECUTION"
    return "PROVED_RBE"


def _empty_phase() -> dict[str, Any]:
    return {key: ({name: 0 for name in SPAWN_KEYS} if key == "spawns" else 0) for key in PHASE_KEYS}


def record(classification: str = "SANITIZER_REJECTED", phase: dict[str, Any] | None = None, git_head: str = "0" * 40, git_clean: bool = False) -> dict[str, Any]:
    if type(classification) is not str or classification not in CLASSES: classification = "SANITIZER_REJECTED"
    public = _empty_phase() if phase is None else {key: phase[key] for key in PHASE_KEYS}
    return {"schema_version": 1, "mode": MODE, "classification": classification, "bazel_version": cache.BAZEL_VERSION, "host_platform": cache.HOST_PLATFORM, "remote_platform": REMOTE_PLATFORM, "bazelrc_sha256": cache.BAZELRC_SHA256, "git_head": git_head, "git_clean": git_clean, "manifest_version": cache.VERSION, "manifest_sha256": cache.MANIFEST_SHA256, "target_counts": {"build": 1, "test": 43}, "rbe": public}


def normalize(value: object) -> dict[str, Any]:
    try:
        if type(value) is not dict or set(value) != set(record()) or type(value["schema_version"]) is not int or value["schema_version"] != 1 or type(value["git_clean"]) is not bool or type(value["target_counts"]) is not dict: raise GateError()
        strings = ("mode", "classification", "bazel_version", "host_platform", "remote_platform", "bazelrc_sha256", "git_head", "manifest_version", "manifest_sha256")
        if any(type(value[key]) is not str for key in strings) or (value["mode"], value["bazel_version"], value["host_platform"], value["remote_platform"], value["bazelrc_sha256"], value["manifest_version"], value["manifest_sha256"], value["target_counts"]) != (MODE, cache.BAZEL_VERSION, cache.HOST_PLATFORM, REMOTE_PLATFORM, cache.BAZELRC_SHA256, cache.VERSION, cache.MANIFEST_SHA256, {"build": 1, "test": 43}) or value["classification"] not in CLASSES or not re.fullmatch(r"[0-9a-f]{40}", value["git_head"]): raise GateError()
        source = value["rbe"]
        if type(source) is not dict or set(source) != PHASE_KEYS or type(source["spawns"]) is not dict or set(source["spawns"]) != SPAWN_KEYS: raise GateError()
        phase = {key: cache._count(source[key]) for key in PHASE_KEYS - {"spawns"}}; phase["spawns"] = {key: cache._count(source["spawns"][key]) for key in SPAWN_KEYS}; spawns = phase["spawns"]
        if sum(spawns[key] for key in ("remote_execution", "remote_cache_hit", "local", "worker", "linux_sandbox", "other")) != spawns["count"] or spawns["valid_digest_count"] > spawns["count"] or any(spawns[key] > spawns["count"] for key in ("remotable_error_count", "cache_hit_error_count", "status_error_count", "exit_error_count")): raise GateError()
        if value["classification"] == "PROVED_RBE" and (not value["git_clean"] or value["git_head"] == "0" * 40 or classify(phase, "success") != "PROVED_RBE"): raise GateError()
        result = record(value["classification"], phase, value["git_head"], value["git_clean"])
        return result if result == value else record()
    except Exception: return record()


def _remove_reserved(root: Path, parent_fd: int) -> bool:
    reserved_fd = None
    try:
        temp = Path(tempfile.gettempdir()).resolve(); parent = os.fstat(parent_fd); expected = os.stat(temp, follow_symlinks=False)
        if not root.name.startswith("slug-buildbuddy-full-rbe-") or root.parent.resolve() != temp or not stat.S_ISDIR(parent.st_mode) or (parent.st_dev, parent.st_ino) != (expected.st_dev, expected.st_ino): return False
        try: item = os.stat(root.name, dir_fd=parent_fd, follow_symlinks=False)
        except FileNotFoundError: return True
        if not stat.S_ISDIR(item.st_mode): os.unlink(root.name, dir_fd=parent_fd); return False
        reserved_fd = os.open(root.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd); opened = os.fstat(reserved_fd); identity = opened.st_dev, opened.st_ino
        if (item.st_dev, item.st_ino) != identity: return False
        return cache._remove_original(parent_fd, reserved_fd, identity)
    except Exception: return False
    finally:
        if reserved_fd is not None: os.close(reserved_fd)


def run_gate(bazel: str = "bazel", runner: Callable[..., subprocess.CompletedProcess[bytes]] = subprocess.run) -> dict[str, Any]:
    old, root, parent_fd, root_fd, phase_fd, output_fd = os.umask(0o077), None, None, None, None, None
    result, root_id, phase_id, output_id, started, head = record(), None, None, None, False, "0" * 40
    try:
        build, tests = cache.load_manifest(); labels = (build,) + tests; head = cache._preflight()
        root = Path(tempfile.mkdtemp(prefix="slug-buildbuddy-full-rbe-"))
        if stat.S_IMODE(root.stat().st_mode) != 0o700: raise GateError()
        try: root.resolve().relative_to(REPO_ROOT.resolve()); raise GateError()
        except ValueError: pass
        parent_fd = os.open(root.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW); root_fd = os.open(root.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd); item = os.fstat(root_fd); root_id = item.st_dev, item.st_ino
        phase_root = root / "rbe"; phase_root.mkdir(); phase_fd = os.open("rbe", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=root_fd); item = os.fstat(phase_fd); phase_id = item.st_dev, item.st_ino
        output = phase_root / "output"; output.mkdir(); output_fd = os.open("output", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=phase_fd); item = os.fstat(output_fd); output_id = item.st_dev, item.st_ino
        bep, execution, stdout, stderr = (phase_root / name for name in ("bep.json", "execution.json", "stdout", "stderr"))
        for path in (bep, execution, stdout, stderr): cache._hardened().cleanup._private_file(path)
        identities = {path.name: (path.lstat().st_dev, path.lstat().st_ino) for path in (bep, execution)}; started = True
        with stdout.open("ab") as out, stderr.open("ab") as err: done = runner(command(bazel, output, bep, execution, secrets.token_hex(32), labels), cwd=REPO_ROOT, stdout=out, stderr=err, check=False)
        if not cache._anchored(root, root_id, root_fd, "rbe", phase_id, output_fd, output_id): raise GateError("EVIDENCE_INCOMPLETE")
        public, outcome = phase_record(cache._artifact_bytes(phase_fd, bep.name, cache.MAX_BEP_BYTES, identities[bep.name]), cache._artifact_bytes(phase_fd, execution.name, cache.MAX_EXECUTION_BYTES), tests, cache._count(done.returncode))
        public["output_count"] = cache._outputs(output)
        if not cache._anchored(root, root_id, root_fd, "rbe", phase_id, output_fd, output_id): raise GateError("EVIDENCE_INCOMPLETE")
        result = record(classify(public, outcome), public, head, True)
    except (GateError, cache.GateError) as error: result = record(error.classification, git_head=head, git_clean=head != "0" * 40)
    except Exception: result = record("EVIDENCE_INCOMPLETE", git_head=head, git_clean=head != "0" * 40)
    finally:
        os.umask(old); okay = True
        ready = root is not None and parent_fd is not None and root_fd is not None and phase_fd is not None and output_fd is not None and all(value is not None for value in (root_id, phase_id, output_id))
        if ready and started and cache._anchored(root, root_id, root_fd, "rbe", phase_id, output_fd, output_id): okay &= cache._shutdown(bazel, root / "rbe/output", runner) and cache._anchored(root, root_id, root_fd, "rbe", phase_id, output_fd, output_id)
        elif started: okay = False
        if output_fd is not None: os.close(output_fd)
        if phase_fd is not None: os.close(phase_fd)
        if root is not None and parent_fd is not None and root_fd is not None and root_id is not None: okay &= cache._remove_original(parent_fd, root_fd, root_id)
        elif root is not None: okay = False
        if root_fd is not None: os.close(root_fd)
        if root is not None and parent_fd is not None: okay &= _remove_reserved(root, parent_fd)
        elif root is not None: okay = False
        if parent_fd is not None: os.close(parent_fd)
        if root is not None and not cache._clean(): okay = False
        if not okay: result = record()
    return normalize(result)
