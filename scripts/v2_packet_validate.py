#!/usr/bin/env python3
"""Serialize selected Slug V2 oracle fixture validation for one packet."""
from __future__ import annotations

import argparse
import fcntl
import json
import os
import subprocess
import sys
import tempfile
from contextlib import contextmanager
from pathlib import Path
from typing import Iterator


REPO_ROOT = Path(__file__).resolve().parents[1]
FIXTURES_ROOT = REPO_ROOT / "tests" / "v2_oracle" / "fixtures"
UNIX_SOCKET_PATH_MAX = 107

if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tools.v2_oracle_lib.fixture import Fixture, discover_fixtures


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Build Slug once and run selected V2 oracle fixtures serially."
    )
    parser.add_argument("--fixture", action="append", required=True, metavar="NAME")
    parser.add_argument("--timeout", type=int, default=120, metavar="SECONDS")
    return parser


def _validate_selection(names: list[str]) -> list[Fixture]:
    fixtures = {fixture.name: fixture for fixture in discover_fixtures(FIXTURES_ROOT)}
    selected: list[Fixture] = []
    failures: list[str] = []
    seen: set[str] = set()
    for name in names:
        if name in seen:
            failures.append(f"duplicate fixture selection: {name}")
            continue
        seen.add(name)
        fixture = fixtures.get(name)
        if fixture is None:
            failures.append(f"unknown fixture: {name}")
            continue
        try:
            expected = json.loads(fixture.expected_oracle.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            failures.append(f"{name}: cannot read expected/oracle.json: {exc}")
            continue
        if not isinstance(expected, dict):
            failures.append(f"{name}: expected/oracle.json must be a JSON object")
            continue
        if expected.get("generated") is not True:
            failures.append(f"{name}: expected/oracle.json must have generated=true")
            continue
        selected.append(fixture)
    if failures:
        raise ValueError("\n".join(failures))
    return selected


def _target_dir() -> Path:
    configured = os.environ.get("CARGO_TARGET_DIR")
    if not configured:
        return REPO_ROOT / "target"
    candidate = Path(configured)
    return candidate if candidate.is_absolute() else REPO_ROOT / candidate


def _slug_binary() -> Path:
    return _target_dir() / "debug" / "slug"


def _unique_run_root() -> Path:
    root = REPO_ROOT / "target" / "v2p"
    root.mkdir(parents=True, exist_ok=True)
    return Path(tempfile.mkdtemp(prefix="p", dir=root))


def _daemon_socket_path(run_root: Path, fixture: Fixture) -> Path:
    return run_root / "ob" / fixture.name / "slug" / "slugd.sock"


def _validate_daemon_socket_paths(selected: list[Fixture], run_root: Path) -> list[str]:
    failures: list[str] = []
    for fixture in selected:
        if not fixture.daemon:
            continue
        socket_path = _daemon_socket_path(run_root, fixture)
        if len(os.fsencode(socket_path)) > UNIX_SOCKET_PATH_MAX:
            failures.append(
                f"{fixture.name}: daemon socket path exceeds {UNIX_SOCKET_PATH_MAX} bytes: {socket_path}"
            )
    return failures


@contextmanager
def _packet_lock() -> Iterator[None]:
    lock_path = REPO_ROOT / "target" / "v2_packet_validate" / "packet.lock"
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    with lock_path.open("a+", encoding="utf-8") as lock_file:
        try:
            fcntl.flock(lock_file.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as exc:
            raise RuntimeError("another V2 packet validation is already running") from exc
        try:
            yield
        finally:
            fcntl.flock(lock_file.fileno(), fcntl.LOCK_UN)


def _build_slug() -> bool:
    env = os.environ.copy()
    env["CARGO_BUILD_JOBS"] = "1"
    try:
        completed = subprocess.run(
            ["cargo", "build", "-p", "slug_cli_v2"],
            cwd=REPO_ROOT,
            env=env,
            check=False,
        )
    except OSError:
        return False
    return completed.returncode == 0


def _daemon_markers(run_root: Path) -> list[Path]:
    return sorted(
        path for name in ("slugd.sock", "slugd.pid") for path in run_root.rglob(name)
    )


def _run_fixture(fixture: Fixture, slug: Path, run_root: Path, timeout: int) -> int | None:
    try:
        return subprocess.run(
            [
                sys.executable,
                "-B",
                "-m",
                "tools.v2_oracle",
                "run",
                "--tool",
                "slug",
                "--slug",
                str(slug),
                "--run-root",
                str(run_root),
                "--timeout",
                str(timeout),
                "--fixture",
                fixture.name,
            ],
            cwd=REPO_ROOT,
            check=False,
        ).returncode
    except OSError:
        return None


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        selected = _validate_selection(args.fixture)
    except (OSError, ValueError) as exc:
        print(f"selection failed: {exc}", file=sys.stderr)
        return 2

    try:
        with _packet_lock():
            run_root = _unique_run_root()
            print(f"artifacts: {run_root}")
            daemon_path_failures = _validate_daemon_socket_paths(selected, run_root)
            if daemon_path_failures:
                for failure in daemon_path_failures:
                    print(failure, file=sys.stderr)
                print(f"result: failed ({len(daemon_path_failures)} failure(s))")
                return 1
            if not _build_slug():
                print("Slug build failed", file=sys.stderr)
                return 1
            slug = _slug_binary()
            if not slug.is_file() or not os.access(slug, os.X_OK):
                print(f"Slug binary is missing or not executable: {slug}", file=sys.stderr)
                return 1

            failures: list[str] = []
            for fixture in selected:
                exit_code = _run_fixture(fixture, slug, run_root, args.timeout)
                if exit_code is None:
                    failures.append(f"{fixture.name}: oracle could not start")
                elif exit_code != 0:
                    failures.append(f"{fixture.name}: oracle failed")
                markers = _daemon_markers(run_root)
                if markers:
                    failures.extend(f"{fixture.name}: leftover daemon marker: {marker}" for marker in markers)

            if failures:
                for failure in failures:
                    print(failure, file=sys.stderr)
                print(f"result: failed ({len(failures)} failure(s))")
                return 1
            print(f"result: ok ({len(selected)} fixture(s))")
            return 0
    except RuntimeError as exc:
        print(f"validation lock failed: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
