# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict

"""Plan 61 bzlmod validation guardrails."""

import base64
import hashlib
import json
import re
from pathlib import Path

import pytest
from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.api.buck_result import BuckException
from buck2.tests.e2e_util.buck_workspace import buck_test


BzlmodCounters = dict[str, int]


async def _bzlmod_counters(
    buck: Buck,
    *args: str,
    rel_cwd: Path | None = None,
    env: dict[str, str] | None = None,
) -> BzlmodCounters:
    result = await buck.audit("bzlmod-counters", *args, rel_cwd=rel_cwd, env=env)
    counters = json.loads(result.stdout)
    assert isinstance(counters, dict)
    return counters


async def _audit_cells_and_counters(
    buck: Buck,
    rel_cwd: Path | None = None,
) -> tuple[str, BzlmodCounters]:
    result = await buck.audit("cell", rel_cwd=rel_cwd)
    return result.stdout, await _bzlmod_counters(buck, rel_cwd=rel_cwd)


def _write(path: Path, content: str) -> None:
    path.write_text(content)


def _write_minimal_lockfile(path: Path) -> None:
    _write(
        path,
        json.dumps(
            {
                "lockFileVersion": 26,
                "registryFileHashes": {},
                "selectedYankedVersions": {},
                "moduleExtensions": {},
                "facts": {},
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
    )


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _protobuf_varint(value: int) -> bytes:
    out = bytearray()
    while value >= 0x80:
        out.append((value & 0x7F) | 0x80)
        value >>= 7
    out.append(value)
    return bytes(out)


def _bazel_fingerprint_add_strings(strings: list[str]) -> str:
    payload = bytearray()
    _extend_bazel_fingerprint_add_strings(payload, strings)
    return hashlib.sha256(payload).hexdigest()


def _extend_bazel_fingerprint_add_strings(payload: bytearray, strings: list[str]) -> None:
    payload.extend(_protobuf_varint(len(strings)))
    for string in strings:
        encoded = string.encode()
        payload.extend(_protobuf_varint(len(encoded)))
        payload.extend(encoded)


def _dirents_digest(path: Path) -> str:
    return _bazel_fingerprint_add_strings(sorted(child.name for child in path.iterdir()))


def _dirtree_digest(path: Path) -> str:
    names = sorted(child.name for child in path.iterdir())
    subdir_digests: list[str] = []
    file_values: list[tuple[int, bytes | None]] = []
    for name in names:
        child = path / name
        if child.is_dir():
            subdir_digests.append(_dirtree_digest(child))
            file_values.append((2, None))
        elif child.is_file():
            file_values.append((0, hashlib.sha256(child.read_bytes()).digest()))
        else:
            file_values.append((1, None))

    payload = bytearray()
    _extend_bazel_fingerprint_add_strings(payload, names)
    _extend_bazel_fingerprint_add_strings(payload, subdir_digests)
    for file_state_type_ordinal, digest in file_values:
        payload.extend(_protobuf_varint(file_state_type_ordinal))
        if digest is not None:
            payload.extend(digest)
    return hashlib.sha256(payload).hexdigest()


def _slug_bzl_transitive_digest(
    extension_id: str,
    project_root: Path | None = None,
) -> str:
    if project_root is not None:
        root_bzl = _extension_bzl_path(extension_id, project_root)
        if root_bzl is not None and root_bzl.is_file():
            seen: set[Path] = set()

            def collect(path: Path) -> None:
                path = path.resolve()
                if path in seen:
                    return
                seen.add(path)
                content = path.read_text()
                for load in re.findall(r"""load\(\s*["']([^"']+)["']""", content):
                    loaded = _label_bzl_path(load, project_root, path.parent)
                    if loaded is not None and loaded.is_file():
                        collect(loaded)

            collect(root_bzl)
            if seen:
                hasher = hashlib.sha256()
                hasher.update(b"bzl_transitive_v2:")
                hasher.update(extension_id.encode())
                hasher.update(b"\0")
                for path in sorted(seen):
                    hasher.update(path.relative_to(project_root).as_posix().encode())
                    hasher.update(b"\0")
                    hasher.update(path.read_bytes())
                    hasher.update(b"\0")
                return base64.b64encode(hasher.digest()).decode()

    digest = hashlib.sha256(b"bzl_transitive_v1:" + extension_id.encode()).digest()
    return base64.b64encode(digest).decode()


def _extension_bzl_path(extension_id: str, project_root: Path) -> Path | None:
    return _label_bzl_path(extension_id.split("%", 1)[0], project_root, None)


def _label_bzl_path(
    label: str,
    project_root: Path,
    current_dir: Path | None,
) -> Path | None:
    if label.startswith("@@"):
        if "//" not in label:
            return None
        target = label.split("//", 1)[1]
    elif label.startswith("@"):
        if "//" not in label:
            return None
        target = label.split("//", 1)[1]
    elif label.startswith("//"):
        target = label[2:]
    elif label.startswith(":"):
        return current_dir / label[1:] if current_dir is not None else None
    elif "//" in label:
        target = label.split("//", 1)[1]
    else:
        return current_dir / label if current_dir is not None else None

    if ":" not in target:
        return None
    package, name = target.split(":", 1)
    return project_root / package / name if package else project_root / name


def _slug_usages_digest_without_tags(extension_id: str, module_name: str) -> str:
    digest = hashlib.sha256(extension_id.encode() + module_name.encode()).digest()
    return base64.b64encode(digest).decode()


def _slug_usages_digest(
    extension_id: str,
    tags_by_module: dict[str, list[tuple[str, dict[str, object]]]],
) -> str:
    hasher = hashlib.sha256()
    hasher.update(extension_id.encode())
    for module_name in sorted(tags_by_module):
        hasher.update(module_name.encode())
        tags = tags_by_module[module_name]
        for tag_name, kwargs in sorted(tags, key=_tag_hash_input):
            hasher.update(_tag_hash_input((tag_name, kwargs)))
    return base64.b64encode(hasher.digest()).decode()


def _tag_hash_input(tag: tuple[str, dict[str, object]]) -> bytes:
    tag_name, kwargs = tag
    out = bytearray()
    out.extend(b"tag:")
    out.extend(tag_name.encode())
    out.extend(b"\0")
    for key, value in sorted(kwargs.items()):
        out.extend(b"kw:")
        out.extend(key.encode())
        out.extend(b"=")
        out.extend(_tag_value_hash_input(value))
        out.extend(b"\0")
    return bytes(out)


def _tag_value_hash_input(value: object) -> bytes:
    if isinstance(value, bool):
        return b"bool:" + bytes([int(value)])
    if isinstance(value, int):
        return b"int:" + value.to_bytes(8, byteorder="little", signed=True)
    if isinstance(value, str):
        if value.startswith(("//", "@", ":")):
            return b"label:" + value.encode()
        return b"string:" + value.encode()
    if value is None:
        return b"none"
    if isinstance(value, list):
        out = bytearray()
        out.extend(b"list:")
        out.extend(len(value).to_bytes(8, byteorder="little", signed=False))
        for item in value:
            out.extend(_tag_value_hash_input(item))
            out.extend(b"\0")
        return bytes(out)
    if isinstance(value, dict):
        out = bytearray()
        out.extend(b"dict:")
        out.extend(len(value).to_bytes(8, byteorder="little", signed=False))
        for key, item in sorted(value.items()):
            out.extend(str(key).encode())
            out.extend(b"=")
            out.extend(_tag_value_hash_input(item))
            out.extend(b"\0")
        return bytes(out)
    raise TypeError(f"unsupported tag value for Plan 61 digest: {value!r}")


def _write_replay_lockfile(
    path: Path,
    *,
    extension_id: str,
    module_name: str,
    project_root: Path,
    repo_path: Path,
    recorded_inputs: list[str] | None = None,
    repo_paths: dict[str, Path] | None = None,
    facts: dict[str, object] | None = None,
) -> None:
    generated_repo_specs = {
        repo_name: {
            "repoRuleId": (
                "@@bazel_tools//tools/build_defs/repo:"
                "local.bzl%local_repository"
            ),
            "attributes": {
                "path": str(path),
            },
        }
        for repo_name, path in (repo_paths or {"replayed_repo": repo_path}).items()
    }
    _write(
        path,
        json.dumps(
            {
                "lockFileVersion": 26,
                "registryFileHashes": {},
                "selectedYankedVersions": {},
                "moduleExtensions": {
                    extension_id: {
                        "general": {
                            "bzlTransitiveDigest": _slug_bzl_transitive_digest(
                                extension_id,
                                project_root,
                            ),
                            "usagesDigest": _slug_usages_digest_without_tags(
                                extension_id, module_name
                            ),
                            "recordedInputs": recorded_inputs or [],
                            "generatedRepoSpecs": generated_repo_specs,
                            "moduleExtensionMetadata": None,
                        },
                    },
                },
                "facts": (
                    {extension_id: facts}
                    if facts is not None
                    else {}
                ),
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
    )


def _skip(reason: str) -> pytest.MarkDecorator:
    return pytest.mark.skip(reason=reason)


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_root_module_bazel_edit_invalidates_bzlmod_graph(buck: Buck) -> None:
    """Bazel anchors: ModuleFileFunction.java and BazelModuleResolutionFunction.java."""
    before = await _bzlmod_counters(buck)
    output, first = await _audit_cells_and_counters(buck)
    assert "plan61_guardrails" in output
    assert first["module_file_parse"] > before["module_file_parse"]
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_guardrails_edited")\n""",
    )

    output, second = await _audit_cells_and_counters(buck)
    assert "plan61_guardrails_edited" in output
    assert second["module_file_parse"] > first["module_file_parse"]
    assert second["bzlmod_resolution_compute"] > first["bzlmod_resolution_compute"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_local_override_module_edit_invalidates_only_affected_nodes(
    buck: Buck,
) -> None:
    """Bazel anchors: ModuleFileFunction.java and Bazel module override docs."""
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_guardrails")

bazel_dep(name = "local_lib")
local_path_override(
    module_name = "local_lib",
    path = "libs/local_lib",
)
""",
    )

    before = await _bzlmod_counters(buck)
    output, first = await _audit_cells_and_counters(buck)
    assert "local_lib" in output
    assert first["module_file_parse"] > before["module_file_parse"]
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    _write(
        buck.cwd / "libs/local_lib/MODULE.bazel",
        """module(name = "local_lib", version = "1.1")\n""",
    )

    output, second = await _audit_cells_and_counters(buck)
    assert "local_lib" in output
    assert second["module_file_parse"] > first["module_file_parse"]
    assert second["bzlmod_resolution_compute"] > first["bzlmod_resolution_compute"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_included_module_segment_edit_invalidates_bzlmod_graph(
    buck: Buck,
) -> None:
    """Bazel anchors: ModuleFileFunction.java include handling and ModuleFileValue.java inputs."""
    _write(
        buck.cwd / "deps.MODULE.bazel",
        "# initially empty included module segment\n",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_guardrails")
include("//:deps.MODULE.bazel")
""",
    )

    before = await _bzlmod_counters(buck)
    output, first = await _audit_cells_and_counters(buck)
    assert "plan61_guardrails" in output
    assert "included_lib" not in output
    assert first["module_file_parse"] > before["module_file_parse"]
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    included_lib = buck.cwd / "libs/included_lib"
    included_lib.mkdir(parents=True)
    _write(included_lib / "MODULE.bazel", 'module(name = "included_lib", version = "1.0")\n')
    _write(included_lib / "BUILD.bazel", 'filegroup(name = "included", srcs = [])\n')
    _write(
        buck.cwd / "deps.MODULE.bazel",
        """bazel_dep(name = "included_lib")
local_path_override(
    module_name = "included_lib",
    path = "libs/included_lib",
)
""",
    )

    output, second = await _audit_cells_and_counters(buck)
    assert "included_lib" in output
    assert second["module_file_parse"] > first["module_file_parse"]
    assert second["bzlmod_resolution_compute"] > first["bzlmod_resolution_compute"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_included_module_segment_variables_do_not_leak_to_root(
    buck: Buck,
) -> None:
    """Bazel anchor: ModuleFileGlobals.java include variable-binding semantics."""
    _write(
        buck.cwd / "ext.MODULE.bazel",
        """ext = use_extension("//:defs.bzl", "ext")
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_include_scope")
include("//:ext.MODULE.bazel")
use_repo(ext, "generated_repo")
""",
    )

    with pytest.raises(BuckException) as exc:
        await buck.audit("cell")

    assert "ext" in exc.value.stderr


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_included_module_segment_create_delete_invalidates_bzlmod_graph(
    buck: Buck,
) -> None:
    """Bazel anchors: ModuleFileFunction.java include lookup and ModuleFileValue inputs."""
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_include_create_delete")
include("//:deps.MODULE.bazel")
""",
    )

    with pytest.raises(BuckException):
        await buck.audit("cell")

    created_lib = buck.cwd / "libs/created_include_lib"
    created_lib.mkdir(parents=True)
    _write(
        created_lib / "MODULE.bazel",
        'module(name = "created_include_lib", version = "1.0")\n',
    )
    _write(created_lib / "BUILD.bazel", 'filegroup(name = "created", srcs = [])\n')
    _write(
        buck.cwd / "deps.MODULE.bazel",
        """bazel_dep(name = "created_include_lib")
local_path_override(
    module_name = "created_include_lib",
    path = "libs/created_include_lib",
)
""",
    )

    output, created = await _audit_cells_and_counters(buck)
    assert "created_include_lib" in output
    assert created["module_file_parse"] > 0
    assert created["bzlmod_resolution_compute"] > 0

    (buck.cwd / "deps.MODULE.bazel").unlink()
    with pytest.raises(BuckException):
        await buck.audit("cell")


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_two_workspaces_do_not_share_bzlmod_state(
    buck: Buck,
) -> None:
    """Bazel anchors: output-base-per-workspace plus BazelDepGraphValue/ModuleKey."""
    first_root = Path("two_workspaces/first")
    second_root = Path("two_workspaces/second")

    before = await _bzlmod_counters(buck, rel_cwd=first_root)
    first_output, first = await _audit_cells_and_counters(buck, rel_cwd=first_root)
    first_daemon_dir = (await buck.debug("daemon-dir", rel_cwd=first_root)).stdout.strip()
    assert "shared_plan61_workspace" in first_output
    assert "first_only_lib" in first_output
    assert "second_only_lib" not in first_output
    assert "shared_generated" in first_output
    assert first["module_file_parse"] > before["module_file_parse"]
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    second_before = await _bzlmod_counters(buck, rel_cwd=second_root)
    second_output, second = await _audit_cells_and_counters(buck, rel_cwd=second_root)
    second_daemon_dir = (await buck.debug("daemon-dir", rel_cwd=second_root)).stdout.strip()
    assert "shared_plan61_workspace" in second_output
    assert "second_only_lib" in second_output
    assert "first_only_lib" not in second_output
    assert "shared_generated" in second_output

    if first_daemon_dir != second_daemon_dir:
        assert second["module_file_parse"] > second_before["module_file_parse"]
        assert (
            second["bzlmod_resolution_compute"]
            > second_before["bzlmod_resolution_compute"]
        )
    else:
        assert second["module_file_parse"] > first["module_file_parse"]
        assert second["bzlmod_resolution_compute"] > first["bzlmod_resolution_compute"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_visible_lockfile_read_is_observable_and_ordinary_audit_is_read_only(
    buck: Buck,
) -> None:
    """Bazel anchors: BazelLockFileValue.java KEY and BazelLockFileFunction.java."""
    lockfile = buck.cwd / "MODULE.bazel.lock"
    _write_minimal_lockfile(lockfile)
    before_sha = _sha256(lockfile)

    output = (await buck.audit("cell")).stdout
    after = await _bzlmod_counters(buck)

    assert "plan61_guardrails" in output
    assert after["lockfile_read"] > 0
    assert after["lockfile_write_attempt"] == 0
    assert _sha256(lockfile) == before_sha


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_hidden_lockfile_read_is_observable_before_extension_replay(
    buck: Buck,
) -> None:
    """Bazel anchors: BazelLockFileValue.HIDDEN_KEY and SingleExtensionEvalFunction."""
    daemon_dir = Path((await buck.debug("daemon-dir")).stdout.strip())
    hidden_lockfile = daemon_dir / "MODULE.bazel.lock"
    before = await _bzlmod_counters(buck, "--lockfile_mode=off")

    hidden_lockfile.parent.mkdir(parents=True, exist_ok=True)
    _write_minimal_lockfile(hidden_lockfile)

    await buck.audit("cell")
    after = await _bzlmod_counters(buck, "--lockfile_mode=off")

    assert after["lockfile_read"] > before["lockfile_read"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_malformed_hidden_lockfile_is_ignored(
    buck: Buck,
) -> None:
    """Bazel anchor: BazelLockFileFunction treats hidden parse failures as empty."""
    daemon_dir = Path((await buck.debug("daemon-dir")).stdout.strip())
    hidden_lockfile = daemon_dir / "MODULE.bazel.lock"
    before = await _bzlmod_counters(buck, "--lockfile_mode=off")

    hidden_lockfile.parent.mkdir(parents=True, exist_ok=True)
    _write(hidden_lockfile, "{ this is not json }\n")

    output = (await buck.audit("cell")).stdout
    after = await _bzlmod_counters(buck, "--lockfile_mode=off")

    assert "plan61_guardrails" in output
    assert after["lockfile_read"] > before["lockfile_read"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_lockfile_mode_off_does_not_read_lockfiles(buck: Buck) -> None:
    """Bazel anchor: SingleExtensionEvalFunction skips lockfiles in OFF mode."""
    _write_minimal_lockfile(buck.cwd / "MODULE.bazel.lock")

    await buck.audit("cell", "--lockfile_mode=off")
    after = await _bzlmod_counters(buck, "--lockfile_mode=off")

    assert after["lockfile_read"] == 0


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_lockfile_mode_error_rejects_invalid_visible_lockfile(
    buck: Buck,
) -> None:
    """Bazel anchor: BazelLockFileFunction errors on incompatible lockfiles in ERROR mode."""
    _write(buck.cwd / "MODULE.bazel.lock", "{ this is not json }\n")

    with pytest.raises(BuckException):
        await buck.audit("cell", "--lockfile_mode=error")


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_lockfile_mode_error_rejects_changed_extension_facts(
    buck: Buck,
) -> None:
    """Bazel anchor: SingleExtensionEvalFunction ERROR-mode facts validation."""
    extension_id = "@@plan61_facts_error+//:facts_ext.bzl%facts_ext"
    lockfile = buck.cwd / "MODULE.bazel.lock"
    _write(
        buck.cwd / "facts_ext.bzl",
        """def _repo_impl(repository_ctx):
    repository_ctx.file("data.txt", "facts repo payload\\n")
    repository_ctx.file("BUILD.bazel", "exports_files([\\"data.txt\\"])\\nfilegroup(name = \\"data\\", srcs = [\\"data.txt\\"])\\n")

facts_repo_rule = repository_rule(
    implementation = _repo_impl,
)

def _facts_ext_impl(module_ctx):
    facts_repo_rule(name = "facts_repo")
    return module_ctx.extension_metadata(
        facts = {"resource": {"checksum": "new"}},
    )

facts_ext = module_extension(
    implementation = _facts_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_facts_error")

facts = use_extension("//:facts_ext.bzl", "facts_ext")
use_repo(facts, "facts_repo")
""",
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_facts_repo",
    srcs = ["@facts_repo//:data"],
)
""",
    )
    _write(
        lockfile,
        json.dumps(
            {
                "lockFileVersion": 26,
                "registryFileHashes": {},
                "selectedYankedVersions": {},
                "moduleExtensions": {},
                "facts": {
                    extension_id: {"resource": {"checksum": "old"}},
                },
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
    )
    before_sha = _sha256(lockfile)
    before = await _bzlmod_counters(buck, "--lockfile_mode=error")

    failure_stderr: str | None = None
    try:
        await buck.build("//:uses_facts_repo", "--lockfile_mode=error")
    except BuckException as e:
        failure_stderr = e.stderr
    after = await _bzlmod_counters(buck, "--lockfile_mode=error")

    if failure_stderr is None:
        pytest.fail("facts mismatch build unexpectedly succeeded")

    assert "MODULE.bazel.lock is no longer up-to-date" in failure_stderr
    assert "has changed its facts" in failure_stderr
    assert '"checksum":"new"' in failure_stderr
    assert '"checksum":"old"' in failure_stderr
    assert "bazel mod deps --lockfile_mode=update" in failure_stderr
    assert after["extension_eval"] > before["extension_eval"]
    assert after["lockfile_write_attempt"] == before["lockfile_write_attempt"]
    assert _sha256(lockfile) == before_sha


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_default_lockfile_mode_rejects_invalid_extension_digest(
    buck: Buck,
) -> None:
    """Bazel anchor: BazelLockFileFunction rejects non-Base64 extension digests."""
    digest = base64.b64encode(bytes(32)).decode()
    _write(
        buck.cwd / "MODULE.bazel.lock",
        json.dumps(
            {
                "lockFileVersion": 26,
                "registryFileHashes": {},
                "selectedYankedVersions": {},
                "moduleExtensions": {
                    "@@plan61_guardrails+//:replay_ext.bzl%replay_ext": {
                        "general": {
                            "bzlTransitiveDigest": f"sha256-{digest}",
                            "usagesDigest": digest,
                            "recordedInputs": [],
                            "generatedRepoSpecs": {},
                            "moduleExtensionMetadata": None,
                        },
                    },
                },
                "facts": {},
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
    )

    with pytest.raises(BuckException):
        await buck.audit("cell")


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_extension_bzl_edit_invalidates_or_rejects_replay(
    buck: Buck,
) -> None:
    """Bazel anchors: RegularRunnableExtension and SingleExtensionEvalFunction."""
    module_name = "plan61_replay"
    extension_id = "@plan61_replay//:replay_ext.bzl%replay_ext"
    replayed_repo = buck.cwd / "replayed_repo"
    replayed_repo.mkdir(exist_ok=True)
    _write(replayed_repo / "BUILD.bazel", "filegroup(name = \"data\")\n")
    _write(
        buck.cwd / "replay_ext.bzl",
        """def _replay_ext_impl(module_ctx):
    fail("extension should replay from lockfile")

replay_ext = module_extension(
    implementation = _replay_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "{module_name}")

replay = use_extension("//:replay_ext.bzl", "replay_ext")
use_repo(replay, "replayed_repo")
""",
    )
    _write_replay_lockfile(
        buck.cwd / "MODULE.bazel.lock",
        extension_id=extension_id,
        module_name=module_name,
        project_root=buck.cwd,
        repo_path=replayed_repo,
    )

    before = await _bzlmod_counters(buck)
    await buck.audit("cell")
    first = await _bzlmod_counters(buck)
    assert first["extension_replay_hit"] > before["extension_replay_hit"]
    assert first["extension_eval"] == before["extension_eval"]

    _write(
        buck.cwd / "replay_ext.bzl",
        """def _replay_ext_impl(module_ctx):
    fail("edited extension should make the lockfile replay stale")

replay_ext = module_extension(
    implementation = _replay_ext_impl,
)
""",
    )

    await buck.audit("cell")
    second = await _bzlmod_counters(buck)

    assert second["extension_replay_miss_reason"] > first["extension_replay_miss_reason"]
    assert second["extension_replay_hit"] == first["extension_replay_hit"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_transitive_extension_bzl_edit_invalidates_or_rejects_replay(
    buck: Buck,
) -> None:
    """Bazel anchors: RegularRunnableExtension and SingleExtensionEvalFunction."""
    module_name = "plan61_transitive_replay"
    extension_id = "@plan61_transitive_replay//:replay_ext.bzl%replay_ext"
    replayed_repo = buck.cwd / "replayed_repo"
    replayed_repo.mkdir(exist_ok=True)
    _write(replayed_repo / "BUILD.bazel", "filegroup(name = \"data\")\n")
    _write(
        buck.cwd / "replay_helper.bzl",
        """HELPER_SENTINEL = "initial helper digest input"
""",
    )
    _write(
        buck.cwd / "replay_ext.bzl",
        """load("//:replay_helper.bzl", "HELPER_SENTINEL")

def _replay_ext_impl(module_ctx):
    fail("extension should replay from lockfile: %s" % HELPER_SENTINEL)

replay_ext = module_extension(
    implementation = _replay_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "{module_name}")

replay = use_extension("//:replay_ext.bzl", "replay_ext")
use_repo(replay, "replayed_repo")
""",
    )
    _write_replay_lockfile(
        buck.cwd / "MODULE.bazel.lock",
        extension_id=extension_id,
        module_name=module_name,
        project_root=buck.cwd,
        repo_path=replayed_repo,
    )

    before = await _bzlmod_counters(buck)
    await buck.audit("cell")
    first = await _bzlmod_counters(buck)
    assert first["extension_replay_hit"] > before["extension_replay_hit"]
    assert first["extension_eval"] == before["extension_eval"]

    _write(
        buck.cwd / "replay_helper.bzl",
        """HELPER_SENTINEL = "edited helper digest input"
""",
    )

    await buck.audit("cell")
    second = await _bzlmod_counters(buck)

    assert second["extension_replay_miss_reason"] > first["extension_replay_miss_reason"]
    assert second["extension_replay_hit"] == first["extension_replay_hit"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_valid_lockfile_replay_materializes_generated_repo_without_extension_eval(
    buck: Buck,
) -> None:
    """Bazel anchor: SingleExtensionEvalFunction valid lockfile replay path."""
    module_name = "plan61_replay_materialization"
    extension_id = "@plan61_replay_materialization//:replay_ext.bzl%replay_ext"
    replayed_repo = buck.cwd / "replayed_repo"
    replayed_repo.mkdir(exist_ok=True)
    _write(replayed_repo / "data.txt", "replayed repo payload\n")
    _write(
        replayed_repo / "BUILD.bazel",
        """exports_files(["data.txt"])
filegroup(name = "data", srcs = ["data.txt"])
""",
    )
    _write(
        buck.cwd / "replay_ext.bzl",
        """def _replay_ext_impl(module_ctx):
    fail("extension should not execute when generatedRepoSpecs replay is valid")

replay_ext = module_extension(
    implementation = _replay_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "{module_name}")

replay = use_extension("//:replay_ext.bzl", "replay_ext")
use_repo(replay, "replayed_repo")
""",
    )
    _write_replay_lockfile(
        buck.cwd / "MODULE.bazel.lock",
        extension_id=extension_id,
        module_name=module_name,
        project_root=buck.cwd,
        repo_path=replayed_repo,
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_replayed_repo",
    srcs = ["@replayed_repo//:data"],
)
""",
    )

    before = await _bzlmod_counters(buck)
    await buck.build("//:uses_replayed_repo")
    after = await _bzlmod_counters(buck)

    assert after["extension_replay_hit"] > before["extension_replay_hit"]
    assert after["extension_eval"] == before["extension_eval"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_lockfile_replay_recorded_file_input_edit_rejects_cache(
    buck: Buck,
) -> None:
    """Bazel anchors: RepoRecordedInput.File and SingleExtensionEvalFunction."""
    module_name = "plan61_recorded_file"
    extension_id = "@plan61_recorded_file//:replay_ext.bzl%replay_ext"
    watched = buck.cwd / "watched.txt"
    _write(watched, "first\n")
    replayed_repo = buck.cwd / "replayed_repo"
    replayed_repo.mkdir(exist_ok=True)
    _write(replayed_repo / "data.txt", "replayed repo payload\n")
    _write(
        replayed_repo / "BUILD.bazel",
        """exports_files(["data.txt"])
filegroup(name = "data", srcs = ["data.txt"])
""",
    )
    _write(
        buck.cwd / "replay_ext.bzl",
        """def _replay_ext_impl(module_ctx):
    fail("recorded input replay rejected")

replay_ext = module_extension(
    implementation = _replay_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "{module_name}")

replay = use_extension("//:replay_ext.bzl", "replay_ext")
use_repo(replay, "replayed_repo")
""",
    )
    _write_replay_lockfile(
        buck.cwd / "MODULE.bazel.lock",
        extension_id=extension_id,
        module_name=module_name,
        project_root=buck.cwd,
        repo_path=replayed_repo,
        recorded_inputs=[f"FILE:@@//watched.txt {_sha256(watched)}"],
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_replayed_repo",
    srcs = ["@replayed_repo//:data"],
)
""",
    )

    before = await _bzlmod_counters(buck)
    await buck.audit("cell")
    first = await _bzlmod_counters(buck)

    assert first["extension_replay_hit"] > before["extension_replay_hit"]
    assert first["extension_eval"] == before["extension_eval"]

    _write(watched, "second\n")

    await buck.audit("cell")
    second = await _bzlmod_counters(buck)

    assert second["extension_replay_miss_reason"] > first["extension_replay_miss_reason"]
    assert second["extension_replay_hit"] == first["extension_replay_hit"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_lockfile_replay_recorded_dirents_input_edit_rejects_cache(
    buck: Buck,
) -> None:
    """Bazel anchors: RepoRecordedInput.Dirents and SingleExtensionEvalFunction."""
    module_name = "plan61_recorded_dirents"
    extension_id = "@plan61_recorded_dirents//:replay_ext.bzl%replay_ext"
    watched = buck.cwd / "watched_dir"
    watched.mkdir()
    _write(watched / "a.txt", "a\n")
    _write(watched / "b.txt", "b\n")
    replayed_repo = buck.cwd / "replayed_repo"
    replayed_repo.mkdir(exist_ok=True)
    _write(replayed_repo / "data.txt", "replayed repo payload\n")
    _write(
        replayed_repo / "BUILD.bazel",
        """exports_files(["data.txt"])
filegroup(name = "data", srcs = ["data.txt"])
""",
    )
    _write(
        buck.cwd / "replay_ext.bzl",
        """def _replay_ext_impl(module_ctx):
    fail("recorded dirents replay rejected")

replay_ext = module_extension(
    implementation = _replay_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "{module_name}")

replay = use_extension("//:replay_ext.bzl", "replay_ext")
use_repo(replay, "replayed_repo")
""",
    )
    _write_replay_lockfile(
        buck.cwd / "MODULE.bazel.lock",
        extension_id=extension_id,
        module_name=module_name,
        project_root=buck.cwd,
        repo_path=replayed_repo,
        recorded_inputs=[f"DIRENTS:@@//watched_dir {_dirents_digest(watched)}"],
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_replayed_repo",
    srcs = ["@replayed_repo//:data"],
)
""",
    )

    before = await _bzlmod_counters(buck)
    await buck.audit("cell")
    first = await _bzlmod_counters(buck)

    assert first["extension_replay_hit"] > before["extension_replay_hit"]
    assert first["extension_eval"] == before["extension_eval"]

    _write(watched / "c.txt", "c\n")

    await buck.audit("cell")
    second = await _bzlmod_counters(buck)

    assert second["extension_replay_miss_reason"] > first["extension_replay_miss_reason"]
    assert second["extension_replay_hit"] == first["extension_replay_hit"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_lockfile_replay_recorded_dirtree_input_edit_rejects_cache(
    buck: Buck,
) -> None:
    """Bazel anchors: RepoRecordedInput.DirTree and DirectoryTreeDigestFunction."""
    module_name = "plan61_recorded_dirtree"
    extension_id = "@plan61_recorded_dirtree//:replay_ext.bzl%replay_ext"
    watched = buck.cwd / "watched_tree"
    watched.mkdir()
    _write(watched / "a.txt", "a\n")
    (watched / "sub").mkdir()
    _write(watched / "sub" / "b.txt", "b\n")
    replayed_repo = buck.cwd / "replayed_repo"
    replayed_repo.mkdir(exist_ok=True)
    _write(replayed_repo / "data.txt", "replayed repo payload\n")
    _write(
        replayed_repo / "BUILD.bazel",
        """exports_files(["data.txt"])
filegroup(name = "data", srcs = ["data.txt"])
""",
    )
    _write(
        buck.cwd / "replay_ext.bzl",
        """def _replay_ext_impl(module_ctx):
    fail("recorded dirtree replay rejected")

replay_ext = module_extension(
    implementation = _replay_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "{module_name}")

replay = use_extension("//:replay_ext.bzl", "replay_ext")
use_repo(replay, "replayed_repo")
""",
    )
    _write_replay_lockfile(
        buck.cwd / "MODULE.bazel.lock",
        extension_id=extension_id,
        module_name=module_name,
        project_root=buck.cwd,
        repo_path=replayed_repo,
        recorded_inputs=[f"DIRTREE:@@//watched_tree {_dirtree_digest(watched)}"],
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_replayed_repo",
    srcs = ["@replayed_repo//:data"],
)
""",
    )

    before = await _bzlmod_counters(buck)
    await buck.audit("cell")
    first = await _bzlmod_counters(buck)

    assert first["extension_replay_hit"] > before["extension_replay_hit"]
    assert first["extension_eval"] == before["extension_eval"]

    _write(watched / "sub" / "b.txt", "changed\n")

    await buck.audit("cell")
    second = await _bzlmod_counters(buck)

    assert second["extension_replay_miss_reason"] > first["extension_replay_miss_reason"]
    assert second["extension_replay_hit"] == first["extension_replay_hit"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_lockfile_replay_recorded_env_input_change_rejects_cache(
    buck: Buck,
) -> None:
    """Bazel anchors: RepoRecordedInput.EnvVar and RepoEnvironmentFunction."""
    module_name = "plan61_recorded_env"
    extension_id = "@plan61_recorded_env//:replay_ext.bzl%replay_ext"
    replayed_repo = buck.cwd / "replayed_repo"
    replayed_repo.mkdir(exist_ok=True)
    _write(replayed_repo / "data.txt", "replayed repo payload\n")
    _write(
        replayed_repo / "BUILD.bazel",
        """exports_files(["data.txt"])
filegroup(name = "data", srcs = ["data.txt"])
""",
    )
    _write(
        buck.cwd / "replay_ext.bzl",
        """def _replay_ext_impl(module_ctx):
    fail("recorded env replay rejected")

replay_ext = module_extension(
    implementation = _replay_ext_impl,
    environ = ["PLAN61_REPO_ENV"],
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "{module_name}")

replay = use_extension("//:replay_ext.bzl", "replay_ext")
use_repo(replay, "replayed_repo")
""",
    )
    _write_replay_lockfile(
        buck.cwd / "MODULE.bazel.lock",
        extension_id=extension_id,
        module_name=module_name,
        project_root=buck.cwd,
        repo_path=replayed_repo,
        recorded_inputs=["ENV:PLAN61_REPO_ENV first"],
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_replayed_repo",
    srcs = ["@replayed_repo//:data"],
)
""",
    )

    first_args = ("--repo_env=PLAN61_REPO_ENV=first",)
    second_args = ("--repo_env=PLAN61_REPO_ENV=second",)
    before = await _bzlmod_counters(buck, *first_args)
    await buck.audit("cell", *first_args)
    first = await _bzlmod_counters(buck, *first_args)

    assert first["extension_replay_hit"] > before["extension_replay_hit"]
    assert first["extension_eval"] == before["extension_eval"]

    await buck.audit("cell", *second_args)
    second = await _bzlmod_counters(buck, *second_args)

    assert second["extension_replay_miss_reason"] > first["extension_replay_miss_reason"]
    assert second["extension_replay_hit"] == first["extension_replay_hit"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_lockfile_replay_recorded_repo_mapping_change_rejects_cache(
    buck: Buck,
) -> None:
    """Bazel anchors: RepoRecordedInput.RecordedRepoMapping and RepositoryMappingValue."""
    module_name = "plan61_recorded_repo_mapping"
    dep_module_name = "plan61_mapping_dep"
    extension_id = f"@{module_name}//:replay_ext.bzl%replay_ext"
    dep = buck.cwd / "dep"
    dep.mkdir()
    _write(dep / "MODULE.bazel", f'module(name = "{dep_module_name}", version = "1.0")\n')
    _write(dep / "BUILD.bazel", "")
    replayed_repo = buck.cwd / "replayed_repo"
    replayed_repo.mkdir(exist_ok=True)
    _write(replayed_repo / "data.txt", "replayed repo payload\n")
    _write(
        replayed_repo / "BUILD.bazel",
        """exports_files(["data.txt"])
filegroup(name = "data", srcs = ["data.txt"])
""",
    )
    _write(
        buck.cwd / "replay_ext.bzl",
        """def _replay_ext_impl(module_ctx):
    fail("recorded repo mapping replay rejected")

replay_ext = module_extension(
    implementation = _replay_ext_impl,
)
""",
    )
    module_template = f"""module(name = "{module_name}")

bazel_dep(name = "{dep_module_name}", version = "1.0", repo_name = "{{repo_name}}")
local_path_override(module_name = "{dep_module_name}", path = "dep")

replay = use_extension("//:replay_ext.bzl", "replay_ext")
use_repo(replay, "replayed_repo")
"""
    _write(buck.cwd / "MODULE.bazel", module_template.format(repo_name="mapped_dep"))
    _write_replay_lockfile(
        buck.cwd / "MODULE.bazel.lock",
        extension_id=extension_id,
        module_name=module_name,
        project_root=buck.cwd,
        repo_path=replayed_repo,
        recorded_inputs=[f"REPO_MAPPING:,mapped_dep {dep_module_name}"],
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_replayed_repo",
    srcs = ["@replayed_repo//:data"],
)
""",
    )

    before = await _bzlmod_counters(buck)
    await buck.audit("cell")
    first = await _bzlmod_counters(buck)

    assert first["extension_replay_hit"] > before["extension_replay_hit"]
    assert first["extension_eval"] == before["extension_eval"]

    _write(buck.cwd / "MODULE.bazel", module_template.format(repo_name="remapped_dep"))

    await buck.audit("cell")
    second = await _bzlmod_counters(buck)

    assert second["extension_replay_miss_reason"] > first["extension_replay_miss_reason"]
    assert second["extension_replay_hit"] == first["extension_replay_hit"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_lockfile_replay_recorded_repo_mapping_from_extension_repo_source(
    buck: Buck,
) -> None:
    """Bazel anchor: ModuleExtensionRepoMappingEntriesFunction sibling mappings."""
    module_name = "plan61_recorded_extension_repo_mapping"
    extension_id = f"@{module_name}//:replay_ext.bzl%replay_ext"
    replayed_repo = buck.cwd / "replayed_repo"
    replayed_repo.mkdir(exist_ok=True)
    _write(replayed_repo / "data.txt", "replayed repo payload\n")
    _write(
        replayed_repo / "BUILD.bazel",
        """exports_files(["data.txt"])
filegroup(name = "data", srcs = ["data.txt"])
""",
    )
    tool_repo = buck.cwd / "tool_repo"
    tool_repo.mkdir(exist_ok=True)
    _write(tool_repo / "tool.txt", "tool repo payload\n")
    _write(
        tool_repo / "BUILD.bazel",
        """exports_files(["tool.txt"])
filegroup(name = "tool", srcs = ["tool.txt"])
""",
    )
    _write(
        buck.cwd / "replay_ext.bzl",
        """def _replay_ext_impl(module_ctx):
    fail("recorded extension repo mapping replay rejected")

replay_ext = module_extension(
    implementation = _replay_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "{module_name}")

replay = use_extension("//:replay_ext.bzl", "replay_ext")
use_repo(replay, "replayed_repo", "tool_repo")
""",
    )
    _write_replay_lockfile(
        buck.cwd / "MODULE.bazel.lock",
        extension_id=extension_id,
        module_name=module_name,
        project_root=buck.cwd,
        repo_path=replayed_repo,
        repo_paths={
            "replayed_repo": replayed_repo,
            "tool_repo": tool_repo,
        },
        recorded_inputs=[
            "REPO_MAPPING:_main+replay_ext+tool_repo,replayed_repo "
            "_main+replay_ext+replayed_repo"
        ],
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_replayed_repos",
    srcs = [
        "@replayed_repo//:data",
        "@tool_repo//:tool",
    ],
)
""",
    )

    before = await _bzlmod_counters(buck)
    await buck.build("//:uses_replayed_repos")
    after = await _bzlmod_counters(buck)

    assert after["extension_replay_hit"] > before["extension_replay_hit"]
    assert after["extension_eval"] == before["extension_eval"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_extension_tag_attr_edit_invalidates_or_rejects_replay(
    buck: Buck,
) -> None:
    """Bazel anchors: SingleExtensionUsagesValue and SingleExtensionEvalFunction."""
    module_name = "plan61_tag_replay"
    extension_id = "@plan61_tag_replay//:replay_ext.bzl%replay_ext"
    replayed_repo = buck.cwd / "replayed_repo"
    replayed_repo.mkdir(exist_ok=True)
    _write(replayed_repo / "BUILD.bazel", "filegroup(name = \"data\")\n")
    _write(
        buck.cwd / "replay_ext.bzl",
        """def _generated_repo_impl(rctx):
    pass

generated_repo = repository_rule(
    implementation = _generated_repo_impl,
)

def _replay_ext_impl(module_ctx):
    generated_repo(name = "replayed_repo")

replay_ext = module_extension(
    implementation = _replay_ext_impl,
    tag_classes = {
        "config": tag_class(attrs = {"name": attr.string()}),
    },
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "{module_name}")

replay = use_extension("//:replay_ext.bzl", "replay_ext")
replay.config(name = "initial")
use_repo(replay, "replayed_repo")
""",
    )
    _write(
        buck.cwd / "MODULE.bazel.lock",
        json.dumps(
            {
                "lockFileVersion": 26,
                "registryFileHashes": {},
                "selectedYankedVersions": {},
                "moduleExtensions": {
                    extension_id: {
                        "general": {
                            "bzlTransitiveDigest": _slug_bzl_transitive_digest(
                                extension_id,
                                buck.cwd,
                            ),
                            "usagesDigest": _slug_usages_digest(
                                extension_id,
                                {
                                    module_name: [
                                        ("config", {"name": "initial"}),
                                    ],
                                },
                            ),
                            "recordedInputs": [],
                            "generatedRepoSpecs": {
                                "replayed_repo": {
                                    "repoRuleId": (
                                        "@@bazel_tools//tools/build_defs/repo:"
                                        "local.bzl%local_repository"
                                    ),
                                    "attributes": {
                                        "path": str(replayed_repo),
                                    },
                                },
                            },
                            "moduleExtensionMetadata": None,
                        },
                    },
                },
                "facts": {},
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
    )

    before = await _bzlmod_counters(buck)
    await buck.audit("cell")
    first = await _bzlmod_counters(buck)
    assert first["extension_replay_hit"] > before["extension_replay_hit"]
    assert first["extension_eval"] == before["extension_eval"]

    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "{module_name}")

replay = use_extension("//:replay_ext.bzl", "replay_ext")
replay.config(name = "edited")
use_repo(replay, "replayed_repo")
""",
    )

    await buck.audit("cell")
    second = await _bzlmod_counters(buck)

    assert second["extension_replay_miss_reason"] > first["extension_replay_miss_reason"]
    assert second["extension_replay_hit"] == first["extension_replay_hit"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_bad_extension_fails_without_stub_repo(buck: Buck) -> None:
    """Bazel anchors: SingleExtensionFunction and SingleExtensionEvalFunction."""
    canonical_repo = "_main+bad_ext+failed_repo"
    repo_dir = buck.cwd / "bazel-external" / canonical_repo
    marker = repo_dir / ".slug_repo_complete"
    _write(
        buck.cwd / "bad_ext.bzl",
        """def _bad_ext_impl(module_ctx):
    fail("PLAN61_BAD_EXTENSION_EVAL")

bad_ext = module_extension(
    implementation = _bad_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_bad_extension")

bad = use_extension("//:bad_ext.bzl", "bad_ext")
use_repo(bad, "failed_repo")
""",
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """load("@failed_repo//:defs.bzl", "TELEMETRY")

filegroup(name = "uses_failed_repo")
""",
    )

    before = await _bzlmod_counters(buck)
    failure_stderr: str | None = None
    try:
        await buck.build("//:uses_failed_repo")
    except BuckException as e:
        failure_stderr = e.stderr
    after = await _bzlmod_counters(buck)

    if failure_stderr is None:
        pytest.fail(
            "bad extension build unexpectedly succeeded; "
            f"repo_dir_exists={repo_dir.exists()} "
            f"marker_exists={marker.exists()}"
        )

    assert "PLAN61_BAD_EXTENSION_EVAL" in failure_stderr
    assert "Stub repo" not in failure_stderr
    assert after["extension_eval"] > before["extension_eval"]
    assert not repo_dir.exists()
    assert not marker.exists()


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_unknown_repo_rule_fails_without_stub_repo(buck: Buck) -> None:
    """Bazel anchors: RepoDefinitionFunction and RepositoryFetchFunction."""
    module_name = "plan61_unknown_repo_rule"
    extension_id = "@plan61_unknown_repo_rule//:unknown_ext.bzl%unknown_ext"
    canonical_repo = "_main+unknown_ext+unknown_repo"
    repo_dir = buck.cwd / "bazel-external" / canonical_repo
    marker = repo_dir / ".slug_repo_complete"
    unknown_rule = "plan61_unknown_repository_rule"
    _write(
        buck.cwd / "unknown_ext.bzl",
        """def _unknown_ext_impl(module_ctx):
    fail("extension should replay from lockfile")

unknown_ext = module_extension(
    implementation = _unknown_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "{module_name}")

unknown = use_extension("//:unknown_ext.bzl", "unknown_ext")
use_repo(unknown, "unknown_repo")
""",
    )
    _write(
        buck.cwd / "MODULE.bazel.lock",
        json.dumps(
            {
                "lockFileVersion": 26,
                "registryFileHashes": {},
                "selectedYankedVersions": {},
                "moduleExtensions": {
                    extension_id: {
                        "general": {
                                "bzlTransitiveDigest": _slug_bzl_transitive_digest(
                                    extension_id,
                                    buck.cwd,
                                ),
                            "usagesDigest": _slug_usages_digest_without_tags(
                                extension_id, module_name
                            ),
                            "recordedInputs": [],
                            "generatedRepoSpecs": {
                                "unknown_repo": {
                                    "repoRuleId": unknown_rule,
                                    "attributes": {},
                                },
                            },
                            "moduleExtensionMetadata": None,
                        },
                    },
                },
                "facts": {},
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """load("@unknown_repo//:defs.bzl", "PLAN61_UNKNOWN_REPO_SENTINEL")

filegroup(name = "uses_unknown_repo")
""",
    )

    failure_stderr: str | None = None
    try:
        await buck.build("//:uses_unknown_repo")
    except BuckException as e:
        failure_stderr = e.stderr

    if failure_stderr is None:
        pytest.fail(
            "unknown repo rule build unexpectedly succeeded; "
            f"repo_dir_exists={repo_dir.exists()} "
            f"marker_exists={marker.exists()}"
        )

    assert unknown_rule in failure_stderr
    assert "Stub repository" not in failure_stderr
    assert not repo_dir.exists()
    assert not marker.exists()


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_stale_repo_marker_does_not_mask_changed_repo_spec_or_outputs(
    buck: Buck,
) -> None:
    """Bazel anchors: DigestWriter, RepoRecordedInput, and RepositoryFetchFunction."""
    canonical_repo = "+new_local_repository+marker_repo"
    repo_dir = buck.cwd / "bazel-external" / canonical_repo
    marker = repo_dir / ".slug_repo_complete"
    repo_src = buck.cwd / "repo_src"

    repo_src.mkdir()
    repo_dir.mkdir(parents=True)
    _write(repo_src / "fresh.txt", "fresh output from current repo spec\n")
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_marker")

repo = use_repo_rule("@@bazel_tools//tools/build_defs/repo:local.bzl", "new_local_repository")
repo(
    name = "marker_repo",
    path = "repo_src",
    build_file_content = \"\"\"exports_files(["fresh.txt"])
\"\"\",
)
""",
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_marker_repo",
    srcs = ["@marker_repo//:fresh.txt"],
)
""",
    )

    _write(repo_dir / "BUILD.bazel", """exports_files(["stale.txt"])\n""")
    _write(repo_dir / "stale.txt", "stale output from prior repo spec\n")
    _write(marker, "complete\n")

    before = await _bzlmod_counters(buck)
    failure_stderr: str | None = None
    try:
        await buck.build("//:uses_marker_repo")
    except BuckException as e:
        failure_stderr = e.stderr
    after = await _bzlmod_counters(buck)

    if failure_stderr is not None:
        assert "Unknown target `fresh.txt`" not in failure_stderr
        assert "stale.txt" not in failure_stderr
        assert after["repo_materialization_miss_reason"] > before[
            "repo_materialization_miss_reason"
        ]
        assert after["repo_materialization_hit"] == before["repo_materialization_hit"]
        return

    assert after["repo_materialization_miss_reason"] > before[
        "repo_materialization_miss_reason"
    ]
    assert after["repo_materialization_hit"] == before["repo_materialization_hit"]
    assert not (repo_dir / "stale.txt").exists()
    assert (repo_dir / "fresh.txt").exists()


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_no_stub_failures_cover_missing_generated_repo_and_repo_rule_failure(
    buck: Buck,
) -> None:
    """Bazel anchors: SingleExtensionFunction, RepoDefinitionFunction, RepositoryFetchFunction."""

    async def run_case(
        *,
        rel_cwd: Path,
        canonical_repo: str,
        target: str,
    ) -> tuple[str | None, BzlmodCounters, BzlmodCounters, Path]:
        repo_dir = buck.cwd / rel_cwd / "bazel-external" / canonical_repo
        before = await _bzlmod_counters(buck, rel_cwd=rel_cwd)
        failure_stderr: str | None = None
        try:
            await buck.build(target, rel_cwd=rel_cwd)
        except BuckException as e:
            failure_stderr = e.stderr
        after = await _bzlmod_counters(buck, rel_cwd=rel_cwd)
        return failure_stderr, before, after, repo_dir

    missing_rel = Path("missing_generated_repo")
    missing_rel_abs = buck.cwd / missing_rel
    missing_rel_abs.mkdir()
    _write(missing_rel_abs / ".buckroot", "")
    _write(
        missing_rel_abs / "empty_ext.bzl",
        """def _empty_ext_impl(module_ctx):
    pass

empty_ext = module_extension(
    implementation = _empty_ext_impl,
)
""",
    )
    _write(
        missing_rel_abs / "MODULE.bazel",
        """module(name = "plan61_missing_generated_repo")

empty = use_extension("//:empty_ext.bzl", "empty_ext")
use_repo(empty, "missing_repo")
""",
    )
    _write(
        missing_rel_abs / "BUILD.bazel",
        """load("@missing_repo//:defs.bzl", "TELEMETRY")

filegroup(name = "uses_missing_repo")
""",
    )

    missing_stderr, missing_before, missing_after, missing_repo_dir = await run_case(
        rel_cwd=missing_rel,
        canonical_repo="_main+empty_ext+missing_repo",
        target="//:uses_missing_repo",
    )

    repo_rule_rel = Path("repo_rule_failure")
    repo_rule_rel_abs = buck.cwd / repo_rule_rel
    repo_rule_rel_abs.mkdir()
    _write(repo_rule_rel_abs / ".buckroot", "")
    _write(
        repo_rule_rel_abs / "MODULE.bazel",
        """module(name = "plan61_repo_rule_failure")

repo = use_repo_rule("@@bazel_tools//tools/build_defs/repo:local.bzl", "local_repository")
repo(name = "broken_local_repo")
""",
    )
    _write(
        repo_rule_rel_abs / "BUILD.bazel",
        """load("@broken_local_repo//:defs.bzl", "TELEMETRY")

filegroup(name = "uses_broken_local_repo")
""",
    )

    repo_rule_stderr, repo_rule_before, repo_rule_after, repo_rule_repo_dir = (
        await run_case(
            rel_cwd=repo_rule_rel,
            canonical_repo="+local_repository+broken_local_repo",
            target="//:uses_broken_local_repo",
        )
    )

    missing_marker = missing_repo_dir / ".slug_repo_complete"
    repo_rule_marker = repo_rule_repo_dir / ".slug_repo_complete"
    missing_stub_build = missing_repo_dir / "BUILD.bazel"
    repo_rule_stub_build = repo_rule_repo_dir / "BUILD.bazel"
    missing_stub_defs = missing_repo_dir / "defs.bzl"
    repo_rule_stub_defs = repo_rule_repo_dir / "defs.bzl"

    if missing_stderr is None or repo_rule_stderr is None:
        pytest.fail(
            "no-stub failure build unexpectedly succeeded; "
            f"missing_stderr={missing_stderr!r} "
            f"missing_repo_dir_exists={missing_repo_dir.exists()} "
            f"missing_marker_exists={missing_marker.exists()} "
            f"missing_stub_build_exists={missing_stub_build.exists()} "
            f"missing_stub_defs_exists={missing_stub_defs.exists()} "
            f"repo_rule_stderr={repo_rule_stderr!r} "
            f"repo_rule_repo_dir_exists={repo_rule_repo_dir.exists()} "
            f"repo_rule_marker_exists={repo_rule_marker.exists()} "
            f"repo_rule_stub_build_exists={repo_rule_stub_build.exists()} "
            f"repo_rule_stub_defs_exists={repo_rule_stub_defs.exists()}"
        )

    assert "missing_repo" in missing_stderr
    assert "did not generate repo" in missing_stderr or "not found" in missing_stderr
    assert "Stub repo" not in missing_stderr
    assert missing_after["extension_eval"] > missing_before["extension_eval"]
    assert not missing_repo_dir.exists()
    assert not missing_marker.exists()
    assert not missing_stub_build.exists()
    assert not missing_stub_defs.exists()

    assert "broken_local_repo" in repo_rule_stderr
    assert "path" in repo_rule_stderr
    assert "Stub repo" not in repo_rule_stderr
    assert not repo_rule_repo_dir.exists()
    assert not repo_rule_marker.exists()
    assert not repo_rule_stub_build.exists()
    assert not repo_rule_stub_defs.exists()
