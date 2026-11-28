"""Compare public Slug Rust cquery against the pinned Bazel CrateInfo rows."""

from __future__ import annotations

import argparse
from collections.abc import Callable
import json
from pathlib import Path
import shutil
import subprocess
import sys

from tools.v2_oracle import rust_toolchain_fixture
from tools.v2_oracle_lib.bounded_process import capture
from tools.v2_oracle_lib.compare import load_expected
from tools.v2_oracle_lib.fixture import FixtureCommand, load_fixture
from tools.v2_oracle_lib.runner import _apply_mutations

CASES = (
    "crate_info_edition_2024",
    "warm_crate_info_edition_2024",
    "crate_info_edition_2021",
    "crate_info_restored_edition_2024",
)


def _cases() -> tuple[tuple[FixtureCommand, bytes], ...]:
    fixture = load_fixture(rust_toolchain_fixture.FIXTURE_ROOT)
    expected = load_expected(fixture.expected_oracle)
    if fixture.provenance.bazel_release != "9.2.0" or not expected or expected["tool"] != "bazel":
        raise ValueError("Rust comparison requires the pinned Bazel 9.2.0 oracle")
    if fixture.env:
        raise ValueError("Rust comparison fixture environment changed")
    commands = [command for command in fixture.commands if command.name in CASES]
    rows = [row for row in expected["commands"] if row["name"] in CASES]
    if tuple(command.name for command in commands) != CASES or tuple(row["name"] for row in rows) != CASES:
        raise ValueError("Rust CrateInfo command or oracle order changed")
    result = []
    for command, row in zip(commands, rows, strict=True):
        if (
            command.argv[0] != "cquery"
            or command.expected_exit != 0
            or command.compare != "exact"
            or command.env
            or command.startup_argv
            or command.argv != tuple(row["argv"])
            or row["exit_code"] != 0
            or not row["stdout"]
        ):
            raise ValueError(f"{command.name}: invalid exact cquery comparison contract")
        result.append((command, row["stdout"].encode("utf-8")))
    return tuple(result)


def _run_lane(
    directory: Path,
    inputs: dict,
    cases: tuple[tuple[FixtureCommand, bytes], ...],
    startup: list[str],
    timeout: float,
    sample_daemon: Callable[[], int] | None = None,
) -> None:
    directory.mkdir()
    workspace = directory / "workspace"
    shutil.copytree(inputs["workspace"], workspace)
    repositories = [
        f"--registry={inputs['registry_uri']}",
        *(f"--distdir={path}" for path in inputs["distdirs"]),
    ]
    for command, expected in cases:
        _apply_mutations(workspace, command)
        result = capture(
            [*startup, *command.argv, *repositories],
            cwd=workspace, timeout=timeout, log_prefix=directory / command.name,
        )
        if result.returncode != 0 or result.stdout != expected or result.stderr:
            raise RuntimeError(
                f"{directory.name}/{command.name}: expected exit 0, exact CrateInfo stdout "
                f"and empty Slug stderr, observed exit {result.returncode}; see {directory}"
            )
        if sample_daemon is not None:
            sample_daemon()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--slug", type=Path, required=True, help="absolute path to the built CLI")
    parser.add_argument("--cache-root", type=Path, required=True, help="pinned Bazel archive CAS sha256 directory")
    parser.add_argument("--output", type=Path, required=True, help="fresh destination in a protected parent")
    parser.add_argument("--timeout", type=int, default=120, help="deadline per cquery in seconds")
    args = parser.parse_args()
    if not args.slug.is_absolute() or args.timeout <= 0:
        parser.error("--slug must be absolute and --timeout must be positive")

    from tools.v2_oracle.installed_daemon_witness import InstalledDaemon

    try:
        slug = args.slug.resolve(strict=True)
        cases = _cases()
        destination = args.output.absolute()
        output_base = destination / "o"
        managed = InstalledDaemon(output_base, slug)
        destination.mkdir(mode=0o700)
        inputs = rust_toolchain_fixture.assemble(args.cache_root, destination / "inputs")
        _run_lane(destination / "one-shot", inputs, cases, [str(slug)], args.timeout)
        with managed as daemon:
            _run_lane(
                destination / "daemon", inputs, cases,
                [str(slug), f"--output_base={output_base}"], args.timeout, daemon.sample,
            )
        print(json.dumps({
            "fixture": rust_toolchain_fixture.FIXTURE_ROOT.name,
            "cases_per_route": len(cases),
            "routes": ["one-shot", "daemon"],
            "status": "ok",
        }))
        return 0
    except (OSError, ValueError, KeyError, TypeError, RuntimeError, subprocess.TimeoutExpired) as error:
        print(f"Rust cquery comparison failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
