"""Closed, build-only BuildBuddy cache prime/replay evidence."""
from __future__ import annotations

import hashlib
import json
import os
import re
import secrets
import stat
import subprocess
import tempfile
from pathlib import Path
from typing import Any, Callable, Iterable

from tools.v2_oracle_lib import buildbuddy_cache as parsed
from tools.v2_oracle_lib import buildbuddy_prime_diagnostic as cleanup

REPO_ROOT = Path(__file__).resolve().parents[2]
LABEL = "//app/slug_cli_v2:slug"
CLASSES = frozenset(("PROVED_BUILD_CACHE", "CONFIG_DRIFT", "REMOTE_UNAVAILABLE", "COMMAND_LINE_FAILURE", "TARGET_FAILURE", "CACHE_MISS_OR_MIXED_REPLAY", "EVIDENCE_INCOMPLETE", "SANITIZER_REJECTED"))
PHASE_KEYS = frozenset(("process_success_count", "build_finished_success_count", "target_success_count", "output_count", "persistent_action_cache_hit_count", "eligible_spawns"))
SPAWN_KEYS = frozenset(("count", "digest_multiset_sha256", "cache_error_count", "status_error_count", "exit_error_count", "local", "worker", "linux_sandbox", "remote_cache_hit", "other"))


class GateError(Exception):
    def __init__(self, classification: str = "SANITIZER_REJECTED"):
        self.classification = classification if classification in CLASSES else "SANITIZER_REJECTED"


def command(bazel: str, output: Path, bep: Path, execution: Path, nonce: str) -> list[str]:
    if not isinstance(nonce, str) or not re.fullmatch(r"[0-9a-f]{64}", nonce):
        raise GateError()
    return [bazel, f"--output_base={output}", "build", "--config=buildbuddy-cache", "--noremote_accept_cached", "--@rules_rust//rust/toolchain/channel=nightly", "--remote_executor=", "--bes_backend=", "--bes_results_url=", "--disk_cache=", "--noremote_local_fallback", f"--action_env=SLUG_BUILDBUDDY_BUILD_CACHE_NONCE={nonce}", f"--build_event_json_file={bep}", f"--execution_log_json_file={execution}", LABEL]


def _count(value: Any) -> int:
    if type(value) is not int or value < 0:
        raise GateError("EVIDENCE_INCOMPLETE")
    return value


def spawns(entries: Iterable[dict[str, Any]], phase: str) -> dict[str, Any]:
    runners = {key: 0 for key in ("local", "worker", "linux_sandbox", "remote_cache_hit", "other")}
    digests: list[str] = []
    errors = {key: 0 for key in ("cache", "status", "exit")}
    for item in entries:
        event = item.get("spawn", item.get("SpawnExec", item))
        if not isinstance(event, dict):
            raise GateError("EVIDENCE_INCOMPLETE")
        if not (parsed._boolean(event, "cacheable") and parsed._boolean(event, "remote_cacheable", "remoteCacheable")):
            continue
        runner = event.get("runner")
        key = {"local": "local", "worker": "worker", "linux-sandbox": "linux_sandbox", "remote cache hit": "remote_cache_hit"}.get(runner, "other")
        runners[key] += 1
        digests.append(parsed._digest(parsed._field(event, "action_digest", "actionDigest", "digest")))
        hit = parsed._field(event, "cache_hit", "cacheHit")
        errors["cache"] += int(not isinstance(hit, bool) or hit != (phase == "replay"))
        errors["status"] += int(parsed._field(event, "status", default="") not in ("", None))
        errors["exit"] += int(_count(parsed._field(event, "exit_code", "exitCode", default=0)) != 0)
    return {"count": len(digests), "digest_multiset_sha256": hashlib.sha256("\n".join(sorted(digests)).encode()).hexdigest(), "cache_error_count": errors["cache"], "status_error_count": errors["status"], "exit_error_count": errors["exit"], **runners}


def phase_record(bep: bytes, execution: bytes, process_exit: int, phase: str) -> dict[str, Any]:
    finished_events: list[Any] = []
    target_successes = 0
    persistent_hits = 0
    remote_failure = False
    for event in parsed.json_sequence(bep):
        ident = event.get("id")
        if not isinstance(ident, dict):
            raise GateError("EVIDENCE_INCOMPLETE")
        if "buildFinished" in ident:
            finished_events.append(event.get("finished"))
        target = ident.get("targetCompleted")
        if isinstance(target, dict) and target.get("label") == LABEL and event.get("completed", {}).get("success") is True:
            target_successes += 1
        metrics = event.get("buildMetrics", {}).get("actionSummary", {}).get("actionCacheStatistics", {})
        if isinstance(metrics, dict):
            persistent_hits += _count(metrics.get("hits", 0))
        remote_failure |= isinstance(event.get("aborted"), dict) and event["aborted"].get("reason") == "REMOTE_ENVIRONMENT_FAILURE"
    if len(finished_events) != 1 or not isinstance(finished_events[0], dict):
        raise GateError("EVIDENCE_INCOMPLETE")
    finished = finished_events[0]
    if not isinstance(finished.get("exitCode"), dict):
        raise GateError("EVIDENCE_INCOMPLETE")
    exit_data = finished["exitCode"]
    name, code = exit_data.get("name"), _count(exit_data.get("code", 0))
    success = name == "SUCCESS" and code == 0
    outcome = "remote" if remote_failure or name in {"REMOTE_ERROR", "REMOTE_ENVIRONMENTAL_ERROR"} else "command" if name == "COMMAND_LINE_ERROR" and code == 2 else "success" if success else "target"
    return {"process_success_count": int(process_exit == 0), "build_finished_success_count": int(success), "target_success_count": target_successes, "output_count": 0, "persistent_action_cache_hit_count": persistent_hits, "eligible_spawns": spawns(parsed.json_sequence(execution), phase), "_outcome": outcome}


def _outputs(base: Path) -> int:
    try:
        matches = [path for path in (base / "execroot").rglob("slug") if path.parts[-4:] == ("bin", "app", "slug_cli_v2", "slug")]
    except OSError:
        raise GateError("EVIDENCE_INCOMPLETE") from None
    count = 0
    for path in matches:
        try:
            metadata = path.lstat()
        except OSError:
            raise GateError("EVIDENCE_INCOMPLETE") from None
        count += int(stat.S_ISREG(metadata.st_mode) and bool(metadata.st_mode & (stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)))
    return count


def classify(prime: dict[str, Any], replay: dict[str, Any]) -> str:
    records = (prime, replay)
    if any(record["_outcome"] == "remote" for record in records): return "REMOTE_UNAVAILABLE"
    if any(record["_outcome"] == "command" for record in records): return "COMMAND_LINE_FAILURE"
    if any(record["_outcome"] != "success" or any(record[key] != 1 for key in ("process_success_count", "build_finished_success_count", "target_success_count", "output_count")) for record in records): return "TARGET_FAILURE"
    a, b = prime["eligible_spawns"], replay["eligible_spawns"]
    if not a["count"] or not b["count"]: return "EVIDENCE_INCOMPLETE"
    if a["count"] != b["count"] or a["digest_multiset_sha256"] != b["digest_multiset_sha256"]: return "CACHE_MISS_OR_MIXED_REPLAY"
    if any(record["persistent_action_cache_hit_count"] for record in records): return "CACHE_MISS_OR_MIXED_REPLAY"
    if any(a[key] or b[key] for key in ("cache_error_count", "status_error_count", "exit_error_count")): return "CACHE_MISS_OR_MIXED_REPLAY"
    if any(a[key] for key in ("remote_cache_hit", "other")) or a["local"] + a["worker"] + a["linux_sandbox"] != a["count"]: return "CACHE_MISS_OR_MIXED_REPLAY"
    if b["remote_cache_hit"] != b["count"] or any(b[key] for key in ("local", "worker", "linux_sandbox", "other")): return "CACHE_MISS_OR_MIXED_REPLAY"
    return "PROVED_BUILD_CACHE"


def record(classification: str, prime: dict[str, Any] | None = None, replay: dict[str, Any] | None = None) -> dict[str, Any]:
    empty = {"process_success_count": 0, "build_finished_success_count": 0, "target_success_count": 0, "output_count": 0, "persistent_action_cache_hit_count": 0, "eligible_spawns": {"count": 0, "digest_multiset_sha256": hashlib.sha256(b"").hexdigest(), "cache_error_count": 0, "status_error_count": 0, "exit_error_count": 0, "local": 0, "worker": 0, "linux_sandbox": 0, "remote_cache_hit": 0, "other": 0}}
    if type(classification) is not str or classification not in CLASSES: classification = "SANITIZER_REJECTED"
    if prime is None or replay is None: prime = replay = empty
    def public(item: dict[str, Any]) -> dict[str, Any]: return {key: value for key, value in item.items() if key != "_outcome"}
    return {"schema_version": 1, "mode": "buildbuddy-build-cache-only", "classification": classification, "prime": public(prime), "replay": public(replay)}


def normalize(value: object) -> dict[str, Any]:
    """Rebuild only the closed public schema; reject all other values."""
    try:
        if type(value) is not dict or set(value) != {"schema_version", "mode", "classification", "prime", "replay"}:
            raise GateError()
        classification = value["classification"]
        if type(value["schema_version"]) is not int or value["schema_version"] != 1 or type(value["mode"]) is not str or value["mode"] != "buildbuddy-build-cache-only" or type(classification) is not str or classification not in CLASSES:
            raise GateError()

        def phase(item: object) -> dict[str, Any]:
            if type(item) is not dict or set(item) != PHASE_KEYS or type(item["eligible_spawns"]) is not dict or set(item["eligible_spawns"]) != SPAWN_KEYS:
                raise GateError()
            public = {key: _count(item[key]) for key in PHASE_KEYS - {"eligible_spawns"}}
            source = item["eligible_spawns"]
            digest = source["digest_multiset_sha256"]
            if type(digest) is not str or not re.fullmatch(r"[0-9a-f]{64}", digest):
                raise GateError()
            spawns = {key: _count(source[key]) for key in SPAWN_KEYS - {"digest_multiset_sha256"}}
            if sum(spawns[key] for key in ("local", "worker", "linux_sandbox", "remote_cache_hit", "other")) != spawns["count"] or any(spawns[key] > spawns["count"] for key in ("cache_error_count", "status_error_count", "exit_error_count")):
                raise GateError()
            spawns["digest_multiset_sha256"] = digest
            public["eligible_spawns"] = {key: spawns[key] for key in ("count", "digest_multiset_sha256", "cache_error_count", "status_error_count", "exit_error_count", "local", "worker", "linux_sandbox", "remote_cache_hit", "other")}
            return {key: public[key] for key in ("process_success_count", "build_finished_success_count", "target_success_count", "output_count", "persistent_action_cache_hit_count", "eligible_spawns")}

        return record(classification, phase(value["prime"]), phase(value["replay"]))
    except Exception:
        return record("SANITIZER_REJECTED")


def _private_bytes(directory_fd: int, name: str, identity: tuple[int, int]) -> bytes:
    fd = None
    try:
        fd = os.open(name, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW, dir_fd=directory_fd)
        metadata = os.fstat(fd)
        current = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1 or (metadata.st_dev, metadata.st_ino) != identity or (current.st_dev, current.st_ino) != identity or metadata.st_mode & (stat.S_IRWXG | stat.S_IRWXO):
            raise GateError("EVIDENCE_INCOMPLETE")
        chunks: list[bytes] = []
        while chunk := os.read(fd, 1 << 20):
            chunks.append(chunk)
        current = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
        if (current.st_dev, current.st_ino) != identity:
            raise GateError("EVIDENCE_INCOMPLETE")
        return b"".join(chunks)
    except OSError:
        raise GateError("EVIDENCE_INCOMPLETE") from None
    finally:
        if fd is not None:
            os.close(fd)


def _execution_bytes(directory_fd: int, name: str) -> bytes:
    """Read a direct child only after binding its current no-follow descriptor."""
    fd = None
    try:
        fd = os.open(name, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW, dir_fd=directory_fd)
        item = os.fstat(fd)
        identity = item.st_dev, item.st_ino
        current = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
        if not stat.S_ISREG(item.st_mode) or stat.S_IMODE(item.st_mode) != 0o600 or item.st_nlink != 1 or (current.st_dev, current.st_ino) != identity:
            raise GateError("EVIDENCE_INCOMPLETE")
        chunks: list[bytes] = []
        while chunk := os.read(fd, 1 << 20):
            chunks.append(chunk)
        after = os.fstat(fd); current = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
        if not stat.S_ISREG(after.st_mode) or stat.S_IMODE(after.st_mode) != 0o600 or after.st_nlink != 1 or (after.st_dev, after.st_ino) != identity or (current.st_dev, current.st_ino) != identity:
            raise GateError("EVIDENCE_INCOMPLETE")
        return b"".join(chunks)
    except OSError:
        raise GateError("EVIDENCE_INCOMPLETE") from None
    finally:
        if fd is not None:
            os.close(fd)


def _anchored(root: Path, root_identity: tuple[int, int], root_fd: int, phase: str, phase_identity: tuple[int, int], output_fd: int | None = None, output_identity: tuple[int, int] | None = None) -> bool:
    phase_fd = None
    try:
        current_root, opened_root = root.lstat(), os.fstat(root_fd)
        current_phase = os.stat(phase, dir_fd=root_fd, follow_symlinks=False)
        root_ok = stat.S_ISDIR(current_root.st_mode) and (current_root.st_dev, current_root.st_ino) == root_identity == (opened_root.st_dev, opened_root.st_ino)
        phase_ok = stat.S_ISDIR(current_phase.st_mode) and (current_phase.st_dev, current_phase.st_ino) == phase_identity
        if output_fd is None or output_identity is None:
            return root_ok and phase_ok
        phase_fd = os.open(phase, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=root_fd)
        current_output, opened_output = os.stat("output", dir_fd=phase_fd, follow_symlinks=False), os.fstat(output_fd)
        return root_ok and phase_ok and stat.S_ISDIR(current_output.st_mode) and (current_output.st_dev, current_output.st_ino) == output_identity == (opened_output.st_dev, opened_output.st_ino)
    except OSError:
        return False
    finally:
        if phase_fd is not None:
            os.close(phase_fd)


def _clean() -> bool: return cleanup._clean_git() and cleanup._no_slugd()


def _remove_reserved(root: Path, parent_fd: int) -> bool:
    try:
        item = os.stat(root.name, dir_fd=parent_fd, follow_symlinks=False)
    except FileNotFoundError:
        return True
    except OSError:
        return False
    if stat.S_ISDIR(item.st_mode):
        return cleanup._remove_root(root)
    try:
        os.unlink(root.name, dir_fd=parent_fd)
        return False
    except FileNotFoundError:
        return True
    except OSError:
        return False


def _remove_original(parent_fd: int, root_fd: int, identity: tuple[int, int]) -> bool:
    def clear(directory_fd: int) -> None:
        for name in os.listdir(directory_fd):
            item = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
            if stat.S_ISDIR(item.st_mode):
                child_fd = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=directory_fd)
                try:
                    opened = os.fstat(child_fd); os.fchmod(child_fd, stat.S_IMODE(opened.st_mode) | stat.S_IRWXU); clear(child_fd)
                finally:
                    os.close(child_fd)
                current = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
                if (current.st_dev, current.st_ino) != (opened.st_dev, opened.st_ino): raise OSError
                os.rmdir(name, dir_fd=directory_fd)
            else:
                os.unlink(name, dir_fd=directory_fd)
    try:
        opened = os.fstat(root_fd)
        if not stat.S_ISDIR(opened.st_mode) or (opened.st_dev, opened.st_ino) != identity: return False
        os.fchmod(root_fd, stat.S_IMODE(opened.st_mode) | stat.S_IRWXU); clear(root_fd)
        names = [name for name in os.listdir(parent_fd) if (lambda item: stat.S_ISDIR(item.st_mode) and (item.st_dev, item.st_ino) == identity)(os.stat(name, dir_fd=parent_fd, follow_symlinks=False))]
        if len(names) != 1: return False
        os.rmdir(names[0], dir_fd=parent_fd)
        return True
    except Exception:
        return False


def _shutdown(bazel: str, output: Path, runner: Callable[..., subprocess.CompletedProcess[bytes]]) -> bool:
    try:
        return runner([bazel, "--ignore_all_rc_files", f"--output_base={output}", "shutdown"], cwd=REPO_ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False).returncode == 0
    except Exception:
        return False


def run_gate(bazel: str = "bazel", runner: Callable[..., subprocess.CompletedProcess[bytes]] = subprocess.run) -> dict[str, Any]:
    old_umask, root, parent_fd, root_fd, result = os.umask(0o077), None, None, None, record("SANITIZER_REJECTED")
    root_identity: tuple[int, int] | None = None
    phases: dict[str, tuple[int, tuple[int, int], int, tuple[int, int]]] = {}
    try:
        if not _clean(): raise GateError("CONFIG_DRIFT")
        root = Path(tempfile.mkdtemp(prefix="slug-buildbuddy-prime-"))
        if stat.S_IMODE(root.stat().st_mode) != 0o700: raise GateError()
        try: root.resolve().relative_to(REPO_ROOT.resolve()); raise GateError()
        except ValueError: pass
        parent_fd = os.open(root.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        root_fd = os.open(root.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd)
        root_metadata = os.fstat(root_fd); root_identity = (root_metadata.st_dev, root_metadata.st_ino)
        nonce, records = secrets.token_hex(32), {}
        for phase in ("prime", "replay"):
            phase_root = root / phase; phase_root.mkdir()
            (phase_root / "output").mkdir()
            bep, execution = phase_root / "bep.json", phase_root / "execution.json"
            for evidence in (bep, execution): cleanup._private_file(evidence)
            identities = {path.name: (path.lstat().st_dev, path.lstat().st_ino) for path in (bep, execution)}
            phase_fd = output_fd = None
            try:
                phase_fd = os.open(phase, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=root_fd)
                output_fd = os.open("output", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=phase_fd)
                phase_metadata, output_metadata = os.fstat(phase_fd), os.fstat(output_fd)
                phase_identity, output_identity = (phase_metadata.st_dev, phase_metadata.st_ino), (output_metadata.st_dev, output_metadata.st_ino)
            except Exception:
                if output_fd is not None: os.close(output_fd)
                if phase_fd is not None: os.close(phase_fd)
                raise
            phases[phase] = (phase_fd, phase_identity, output_fd, output_identity)
            with (phase_root / "stdout").open("xb") as stdout, (phase_root / "stderr").open("xb") as stderr:
                done = runner(command(bazel, phase_root / "output", bep, execution, nonce), cwd=REPO_ROOT, stdout=stdout, stderr=stderr, check=False)
            if not _anchored(root, root_identity, root_fd, phase, phase_identity, output_fd, output_identity): raise GateError("EVIDENCE_INCOMPLETE")
            item = phase_record(_private_bytes(phase_fd, bep.name, identities[bep.name]), _execution_bytes(phase_fd, execution.name), _count(done.returncode), phase)
            if not _anchored(root, root_identity, root_fd, phase, phase_identity, output_fd, output_identity): raise GateError("EVIDENCE_INCOMPLETE")
            item["output_count"] = _outputs(phase_root / "output")
            if not _anchored(root, root_identity, root_fd, phase, phase_identity, output_fd, output_identity): raise GateError("EVIDENCE_INCOMPLETE")
            records[phase] = item
        result = record(classify(records["prime"], records["replay"]), records["prime"], records["replay"])
    except GateError as error:
        result = record(error.classification)
    except Exception:
        result = record("EVIDENCE_INCOMPLETE")
    finally:
        os.umask(old_umask); okay = True
        if root is not None and root_fd is not None and root_identity is not None:
            for phase, (phase_fd, phase_identity, output_fd, output_identity) in phases.items():
                if not _anchored(root, root_identity, root_fd, phase, phase_identity, output_fd, output_identity):
                    okay = False; continue
                okay &= _shutdown(bazel, root / phase / "output", runner)
                if not _anchored(root, root_identity, root_fd, phase, phase_identity, output_fd, output_identity): okay = False
            for phase_fd, _, output_fd, _ in phases.values(): os.close(output_fd); os.close(phase_fd)
            if parent_fd is not None: okay &= _remove_original(parent_fd, root_fd, root_identity)
            else: okay = False
            os.close(root_fd); root_fd = None
        elif root_fd is not None:
            os.close(root_fd); root_fd = None
            okay = False
        if root is not None and parent_fd is not None:
            okay &= _remove_reserved(root, parent_fd)
        elif root is not None:
            okay = False
        if parent_fd is not None: os.close(parent_fd)
        if root is not None and not _clean(): okay = False
        if not okay: result = record("SANITIZER_REJECTED")
    return result
