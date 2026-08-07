"""Fail-closed, bounded normal-RC BuildBuddy prime diagnostic."""
from __future__ import annotations

import os
import secrets
import stat
import subprocess
import tempfile
from pathlib import Path
from typing import Callable

from tools.v2_oracle_lib import buildbuddy_cache

REPO_ROOT = buildbuddy_cache.REPO_ROOT
MAX_STDERR_BYTES = 65_536
LABELS = ("//app/slug_cli_v2:slug",)
NONE = "NONE"
CLASSIFICATIONS = frozenset(("NORMAL_RC_PRIME_DIAGNOSED", "NORMAL_RC_PRIME_UNEXPLAINED", "SANITIZER_REJECTED"))
MAPPINGS = {
    "--noremote_local_fallback": "CHECKED_IN_OPTION_NOREMOTE_LOCAL_FALLBACK",
    "--build_event_publish_all_actions": "CHECKED_IN_OPTION_BUILD_EVENT_PUBLISH_ALL_ACTIONS",
    "--noremote_accept_cached": "CHECKED_IN_OPTION_NOREMOTE_ACCEPT_CACHED",
    "--remote_upload_local_results": "CHECKED_IN_OPTION_REMOTE_UPLOAD_LOCAL_RESULTS",
    "--noremote_cache_async": "CHECKED_IN_OPTION_NOREMOTE_CACHE_ASYNC",
}


def record(classification: str, diagnosis: str = NONE) -> dict[str, object]:
    if classification not in CLASSIFICATIONS:
        classification, diagnosis = "SANITIZER_REJECTED", NONE
    elif classification == "NORMAL_RC_PRIME_DIAGNOSED" and diagnosis not in MAPPINGS.values():
        classification, diagnosis = "SANITIZER_REJECTED", NONE
    elif classification != "NORMAL_RC_PRIME_DIAGNOSED":
        diagnosis = NONE
    return {"schema_version": 1, "classification": classification, "diagnosis": diagnosis}


def normalize(value: object) -> dict[str, object]:
    if not isinstance(value, dict) or set(value) != {"schema_version", "classification", "diagnosis"}:
        return record("SANITIZER_REJECTED")
    if value.get("schema_version") != 1 or not isinstance(value.get("classification"), str) or not isinstance(value.get("diagnosis"), str):
        return record("SANITIZER_REJECTED")
    normalized = record(value["classification"], value["diagnosis"])
    return normalized if normalized == value else record("SANITIZER_REJECTED")


def sanitize(stderr: Path) -> str:
    """Return an allowlisted diagnosis, or NONE without exposing terminal bytes."""
    try:
        fd = os.open(stderr, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW)
        with os.fdopen(fd, "rb") as file:
            metadata = os.fstat(file.fileno())
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_size > MAX_STDERR_BYTES:
                return NONE
            data = file.read(metadata.st_size)
            if len(data) != metadata.st_size:
                return NONE
        text = data.decode("utf-8")
    except (OSError, UnicodeDecodeError):
        return NONE
    for flag, diagnosis in MAPPINGS.items():
        payload = f"ERROR: {flag} :: Unrecognized option: {flag}"
        if text in (payload, payload + "\n", payload + "\r\n"):
            return diagnosis
    return NONE


def _private_file(path: Path) -> None:
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    os.close(fd)
    if stat.S_IMODE(path.stat().st_mode) != 0o600:
        raise OSError


def _clean_git() -> bool:
    try:
        result = subprocess.run(
            ["git", "status", "--porcelain"], cwd=REPO_ROOT,
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False,
        )
        return result.returncode == 0 and result.stdout == b""
    except OSError:
        return False


def _no_slugd() -> bool:
    try:
        result = subprocess.run(
            ["pgrep", "-x", "slugd"], stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL, check=False,
        )
        return result.returncode == 1
    except OSError:
        return False


def _shutdown(bazel: str, output_base: Path, runner: Callable[..., subprocess.CompletedProcess[bytes]]) -> bool:
    try:
        result = runner(
            [bazel, "--ignore_all_rc_files", f"--output_base={output_base}", "shutdown"],
            cwd=REPO_ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False,
        )
        return result.returncode == 0
    except Exception:
        return False


def _remove_root(root: Path) -> bool:
    if not root.name.startswith("slug-buildbuddy-prime-") or root.parent.resolve() != Path(tempfile.gettempdir()).resolve():
        return False
    parent_fd = root_fd = None

    def same_directory(parent: int, name: str, opened: os.stat_result) -> bool:
        current = os.stat(name, dir_fd=parent, follow_symlinks=False)
        return stat.S_ISDIR(current.st_mode) and (current.st_dev, current.st_ino) == (opened.st_dev, opened.st_ino)

    def clear(directory_fd: int) -> None:
        for name in os.listdir(directory_fd):
            metadata = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
            if stat.S_ISDIR(metadata.st_mode):
                child_fd = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=directory_fd)
                try:
                    child = os.fstat(child_fd)
                    os.fchmod(child_fd, stat.S_IMODE(child.st_mode) | stat.S_IRUSR | stat.S_IWUSR | stat.S_IXUSR)
                    clear(child_fd)
                finally:
                    os.close(child_fd)
                if not same_directory(directory_fd, name, child):
                    raise OSError
                os.rmdir(name, dir_fd=directory_fd)
            else:
                os.unlink(name, dir_fd=directory_fd)

    try:
        parent_fd = os.open(root.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        root_fd = os.open(root.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd)
        original = os.fstat(root_fd)
        os.fchmod(root_fd, stat.S_IMODE(original.st_mode) | stat.S_IRUSR | stat.S_IWUSR | stat.S_IXUSR)
        clear(root_fd)
        current = os.stat(root.name, dir_fd=parent_fd, follow_symlinks=False)
        if not stat.S_ISDIR(current.st_mode) or (current.st_dev, current.st_ino) != (original.st_dev, original.st_ino):
            return False
        os.rmdir(root.name, dir_fd=parent_fd)
        try:
            os.stat(root.name, dir_fd=parent_fd, follow_symlinks=False)
        except FileNotFoundError:
            return True
        return False
    except Exception:
        return False
    finally:
        if root_fd is not None:
            os.close(root_fd)
        if parent_fd is not None:
            os.close(parent_fd)


def run(bazel: str = "bazel", runner: Callable[..., subprocess.CompletedProcess[bytes]] = subprocess.run) -> dict[str, object]:
    """Run one prime only; terminal, BEP, and execution outputs are never exposed."""
    old_umask = os.umask(0o077)
    root: Path | None = None
    output_base: Path | None = None
    outcome = record("SANITIZER_REJECTED")
    try:
        if not _clean_git() or not _no_slugd():
            return outcome
        root = Path(tempfile.mkdtemp(prefix="slug-buildbuddy-prime-"))
        if stat.S_IMODE(root.stat().st_mode) != 0o700:
            return outcome
        try:
            root.resolve().relative_to(REPO_ROOT.resolve())
            return outcome
        except ValueError:
            pass
        output_base = root / "output"
        stderr, stdout, bep, execution = root / "stderr", root / "stdout", root / "bep.json", root / "execution.json"
        for path in (stderr, stdout, bep, execution):
            _private_file(path)
        argv = buildbuddy_cache.command("prime", bazel, output_base, bep, execution, secrets.token_hex(32), LABELS)
        with stdout.open("ab") as stdout_file, stderr.open("ab") as stderr_file:
            result = runner(argv, cwd=REPO_ROOT, stdout=stdout_file, stderr=stderr_file, check=False)
        if result.returncode != 2:
            outcome = record("NORMAL_RC_PRIME_UNEXPLAINED")
        else:
            diagnosis = sanitize(stderr)
            outcome = record("NORMAL_RC_PRIME_DIAGNOSED", diagnosis) if diagnosis != NONE else record("SANITIZER_REJECTED")
    except Exception:
        outcome = record("SANITIZER_REJECTED")
    finally:
        os.umask(old_umask)
        cleanup_ok = root is not None
        if output_base is not None:
            cleanup_ok = _shutdown(bazel, output_base, runner)
        if root is not None:
            if not _remove_root(root):
                cleanup_ok = False
        if not _clean_git() or not _no_slugd():
            cleanup_ok = False
        if not cleanup_ok:
            outcome = record("SANITIZER_REJECTED")
    return outcome
