"""Fail-closed lifecycle guard for one frozen prime output-semantics child."""
from __future__ import annotations

import json
import os
import stat
import subprocess
import tempfile
from pathlib import Path
from typing import Callable

from tools.v2_oracle_lib import buildbuddy_build_cache_prime_output_semantics_probe as child_probe
from tools.v2_oracle_lib import buildbuddy_build_cache_artifact_probe as lifecycle
from tools.v2_oracle_lib import buildbuddy_prime_diagnostic as cleanup

REPO_ROOT = Path(__file__).resolve().parents[2]
MODE = "buildbuddy-build-cache-prime-lifecycle-guard"
PREFIX = "slug-buildbuddy-prime-"
MAX_STDOUT_BYTES = 2 * 1024
CLASSIFICATIONS = frozenset(("LIFECYCLE_RECORDED", "SANITIZER_REJECTED"))
LIFECYCLES = frozenset((
    "NOT_RECORDED", "PRECHECK_REJECTED", "CHILD_REJECTED",
    "ROOT_RESIDUE_REMOVED", "ROOT_RESIDUE_REJECTED", "POSTCHECK_REJECTED",
    "LIFECYCLE_CLEAN",
))
RECORDED_LIFECYCLES = LIFECYCLES - {"NOT_RECORDED"}


def _child_default() -> dict[str, object]:
    return child_probe.record()


def record(
    classification: str = "SANITIZER_REJECTED", lifecycle: str = "NOT_RECORDED",
    child: object | None = None,
) -> dict[str, object]:
    normalized = child_probe.normalize(_child_default() if child is None else child)
    valid = type(classification) is str and type(lifecycle) is str
    if classification == "SANITIZER_REJECTED":
        if not (valid and lifecycle == "NOT_RECORDED" and normalized == _child_default()):
            classification, lifecycle, normalized = "SANITIZER_REJECTED", "NOT_RECORDED", _child_default()
    elif valid and classification == "LIFECYCLE_RECORDED" and lifecycle in RECORDED_LIFECYCLES:
        if lifecycle != "LIFECYCLE_CLEAN" or normalized["classification"] != "STAGE_RECORDED":
            normalized = _child_default()
    else:
        classification, lifecycle, normalized = "SANITIZER_REJECTED", "NOT_RECORDED", _child_default()
    return {
        "schema_version": 1, "mode": MODE, "classification": classification,
        "lifecycle": lifecycle, "child": normalized,
    }


def normalize(value: object) -> dict[str, object]:
    if type(value) is not dict or set(value) != set(record()):
        return record()
    if type(value.get("schema_version")) is not int or value.get("schema_version") != 1 or type(value.get("mode")) is not str or value.get("mode") != MODE:
        return record()
    if type(value.get("classification")) is not str or type(value.get("lifecycle")) is not str:
        return record()
    result = record(value["classification"], value["lifecycle"], value.get("child"))
    return result if result == value else record()


def _clean() -> bool:
    try:
        git_clean = cleanup._clean_git()
    except Exception:
        git_clean = False
    try:
        no_daemon = cleanup._no_slugd()
    except Exception:
        no_daemon = False
    return git_clean and no_daemon


def _roots() -> tuple[tuple[Path, tuple[int, int]], ...] | None:
    """Return only direct, real reserved roots; never open or inspect contents."""
    try:
        roots: list[tuple[Path, tuple[int, int]]] = []
        with os.scandir(tempfile.gettempdir()) as entries:
            for entry in entries:
                if entry.name.startswith(PREFIX):
                    item = entry.stat(follow_symlinks=False)
                    if not stat.S_ISDIR(item.st_mode):
                        return None
                    roots.append((Path(entry.path), (item.st_dev, item.st_ino)))
        return tuple(sorted(roots))
    except Exception:
        return None


def _safe_roots() -> tuple[tuple[Path, tuple[int, int]], ...] | None:
    try:
        return _roots()
    except Exception:
        return None


def _remove_original(root: Path, identity: tuple[int, int]) -> bool:
    """Remove exactly the scanned root, never a replacement at its name."""
    parent_fd = root_fd = None
    try:
        parent_fd = os.open(root.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        root_fd = os.open(root.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd)
        opened = os.fstat(root_fd)
        if (opened.st_dev, opened.st_ino) != identity:
            return False
        return lifecycle._remove_original(parent_fd, root_fd, identity)
    except Exception:
        return False
    finally:
        if root_fd is not None:
            os.close(root_fd)
        if parent_fd is not None:
            os.close(parent_fd)


def _read_stdout(stream: object) -> dict[str, object] | None:
    try:
        stream.seek(0)
        data = stream.read(MAX_STDOUT_BYTES + 1)
        if type(data) is not bytes or len(data) > MAX_STDOUT_BYTES:
            return None
        value = json.loads(data.decode("utf-8"))
    except (AttributeError, OSError, UnicodeDecodeError, json.JSONDecodeError):
        return None
    normalized = child_probe.normalize(value)
    expected = (json.dumps(normalized, sort_keys=True, separators=(",", ":")) + "\n").encode()
    return normalized if data == expected else None


def _empty(stream: object) -> bool:
    try:
        stream.seek(0)
        return stream.read(1) == b""
    except (AttributeError, OSError):
        return False


def run_guard(runner: Callable[..., subprocess.CompletedProcess[bytes]] = subprocess.run) -> dict[str, object]:
    """Run one child, retaining only its strict normalized fixed record."""
    outcome = record()
    before = _safe_roots()
    if not _clean() or before != ():
        outcome = record("LIFECYCLE_RECORDED", "PRECHECK_REJECTED")
    else:
        child = _child_default()
        try:
            with tempfile.TemporaryFile("w+b") as stdout, tempfile.TemporaryFile("w+b") as stderr:
                done = runner(
                    ["python3", str(REPO_ROOT / "tools/v2_oracle/buildbuddy_build_cache_prime_output_semantics_probe.py")],
                    cwd=REPO_ROOT, stdout=stdout, stderr=stderr, check=False, shell=False,
                )
                parsed = _read_stdout(stdout)
                child_ok = (
                    type(done.returncode) is int and done.returncode == 0
                    and _empty(stderr) and parsed is not None
                    and parsed["classification"] == "STAGE_RECORDED"
                )
                if child_ok:
                    child = parsed
                else:
                    outcome = record("LIFECYCLE_RECORDED", "CHILD_REJECTED")
        except Exception:
            outcome = record("LIFECYCLE_RECORDED", "CHILD_REJECTED")
        after = _safe_roots()
        if after is None or len(after) > 1:
            outcome = record("LIFECYCLE_RECORDED", "ROOT_RESIDUE_REJECTED")
        elif len(after) == 1:
            try:
                removed = _remove_original(*after[0])
            except Exception:
                removed = False
            survivor = _safe_roots()
            outcome = record(
                "LIFECYCLE_RECORDED",
                "ROOT_RESIDUE_REMOVED" if outcome["lifecycle"] == "NOT_RECORDED" and removed and survivor == () else "ROOT_RESIDUE_REJECTED",
            )
        elif outcome["lifecycle"] == "NOT_RECORDED":
            outcome = record("LIFECYCLE_RECORDED", "LIFECYCLE_CLEAN", child)
    if not _clean():
        outcome = record("LIFECYCLE_RECORDED", "POSTCHECK_REJECTED")
    return normalize(outcome)
