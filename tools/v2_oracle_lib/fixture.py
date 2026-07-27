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
    op: str | None = None
    name_bytes_hex: str | None = None
    destination: str | None = None
    find: str | None = None
    replace: str | None = None
    content: str | None = None


@dataclass(frozen=True)
class FixtureCommand:
    name: str
    argv: tuple[str, ...]
    compare: str
    startup_argv: tuple[str, ...] = ()
    capture_server_epoch: bool = False
    capture_startup_diagnostics: bool = False
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
class ReapiConfig:
    remote_executor: bool = False
    default_exec_properties: tuple[str, ...] = ()
    worker_platform_properties: tuple[str, ...] = ()


@dataclass(frozen=True)
class FixtureProvenance:
    bazel_release: str | None = None
    bazel_commit: str | None = None
    source_anchors: tuple[str, ...] = ()
    translation_notes: str | None = None
    generation_command: str | None = None
    verification_command: str | None = None


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
    reapi: ReapiConfig = field(default_factory=ReapiConfig)
    provenance: FixtureProvenance = field(default_factory=FixtureProvenance)
    daemon: bool = False
    startup_argv: tuple[str, ...] = ()
    env: tuple[tuple[str, str], ...] = ()
    observe_server_epochs: bool = False
    required_host_os: str | None = None
    http_registry: bool = False
    http_registry_port: int | None = None

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


def _as_bool(value: Any, field_name: str, *, fallback: bool = False) -> bool:
    if value is None:
        return fallback
    if not isinstance(value, bool):
        raise ValueError(f"{field_name} must be a boolean")
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
        _validate_relative_path(path, "mutation.path")
        op = item.get("op")
        if op is not None and (
            not isinstance(op, str)
            or op
            not in {
                "create",
                "delete",
                "fifo",
                "rename",
                "raw_create",
                "raw_delete",
            }
        ):
            raise ValueError(
                "mutation.op must be create, delete, fifo, rename, raw_create, "
                "or raw_delete"
            )
        find = item.get("find")
        replace = item.get("replace")
        content = item.get("content")
        destination = item.get("destination")
        name_bytes_hex = item.get("name_bytes_hex")
        if find is not None and not isinstance(find, str):
            raise ValueError("mutation.find must be a string")
        if replace is not None and not isinstance(replace, str):
            raise ValueError("mutation.replace must be a string")
        if content is not None and not isinstance(content, str):
            raise ValueError("mutation.content must be a string")
        if destination is not None and (not isinstance(destination, str) or not destination):
            raise ValueError("mutation.destination must be a non-empty string")
        if isinstance(destination, str):
            _validate_relative_path(destination, "mutation.destination")
        if op in {"raw_create", "raw_delete"}:
            try:
                path.encode("ascii")
            except UnicodeEncodeError as error:
                raise ValueError("raw mutation.path must be ASCII") from error
            _validate_raw_name_bytes_hex(name_bytes_hex)
            expected_fields = (
                {"op", "path", "name_bytes_hex", "content"}
                if op == "raw_create"
                else {"op", "path", "name_bytes_hex"}
            )
            if set(item) != expected_fields:
                if op == "raw_create" and content is None:
                    raise ValueError(
                        "raw_create mutation requires path, name_bytes_hex, and content"
                    )
                raise ValueError(
                    f"{op} mutation permits only {', '.join(sorted(expected_fields))}"
                )
        elif name_bytes_hex is not None:
            raise ValueError(f"{op or 'text'} mutation permits only its documented fields")
        elif op == "create":
            if content is None or any(value is not None for value in (find, replace, destination)):
                raise ValueError("create mutation requires content and no find, replace, or destination")
        elif op == "delete":
            if any(value is not None for value in (find, replace, content, destination)):
                raise ValueError("delete mutation permits only path")
        elif op == "fifo":
            if set(item) != {"op", "path"}:
                raise ValueError("fifo mutation permits only op and path")
        elif op == "rename":
            if destination is None or any(value is not None for value in (find, replace, content)):
                raise ValueError("rename mutation requires destination and no find, replace, or content")
        elif content is not None:
            if any(value is not None for value in (find, replace, destination)):
                raise ValueError("content mutation permits only path and content")
        elif destination is not None or find is None or replace is None:
            raise ValueError("mutation requires content or find+replace")
        mutations.append(
            Mutation(
                path=path,
                op=op,
                name_bytes_hex=name_bytes_hex,
                destination=destination,
                find=find,
                replace=replace,
                content=content,
            )
        )
    return tuple(mutations)


def _validate_raw_name_bytes_hex(value: Any) -> None:
    if not isinstance(value, str) or not value:
        raise ValueError("mutation.name_bytes_hex must be nonempty")
    if value != value.lower():
        raise ValueError("mutation.name_bytes_hex must use canonical lowercase")
    if len(value) % 2:
        raise ValueError("mutation.name_bytes_hex must have even-length")
    if any(character not in "0123456789abcdef" for character in value):
        raise ValueError("mutation.name_bytes_hex must be hexadecimal")
    decoded = bytes.fromhex(value)
    if decoded in {b".", b".."} or b"\0" in decoded or b"/" in decoded:
        raise ValueError(
            "mutation.name_bytes_hex must encode one non-special final component"
        )


def _validate_relative_path(value: str, field_name: str) -> None:
    candidate = Path(value)
    if candidate.is_absolute() or ".." in candidate.parts:
        raise ValueError(f"{field_name} must be a relative workspace path without '..'")


def _optional_string(value: Any, field_name: str) -> str | None:
    if value is None:
        return None
    if not isinstance(value, str) or not value:
        raise ValueError(f"{field_name} must be a non-empty string")
    return value


def _parse_provenance(value: Any) -> FixtureProvenance:
    if value is None:
        return FixtureProvenance()
    if not isinstance(value, dict):
        raise ValueError("provenance must be a table")
    return FixtureProvenance(
        bazel_release=_optional_string(value.get("bazel_release"), "provenance.bazel_release"),
        bazel_commit=_optional_string(value.get("bazel_commit"), "provenance.bazel_commit"),
        source_anchors=_as_str_list(value.get("source_anchors"), "provenance.source_anchors"),
        translation_notes=_optional_string(value.get("translation_notes"), "provenance.translation_notes"),
        generation_command=_optional_string(value.get("generation_command"), "provenance.generation_command"),
        verification_command=_optional_string(value.get("verification_command"), "provenance.verification_command"),
    )


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
                startup_argv=_as_str_list(
                    command.get("startup_argv"), "commands.startup_argv"
                ),
                capture_server_epoch=_as_bool(
                    command.get("capture_server_epoch"),
                    "commands.capture_server_epoch",
                ),
                capture_startup_diagnostics=_as_bool(
                    command.get("capture_startup_diagnostics"),
                    "commands.capture_startup_diagnostics",
                ),
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

    http_registry = bool(fixture_data.get("http_registry", False))
    http_registry_port = _as_optional_int(
        fixture_data.get("http_registry_port"), "fixture.http_registry_port"
    )
    if http_registry_port is not None and not 0 <= http_registry_port <= 65535:
        raise ValueError("fixture.http_registry_port must be between 0 and 65535")
    if http_registry_port is not None and not http_registry:
        raise ValueError("fixture.http_registry_port requires fixture.http_registry = true")

    daemon = _as_bool(fixture_data.get("daemon"), "fixture.daemon")
    observe_server_epochs = _as_bool(
        fixture_data.get("observe_server_epochs"), "fixture.observe_server_epochs"
    )
    required_host_os = fixture_data.get("required_host_os")
    if required_host_os not in (None, "posix", "linux"):
        raise ValueError(
            "fixture.required_host_os must be absent, 'posix', or 'linux'"
        )
    if observe_server_epochs and not daemon:
        raise ValueError("fixture.observe_server_epochs requires fixture.daemon = true")
    if not observe_server_epochs and any(command.capture_server_epoch for command in commands):
        raise ValueError("commands.capture_server_epoch requires fixture.observe_server_epochs = true")

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
        reapi=_parse_reapi(raw.get("reapi")),
        provenance=_parse_provenance(raw.get("provenance")),
        daemon=daemon,
        startup_argv=_as_str_list(
            fixture_data.get("startup_argv"), "fixture.startup_argv"
        ),
        env=_as_str_map(fixture_data.get("env"), "fixture.env"),
        observe_server_epochs=observe_server_epochs,
        required_host_os=required_host_os,
        http_registry=http_registry,
        http_registry_port=http_registry_port,
    )


def _parse_reapi(value: Any) -> ReapiConfig:
    if value is None:
        return ReapiConfig()
    if not isinstance(value, dict):
        raise ValueError("reapi must be a table")
    remote_executor = bool(value.get("remote_executor", False))
    exec_properties = _as_str_list(
        value.get("default_exec_properties"), "reapi.default_exec_properties"
    )
    return ReapiConfig(
        remote_executor=remote_executor,
        default_exec_properties=exec_properties,
        worker_platform_properties=_as_str_list(
            value.get("worker_platform_properties"), "reapi.worker_platform_properties"
        ),
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
