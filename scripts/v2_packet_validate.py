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
        description="Run selected V2 oracle fixtures serially."
    )
    parser.add_argument("--fixture", action="append", required=True, metavar="NAME")
    parser.add_argument("--timeout", type=int, default=120, metavar="SECONDS")
    parser.add_argument(
        "--installed-slug", type=Path, metavar="PATH",
        help="installed Slug executable to verify",
    )
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


def _built_slug_is_installed() -> bool:
    return sys.platform == "linux" and os.uname().machine == "x86_64" and sys.maxsize > 2**32


def _unique_run_root(*, installed: bool) -> Path:
    if installed:
        return Path(tempfile.mkdtemp(prefix="v2p.")).resolve()
    root = REPO_ROOT / "target" / "v2p"
    root.mkdir(parents=True, exist_ok=True)
    return Path(tempfile.mkdtemp(prefix="p", dir=root))


def _slug_output_base(run_root: Path, fixture: Fixture) -> Path:
    return run_root / "ob" / fixture.name / "slug"


def _validate_daemon_socket_paths(
    selected: list[Fixture], run_root: Path, *, installed: bool
) -> list[str]:
    failures: list[str] = []
    for fixture in selected:
        if not fixture.daemon:
            continue
        output_base = _slug_output_base(run_root, fixture)
        socket_path = (
            output_base / ".slugd-installed" / "stage" / "s"
            if installed
            else output_base / "slugd.sock"
        )
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


def _daemon_markers(run_root: Path, fixture: Fixture, *, installed: bool) -> list[Path]:
    base = _slug_output_base(run_root, fixture)
    generic = ("slugd.sock", "slugd.pid")
    markers = [
        base / ".slugd-installed" / generation for generation in ("live", "stage")
    ]
    if installed:
        markers.extend(base / name for name in generic)
    else:
        markers.extend(path for name in generic for path in run_root.rglob(name))
    return sorted(path for path in markers if os.path.lexists(path))


def _run_fixture(
    fixture: Fixture, slug: Path, run_root: Path, timeout: int, *, installed: bool
) -> int | None:
    try:
        environment = None
        if installed:
            # Prepare scratch before the subprocess observes its workspace ancestors.
            scratch = tempfile.mkdtemp(prefix="scratch.", dir=run_root)
            environment = {**os.environ, "TMPDIR": scratch}
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
                *(["--slug-daemon-route=installed"] if installed else []),
                "--run-root",
                str(run_root),
                "--timeout",
                str(timeout),
                "--fixture",
                fixture.name,
            ],
            cwd=REPO_ROOT,
            env=environment,
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
            supplied_slug = args.installed_slug
            installed = supplied_slug is not None or _built_slug_is_installed()
            run_root = _unique_run_root(installed=installed)
            print(f"artifacts: {run_root}")
            daemon_path_failures = _validate_daemon_socket_paths(
                selected, run_root, installed=installed
            )
            if daemon_path_failures:
                for failure in daemon_path_failures:
                    print(failure, file=sys.stderr)
                print(f"result: failed ({len(daemon_path_failures)} failure(s))")
                return 1
            if supplied_slug is None and not _build_slug():
                print("Slug build failed", file=sys.stderr)
                return 1
            slug = supplied_slug.resolve() if supplied_slug is not None else _slug_binary()
            if not slug.is_file() or not os.access(slug, os.X_OK):
                print(f"Slug binary is missing or not executable: {slug}", file=sys.stderr)
                return 1

            failures: list[str] = []
            for fixture in selected:
                exit_code = _run_fixture(
                    fixture, slug, run_root, args.timeout, installed=installed
                )
                if exit_code is None:
                    failures.append(f"{fixture.name}: oracle could not start")
                elif exit_code != 0:
                    failures.append(f"{fixture.name}: oracle failed")
                markers = _daemon_markers(run_root, fixture, installed=installed)
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
