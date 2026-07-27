from __future__ import annotations

import json
import os
import shutil
import socket
import subprocess
import sys
import uuid
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from tools.v2_oracle_lib.compare import compare_result, write_failure_artifacts
from tools.v2_oracle_lib.evidence import validate_evidence
from tools.v2_oracle_lib.fixture import (
    Fixture,
    FixtureCommand,
    Mutation,
    ReapiConfig,
    discover_fixtures,
    load_fixture,
)
from tools.v2_oracle_lib.manifest import collect_manifest
from tools.v2_oracle_lib.normalize import normalize_text, path_replacements
from tools.v2_oracle_lib.runner import (
    BazelServerIdentity,
    RunOptions,
    ServerEpochs,
    ToolConfig,
    _argv,
    _apply_mutations,
    _extract_startup_diagnostics,
    _extract_reapi_evidence,
    _parse_loopback_endpoint,
    _read_bazel_server_identity,
    _shutdown_bazel_daemon,
    _wait_for_bazel_shutdown,
    _slug_reapi_argv,
    run_fixture,
)

FIXTURES = ROOT / "tests" / "v2_oracle" / "fixtures"


def scratch_dir(name: str) -> Path:
    path = ROOT / "target" / "v2_oracle_test_tmp" / f"{name}-{os.getpid()}-{uuid.uuid4().hex}"
    path.mkdir(parents=True, exist_ok=False)
    return path


def test_fixture_listing_includes_initial_stage1_set() -> None:
    names = {fixture.name for fixture in discover_fixtures(FIXTURES)}
    assert {
        "version-bazel9",
        "empty-module-build",
        "exports-and-filegroup",
        "simple-rule-action",
        "load-invalidation",
        "module-local-override",
        "negative-no-workspace",
        "glob-directory-invalidation",
    } <= names


def test_fixture_parser_reads_commands_and_mutations() -> None:
    fixture = load_fixture(FIXTURES / "load-invalidation")
    assert fixture.name == "load-invalidation"
    assert [command.name for command in fixture.commands] == [
        "cold_success_v1_order",
        "unchanged_warm_build_no_replay",
        "valid_empty_warm_query",
        "empty_query_after_dependency_edit",
        "unchanged_warm_query_no_replay",
        "build_after_query_edit_only_analysis",
        "eligible_analysis_failure_prefix",
        "execution_failure_after_semantic_output",
        "warm_message_after_failures_no_replay",
    ]
    assert [
        (mutation.path, mutation.find, mutation.replace)
        for mutation in fixture.commands[3].mutations
    ] == [
        (
            "pkg/message.bzl",
            "SLUG_TERMINAL_EVENT_DEP_BZL_V1",
            "SLUG_TERMINAL_EVENT_DEP_BZL_V2",
        ),
        (
            "pkg/message.bzl",
            'MESSAGE = "one"',
            'MESSAGE = "two"',
        ),
    ]


def test_fixture_parser_reads_file_operations_and_provenance() -> None:
    fixture = load_fixture(FIXTURES / "glob-directory-invalidation")
    assert [command.mutations[0].op for command in fixture.commands[1:]] == [
        "create",
        "rename",
        "delete",
    ]
    assert fixture.commands[2].mutations[0].destination == "pkg/renamed.txt"
    assert fixture.provenance.bazel_release == "9.2.0"
    assert fixture.provenance.bazel_commit == "8220c6198837d5c13d53fea211cf3282aa12408a"
    assert len(fixture.provenance.source_anchors) == 3


def test_startup_fixture_parser_and_argv_merge_are_strict() -> None:
    root = scratch_dir("startup-parser")
    (root / "workspace").mkdir()
    (root / "expected").mkdir()
    (root / "fixture.toml").write_text(
        """
[fixture]
name = "startup-parser"
daemon = true
startup_argv = ["--nosystem_rc", "--noworkspace_rc"]
observe_server_epochs = true

[fixture.env]
BAZELRC = ""
PRECEDENCE = "fixture"

[[commands]]
name = "capture"
startup_argv = ["--host_jvm_args=-Dfile.encoding=UTF-8"]
argv = ["query", "//:BUILD.bazel"]
capture_server_epoch = true
capture_startup_diagnostics = true

[commands.env]
PRECEDENCE = "command"
""".strip(),
        encoding="utf-8",
    )
    fixture = load_fixture(root)
    command = fixture.commands[0]
    assert fixture.startup_argv == ("--nosystem_rc", "--noworkspace_rc")
    assert dict(fixture.env) == {"BAZELRC": "", "PRECEDENCE": "fixture"}
    assert fixture.observe_server_epochs is True
    assert command.startup_argv == ("--host_jvm_args=-Dfile.encoding=UTF-8",)
    assert command.capture_server_epoch is True
    assert command.capture_startup_diagnostics is True
    assert _argv(
        ToolConfig(name="bazel", executable=Path("/usr/bin/bazel")),
        fixture,
        command,
        Path("/tmp/output-base"),
    ) == [
        "/usr/bin/bazel",
        "--output_base=/tmp/output-base",
        "--nosystem_rc",
        "--noworkspace_rc",
        "--host_jvm_args=-Dfile.encoding=UTF-8",
        "query",
        "//:BUILD.bazel",
    ]


@pytest.mark.parametrize(
    ("fixture_field", "command_field", "value", "message"),
    [
        ("observe_server_epochs", None, '"yes"', "observe_server_epochs"),
        (None, "capture_server_epoch", "1", "capture_server_epoch"),
        (None, "capture_startup_diagnostics", '"yes"', "capture_startup_diagnostics"),
        ("startup_argv", None, '"bad"', "fixture.startup_argv"), ("env", None, "{ BAD = 1 }", "fixture.env.BAD"),
        (None, "startup_argv", '"bad"', "commands.startup_argv"),
    ],
)
def test_startup_fixture_parser_rejects_invalid_fields(
    fixture_field: str | None,
    command_field: str | None,
    value: str,
    message: str,
) -> None:
    root = scratch_dir("startup-parser-bool")
    (root / "workspace").mkdir()
    (root / "expected").mkdir()
    fixture_setting = f"{fixture_field} = {value}\n" if fixture_field is not None else ""
    command_setting = f"{command_field} = {value}\n" if command_field is not None else ""
    (root / "fixture.toml").write_text(
        (
            "[fixture]\n"
            'name = "startup-parser-bool"\n'
            "daemon = true\n"
            f"{fixture_setting}\n"
            "[[commands]]\n"
            'argv = ["query", "//:BUILD.bazel"]\n'
            f"{command_setting}"
        ),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match=message):
        load_fixture(root)


def test_server_epoch_observation_requires_daemon_fixture() -> None:
    root = scratch_dir("startup-parser-daemon")
    (root / "workspace").mkdir()
    (root / "expected").mkdir()
    (root / "fixture.toml").write_text(
        """
[fixture]
name = "startup-parser-daemon"
observe_server_epochs = true

[[commands]]
argv = ["query", "//:BUILD.bazel"]
""".strip(),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="requires fixture.daemon"):
        load_fixture(root)
    text = (root / "fixture.toml").read_text().replace("observe_server_epochs = true", "daemon = true")
    (root / "fixture.toml").write_text(text.replace("[[commands]]", "[[commands]]\ncapture_server_epoch = true"))
    with pytest.raises(ValueError, match="requires fixture.observe_server_epochs"):
        load_fixture(root)


def _http_registry_fixture(root: Path, name: str, port: int = 0) -> Fixture:
    fixture_root = root / "fixture"
    workspace = fixture_root / "workspace"
    expected = fixture_root / "expected"
    registry = workspace / "registry" / "first"
    registry.mkdir(parents=True)
    expected.mkdir()
    (registry / "bazel_registry.json").write_text("{}\n", encoding="utf-8")
    shutil.copy2(
        FIXTURES / "registry-yanked-lockfile-mode" / "http_registry.py",
        fixture_root / "http_registry.py",
    )
    (fixture_root / "fixture.toml").write_text(
        f"""
[fixture]
name = "{name}"
comparison = "semantic"
http_registry = true
http_registry_port = {port}
manifest_roots = ["http_registry_request_counts.json"]

[[commands]]
name = "fetch_registry_index"
argv = [
  "-c",
  "import sys, urllib.request; print(urllib.request.urlopen(sys.argv[1] + '/first/bazel_registry.json').read().decode())",
  "{{{{http_registry}}}}",
]
expected_exit = 0
stdout_contains = ["{{}}"]
""".strip(),
        encoding="utf-8",
    )
    return load_fixture(fixture_root)


def test_fixture_http_registry_concurrent_lifecycle_and_request_manifest() -> None:
    roots = [
        scratch_dir("http-registry-runner-a"),
        scratch_dir("http-registry-runner-b"),
    ]
    fixtures = [
        _http_registry_fixture(root, f"http-registry-runner-{index}")
        for index, root in enumerate(roots)
    ]
    for fixture in fixtures:
        assert fixture.http_registry is True
        assert fixture.http_registry_port == 0

    with ThreadPoolExecutor(max_workers=2) as executor:
        results = list(
            executor.map(
                lambda pair: run_fixture(
                    pair[1],
                    ToolConfig(name="slug", executable=Path(sys.executable)),
                    RunOptions(run_root=pair[0] / "runs", timeout_seconds=30),
                ),
                zip(roots, fixtures),
            )
        )

    endpoints: set[str] = set()
    for fixture, result in zip(fixtures, results):
        assert compare_result(fixture, result, expected=None) == []
        record = result["commands"][0]
        assert record["argv"][-1] == "{{http_registry}}"
        endpoint = record["executed_argv"][-1]
        assert endpoint.startswith("http://127.0.0.1:")
        endpoints.add(endpoint)
        assert record["http_registry_request_counts"] == {
            "/first/bazel_registry.json": 1
        }
        assert [entry["path"] for entry in record["manifest"]] == [
            "http_registry_request_counts.json"
        ]
        with pytest.raises(OSError):
            socket.create_connection(
                ("127.0.0.1", int(endpoint.rsplit(":", 1)[1])), timeout=0.2
            )
    assert len(endpoints) == 2


def test_fixture_http_registry_bind_failure_is_bounded_and_reports_stderr() -> None:
    root = scratch_dir("http-registry-bind-failure")
    occupied = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    occupied.bind(("127.0.0.1", 0))
    occupied.listen()
    port = occupied.getsockname()[1]
    fixture = _http_registry_fixture(root, "http-registry-bind-failure", port)

    try:
        with pytest.raises(RuntimeError, match="failed to start") as error:
            run_fixture(
                fixture,
                ToolConfig(name="slug", executable=Path(sys.executable)),
                RunOptions(run_root=root / "runs", timeout_seconds=30),
            )
    finally:
        occupied.close()

    assert "Address already in use" in str(error.value)


@pytest.mark.parametrize(
    ("mutation", "message"),
    [
        ('op = "create"\npath = "x.txt"', "create mutation requires content"),
        ('op = "delete"\npath = "x.txt"\ncontent = "x"', "delete mutation permits only path"),
        ('op = "rename"\npath = "x.txt"', "rename mutation requires destination"),
        ('op = "create"\npath = "../x.txt"\ncontent = "x"', "relative workspace path"),
        ('op = "rename"\npath = "x.txt"\ndestination = "/tmp/x.txt"', "relative workspace path"),
        ('path = "x.txt"\ncontent = "x"\nfind = "old"', "content mutation permits only"),
    ],
)
def test_fixture_parser_rejects_illegal_file_operations(mutation: str, message: str) -> None:
    root = scratch_dir("mutation-parser")
    (root / "workspace").mkdir()
    (root / "expected").mkdir()
    (root / "fixture.toml").write_text(
        "[fixture]\nname = \"bad-mutation\"\n\n[[commands]]\nargv = [\"query\"]\n\n[[commands.mutations]]\n"
        + mutation
        + "\n",
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match=message):
        load_fixture(root)


def test_runner_file_operations_and_rejections_stay_in_workspace() -> None:
    root = scratch_dir("mutation-runner")
    workspace = root / "workspace"
    workspace.mkdir()
    (workspace / "source.txt").write_text("source\n", encoding="utf-8")
    fixture_root = root / "fixture"
    fixture_root.mkdir()
    (fixture_root / "fixture.toml").write_text(
        """
[fixture]
name = "mutation-runner"

[[commands]]
argv = ["-c", "pass"]

[[commands.mutations]]
op = "create"
path = "created.txt"
content = "created\\n"

[[commands.mutations]]
op = "rename"
path = "created.txt"
destination = "renamed.txt"

[[commands.mutations]]
op = "delete"
path = "renamed.txt"
""".strip(),
        encoding="utf-8",
    )
    command = load_fixture(fixture_root).commands[0]
    assert _apply_mutations(workspace, command) == [
        {"op": "create", "path": "created.txt"},
        {"op": "rename", "path": "created.txt", "destination": "renamed.txt"},
        {"op": "delete", "path": "renamed.txt"},
    ]
    assert not (workspace / "renamed.txt").exists()

    def mutation_command(mutation: Mutation) -> FixtureCommand:
        return FixtureCommand(
            name="rejection", argv=("-c", "pass"), compare="semantic", mutations=(mutation,)
        )

    with pytest.raises(FileNotFoundError, match="source does not exist"):
        _apply_mutations(workspace, mutation_command(Mutation(path="missing.txt", op="delete")))
    with pytest.raises(FileExistsError, match="create destination exists"):
        _apply_mutations(
            workspace,
            mutation_command(Mutation(path="source.txt", op="create", content="duplicate\n")),
        )
    with pytest.raises(FileExistsError, match="rename destination exists"):
        _apply_mutations(
            workspace,
            mutation_command(Mutation(path="source.txt", op="rename", destination="source.txt")),
        )
    with pytest.raises(FileNotFoundError, match="existing real directory"):
        _apply_mutations(
            workspace,
            mutation_command(
                Mutation(path="missing/created.txt", op="create", content="no parent\n")
            ),
        )
    with pytest.raises(FileNotFoundError, match="existing real directory"):
        _apply_mutations(
            workspace,
            mutation_command(
                Mutation(path="source.txt", op="rename", destination="missing/renamed.txt")
            ),
        )

    target_dir = workspace / "target-dir"
    target_dir.mkdir()
    (workspace / "directory-link").symlink_to(target_dir, target_is_directory=True)
    _apply_mutations(
        workspace,
        mutation_command(Mutation(path="directory-link", op="rename", destination="renamed-directory-link")),
    )
    assert (workspace / "renamed-directory-link").is_symlink()

    (workspace / "dangling-link").symlink_to("missing-target")
    (workspace / "cycle-link").symlink_to("cycle-link")
    _apply_mutations(
        workspace,
        mutation_command(Mutation(path="dangling-link", op="rename", destination="parked-dangling-link")),
    )
    assert (workspace / "parked-dangling-link").is_symlink()
    (workspace / "deletable-dangling-link").symlink_to("missing-delete-target")
    _apply_mutations(
        workspace,
        mutation_command(Mutation(path="deletable-dangling-link", op="delete")),
    )
    assert not (workspace / "deletable-dangling-link").is_symlink()
    _apply_mutations(
        workspace,
        mutation_command(Mutation(path="cycle-link", op="rename", destination="parked-cycle-link")),
    )
    _apply_mutations(
        workspace,
        mutation_command(Mutation(path="parked-cycle-link", op="rename", destination="cycle-link")),
    )
    assert (workspace / "cycle-link").is_symlink()

    (workspace / "dangling-destination").symlink_to("missing-destination")
    with pytest.raises(FileExistsError, match="rename destination exists"):
        _apply_mutations(
            workspace,
            mutation_command(Mutation(path="source.txt", op="rename", destination="dangling-destination")),
        )
    assert (workspace / "source.txt").is_file()
    assert (workspace / "dangling-destination").is_symlink()

    outside = root / "outside"
    outside.mkdir()
    (outside / "victim.txt").write_text("outside\n", encoding="utf-8")
    (workspace / "escape").symlink_to(outside, target_is_directory=True)
    with pytest.raises(ValueError, match="escapes workspace"):
        _apply_mutations(
            workspace,
            mutation_command(Mutation(path="escape/x.txt", op="create", content="nope\n")),
        )
    with pytest.raises(ValueError, match="escapes workspace"):
        _apply_mutations(
            workspace,
            mutation_command(Mutation(path="escape/victim.txt", op="delete")),
        )
    assert (outside / "victim.txt").is_file()


def test_runner_expands_workspace_uri_only_in_copied_utf8_regular_files() -> None:
    root = scratch_dir("workspace-uri-initial")
    fixture_root = root / "fixture"
    source_workspace = fixture_root / "workspace"
    expected = fixture_root / "expected"
    source_workspace.mkdir(parents=True)
    expected.mkdir()
    source_text = (
        b"first {{workspace_uri}}\r\n"
        b"second {{workspace_uri}} {{unknown_token}} {{http_registry}}\r\n"
    )
    (source_workspace / "text.txt").write_bytes(source_text)
    binary = b"\xff{{workspace_uri}}\x00"
    (source_workspace / "binary.dat").write_bytes(binary)
    outside = root / "outside.txt"
    outside.write_text("{{workspace_uri}}\n", encoding="utf-8")
    (source_workspace / "linked.txt").symlink_to(outside)
    (fixture_root / "fixture.toml").write_text(
        """
[fixture]
name = "workspace-uri-initial"

[[commands]]
argv = ["-c", "pass"]
""".strip(),
        encoding="utf-8",
    )

    result = run_fixture(
        load_fixture(fixture_root),
        ToolConfig(name="slug", executable=Path(sys.executable)),
        RunOptions(run_root=root / "runs", timeout_seconds=30),
    )
    copied_workspace = Path(result["commands"][0]["cwd"])
    workspace_uri = copied_workspace.resolve().as_uri()
    assert (copied_workspace / "text.txt").read_bytes() == (
        f"first {workspace_uri}\r\n"
        f"second {workspace_uri} {{{{unknown_token}}}} {{{{http_registry}}}}\r\n"
    ).encode("utf-8")
    assert (source_workspace / "text.txt").read_bytes() == source_text
    assert (copied_workspace / "binary.dat").read_bytes() == binary
    assert (copied_workspace / "linked.txt").is_symlink()
    assert outside.read_text(encoding="utf-8") == "{{workspace_uri}}\n"


def test_mutations_expand_workspace_uri_operands_but_record_raw_templates() -> None:
    workspace = scratch_dir("workspace-uri-mutations") / "workspace"
    workspace.mkdir()
    workspace_uri = workspace.resolve().as_uri()
    (workspace / "message.txt").write_text(f"before {workspace_uri}\n", encoding="utf-8")
    (workspace / "content.txt").write_text("old\n", encoding="utf-8")
    raw_find = "before {{workspace_uri}}"
    raw_replace = "after {{workspace_uri}}"
    raw_content = "created {{workspace_uri}} {{unknown_token}}\n"
    raw_path = "literal-{{workspace_uri}}.txt"
    raw_destination = "renamed-{{workspace_uri}}.txt"
    command = FixtureCommand(
        name="workspace-uri-mutations",
        argv=("-c", "pass"),
        compare="semantic",
        mutations=(
            Mutation(path="message.txt", find=raw_find, replace=raw_replace),
            Mutation(path=raw_path, op="create", content=raw_content),
            Mutation(path=raw_path, op="rename", destination=raw_destination),
            Mutation(path="content.txt", content="replacement {{workspace_uri}}\n"),
        ),
    )

    applied = _apply_mutations(workspace, command)
    assert (workspace / "message.txt").read_text(encoding="utf-8") == f"after {workspace_uri}\n"
    assert (workspace / raw_destination).read_text(encoding="utf-8") == (
        f"created {workspace_uri} {{{{unknown_token}}}}\n"
    )
    assert not (workspace / raw_path).exists()
    assert (workspace / "content.txt").read_text(encoding="utf-8") == (
        f"replacement {workspace_uri}\n"
    )
    assert applied == [
        {
            "path": "message.txt",
            "find": raw_find,
            "replace": raw_replace,
            "old_digest_hint": str(len(f"before {workspace_uri}\n")),
        },
        {"op": "create", "path": raw_path, "content": raw_content},
        {"op": "rename", "path": raw_path, "destination": raw_destination},
        {
            "path": "content.txt",
            "find": None,
            "replace": None,
            "content": "replacement {{workspace_uri}}\n",
            "old_digest_hint": str(len("old\n")),
        },
    ]


def test_runner_preserves_registry_argv_and_conditional_http_registry_substitution() -> None:
    tool = ToolConfig(name="slug", executable=Path(sys.executable))
    fixture = Fixture(
        name="transport-tokens",
        root=Path("/fixture"),
        workspace=Path("/fixture/workspace"),
        expected=Path("/fixture/expected"),
    )
    command = FixtureCommand(
        name="transport-tokens",
        argv=("-c", "pass", "--registry=file://%workspace%/registry", "{{http_registry}}"),
        compare="semantic",
    )
    assert _argv(tool, fixture, command, Path("/tmp/output-base")) == [
        str(Path(sys.executable)),
        "-c",
        "pass",
        "--registry=file://%workspace%/registry",
        "{{http_registry}}",
    ]


def _startup_bep(path: Path, combined_forms: list[object], payload_label: str = "original") -> None:
    options = []
    for value in combined_forms:
        if value is None:
            options.append({})
        elif isinstance(value, dict):
            options.append(value)
        else:
            options.append({"combinedForm": value})
    path.write_text(
        "\n".join(
            [
                json.dumps({"id": {"started": {}}, "started": {"uuid": "ignored"}}),
                json.dumps(
                    {
                        "id": {
                            "structuredCommandLine": {
                                "commandLineLabel": "original"
                            }
                        },
                        "structuredCommandLine": {
                            "commandLineLabel": payload_label,
                            "sections": [
                                {
                                    "sectionLabel": "startup options",
                                    "optionList": {"option": options},
                                },
                                {"sectionLabel": "command", "chunkList": {}},
                            ],
                        },
                    }
                ),
            ]
        )
        + "\n",
        encoding="utf-8",
    )


def test_startup_diagnostics_preserve_absent_duplicates_and_exact_rc_messages() -> None:
    root = scratch_dir("startup-diagnostics")
    workspace = root / "workspace"
    run_dir = root / "run"
    output_base = root / "output-base"
    workspace.mkdir()
    run_dir.mkdir()
    output_base.mkdir()
    bep = run_dir / "startup.json"
    _startup_bep(
        bep,
        [
            f"--output_base={output_base}",
            None,
            "--host_jvm_args=-Dduplicate",
            "--host_jvm_args=-Dduplicate",
        ],
    )
    stderr = (
        "INFO: Reading 'startup' options from "
        f"{workspace}/startup-diagnostics.bazelrc: "
        "--host_jvm_args=-Dfile.encoding=ISO-8859-1, "
        "--host_jvm_args=-Dfile.encoding=UTF-8\n"
        "INFO: unrelated\n"
        "Reading 'startup' options from partial\n"
    )
    combined, announcements = _extract_startup_diagnostics(
        bep,
        stderr,
        path_replacements(
            workspace=workspace, run_dir=run_dir, output_base=output_base
        ),
    )
    assert combined == [
        "--output_base=<output_base>",
        "",
        "--host_jvm_args=-Dduplicate",
        "--host_jvm_args=-Dduplicate",
    ]
    assert announcements == [
        "Reading 'startup' options from <workspace>/startup-diagnostics.bazelrc: "
        "--host_jvm_args=-Dfile.encoding=ISO-8859-1, "
        "--host_jvm_args=-Dfile.encoding=UTF-8"
    ]


@pytest.mark.parametrize(
    ("payload", "message"),
    [
        ("[]\n", "object"),
        ("{}\n", "original"),
        (
            json.dumps(
                {
                    "id": {
                        "structuredCommandLine": {"commandLineLabel": "original"}
                    },
                    "structuredCommandLine": {
                        "commandLineLabel": "original",
                        "sections": [],
                    },
                }
            )
            + "\n",
            "startup options",
        ),
    ],
)
def test_startup_diagnostics_reject_schema_and_cardinality(
    payload: str, message: str
) -> None:
    bep = scratch_dir("startup-schema") / "startup.json"
    bep.write_text(payload, encoding="utf-8")
    with pytest.raises(RuntimeError, match=message):
        _extract_startup_diagnostics(bep, "", {})


@pytest.mark.parametrize("value", [1, {"optionName": "synthetic"}])
def test_startup_diagnostics_reject_nonstring_or_nonempty_missing_combined_form(value: object) -> None:
    bep = scratch_dir("startup-schema-value") / "startup.json"
    _startup_bep(bep, [value])
    with pytest.raises(RuntimeError, match="combinedForm"):
        _extract_startup_diagnostics(bep, "", {})
    _startup_bep(bep, [], "not-original")
    with pytest.raises(RuntimeError, match="payload label"):
        _extract_startup_diagnostics(bep, "", {})


@pytest.mark.parametrize(
    ("raw", "expected"),
    [
        ("127.0.0.1:1234", ("127.0.0.1", 1234)),
        ("[::1]:4321", ("::1", 4321)),
    ],
)
def test_parse_loopback_endpoint(raw: str, expected: tuple[str, int]) -> None:
    assert _parse_loopback_endpoint(raw) == expected


@pytest.mark.parametrize(
    "raw",
    ["example.com:1234", "127.0.0.1:0", "::1:1234", "[::1]", "garbage"],
)
def test_parse_loopback_endpoint_rejects_invalid_values(raw: str) -> None:
    with pytest.raises(RuntimeError, match="endpoint"):
        _parse_loopback_endpoint(raw)


def test_server_epochs_are_stable_and_reject_endpoint_changes() -> None:
    epochs = ServerEpochs()
    first = BazelServerIdentity(101, "one", ("127.0.0.1", 1001))
    second = BazelServerIdentity(102, "two", ("::1", 1002))
    assert epochs.observe(first) == 1
    assert epochs.observe(first) == 1
    assert epochs.observe(second) == 2
    with pytest.raises(RuntimeError, match="endpoint changed"):
        epochs.observe(BazelServerIdentity(101, "one", ("127.0.0.1", 9999)))
    assert epochs.identities == (first, second)


def test_bazel_identity_rejects_missing_stale_and_unreachable_metadata(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    output_base = scratch_dir("bazel-identity")
    server = output_base / "server"
    server.mkdir()
    with pytest.raises(RuntimeError, match="server.pid.txt"):
        _read_bazel_server_identity(output_base)
    (server / "server.pid.txt").write_text("4321\n", encoding="utf-8")
    (server / "server.starttime").write_text("99\n", encoding="utf-8")
    (server / "command_port").write_text("127.0.0.1:9876\n", encoding="utf-8")
    monkeypatch.setattr(
        "tools.v2_oracle_lib.runner._process_identity_alive", lambda identity: False
    )
    with pytest.raises(RuntimeError, match="stale"):
        _read_bazel_server_identity(output_base)
    monkeypatch.setattr(
        "tools.v2_oracle_lib.runner._process_identity_alive", lambda identity: True
    )
    monkeypatch.setattr(
        "tools.v2_oracle_lib.runner._endpoint_reachable", lambda endpoint: False
    )
    with pytest.raises(RuntimeError, match="unreachable"):
        _read_bazel_server_identity(output_base)


def test_opted_runner_executes_one_process_per_row_and_observes_every_epoch(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    root = scratch_dir("startup-runner")
    workspace = root / "fixture" / "workspace"
    workspace.mkdir(parents=True)
    (workspace / "BUILD.bazel").write_text("", encoding="utf-8")
    commands = (
        FixtureCommand(
            name="uncaptured-first",
            argv=("query", "//:BUILD.bazel"),
            compare="semantic",
            startup_argv=("--host_jvm_args=-Done",),
            env=(("ROW", "first"),),
        ),
        FixtureCommand(
            name="captured-second",
            argv=("query", "//:BUILD.bazel"),
            compare="semantic",
            capture_server_epoch=True,
        ),
        FixtureCommand(
            name="diagnostic-third",
            argv=("query", "//:BUILD.bazel"),
            compare="semantic",
            capture_server_epoch=True,
            capture_startup_diagnostics=True,
        ),
    )
    fixture = Fixture(
        name="startup-runner",
        root=workspace.parent,
        workspace=workspace,
        expected=workspace.parent / "expected",
        daemon=True,
        startup_argv=("--nosystem_rc",),
        env=(("BASELINE", "fixture"), ("ROW", "fixture")),
        observe_server_epochs=True,
        commands=commands,
    )
    calls: list[tuple[list[str], dict[str, str]]] = []

    def fake_run(
        argv: list[str], **kwargs: object
    ) -> subprocess.CompletedProcess[str]:
        calls.append((list(argv), dict(kwargs["env"])))  # type: ignore[arg-type]
        bep_args = [
            arg for arg in argv if arg.startswith("--build_event_json_file=")
        ]
        if bep_args:
            assert len(bep_args) == 1
            _startup_bep(Path(bep_args[0].split("=", 1)[1]), ["--nosystem_rc"])
        return subprocess.CompletedProcess(argv, 0, "//:BUILD.bazel\n", "")

    identities = iter(
        [
            BazelServerIdentity(101, "one", ("127.0.0.1", 1001)),
            BazelServerIdentity(102, "two", ("127.0.0.1", 1002)),
            BazelServerIdentity(102, "two", ("127.0.0.1", 1002)),
        ]
    )
    shutdown: list[tuple[BazelServerIdentity, ...]] = []
    monkeypatch.setattr("tools.v2_oracle_lib.runner.subprocess.run", fake_run)
    monkeypatch.setattr(
        "tools.v2_oracle_lib.runner._read_bazel_server_identity",
        lambda output_base: next(identities),
    )
    monkeypatch.setattr(
        "tools.v2_oracle_lib.runner._shutdown_bazel_daemon",
        lambda *args: shutdown.append(args[5]),
    )
    tool = ToolConfig(name="bazel", executable=Path("/usr/bin/bazel"))
    result = run_fixture(fixture, tool, RunOptions(run_root=root / "runs"))
    assert len(calls) == len(commands)
    assert [record["name"] for record in result["commands"]] == [command.name for command in commands]
    assert calls[0][0][2:5] == ["--nosystem_rc", "--host_jvm_args=-Done", "query"]
    assert calls[0][1]["BASELINE"] == "fixture"
    assert calls[0][1]["ROW"] == "first"
    assert "server_epoch" not in result["commands"][0]
    assert result["commands"][1]["server_epoch"] == 2
    assert result["commands"][2]["server_epoch"] == 2
    bep_paths = [
        arg
        for argv, _ in calls
        for arg in argv
        if arg.startswith("--build_event_json_file=")
    ]
    assert len(bep_paths) == len(set(bep_paths)) == 1
    assert bep_paths[0].endswith("/startup-bep/03.json")
    assert calls[2][0][-1] == bep_paths[0]
    assert not (Path(result["run_dir"]) / "startup-bep").exists()
    assert shutdown == [
        (
            BazelServerIdentity(101, "one", ("127.0.0.1", 1001)),
            BazelServerIdentity(102, "two", ("127.0.0.1", 1002)),
        )
    ]
    monkeypatch.setattr("tools.v2_oracle_lib.runner._read_bazel_server_identity", lambda _: BazelServerIdentity(103, "three", ("127.0.0.1", 1003)))
    monkeypatch.setattr("tools.v2_oracle_lib.runner.shutil.rmtree", lambda _: (_ for _ in ()).throw(OSError("purge failed")))
    with pytest.raises(OSError, match="purge failed"):
        run_fixture(fixture, tool, RunOptions(run_root=root / "purge-runs"))


def test_slug_epoch_observation_refuses_before_workspace_copy(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    fixture = Fixture(
        name="slug-refusal",
        root=Path("/fixture"),
        workspace=Path("/fixture/workspace"),
        expected=Path("/fixture/expected"),
        daemon=True,
        observe_server_epochs=True,
    )
    monkeypatch.setattr(
        "tools.v2_oracle_lib.runner._copy_workspace",
        lambda fixture, run_dir: pytest.fail("workspace copied"),
    )
    with pytest.raises(RuntimeError, match="authenticated Slug Status"):
        run_fixture(
            fixture,
            ToolConfig(name="slug", executable=Path("/slug")),
            RunOptions(run_root=scratch_dir("slug-refusal-run")),
        )


def test_bazel_shutdown_reuses_fixture_baseline_environment_cwd_and_waits(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    calls: list[tuple[list[str], Path, dict[str, str]]] = []
    waited: list[tuple[BazelServerIdentity, ...]] = []

    def fake_run(
        argv: list[str], **kwargs: object
    ) -> subprocess.CompletedProcess[str]:
        calls.append(
            (
                argv,
                Path(str(kwargs["cwd"])),
                dict(kwargs["env"]),  # type: ignore[arg-type]
            )
        )
        return subprocess.CompletedProcess(argv, 0, "", "")

    monkeypatch.setattr("tools.v2_oracle_lib.runner.subprocess.run", fake_run)
    monkeypatch.setattr(
        "tools.v2_oracle_lib.runner._wait_for_bazel_shutdown",
        lambda identities, timeout_seconds: waited.append(tuple(identities)),
    )
    identity = BazelServerIdentity(101, "one", ("127.0.0.1", 1001))
    _shutdown_bazel_daemon(
        ToolConfig(name="bazel", executable=Path("/usr/bin/bazel")),
        Path("/tmp/output-base"),
        Path("/tmp/workspace"),
        ("--nosystem_rc", "--noworkspace_rc", "--nohome_rc"),
        (("BAZELRC", ""),),
        (identity,),
        5,
    )
    assert calls[0][0] == [
        "/usr/bin/bazel",
        "--output_base=/tmp/output-base",
        "--nosystem_rc",
        "--noworkspace_rc",
        "--nohome_rc",
        "shutdown",
    ]
    assert calls[0][1] == Path("/tmp/workspace")
    assert calls[0][2]["BAZELRC"] == ""
    assert all("--host_jvm_args" not in arg for arg in calls[0][0])
    assert waited == [(identity,)]


def test_bazel_shutdown_exit_and_death_timeout_are_failures(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(
        "tools.v2_oracle_lib.runner.subprocess.run",
        lambda *args, **kwargs: subprocess.CompletedProcess(args[0], 2, "", "bad"),
    )
    with pytest.raises(RuntimeError, match="exit code 2"):
        _shutdown_bazel_daemon(
            ToolConfig(name="bazel", executable=Path("/usr/bin/bazel")),
            Path("/tmp/output-base"),
            Path("/tmp/workspace"),
            (),
            (),
            (),
            1,
        )
    identity = BazelServerIdentity(101, "one", ("127.0.0.1", 1001))
    monkeypatch.setattr(
        "tools.v2_oracle_lib.runner._process_identity_alive", lambda identity: True
    )
    monkeypatch.setattr(
        "tools.v2_oracle_lib.runner._endpoint_reachable", lambda endpoint: False
    )
    with pytest.raises(RuntimeError, match="did not terminate"):
        _wait_for_bazel_shutdown((identity,), 0)
    monkeypatch.setattr(
        "tools.v2_oracle_lib.runner._process_identity_alive", lambda identity: False
    )
    monkeypatch.setattr(
        "tools.v2_oracle_lib.runner._endpoint_reachable", lambda endpoint: True
    )
    with pytest.raises(RuntimeError, match="reachable endpoints=1"):
        _wait_for_bazel_shutdown((identity,), 0)


def test_runner_preserves_primary_failure_when_bazel_cleanup_also_fails(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    root = scratch_dir("startup-cleanup-chain")
    workspace = root / "fixture" / "workspace"
    workspace.mkdir(parents=True)
    fixture = Fixture(
        name="startup-cleanup-chain",
        root=workspace.parent,
        workspace=workspace,
        expected=workspace.parent / "expected",
        daemon=True,
        observe_server_epochs=True,
        commands=(
            FixtureCommand(
                name="execution-failure",
                argv=("query", "//:BUILD.bazel"),
                compare="semantic",
                mutations=(
                    Mutation(path="missing", find="old", replace="new"),
                ),
            ),
        ),
    )
    cleanup_error = RuntimeError("cleanup failure")
    monkeypatch.setattr(
        "tools.v2_oracle_lib.runner._shutdown_bazel_daemon",
        lambda *args: (_ for _ in ()).throw(cleanup_error),
    )
    with pytest.raises(FileNotFoundError) as failure:
        run_fixture(
            fixture,
            ToolConfig(name="bazel", executable=Path("/usr/bin/bazel")),
            RunOptions(run_root=root / "runs"),
        )
    assert failure.value.__cause__ is cleanup_error


def test_nonroot_startup_fixture_has_exact_22_command_contract() -> None:
    fixture = load_fixture(FIXTURES / "nonroot-interim-module-graph")
    assert fixture.startup_argv == (
        "--nosystem_rc",
        "--noworkspace_rc",
        "--nohome_rc",
    )
    assert fixture.env == (("BAZELRC", ""),)
    assert fixture.observe_server_epochs
    appended = fixture.commands[14:]
    assert [command.name for command in appended] == [
        "fresh_conflicting_last_wins_utf8",
        "fresh_conflicting_cli_startup_diagnostics",
        "reordered_same_multiset_retains_utf8",
        "reordered_cli_startup_diagnostics",
        "rc_source_change_same_multiset_reuses",
        "rc_source_startup_diagnostics",
        "occurrence_change_restarts_latin1",
        "zero_occurrences_restores_default",
    ]
    assert len(fixture.commands) == 22
    query_argv = (
        "query",
        "//:BUILD.bazel",
        "--lockfile_mode=off",
        "--registry=file://%workspace%/registry",
        "--output=label",
        "--noshow_progress",
    )
    assert [command.argv for command in appended if command.capture_startup_diagnostics] == [
        query_argv,
        query_argv,
        (*query_argv, "--announce_rc"),
    ]
    assert [len(command.mutations) for command in appended] == [
        7,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ]
    assert all(
        command.manifest_roots == ()
        for command in appended
        if command.capture_startup_diagnostics
    )
    assert all(command.stdout_patterns == (r"\A//:BUILD\.bazel\Z",) for command in appended if command.capture_startup_diagnostics)


def test_compare_checks_only_declared_startup_capture_fields() -> None:
    captured = FixtureCommand(
        name="captured",
        argv=("query", "//:BUILD.bazel"),
        compare="semantic",
        capture_server_epoch=True,
        capture_startup_diagnostics=True,
    )
    uncaptured = FixtureCommand(
        name="uncaptured", argv=("query", "//:BUILD.bazel"), compare="semantic"
    )
    fixture = Fixture(
        name="startup-compare",
        root=Path("/fixture"),
        workspace=Path("/fixture/workspace"),
        expected=Path("/fixture/expected"),
        commands=(captured, uncaptured),
    )
    expected = {
        "commands": [
            {
                "exit_code": 0,
                "manifest": [],
                "server_epoch": 2,
                "startup_combined_forms": ["one", "", "two"],
                "startup_announcements": ["announcement"], "stdout": "//:BUILD.bazel\n",
            },
            {"exit_code": 0, "manifest": [], "server_epoch": 99},
        ]
    }
    actual = {
        "tool": "bazel",
        "commands": [
            {
                "exit_code": 0,
                "normalized_stdout": "",
                "normalized_stderr": "",
                "manifest": [],
                "server_epoch": 3,
                "startup_combined_forms": ["one", "two"],
                "startup_announcements": [], "stdout": "//:BUILD.bazel\n",
            },
            {
                "exit_code": 0,
                "normalized_stdout": "",
                "normalized_stderr": "",
                "manifest": [],
                "server_epoch": 1,
            },
        ],
    }
    assert compare_result(fixture, actual, expected) == [
        "captured: server_epoch differs from oracle",
        "captured: startup_combined_forms differs from oracle",
        "captured: startup_announcements differs from oracle",
    ]
    actual["commands"][0]["stdout"] = "//:BUILD.bazel"
    assert "captured: stdout differs from oracle" in compare_result(fixture, actual, expected)
    missing = {"commands": [{"exit_code": 0, "manifest": []}, expected["commands"][1]]}
    failures = compare_result(fixture, actual, missing)
    assert "captured: server_epoch is missing from oracle" in failures
    assert "captured: startup_combined_forms is missing from oracle" in failures
    assert "captured: startup_announcements is missing from oracle" in failures


def test_normalize_text_strips_host_specific_noise() -> None:
    workspace = scratch_dir("normalize") / "workspace"
    workspace.mkdir()
    text = f"\x1b[31m{workspace}\\pkg\\x built in 1.25s id 123e4567-e89b-12d3-a456-426614174000\x1b[0m"
    normalized = normalize_text(text, path_replacements(workspace=workspace))
    assert "\x1b" not in normalized
    assert "<workspace>/pkg/x" in normalized
    assert "<duration>" in normalized
    assert "<uuid>" in normalized


def test_normalize_text_replaces_percent_encoded_workspace_uri() -> None:
    workspace = scratch_dir("normalize-uri") / "space café"
    workspace.mkdir()
    workspace_uri = workspace.resolve().as_uri()
    assert "%20" in workspace_uri
    assert "%C3%A9" in workspace_uri
    assert normalize_text(workspace_uri, path_replacements(workspace=workspace)) == "file://<workspace>"


def test_normalize_text_strips_prior_run_workspace_paths() -> None:
    text = (
        "old --workspace_directory=c:/users/walter gray/appdata/local/temp/slug-v2-oracle/"
        "runs/query-basic/20260626-201440-11824-bazel/workspace/pkg/BUILD.bazel\n"
        "new --workspace_directory=c:/dev/kuro/target/v2o/"
        "runs/query-basic/20260626-201458-16100-bazel/workspace"
    )
    normalized = normalize_text(text)
    assert "<workspace>/pkg/BUILD.bazel" in normalized
    assert "--workspace_directory=<workspace>" in normalized
    assert "slug-v2-oracle/runs" not in normalized
    assert "target/v2o/runs" not in normalized


def test_manifest_records_digest_and_mode() -> None:
    root = scratch_dir("manifest")
    output = root / "out.txt"
    output.write_bytes(b"hello\n")
    manifest = collect_manifest(root)
    file_entry = next(entry for entry in manifest if entry["path"] == "out.txt")
    assert file_entry["type"] == "file"
    assert file_entry["digest"] == "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03"
    assert file_entry["mode"].startswith("0o")


def test_compare_writes_compact_failure_artifact() -> None:
    fixture = load_fixture(FIXTURES / "version-bazel9")
    actual = {
        "commands": [
            {
                "exit_code": 0,
                "normalized_stdout": "Slug V1",
                "normalized_stderr": "",
                "manifest": [],
            }
        ]
    }
    failures = compare_result(fixture, actual, expected=None)
    assert failures
    artifact = write_failure_artifacts(scratch_dir("compare"), fixture, failures, actual, expected=None)
    assert (artifact / "failures.txt").is_file()
    assert "Slug V2" in (artifact / "failures.txt").read_text(encoding="utf-8")


def test_runner_copies_workspace_and_applies_mutation() -> None:
    tmp_path = scratch_dir("runner")
    fixture_root = tmp_path / "fixture"
    workspace = fixture_root / "workspace"
    expected = fixture_root / "expected"
    workspace.mkdir(parents=True)
    expected.mkdir()
    (workspace / "message.txt").write_text("one\n", encoding="utf-8")
    (fixture_root / "fixture.toml").write_text(
        """
[fixture]
name = "runner-smoke"
comparison = "message_shape"
manifest_roots = ["message.txt"]

[[commands]]
name = "print_message"
argv = ["-c", "import os; from pathlib import Path; print(Path('message.txt').read_text().strip()); print(os.environ['SLUG_ORACLE_ENV_SMOKE'])"]
compare = "message_shape"
expected_exit = 0
stdout_patterns = ["two", "env-set"]

[commands.env]
SLUG_ORACLE_ENV_SMOKE = "env-set"

[[commands.mutations]]
path = "message.txt"
find = "one"
replace = "two"
""".strip(),
        encoding="utf-8",
    )
    fixture = load_fixture(fixture_root)
    result = run_fixture(
        fixture,
        ToolConfig(name="slug", executable=Path(sys.executable)),
        RunOptions(run_root=tmp_path / "runs", timeout_seconds=30),
    )
    failures = compare_result(fixture, result, expected=None)
    assert failures == []
    assert (workspace / "message.txt").read_text(encoding="utf-8") == "one\n"
    assert result["commands"][0]["env_overrides"] == {"SLUG_ORACLE_ENV_SMOKE": "env-set"}
    assert result["commands"][0]["manifest"][0]["digest"]


def test_cli_list_outputs_fixture_names() -> None:
    completed = subprocess.run(
        [sys.executable, str(ROOT / "tools" / "v2_oracle"), "list"],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    assert completed.returncode == 0, completed.stderr
    assert "empty-module-build" in completed.stdout


def test_expected_oracle_metadata_is_documented() -> None:
    for fixture in discover_fixtures(FIXTURES):
        data = json.loads(fixture.expected_oracle.read_text(encoding="utf-8"))
        assert data["fixture"] == fixture.name
        assert isinstance(data["generated"], bool)
        if data["generated"]:
            assert data["tool"] == "bazel"
            assert data["commands"]
        else:
            assert data["oracle_notes"]

def test_validate_evidence_accepts_reapi_rows() -> None:
    evidence = scratch_dir("evidence") / "evidence.jsonl"
    evidence.write_text(
        json.dumps(
            {
                "executor_boundary": "reapi",
                "backend": "nativelink",
                "reapi_actions": 1,
                "direct_local_actions": 0,
                "action_digest": "abc/1",
                "uploaded_digests": ["def/2"],
                "materialized_outputs": ["ghi/3"],
                "what_ran": ["Remote"],
            }
        )
        + "\n",
        encoding="utf-8",
    )
    assert validate_evidence(evidence) == []


def test_validate_evidence_rejects_direct_local_rows() -> None:
    evidence = scratch_dir("evidence-bad") / "evidence.jsonl"
    evidence.write_text(
        json.dumps(
            {
                "executor_boundary": "local",
                "backend": "local",
                "reapi_actions": 0,
                "direct_local_actions": 1,
                "action_digests": [],
                "uploaded_digests": [],
                "materialized_outputs": [],
                "what_ran": ["Local"],
            }
        )
        + "\n",
        encoding="utf-8",
    )
    failures = validate_evidence(evidence)
    assert any("executor_boundary" in failure for failure in failures)
    assert any("direct_local_actions" in failure for failure in failures)
    assert any("forbidden what_ran" in failure for failure in failures)


def test_cli_validate_evidence_outputs_status() -> None:
    evidence = scratch_dir("evidence-cli") / "evidence.jsonl"
    evidence.write_text(
        json.dumps(
            {
                "executor_boundary": "reapi",
                "backend": "nativelink",
                "reapi_actions": 1,
                "direct_local_actions": 0,
                "action_digests": ["abc/1"],
                "uploaded_digests": ["def/2"],
                "materialized_outputs": ["ghi/3"],
                "what_ran": [],
            }
        )
        + "\n",
        encoding="utf-8",
    )
    completed = subprocess.run(
        [sys.executable, str(ROOT / "tools" / "v2_oracle"), "validate-evidence", str(evidence)],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    assert completed.returncode == 0, completed.stderr
    assert '"status": "ok"' in completed.stdout


def test_fixture_parser_reads_reapi_section() -> None:
    fixture = load_fixture(FIXTURES / "simple-rule-action")
    assert fixture.reapi.remote_executor is True


def test_extract_reapi_evidence_parses_slug_stderr() -> None:
    stderr = (
        '{"success":true,"command":"build","analyzed_target_count":1,'
        '"declared_action_count":1,"reapi_actions":1,"direct_local_actions":0,'
        '"ac_hits":0,"ac_misses":1,'
        '"action_digests":["abc/1"],"uploaded_digests":["def/2"],'
        '"materialized_outputs":["ghi/3"],"runtime_mode":"one-shot",'
        '"completed_boundary":"reapi_native_execution"}\n'
    )
    evidence = _extract_reapi_evidence(stderr)
    assert evidence is not None
    assert evidence["reapi_actions"] == 1
    assert evidence["direct_local_actions"] == 0
    assert evidence["action_digests"] == ["abc/1"]


def test_extract_reapi_evidence_returns_none_for_non_reapi_stderr() -> None:
    stderr = '{"error":"analysis_not_implemented","command":"build"}\n'
    assert _extract_reapi_evidence(stderr) is None


def _remote_fixture(command: FixtureCommand) -> Fixture:
    return Fixture(
        name="reapi-applicability",
        root=Path("/fixture"),
        workspace=Path("/fixture/workspace"),
        expected=Path("/fixture/expected"),
        commands=(command,),
        reapi=ReapiConfig(remote_executor=True),
    )


def _actual_command(
    command: FixtureCommand, *, actual_exit: int | None = None
) -> dict[str, object]:
    return {
        "tool": "slug",
        "commands": [
            {
                "exit_code": (
                    command.expected_exit if actual_exit is None else actual_exit
                ),
                "name": command.name,
                "normalized_stdout": "",
                "normalized_stderr": "",
                "manifest": [],
            }
        ],
    }


def test_successful_remote_build_gets_flags_and_requires_evidence() -> None:
    command = FixtureCommand(
        name="successful_build",
        argv=("build", "//pkg:probe"),
        compare="semantic",
        expected_exit=0,
    )
    argv = _slug_reapi_argv(
        ["slug", "build", "//pkg:probe"],
        command,
        "grpc://127.0.0.1:50051",
        ("cpu=x86_64",),
    )
    assert argv[-2:] == [
        "--remote_executor=grpc://127.0.0.1:50051",
        "--remote_default_exec_properties=cpu=x86_64",
    ]
    failures = compare_result(
        _remote_fixture(command), _actual_command(command), expected=None
    )
    assert any("REAPI evidence was not emitted" in failure for failure in failures)


def test_expected_failing_remote_build_gets_flags_without_evidence_requirement() -> None:
    command = FixtureCommand(
        name="failed_build",
        argv=("build", "//pkg:failure"),
        compare="message_shape",
        expected_exit=1,
    )
    argv = _slug_reapi_argv(
        ["slug", "build", "//pkg:failure"],
        command,
        "grpc://127.0.0.1:50051",
        (),
    )
    assert argv[-1] == "--remote_executor=grpc://127.0.0.1:50051"
    assert compare_result(
        _remote_fixture(command), _actual_command(command), expected=None
    ) == []


def test_reapi_evidence_applicability_is_independent_of_actual_exit() -> None:
    successful_contract = FixtureCommand(
        name="unexpected_build_failure",
        argv=("build", "//pkg:probe"),
        compare="message_shape",
        expected_exit=0,
    )
    failures = compare_result(
        _remote_fixture(successful_contract),
        _actual_command(successful_contract, actual_exit=1),
        expected=None,
    )
    assert any("exit code 1 != 0" in failure for failure in failures)
    assert any("REAPI evidence was not emitted" in failure for failure in failures)

    failing_contract = FixtureCommand(
        name="unexpected_build_success",
        argv=("build", "//pkg:failure"),
        compare="message_shape",
        expected_exit=1,
    )
    failures = compare_result(
        _remote_fixture(failing_contract),
        _actual_command(failing_contract, actual_exit=0),
        expected=None,
    )
    assert any("exit code 0 != 1" in failure for failure in failures)
    assert not any("REAPI evidence" in failure for failure in failures)


def test_remote_fixture_query_gets_neither_flags_nor_evidence_requirement() -> None:
    command = FixtureCommand(
        name="query",
        argv=("query", "//pkg:probe"),
        compare="message_shape",
        expected_exit=0,
    )
    base_argv = ["slug", "query", "//pkg:probe"]
    assert (
        _slug_reapi_argv(
            base_argv,
            command,
            "grpc://127.0.0.1:50051",
            ("cpu=x86_64",),
        )
        == base_argv
    )
    assert compare_result(
        _remote_fixture(command), _actual_command(command), expected=None
    ) == []


def test_compare_rejects_missing_evidence_for_remote_fixture() -> None:
    fixture = load_fixture(FIXTURES / "simple-rule-action")
    actual = {
        "tool": "slug",
        "commands": [
            {
                "exit_code": 0,
                "name": "build_write_file",
                "normalized_stdout": "",
                "normalized_stderr": "",
                "manifest": [],
            }
        ],
    }
    failures = compare_result(fixture, actual, expected=None)
    assert any("REAPI evidence" in failure for failure in failures)


def test_compare_accepts_valid_reapi_evidence() -> None:
    fixture = load_fixture(FIXTURES / "simple-rule-action")
    oracle_manifest = [
        {
            "digest": "dc5b456bbed0dafb1a5719d46d4484453b730745b12083e67b240c953e427a49",
            "mode": "0o555",
            "path": "write_file.txt",
            "root": "bazel-bin/pkg",
            "size": 21,
            "symlink_target": None,
            "type": "file",
        }
    ]
    expected = {
        "commands": [
            {
                "exit_code": 0,
                "manifest": oracle_manifest,
            }
        ]
    }
    actual = {
        "tool": "slug",
        "commands": [
            {
                "exit_code": 0,
                "name": "build_write_file",
                "normalized_stdout": "",
                "normalized_stderr": "",
                "manifest": oracle_manifest,
                "reapi_evidence": {
                    "reapi_actions": 1,
                    "direct_local_actions": 0,
                    "action_digests": ["abc/1"],
                    "uploaded_digests": ["def/2"],
                    "materialized_outputs": ["ghi/3"],
                },
            }
        ],
    }
    failures = compare_result(fixture, actual, expected=expected)
    assert failures == []


def test_fixture_parser_reads_platform_exec_properties() -> None:
    fixture = load_fixture(FIXTURES / "platform-exec-properties-reapi")
    assert fixture.reapi.remote_executor is True
    assert fixture.reapi.default_exec_properties == ("container-image=toolchain:v1",)
    assert fixture.reapi.worker_platform_properties == ("container-image=toolchain:v1",)


def test_compare_rejects_missing_platform_property_in_evidence() -> None:
    fixture = load_fixture(FIXTURES / "platform-exec-properties-reapi")
    oracle_manifest = [
        {
            "digest": "ac0cb855e0243634730f146e7b14a0dbc8ed0c3271e7b6ca4974c116a87f2a28",
            "mode": "0o555",
            "path": "probe.txt",
            "root": "bazel-bin/pkg",
            "size": 5,
            "symlink_target": None,
            "type": "file",
        }
    ]
    expected = {"commands": [{"exit_code": 0, "manifest": oracle_manifest}]}
    actual = {
        "tool": "slug",
        "commands": [
            {
                "exit_code": 0,
                "name": "build_probe",
                "normalized_stdout": "",
                "normalized_stderr": "",
                "manifest": oracle_manifest,
                "reapi_evidence": {
                    "reapi_actions": 1,
                    "direct_local_actions": 0,
                    "action_digests": ["abc/1"],
                    "uploaded_digests": ["def/2"],
                    "materialized_outputs": ["ghi/3"],
                    "platform_properties": {},
                },
            }
        ],
    }
    failures = compare_result(fixture, actual, expected=expected)
    assert any("container-image" in failure for failure in failures)


def test_compare_accepts_matching_platform_property_in_evidence() -> None:
    fixture = load_fixture(FIXTURES / "platform-exec-properties-reapi")
    oracle_manifest = [
        {
            "digest": "ac0cb855e0243634730f146e7b14a0dbc8ed0c3271e7b6ca4974c116a87f2a28",
            "mode": "0o555",
            "path": "probe.txt",
            "root": "bazel-bin/pkg",
            "size": 5,
            "symlink_target": None,
            "type": "file",
        }
    ]
    expected = {"commands": [{"exit_code": 0, "manifest": oracle_manifest}]}
    actual = {
        "tool": "slug",
        "commands": [
            {
                "exit_code": 0,
                "name": "build_probe",
                "normalized_stdout": "",
                "normalized_stderr": "",
                "manifest": oracle_manifest,
                "reapi_evidence": {
                    "reapi_actions": 1,
                    "direct_local_actions": 0,
                    "action_digests": ["abc/1"],
                    "uploaded_digests": ["def/2"],
                    "materialized_outputs": ["ghi/3"],
                    "platform_properties": {"container-image": "toolchain:v1"},
                },
            }
        ],
    }
    failures = compare_result(fixture, actual, expected=expected)
    assert failures == []


def test_fixture_parser_reads_daemon_flag() -> None:
    fixture = load_fixture(FIXTURES / "load-invalidation")
    assert fixture.daemon is True
    assert fixture.reapi.remote_executor is True


def test_fixture_parser_daemon_defaults_false() -> None:
    fixture = load_fixture(FIXTURES / "simple-rule-action")
    assert fixture.daemon is False


def test_reapi_evidence_comparison_accepts_ac_hit_with_empty_uploads() -> None:
    """An action-cache hit has no uploads (blobs already in CAS)."""
    fixture = load_fixture(FIXTURES / "reapi-action-cache-hit")
    oracle_manifest = [
        {
            "digest": "3673014e72b67383be302485694555a57ad393afdebaed6ded110a775bd0556d",
            "mode": "0o555",
            "path": "probe.txt",
            "root": "bazel-bin/pkg",
            "size": 6,
        }
    ]
    cmd_miss = {
        "name": "prime_cache",
        "exit_code": 0,
        "manifest": oracle_manifest,
        "reapi_evidence": {
            "reapi_actions": 1,
            "direct_local_actions": 0,
            "ac_hits": 0,
            "ac_misses": 1,
            "action_digests": ["abc/140"],
            "uploaded_digests": ["def/10"],
            "materialized_outputs": ["3673014e.../6"],
        },
    }
    cmd_hit = {
        "name": "replay_cache",
        "exit_code": 0,
        "manifest": oracle_manifest,
        "reapi_evidence": {
            "reapi_actions": 1,
            "direct_local_actions": 0,
            "ac_hits": 1,
            "ac_misses": 0,
            "action_digests": ["abc/140"],
            "uploaded_digests": [],
            "materialized_outputs": ["3673014e.../6"],
        },
    }
    actual = {"tool": "slug", "commands": [cmd_miss, cmd_hit]}
    expected = {
        "commands": [
            {"name": "prime_cache", "exit_code": 0, "manifest": oracle_manifest},
            {"name": "replay_cache", "exit_code": 0, "manifest": oracle_manifest},
        ]
    }
    failures = compare_result(fixture, actual, expected=expected)
    assert failures == []


def test_reapi_evidence_comparison_rejects_empty_uploads_on_ac_miss() -> None:
    """An AC miss must have nonempty uploaded_digests."""
    fixture = load_fixture(FIXTURES / "reapi-action-cache-hit")
    oracle_manifest = [
        {
            "digest": "3673014e72b67383be302485694555a57ad393afdebaed6ded110a775bd0556d",
            "mode": "0o555",
            "path": "probe.txt",
            "root": "bazel-bin/pkg",
            "size": 6,
        }
    ]
    cmd_miss_empty_uploads = {
        "name": "prime_cache",
        "exit_code": 0,
        "manifest": oracle_manifest,
        "reapi_evidence": {
            "reapi_actions": 1,
            "direct_local_actions": 0,
            "ac_hits": 0,
            "ac_misses": 1,
            "action_digests": ["abc/140"],
            "uploaded_digests": [],
            "materialized_outputs": ["3673014e.../6"],
        },
    }
    cmd_hit = {
        "name": "replay_cache",
        "exit_code": 0,
        "manifest": oracle_manifest,
        "reapi_evidence": {
            "reapi_actions": 1,
            "direct_local_actions": 0,
            "ac_hits": 1,
            "ac_misses": 0,
            "action_digests": ["abc/140"],
            "uploaded_digests": [],
            "materialized_outputs": ["3673014e.../6"],
        },
    }
    actual = {"tool": "slug", "commands": [cmd_miss_empty_uploads, cmd_hit]}
    expected = {
        "commands": [
            {"name": "prime_cache", "exit_code": 0, "manifest": oracle_manifest},
            {"name": "replay_cache", "exit_code": 0, "manifest": oracle_manifest},
        ]
    }
    failures = compare_result(fixture, actual, expected=expected)
    assert any("uploaded_digests must be nonempty" in f for f in failures)
