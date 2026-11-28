"""Compare the pinned rules_rust aquery sequence against same-host Bazel text."""

from __future__ import annotations

import argparse
from collections.abc import Sequence
from dataclasses import dataclass, fields, replace
from itertools import zip_longest
import json
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
from typing import Literal

from tools.v2_oracle.installed_daemon_witness import InstalledDaemon
from tools.v2_oracle.rust_toolchain_fixture import FIXTURE_ROOT, assemble
from tools.v2_oracle_lib.bounded_process import capture
from tools.v2_oracle_lib.compare import load_expected
from tools.v2_oracle_lib.fixture import FixtureCommand, Mutation, load_fixture
from tools.v2_oracle_lib.runner import _apply_mutations

CASES = (
    "rust_actions_edition_2024",
    "warm_rust_actions_edition_2024",
    "rust_actions_edition_2021",
    "rust_actions_restored_edition_2024",
)
EDITIONS = ("2024", "2024", "2021", "2024")
_ARGV = (
    "aquery", 'mnemonic("(Rustc|SymlinkTree|RunfilesTree)", //pkg:probe)',
    "--output=text", "--noshow_progress",
    "--@rules_rust//rust/toolchain/channel=nightly", "--lockfile_mode=off",
)
_CAPTURE_LIMIT = 65_536
_HEX = r"[0-9a-f]{64}"
_CANONICAL_PLATFORM = "@@platforms//host:host"
_WRAPPER = "external/rules_rust+/util/process_wrapper/process_wrapper"
_ARG_PATH_PREFIXES = ("--out-dir=", "--emit=link=", "--extern=support=", "-Ldependency=", "--sysroot=")
_MNEMONICS = ("Rustc", "SymlinkTree", "RunfilesTree")


def load_cases() -> tuple[FixtureCommand, ...]:
    """Use the pinned invocations and apply each paired CrateInfo edit first."""
    fixture = load_fixture(FIXTURE_ROOT)
    expected = load_expected(fixture.expected_oracle)
    if fixture.provenance.bazel_release != "9.2.0" or not expected or expected["tool"] != "bazel":
        raise ValueError("Rust aquery requires the pinned Bazel 9.2.0 fixture")
    commands = [command for command in fixture.commands if command.name in CASES]
    reference = [row for row in expected["commands"] if row["name"] in CASES]
    if (
        fixture.env or tuple(command.name for command in commands) != CASES
        or tuple(row["name"] for row in reference) != CASES
    ):
        raise ValueError("Rust aquery fixture or row order changed")
    by_name = {command.name: command for command in fixture.commands}
    result = []
    for index, (command, row) in enumerate(zip(commands, reference, strict=True)):
        edit = by_name[command.name.replace("rust_actions", "crate_info")].mutations
        expected_edit = () if index < 2 else (Mutation(
            path="MODULE.bazel", find=f'edition = "{EDITIONS[index - 1]}"',
            replace=f'edition = "{EDITIONS[index]}"',
        ),)
        if (
            command.argv != _ARGV or tuple(row["argv"]) != _ARGV
            or command.expected_exit != 0 or row["exit_code"] != 0
            or command.env or command.startup_argv or command.mutations or edit != expected_edit
        ):
            raise ValueError(f"{command.name}: invalid pinned Rust aquery contract")
        result.append(replace(command, mutations=edit))
    return tuple(result)


@dataclass(frozen=True)
class _Artifact:
    domain: Literal["target", "exec", "external", "source"]
    path: str


@dataclass(frozen=True)
class _Paths:
    target: str
    exec: str

    def artifact(self, path: str) -> _Artifact:
        if path.startswith(self.target):
            return _Artifact("target", path[len(self.target):])
        if path.startswith(self.exec):
            return _Artifact("exec", path[len(self.exec):])
        if path.startswith("bazel-out/"):
            raise ValueError(f"unrecognized configured path {path!r}")
        return _Artifact("external" if path.startswith("external/") else "source", path)

    def argument(self, raw: str) -> str | tuple[str, _Artifact]:
        for prefix in _ARG_PATH_PREFIXES:
            if raw.startswith(prefix):
                path = self.artifact(raw[len(prefix):])
                if path.domain != "target":
                    raise ValueError(f"expected a target-configured {prefix} path")
                return prefix, path
        if raw.startswith("bazel-out/"):
            return "", self.artifact(raw)
        return raw


@dataclass(frozen=True)
class _Action:
    inputs: tuple[_Artifact, ...]
    outputs: tuple[_Artifact, ...]
    environment: tuple[str, ...] = ()
    argv: tuple[str | tuple[str, _Artifact], ...] = ()
    execution_info: str | None = None


@dataclass(frozen=True)
class _Row:
    actions: tuple[_Action, ...]
    action_keys: tuple[str, ...]
    rustc_configuration: str


def _value(line: str, prefix: str) -> str:
    if not line.startswith(prefix):
        raise ValueError(f"expected {prefix.strip()!r}")
    return line[len(prefix):]


def _list(line: str, field: str) -> tuple[str, ...]:
    raw = _value(line, f"  {field}: [")
    if not raw.endswith("]"):
        raise ValueError(f"unterminated {field}")
    return tuple(raw[:-1].split(", ")) if raw != "]" else ()


def _argv(lines: list[str]) -> tuple[str, ...]:
    result = []
    for index, line in enumerate(lines):
        prefix = "  Command Line: (exec " if index == 0 else "    "
        ending = ")" if index == len(lines) - 1 else " \\"
        raw = _value(line, prefix)
        if not raw.endswith(ending):
            raise ValueError("invalid Rustc Command Line continuation")
        args = shlex.split(raw[:-len(ending)])
        if len(args) != 1:
            raise ValueError("expected one Rustc argument per Command Line record")
        result.append(args[0])
    if not result:
        raise ValueError("missing Rustc Command Line")
    return tuple(result)


def _parse(stdout: bytes, *, bazel: bool, edition: str) -> _Row:
    text = stdout.decode("utf-8")
    if not text.endswith("\n\n"):
        raise ValueError("missing aquery row terminator")
    rustc, symlink, runfiles = (row.split("\n") for row in text[:-2].split("\n\n"))
    if len(rustc) < 5:
        raise ValueError("incomplete Rustc header")
    configuration = _value(rustc[3], "  Configuration: ")
    if bazel:
        if configuration != "k8-fastbuild":
            raise ValueError("invalid Bazel Rust target configuration")
        target = "bazel-out/k8-fastbuild/bin/"
    else:
        token = _value(configuration, "slugcfg-v2:")
        if not re.fullmatch(_HEX, token):
            raise ValueError("invalid Slug Rust target configuration")
        target = f"bazel-out/slugcfg-v2-{token}/bin/"
    platform = "@platforms//host:host" if bazel else _CANONICAL_PLATFORM
    headers = (
        "action 'Compiling Rust bin probe (1 file)'",
        f"action 'Creating runfiles tree {target}pkg/probe.runfiles'",
        "runfiles for //pkg:probe",
    )
    actions = []
    keys = []
    for mnemonic, header, row in zip(_MNEMONICS, headers, (rustc, symlink, runfiles), strict=True):
        expected_header = [
            header, f"  Mnemonic: {mnemonic}", "  Target: //pkg:probe",
            f"  Configuration: {configuration}", f"  Execution platform: {platform}",
        ]
        if row[:5] != expected_header:
            raise ValueError(f"{mnemonic}: action order, progress, target, configuration, or platform changed")
        payload = row[5:]
        if bazel:
            key_line, *payload = payload
            key = _value(key_line, "  ActionKey: ")
            if not re.fullmatch(_HEX, key):
                raise ValueError(f"{mnemonic}: expected a 64-hex Bazel ActionKey")
            keys.append(key)
        elif payload and payload[0].startswith("  ActionKey:"):
            raise ValueError(f"{mnemonic}: Slug ActionKey projection is deferred")
        inputs, outputs, *rest = payload
        environment, argv, execution_info = (), (), None
        if mnemonic == "Rustc":
            env, *command, checksum_line, platform_line, info = rest
            checksum = _value(checksum_line, "# Configuration: ")
            if (bazel and not re.fullmatch(_HEX, checksum)) or (not bazel and checksum != configuration):
                raise ValueError("Rustc configuration trailer disagrees with its domain")
            if platform_line != f"# Execution platform: {_CANONICAL_PLATFORM}":
                raise ValueError("Rustc execution platform trailer changed")
            environment = _list(env, "Environment")
            argv_raw = _argv(command)
            exec_prefix = argv_raw[0].removesuffix(_WRAPPER)
            expected_exec = "bazel-out/k8-opt-exec/bin/" if bazel else rf"bazel-out/slugcfg-v2-{_HEX}-exec/bin/"
            if not re.fullmatch(expected_exec, exec_prefix):
                raise ValueError("Rustc wrapper does not identify the pinned execution domain")
            paths = _Paths(target, exec_prefix)
            if [arg for arg in argv_raw if arg.startswith("--edition=")] != [f"--edition={edition}"]:
                raise ValueError(f"Rustc must contain exactly the pinned edition {edition}")
            argv = tuple(paths.argument(arg) for arg in argv_raw)
            execution_info = _value(info, "  ExecutionInfo: ")
        elif mnemonic == "SymlinkTree":
            (env,) = rest
            environment = _list(env, "Environment")
        elif rest:
            raise ValueError("RunfilesTree has unexpected trailing fields")
        actions.append(_Action(
            tuple(paths.artifact(path) for path in _list(inputs, "Inputs")),
            tuple(paths.artifact(path) for path in _list(outputs, "Outputs")),
            environment, argv, execution_info,
        ))
    return _Row(tuple(actions), tuple(keys), checksum)


def _compare(expected_row: _Row, actual_row: _Row, *, labels: tuple[str, str] = ("Bazel", "Slug")) -> None:
    expected_label, actual_label = labels
    for mnemonic, expected, actual in zip(_MNEMONICS, expected_row.actions, actual_row.actions, strict=True):
        for field in fields(_Action):
            left, right = getattr(expected, field.name), getattr(actual, field.name)
            if left == right:
                continue
            if isinstance(left, tuple):
                for index, (want, got) in enumerate(zip_longest(left, right)):
                    if want != got:
                        raise ValueError(
                            f"{mnemonic} {field.name}[{index}]: {expected_label} {want!r}, {actual_label} {got!r}"
                        )
            raise ValueError(f"{mnemonic} {field.name}: {expected_label} {left!r}, {actual_label} {right!r}")


def _compare_bazel_edit(cold: _Row, edited: _Row) -> None:
    if cold.action_keys[0] == edited.action_keys[0] or cold.action_keys[1:] != edited.action_keys[1:]:
        raise ValueError("Bazel: the edition edit must change only the Rustc ActionKey")
    if cold.rustc_configuration != edited.rustc_configuration:
        raise ValueError("Bazel: the edition edit changed the Rustc configuration checksum")

    rustc, *support = edited.actions
    # Parsing has already established exactly one edition argument in this action.
    cold_edition, edited_edition = f"--edition={EDITIONS[0]}", f"--edition={EDITIONS[2]}"
    argv = tuple(cold_edition if arg == edited_edition else arg for arg in rustc.argv)
    comparable = replace(edited, actions=(replace(rustc, argv=argv), *support))
    _compare(cold, comparable, labels=("Bazel cold", "Bazel edited"))


def compare_rows(bazel_stdout: bytes, slug_stdout: bytes, *, edition: str) -> None:
    """Compare one bounded capture; configuration prefixes carry distinct domains."""
    bazel = _parse(bazel_stdout, bazel=True, edition=edition)
    slug = _parse(slug_stdout, bazel=False, edition=edition)
    _compare(bazel, slug)


def compare_sequences(bazel_rows: Sequence[bytes], slug_rows: Sequence[bytes]) -> None:
    """Compare cold, warm, changed edition and restoration on one command route."""
    if len(bazel_rows) != len(CASES) or len(slug_rows) != len(CASES):
        raise ValueError("Rust aquery requires all four ordered captures on each tool")
    bazel_results = []
    for name, edition, bazel_stdout, slug_stdout in zip(CASES, EDITIONS, bazel_rows, slug_rows, strict=True):
        try:
            bazel = _parse(bazel_stdout, bazel=True, edition=edition)
            slug = _parse(slug_stdout, bazel=False, edition=edition)
            _compare(bazel, slug)
        except ValueError as error:
            raise ValueError(f"{name}: {error}") from error
        bazel_results.append(bazel)
    for name, rows in (("Bazel", bazel_rows), ("Slug", slug_rows)):
        if rows[0] != rows[1] or rows[0] != rows[3]:
            raise ValueError(f"{name}: warm and restoration output must equal cold output byte for byte")
    _compare_bazel_edit(bazel_results[0], bazel_results[2])


def _read_capture(path: Path) -> bytes:
    with path.open("rb") as stream:
        value = stream.read(_CAPTURE_LIMIT + 1)
    if len(value) > _CAPTURE_LIMIT:
        raise ValueError(f"aquery output exceeded the capture limit at {path}")
    return value


def _run_lane(
    directory: Path,
    inputs: dict,
    cases: tuple[FixtureCommand, ...],
    startup: list[str],
    timeout: float,
    daemon: InstalledDaemon | None = None,
) -> tuple[bytes, ...]:
    directory.mkdir()
    workspace = directory / "workspace"
    shutil.copytree(inputs["workspace"], workspace)
    repositories = [
        f"--registry={inputs['registry_uri']}",
        *(f"--distdir={path}" for path in inputs["distdirs"]),
    ]
    rows = []
    for command in cases:
        _apply_mutations(workspace, command)
        result = capture(
            [*startup, *command.argv, *repositories],
            cwd=workspace,
            timeout=timeout,
            max_bytes=_CAPTURE_LIMIT,
            log_prefix=directory / command.name,
        )
        if result.returncode or result.stderr:
            raise RuntimeError(
                f"{directory.name}/{command.name} exited {result.returncode} "
                f"with {len(result.stderr)} stderr bytes, see {directory}"
            )
        if daemon is not None:
            daemon.sample()
        rows.append(result.stdout)
    return tuple(rows)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--slug", type=Path, required=True, help="protected installed CLI")
    parser.add_argument("--cache-root", type=Path, required=True, help="Bazel archive CAS sha256 directory")
    parser.add_argument("--bazel-output", type=Path, required=True, help="same-host Bazel 9.2 .stdout files indexed by fixture command name")
    parser.add_argument("--output", type=Path, required=True, help="fresh destination in a protected parent")
    parser.add_argument("--timeout", type=int, default=120, help="deadline per aquery in seconds")
    args = parser.parse_args()
    if not args.slug.is_absolute() or args.timeout <= 0:
        parser.error("--slug must be absolute and --timeout must be positive")
    try:
        slug = args.slug.resolve(strict=True)
        cases = load_cases()
        bazel_rows = tuple(_read_capture(args.bazel_output / f"{case.name}.stdout") for case in cases)
        destination = args.output.absolute()
        managed = InstalledDaemon(destination / "o", slug)
        destination.mkdir(mode=0o700)
        inputs = assemble(args.cache_root, destination / "inputs")
        one_shot = _run_lane(destination / "one-shot", inputs, cases, [str(slug)], args.timeout)
        compare_sequences(bazel_rows, one_shot)
        with managed as daemon:
            retained = _run_lane(
                destination / "daemon", inputs, cases,
                [str(slug), f"--output_base={destination / 'o'}"], args.timeout, daemon,
            )
        compare_sequences(bazel_rows, retained)
        print(json.dumps({"cases_per_route": len(cases), "routes": ["one-shot", "daemon"], "status": "ok"}))
        return 0
    except (OSError, ValueError, KeyError, TypeError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"Rust aquery comparison failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
