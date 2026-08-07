"""Fail-closed metadata-only BuildBuddy cache-prime artifact probe."""
from __future__ import annotations

import json
import os
import secrets
import stat
import subprocess
import tempfile
from pathlib import Path
from typing import Any, Callable

from tools.v2_oracle_lib import buildbuddy_build_cache as cache
from tools.v2_oracle_lib import buildbuddy_prime_diagnostic as cleanup

REPO_ROOT = cache.REPO_ROOT
MODE = "buildbuddy-build-cache-prime-artifact-probe"
CLASSES = frozenset(("PROBE_RECORDED", "SANITIZER_REJECTED"))
METADATA = frozenset(("PRIVATE_REGULAR", "NOT_PRIVATE_REGULAR"))


class ProbeError(Exception):
    pass


def record(classification: str = "SANITIZER_REJECTED", process: str = "NONZERO", bep: str = "NOT_PRIVATE_REGULAR", execution: str = "NOT_PRIVATE_REGULAR") -> dict[str, object]:
    if classification not in CLASSES or process not in ("ZERO", "NONZERO") or bep not in METADATA or execution not in METADATA:
        classification, process, bep, execution = "SANITIZER_REJECTED", "NONZERO", "NOT_PRIVATE_REGULAR", "NOT_PRIVATE_REGULAR"
    if classification == "SANITIZER_REJECTED":
        process, bep, execution = "NONZERO", "NOT_PRIVATE_REGULAR", "NOT_PRIVATE_REGULAR"
    return {"schema_version": 1, "mode": MODE, "classification": classification, "process": process, "bep": bep, "execution": execution}


def normalize(value: object) -> dict[str, object]:
    if not isinstance(value, dict) or set(value) != {"schema_version", "mode", "classification", "process", "bep", "execution"}:
        return record()
    if type(value["schema_version"]) is not int or value["schema_version"] != 1 or not all(isinstance(value[key], str) for key in ("mode", "classification", "process", "bep", "execution")):
        return record()
    result = record(value["classification"], value["process"], value["bep"], value["execution"])
    return result if result == value else record()


def _private(path: Path) -> tuple[int, int]:
    cleanup._private_file(path)
    item = path.lstat()
    if not stat.S_ISREG(item.st_mode) or item.st_nlink != 1 or stat.S_IMODE(item.st_mode) != 0o600:
        raise ProbeError
    return item.st_dev, item.st_ino


def _metadata(directory_fd: int, name: str, identity: tuple[int, int]) -> str:
    """Classify a retained file without opening it or reading any bytes."""
    try:
        item = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
        private = stat.S_ISREG(item.st_mode) and item.st_nlink == 1 and stat.S_IMODE(item.st_mode) == 0o600 and item.st_size > 0
        return "PRIVATE_REGULAR" if private and (item.st_dev, item.st_ino) == identity else "NOT_PRIVATE_REGULAR"
    except OSError:
        return "NOT_PRIVATE_REGULAR"


def _anchored(root: Path, root_fd: int, identity: tuple[int, int], phase_identity: tuple[int, int] | None = None) -> bool:
    try:
        item = root.lstat()
        opened = os.fstat(root_fd)
        root_ok = stat.S_ISDIR(item.st_mode) and (item.st_dev, item.st_ino) == identity == (opened.st_dev, opened.st_ino)
        if phase_identity is None: return root_ok
        phase = os.stat("prime", dir_fd=root_fd, follow_symlinks=False)
        return root_ok and stat.S_ISDIR(phase.st_mode) and (phase.st_dev, phase.st_ino) == phase_identity
    except OSError:
        return False


def _clean() -> bool:
    return cleanup._clean_git() and cleanup._no_slugd()


def _shutdown(bazel: str, output: Path, runner: Callable[..., subprocess.CompletedProcess[bytes]]) -> bool:
    try:
        return runner([bazel, "--ignore_all_rc_files", f"--output_base={output}", "shutdown"], cwd=REPO_ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False).returncode == 0
    except Exception:
        return False


def _remove_original(parent_fd: int, root_fd: int, identity: tuple[int, int]) -> bool:
    def clear(directory_fd: int) -> None:
        for name in os.listdir(directory_fd):
            item = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
            if stat.S_ISDIR(item.st_mode):
                child_fd = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=directory_fd)
                try:
                    opened = os.fstat(child_fd); os.fchmod(child_fd, stat.S_IMODE(opened.st_mode) | stat.S_IRWXU); clear(child_fd)
                finally: os.close(child_fd)
                current = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
                if (current.st_dev, current.st_ino) != (opened.st_dev, opened.st_ino): raise OSError
                os.rmdir(name, dir_fd=directory_fd)
            else: os.unlink(name, dir_fd=directory_fd)
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


def run_probe(bazel: str = "bazel", runner: Callable[..., subprocess.CompletedProcess[bytes]] = subprocess.run) -> dict[str, object]:
    old_umask, root, parent_fd, root_fd, phase_fd, root_identity, phase_identity, outcome = os.umask(0o077), None, None, None, None, None, None, record()
    try:
        if not _clean(): raise ProbeError
        root = Path(tempfile.mkdtemp(prefix="slug-buildbuddy-prime-"))
        if stat.S_IMODE(root.stat().st_mode) != 0o700: raise ProbeError
        try: root.resolve().relative_to(REPO_ROOT.resolve()); raise ProbeError
        except ValueError: pass
        parent_fd = os.open(root.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        root_fd = os.open(root.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd)
        item = os.fstat(root_fd); root_identity = (item.st_dev, item.st_ino)
        phase = root / "prime"; phase.mkdir()
        phase_fd = os.open("prime", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=root_fd)
        item = os.fstat(phase_fd); phase_identity = (item.st_dev, item.st_ino)
        stdout, stderr, bep, execution = (phase / name for name in ("stdout", "stderr", "bep.json", "execution.json"))
        identities = {path.name: _private(path) for path in (stdout, stderr, bep, execution)}
        with stdout.open("ab") as out, stderr.open("ab") as err:
            done = runner(cache.command("prime", bazel, phase / "output", bep, execution, secrets.token_hex(32)), cwd=REPO_ROOT, stdout=out, stderr=err, check=False)
        if not _anchored(root, root_fd, root_identity, phase_identity): raise ProbeError
        process = "ZERO" if isinstance(done.returncode, int) and not isinstance(done.returncode, bool) and done.returncode == 0 else "NONZERO"
        outcome = record("PROBE_RECORDED", process, _metadata(phase_fd, bep.name, identities[bep.name]), _metadata(phase_fd, execution.name, identities[execution.name]))
    except Exception:
        outcome = record()
    finally:
        os.umask(old_umask); okay = root is not None and root_fd is not None and root_identity is not None
        if okay and _anchored(root, root_fd, root_identity, phase_identity): okay = _shutdown(bazel, root / "prime" / "output", runner) and _anchored(root, root_fd, root_identity, phase_identity)
        else: okay = False
        if phase_fd is not None: os.close(phase_fd)
        if parent_fd is not None and root_fd is not None and root_identity is not None: okay &= _remove_original(parent_fd, root_fd, root_identity)
        else: okay = False
        if root_fd is not None: os.close(root_fd)
        if parent_fd is not None: os.close(parent_fd)
        if not _clean(): okay = False
        if not okay: outcome = record()
    return outcome
