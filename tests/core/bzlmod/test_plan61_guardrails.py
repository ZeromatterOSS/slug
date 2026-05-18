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
) -> BzlmodCounters:
    result = await buck.audit("bzlmod-counters", *args, rel_cwd=rel_cwd)
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
                return "sha256-" + base64.b64encode(hasher.digest()).decode()

    digest = hashlib.sha256(b"bzl_transitive_v1:" + extension_id.encode()).digest()
    return "sha256-" + base64.b64encode(digest).decode()


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
    return "sha256-" + base64.b64encode(digest).decode()


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
    return "sha256-" + base64.b64encode(hasher.digest()).decode()


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
) -> None:
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
                            "recordedInputs": [],
                            "generatedRepoSpecs": {
                                "replayed_repo": {
                                    "repoRuleId": (
                                        "@@bazel_tools//tools/build_defs/repo:"
                                        "local.bzl%local_repository"
                                    ),
                                    "attributes": {
                                        "path": str(repo_path),
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
async def test_two_workspaces_in_one_daemon_do_not_share_bzlmod_state(
    buck: Buck,
) -> None:
    """Bazel anchors: BazelDepGraphValue.java, ModuleKey.java, and ModuleExtensionId.java."""
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

    second_output, second = await _audit_cells_and_counters(buck, rel_cwd=second_root)
    second_daemon_dir = (await buck.debug("daemon-dir", rel_cwd=second_root)).stdout.strip()
    assert "shared_plan61_workspace" in second_output
    assert "second_only_lib" in second_output
    assert "first_only_lib" not in second_output
    assert "shared_generated" in second_output

    if first_daemon_dir != second_daemon_dir:
        pytest.xfail(
            "Plan 61 same-daemon isolation precondition is not expressible yet: "
            "daemon directories still include project_root, so colliding workspace "
            "roots start separate daemons before bzlmod state sharing can be tested. "
            f"first={first_daemon_dir} second={second_daemon_dir}"
        )

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
            f"marker_exists={marker.exists()} "
            f"stub_fallback_delta={after['stub_fallback_attempt'] - before['stub_fallback_attempt']}"
        )

    assert "PLAN61_BAD_EXTENSION_EVAL" in failure_stderr
    assert "Stub repo" not in failure_stderr
    assert after["extension_eval"] > before["extension_eval"]
    assert after["stub_fallback_attempt"] == before["stub_fallback_attempt"]
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

    before = await _bzlmod_counters(buck)
    failure_stderr: str | None = None
    try:
        await buck.build("//:uses_unknown_repo")
    except BuckException as e:
        failure_stderr = e.stderr
    after = await _bzlmod_counters(buck)

    if failure_stderr is None:
        pytest.fail(
            "unknown repo rule build unexpectedly succeeded; "
            f"repo_dir_exists={repo_dir.exists()} "
            f"marker_exists={marker.exists()} "
            f"stub_fallback_delta={after['stub_fallback_attempt'] - before['stub_fallback_attempt']}"
        )

    assert unknown_rule in failure_stderr
    assert "Stub repository" not in failure_stderr
    assert after["stub_fallback_attempt"] == before["stub_fallback_attempt"]
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
            "missing_stub_fallback_delta="
            f"{missing_after['stub_fallback_attempt'] - missing_before['stub_fallback_attempt']} "
            f"repo_rule_stderr={repo_rule_stderr!r} "
            f"repo_rule_repo_dir_exists={repo_rule_repo_dir.exists()} "
            f"repo_rule_marker_exists={repo_rule_marker.exists()} "
            f"repo_rule_stub_build_exists={repo_rule_stub_build.exists()} "
            f"repo_rule_stub_defs_exists={repo_rule_stub_defs.exists()} "
            "repo_rule_stub_fallback_delta="
            f"{repo_rule_after['stub_fallback_attempt'] - repo_rule_before['stub_fallback_attempt']}"
        )

    assert "missing_repo" in missing_stderr
    assert "did not generate repo" in missing_stderr or "not found" in missing_stderr
    assert "Stub repo" not in missing_stderr
    assert missing_after["extension_eval"] > missing_before["extension_eval"]
    assert missing_after["stub_fallback_attempt"] == missing_before["stub_fallback_attempt"]
    assert not missing_repo_dir.exists()
    assert not missing_marker.exists()
    assert not missing_stub_build.exists()
    assert not missing_stub_defs.exists()

    assert "broken_local_repo" in repo_rule_stderr
    assert "path" in repo_rule_stderr
    assert "Stub repo" not in repo_rule_stderr
    assert repo_rule_after["stub_fallback_attempt"] == repo_rule_before[
        "stub_fallback_attempt"
    ]
    assert not repo_rule_repo_dir.exists()
    assert not repo_rule_marker.exists()
    assert not repo_rule_stub_build.exists()
    assert not repo_rule_stub_defs.exists()
