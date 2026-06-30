from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - only used on Python < 3.11.
    try:
        import tomli as tomllib  # type: ignore[no-redef]
    except ModuleNotFoundError as exc:  # pragma: no cover
        raise RuntimeError("tools/v2_oracle requires Python 3.11+ or tomli") from exc

COMPARE_MODES = {"exact", "message_shape", "semantic"}


@dataclass(frozen=True)
class Mutation:
    path: str
    find: str | None = None
    replace: str | None = None
    content: str | None = None


@dataclass(frozen=True)
class FixtureCommand:
    name: str
    argv: tuple[str, ...]
    compare: str
    expected_exit: int | None = None
    env_allowlist: tuple[str, ...] = ()
    env: tuple[tuple[str, str], ...] = ()
    stdout_patterns: tuple[str, ...] = ()
    stderr_patterns: tuple[str, ...] = ()
    stdout_contains: tuple[str, ...] = ()
    stderr_contains: tuple[str, ...] = ()
    manifest_roots: tuple[str, ...] = ()
    mutations: tuple[Mutation, ...] = ()


@dataclass(frozen=True)
class Fixture:
    name: str
    root: Path
    workspace: Path
    expected: Path
    description: str = ""
    compare: str = "semantic"
    commands: tuple[FixtureCommand, ...] = field(default_factory=tuple)
    manifest_roots: tuple[str, ...] = ()
    oracle_notes: str = ""

    @property
    def expected_oracle(self) -> Path:
        return self.expected / "oracle.json"


def _as_str_list(value: Any, field_name: str) -> tuple[str, ...]:
    if value is None:
        return ()
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise ValueError(f"{field_name} must be a list of strings")
    return tuple(value)



def _as_str_map(value: Any, field_name: str) -> tuple[tuple[str, str], ...]:
    if value is None:
        return ()
    if not isinstance(value, dict):
        raise ValueError(f"{field_name} must be a table of string values")
    items: list[tuple[str, str]] = []
    for key, item in sorted(value.items()):
        if not isinstance(key, str) or not key:
            raise ValueError(f"{field_name} keys must be non-empty strings")
        if not isinstance(item, str):
            raise ValueError(f"{field_name}.{key} must be a string")
        items.append((key, item))
    return tuple(items)

def _as_optional_int(value: Any, field_name: str) -> int | None:
    if value is None:
        return None
    if not isinstance(value, int):
        raise ValueError(f"{field_name} must be an integer")
    return value


def _compare_mode(value: Any, fallback: str) -> str:
    mode = value or fallback
    if not isinstance(mode, str) or mode not in COMPARE_MODES:
        raise ValueError(f"compare must be one of {sorted(COMPARE_MODES)}")
    return mode


def _parse_mutations(items: Any) -> tuple[Mutation, ...]:
    if items is None:
        return ()
    if not isinstance(items, list):
        raise ValueError("commands.mutations must be an array of tables")
    mutations: list[Mutation] = []
    for item in items:
        if not isinstance(item, dict):
            raise ValueError("commands.mutations entries must be tables")
        path = item.get("path")
        if not isinstance(path, str) or not path:
            raise ValueError("mutation.path is required")
        find = item.get("find")
        replace = item.get("replace")
        content = item.get("content")
        if find is not None and not isinstance(find, str):
            raise ValueError("mutation.find must be a string")
        if replace is not None and not isinstance(replace, str):
            raise ValueError("mutation.replace must be a string")
        if content is not None and not isinstance(content, str):
            raise ValueError("mutation.content must be a string")
        if content is None and (find is None or replace is None):
            raise ValueError("mutation requires content or find+replace")
        mutations.append(Mutation(path=path, find=find, replace=replace, content=content))
    return tuple(mutations)


def load_fixture(path: Path) -> Fixture:
    fixture_file = path / "fixture.toml"
    if not fixture_file.is_file():
        raise FileNotFoundError(f"missing fixture.toml under {path}")
    with fixture_file.open("rb") as fh:
        raw = tomllib.load(fh)

    fixture_data = raw.get("fixture", {})
    if not isinstance(fixture_data, dict):
        raise ValueError("[fixture] must be a table")
    name = fixture_data.get("name", path.name)
    if not isinstance(name, str) or not name:
        raise ValueError("fixture.name must be a non-empty string")
    compare = _compare_mode(fixture_data.get("comparison"), "semantic")
    manifest_roots = _as_str_list(fixture_data.get("manifest_roots"), "fixture.manifest_roots")

    commands_raw = raw.get("commands", [])
    if not isinstance(commands_raw, list):
        raise ValueError("[[commands]] must be an array")
    commands: list[FixtureCommand] = []
    for index, command in enumerate(commands_raw):
        if not isinstance(command, dict):
            raise ValueError("command entries must be tables")
        argv = _as_str_list(command.get("argv"), "commands.argv")
        if not argv:
            raise ValueError("commands.argv must not be empty")
        command_name = command.get("name", f"command-{index + 1}")
        if not isinstance(command_name, str) or not command_name:
            raise ValueError("commands.name must be a non-empty string")
        command_manifest_roots = _as_str_list(command.get("manifest_roots"), "commands.manifest_roots")
        commands.append(
            FixtureCommand(
                name=command_name,
                argv=argv,
                compare=_compare_mode(command.get("compare"), compare),
                expected_exit=_as_optional_int(command.get("expected_exit"), "commands.expected_exit"),
                env_allowlist=_as_str_list(command.get("env_allowlist"), "commands.env_allowlist"),
                env=_as_str_map(command.get("env"), "commands.env"),
                stdout_patterns=_as_str_list(command.get("stdout_patterns"), "commands.stdout_patterns"),
                stderr_patterns=_as_str_list(command.get("stderr_patterns"), "commands.stderr_patterns"),
                stdout_contains=_as_str_list(command.get("stdout_contains"), "commands.stdout_contains"),
                stderr_contains=_as_str_list(command.get("stderr_contains"), "commands.stderr_contains"),
                manifest_roots=command_manifest_roots,
                mutations=_parse_mutations(command.get("mutations")),
            )
        )

    return Fixture(
        name=name,
        root=path,
        workspace=path / "workspace",
        expected=path / "expected",
        description=str(fixture_data.get("description", "")),
        compare=compare,
        commands=tuple(commands),
        manifest_roots=manifest_roots,
        oracle_notes=str(fixture_data.get("oracle_notes", "")),
    )


def discover_fixtures(fixtures_root: Path) -> list[Fixture]:
    if not fixtures_root.is_dir():
        raise FileNotFoundError(f"fixture root does not exist: {fixtures_root}")
    fixtures = [load_fixture(path) for path in sorted(fixtures_root.iterdir()) if (path / "fixture.toml").is_file()]
    names = [fixture.name for fixture in fixtures]
    duplicates = sorted({name for name in names if names.count(name) > 1})
    if duplicates:
        raise ValueError(f"duplicate fixture names: {', '.join(duplicates)}")
    return fixtures


def find_fixture(fixtures_root: Path, name: str) -> Fixture:
    for fixture in discover_fixtures(fixtures_root):
        if fixture.name == name:
            return fixture
    raise KeyError(f"unknown fixture {name!r}")