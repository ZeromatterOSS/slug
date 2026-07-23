from __future__ import annotations

import json
import os
import subprocess
import sys
import uuid
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from tools.v2_oracle_lib.compare import compare_result, write_failure_artifacts
from tools.v2_oracle_lib.evidence import validate_evidence
from tools.v2_oracle_lib.fixture import FixtureCommand, Mutation, discover_fixtures, load_fixture
from tools.v2_oracle_lib.manifest import collect_manifest
from tools.v2_oracle_lib.normalize import normalize_text, path_replacements
from tools.v2_oracle_lib.runner import (
    RunOptions,
    ToolConfig,
    _apply_mutations,
    _extract_reapi_evidence,
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
    assert len(fixture.commands) == 2
    assert fixture.commands[1].mutations[0].path == "pkg/message.bzl"
    assert fixture.commands[1].mutations[0].replace == 'MESSAGE = "two"'


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
    outside = root / "outside"
    outside.mkdir()
    (workspace / "escape").symlink_to(outside, target_is_directory=True)
    with pytest.raises(ValueError, match="escapes workspace"):
        _apply_mutations(
            workspace,
            mutation_command(Mutation(path="escape/x.txt", op="create", content="nope\n")),
        )


def test_normalize_text_strips_host_specific_noise() -> None:
    workspace = scratch_dir("normalize") / "workspace"
    workspace.mkdir()
    text = f"\x1b[31m{workspace}\\pkg\\x built in 1.25s id 123e4567-e89b-12d3-a456-426614174000\x1b[0m"
    normalized = normalize_text(text, path_replacements(workspace=workspace))
    assert "\x1b" not in normalized
    assert "<workspace>/pkg/x" in normalized
    assert "<duration>" in normalized
    assert "<uuid>" in normalized


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
