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
from typing import Protocol

import pytest
from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.api.buck_result import BuckException
from buck2.tests.e2e_util.buck_workspace import buck_test


BzlmodCounters = dict[str, int]


class _HashLike(Protocol):
    def update(self, data: bytes) -> None: ...


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
    env: dict[str, str] | None = None,
) -> tuple[str, BzlmodCounters]:
    result = await buck.audit("cell", rel_cwd=rel_cwd, env=env)
    return result.stdout, await _bzlmod_counters(buck, rel_cwd=rel_cwd, env=env)


def _write(path: Path, content: str) -> None:
    path.write_text(content)


def _write_bytes(path: Path, content: bytes) -> None:
    path.write_bytes(content)


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


def _write_minimal_lockfile_with_facts(
    path: Path,
    *,
    extension_id: str,
    facts: dict[str, object],
) -> None:
    _write(
        path,
        json.dumps(
            {
                "lockFileVersion": 26,
                "registryFileHashes": {},
                "selectedYankedVersions": {},
                "moduleExtensions": {},
                "facts": {extension_id: facts},
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
    )


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _write_cached_registry_module(
    cache_home: Path,
    registry_host: str,
    module_name: str,
    module_version: str,
    module_file: str,
    build_file: str = 'filegroup(name = "ok", srcs = [])\n',
) -> Path:
    module_cache = (
        cache_home
        / "slug"
        / "registry"
        / registry_host
        / "modules"
        / module_name
        / module_version
    )
    source_dir = module_cache / "source"
    source_dir.mkdir(parents=True)
    _write(module_cache / "MODULE.bazel", module_file)
    _write(module_cache / "source.json", "{}\n")
    _write(source_dir / ".complete", "")
    _write(source_dir / "BUILD.bazel", build_file)
    return module_cache


def _git_override_cache_dir(
    cache_home: Path,
    module_name: str,
    remote: str,
    commit: str,
    shallow_since: str | None = None,
) -> Path:
    hasher = hashlib.sha256()
    hasher.update(b"slug-git-override-cache-v1")
    _update_digest_str(hasher, remote)
    _update_digest_str(hasher, commit)
    _update_digest_optional_str(hasher, shallow_since)
    source_identity = hasher.hexdigest()[:16]
    return cache_home / "slug" / "overrides" / module_name / f"git-{commit}-{source_identity}"


def _archive_override_cache_dir(
    cache_home: Path,
    module_name: str,
    urls: list[str],
    integrity: str | None = None,
    strip_prefix: str | None = None,
) -> Path:
    hasher = hashlib.sha256()
    hasher.update(b"slug-archive-override-cache-v1")
    for url in urls:
        _update_digest_str(hasher, url)
    _update_digest_optional_str(hasher, integrity)
    _update_digest_optional_str(hasher, strip_prefix)
    source_identity = hasher.hexdigest()[:16]
    return cache_home / "slug" / "overrides" / module_name / f"archive-{source_identity}"


def _update_digest_str(hasher: _HashLike, value: str) -> None:
    hasher.update(b"\0")
    hasher.update(value.encode())


def _update_digest_optional_str(hasher: _HashLike, value: str | None) -> None:
    if value is None:
        hasher.update(b"\0")
        return
    hasher.update(b"\1")
    hasher.update(value.encode())


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
    repo_mappings: dict[str, dict[str, str]] | None = None,
) -> str:
    if project_root is not None:
        root_bzl = _extension_bzl_location(extension_id, project_root, repo_mappings)
        if root_bzl is not None and root_bzl[0].is_file():
            seen_locations: set[tuple[Path, str, str]] = set()
            seen_files: set[Path] = set()

            def collect(location: tuple[Path, str, str]) -> None:
                path, repo, package = location
                if location in seen_locations:
                    return
                seen_locations.add(location)
                seen_files.add(path)
                try:
                    content = path.read_text()
                except OSError:
                    return
                for load in re.findall(r"""load\(\s*["']([^"']+)["']""", content):
                    loaded = _label_bzl_location(
                        load,
                        project_root,
                        (path, repo, package),
                        repo_mappings,
                    )
                    if loaded is None:
                        continue
                    try:
                        loaded[0].relative_to(project_root)
                    except ValueError:
                        continue
                    collect(loaded)

            collect(root_bzl)
            if seen_files:
                hasher = hashlib.sha256()
                hasher.update(b"bzl_transitive_v2:")
                hasher.update(extension_id.encode())
                hasher.update(b"\0")
                for path in sorted(seen_files):
                    hasher.update(path.relative_to(project_root).as_posix().encode())
                    hasher.update(b"\0")
                    try:
                        hasher.update(path.read_bytes())
                    except FileNotFoundError:
                        hasher.update(b"read_error:")
                        hasher.update(b"No such file or directory (os error 2)")
                    except OSError as e:
                        hasher.update(b"read_error:")
                        hasher.update(str(e).encode())
                    hasher.update(b"\0")
                return base64.b64encode(hasher.digest()).decode()

    digest = hashlib.sha256(b"bzl_transitive_v1:" + extension_id.encode()).digest()
    return base64.b64encode(digest).decode()


def _extension_bzl_path(extension_id: str, project_root: Path) -> Path | None:
    location = _extension_bzl_location(extension_id, project_root, None)
    return location[0] if location is not None else None


def _extension_bzl_location(
    extension_id: str,
    project_root: Path,
    repo_mappings: dict[str, dict[str, str]] | None,
) -> tuple[Path, str, str] | None:
    return _label_bzl_location(
        extension_id.split("%", 1)[0],
        project_root,
        None,
        repo_mappings,
    )


def _label_bzl_path(
    label: str,
    project_root: Path,
    current_dir: Path | None,
) -> Path | None:
    current = None
    if current_dir is not None:
        try:
            package = current_dir.relative_to(project_root).as_posix()
        except ValueError:
            package = ""
        current = (current_dir / "__relative_placeholder__.bzl", "", package)
    location = _label_bzl_location(label, project_root, current, None)
    return location[0] if location is not None else None


def _label_bzl_location(
    label: str,
    project_root: Path,
    current: tuple[Path, str, str] | None,
    repo_mappings: dict[str, dict[str, str]] | None,
) -> tuple[Path, str, str] | None:
    if label.startswith("@@"):
        if "//" not in label:
            return None
        repo, target = label[2:].split("//", 1)
        target_parts = _split_bzl_label_target(target)
        if target_parts is None:
            return None
        package, name = target_parts
        external = _bzl_location_for_repo(repo, package, name, project_root)
        if external is not None:
            return external
        if "+" in repo:
            return _bzl_location_for_repo(
                repo, package, name, project_root, include_missing=True
            )
        return _project_bzl_location(project_root, package, name) if "+" not in repo else None
    elif label.startswith("@"):
        if "//" not in label:
            return None
        repo, target = label[1:].split("//", 1)
        target_parts = _split_bzl_label_target(target)
        if target_parts is None:
            return None
        package, name = target_parts
        mapped = False
        if "+" not in repo and current is not None and repo_mappings is not None:
            canonical = _mapped_repo(repo_mappings, current[1], repo)
            if canonical is not None:
                mapped = canonical != repo
                repo = canonical
        external = _bzl_location_for_repo(repo, package, name, project_root)
        if external is not None:
            return external
        if mapped or "+" in repo:
            return _bzl_location_for_repo(
                repo, package, name, project_root, include_missing=True
            )
        return (
            _project_bzl_location(project_root, package, name)
            if not mapped and "+" not in repo
            else None
        )
    elif label.startswith("//"):
        if current is None:
            repo = ""
        else:
            repo = current[1]
        target_parts = _split_bzl_label_target(label[2:])
        if target_parts is None:
            return None
        package, name = target_parts
        external = _bzl_location_for_repo(repo, package, name, project_root)
        if external is not None:
            return external
        if repo and repo != "_main":
            return _bzl_location_for_repo(
                repo, package, name, project_root, include_missing=True
            )
        return None
    elif label.startswith(":"):
        if current is None:
            return None
        external = _bzl_location_for_repo(current[1], current[2], label[1:], project_root)
        if external is not None:
            return external
        if current[1] and current[1] != "_main":
            return _bzl_location_for_repo(
                current[1],
                current[2],
                label[1:],
                project_root,
                include_missing=True,
            )
        return None
    elif "//" in label:
        repo, target = label.split("//", 1)
        target_parts = _split_bzl_label_target(target)
        if target_parts is None:
            return None
        package, name = target_parts
        mapped = False
        if "+" not in repo and current is not None and repo_mappings is not None:
            canonical = _mapped_repo(repo_mappings, current[1], repo)
            if canonical is not None:
                mapped = canonical != repo
                repo = canonical
        external = _bzl_location_for_repo(repo, package, name, project_root)
        if external is not None:
            return external
        if mapped or "+" in repo:
            return _bzl_location_for_repo(
                repo, package, name, project_root, include_missing=True
            )
        return (
            _project_bzl_location(project_root, package, name)
            if not mapped and "+" not in repo
            else None
        )
    else:
        if current is None:
            return None
        return (current[0].parent / label, current[1], current[2])


def _split_bzl_label_target(target: str) -> tuple[str, str] | None:
    if ":" not in target:
        return None
    package, name = target.split(":", 1)
    return package, name


def _bzl_location_for_repo(
    repo: str,
    package: str,
    name: str,
    project_root: Path,
    *,
    include_missing: bool = False,
) -> tuple[Path, str, str] | None:
    if not repo or repo == "_main":
        return _project_bzl_location(project_root, package, name)
    first_missing: tuple[Path, str, str] | None = None
    for candidate in _external_repo_candidates(repo):
        path = project_root / "bazel-external" / candidate
        if package:
            path = path / package
        path = path / name
        if path.is_file():
            return path, candidate, package
        if first_missing is None:
            first_missing = (path, candidate, package)
    if include_missing:
        return first_missing
    return None


def _project_bzl_location(
    project_root: Path,
    package: str,
    name: str,
) -> tuple[Path, str, str]:
    path = project_root / package / name if package else project_root / name
    return path, "", package


def _external_repo_candidates(repo: str) -> list[str]:
    if not repo or repo == "_main":
        return []
    candidates = [repo]
    if repo.endswith("+"):
        candidates.append(repo.removesuffix("+"))
    elif "+" not in repo:
        candidates.append(f"{repo}+")
    return candidates


def _mapped_repo(
    repo_mappings: dict[str, dict[str, str]],
    current_repo: str,
    apparent_repo: str,
) -> str | None:
    for candidate in _source_repo_mapping_candidates(current_repo):
        mapping = repo_mappings.get(candidate)
        if mapping is not None and apparent_repo in mapping:
            return mapping[apparent_repo]
    return None


def _source_repo_mapping_candidates(current_repo: str) -> list[str]:
    candidates: list[str] = []

    def push(candidate: str) -> None:
        if candidate not in candidates:
            candidates.append(candidate)

    push(current_repo)
    if not current_repo or current_repo == "_main":
        push("_main")
        push("")
    elif current_repo.endswith("+"):
        push(current_repo.removesuffix("+"))
    elif "+" not in current_repo:
        push(f"{current_repo}+")
    return candidates


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
    repo_mappings: dict[str, dict[str, str]] | None = None,
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
                                repo_mappings,
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
async def test_warm_noop_audit_cell_reuses_bzlmod_resolution(buck: Buck) -> None:
    """Bazel anchor: BazelModuleResolutionValue is a Skyframe cut-off point."""
    before = await _bzlmod_counters(buck)
    output, first = await _audit_cells_and_counters(buck)
    assert "plan61_guardrails" in output
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    output, second = await _audit_cells_and_counters(buck)
    assert "plan61_guardrails" in output
    assert second["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_warm_noop_local_override_audit_cell_reuses_bzlmod_resolution(
    buck: Buck,
) -> None:
    """Bazel anchor: local override MODULE files are ModuleFileFunction inputs."""
    local_lib = buck.cwd / "libs/local_lib"
    local_lib.mkdir(parents=True, exist_ok=True)
    _write(local_lib / "MODULE.bazel", 'module(name = "local_lib", version = "1.0")\n')
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_local_override_warm")

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
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    output, second = await _audit_cells_and_counters(buck)
    assert "local_lib" in output
    assert second["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_warm_noop_out_of_project_local_override_reuses_polled_dice_input(
    buck: Buck,
) -> None:
    """Bazel anchor: DiscoveryTest.testLocalPathOverride uses an absolute override path."""
    local_lib = buck.cwd.parent / f"{buck.cwd.name}_external_local_lib"
    local_lib.mkdir(parents=True, exist_ok=True)
    _write(local_lib / "MODULE.bazel", 'module(name = "external_local", version = "1.0")\n')
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_external_local_override")

bazel_dep(name = "external_local")
local_path_override(
    module_name = "external_local",
    path = "{local_lib.as_posix()}",
)
""",
    )

    before = await _bzlmod_counters(buck)
    output, first = await _audit_cells_and_counters(buck)
    assert "external_local" in output
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    output, second = await _audit_cells_and_counters(buck)
    assert "external_local" in output
    assert second["module_file_parse"] == first["module_file_parse"]
    assert second["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write(local_lib / "MODULE.bazel", 'module(name = "external_local", version = "1.1")\n')
    output, third = await _audit_cells_and_counters(buck)
    assert "external_local" in output
    assert third["module_file_parse"] > second["module_file_parse"]
    assert third["bzlmod_resolution_compute"] > second["bzlmod_resolution_compute"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_out_of_project_local_override_parse_failure_invalidates_bzlmod_resolution(
    buck: Buck,
) -> None:
    """Bazel anchor: local_path_override MODULE.bazel parse errors are inputs."""
    module_name = "external_local_parse_failure"
    local_lib = buck.cwd.parent / f"{buck.cwd.name}_{module_name}"
    local_lib.mkdir(parents=True, exist_ok=True)
    _write(local_lib / "MODULE.bazel", f'module(name = "{module_name}", version = "1.0")\n')
    _write(
        local_lib / "dep_ext.bzl",
        """def _dep_ext_impl(module_ctx):
    pass

dep_ext = module_extension(
    implementation = _dep_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_external_local_parse_failure")

bazel_dep(name = "{module_name}")
local_path_override(
    module_name = "{module_name}",
    path = "{local_lib.as_posix()}",
)
""",
    )

    before = await _bzlmod_counters(buck)
    output, first = await _audit_cells_and_counters(buck)
    assert module_name in output
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    output, warm = await _audit_cells_and_counters(buck)
    assert module_name in output
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write(local_lib / "MODULE.bazel", f'module(name = "{module_name}", version = )\n')
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell")
    failure_stderr = exc.value.stderr
    assert "Failed to parse MODULE.bazel for local module" in failure_stderr
    assert module_name in failure_stderr

    _write(
        local_lib / "MODULE.bazel",
        f"""module(name = "{module_name}", version = "2.0")
dep = use_extension("//:dep_ext.bzl", "dep_ext")
use_repo(dep, "repaired_repo")
""",
    )
    output, _recovered = await _audit_cells_and_counters(buck)
    assert module_name in output
    assert "repaired_repo" in output


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_out_of_project_local_override_utf8_failure_invalidates_bzlmod_resolution(
    buck: Buck,
) -> None:
    """Bazel anchor: local_path_override MODULE.bazel UTF-8 errors are inputs."""
    module_name = "external_local_utf8_failure"
    local_lib = buck.cwd.parent / f"{buck.cwd.name}_{module_name}"
    local_lib.mkdir(parents=True, exist_ok=True)
    _write(local_lib / "MODULE.bazel", f'module(name = "{module_name}", version = "1.0")\n')
    _write(
        local_lib / "dep_ext.bzl",
        """def _dep_ext_impl(module_ctx):
    pass

dep_ext = module_extension(
    implementation = _dep_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_external_local_utf8_failure")

bazel_dep(name = "{module_name}")
local_path_override(
    module_name = "{module_name}",
    path = "{local_lib.as_posix()}",
)
""",
    )

    before = await _bzlmod_counters(buck)
    output, first = await _audit_cells_and_counters(buck)
    assert module_name in output
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    output, warm = await _audit_cells_and_counters(buck)
    assert module_name in output
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write_bytes(local_lib / "MODULE.bazel", b"\xff\xfeinvalid module file\n")
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell")
    failure_stderr = exc.value.stderr
    assert "Failed to parse MODULE.bazel for local module" in failure_stderr
    assert "valid UTF-8" in failure_stderr
    assert str(local_lib / "MODULE.bazel") in failure_stderr

    _write(
        local_lib / "MODULE.bazel",
        f"""module(name = "{module_name}", version = "2.0")
dep = use_extension("//:dep_ext.bzl", "dep_ext")
use_repo(dep, "repaired_repo")
""",
    )
    output, _recovered = await _audit_cells_and_counters(buck)
    assert module_name in output
    assert "repaired_repo" in output


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_out_of_project_local_override_include_cycle_invalidates_bzlmod_resolution(
    buck: Buck,
) -> None:
    """Bazel anchor: local_path_override MODULE.bazel include cycles are inputs."""
    module_name = "external_local_include_cycle"
    local_lib = buck.cwd.parent / f"{buck.cwd.name}_{module_name}"
    local_lib.mkdir(parents=True, exist_ok=True)
    _write(
        local_lib / "MODULE.bazel",
        f"""module(name = "{module_name}", version = "1.0")
include("//:cycle.MODULE.bazel")
""",
    )
    _write(local_lib / "cycle.MODULE.bazel", "# initially valid included segment\n")
    _write(
        local_lib / "dep_ext.bzl",
        """def _dep_ext_impl(module_ctx):
    pass

dep_ext = module_extension(
    implementation = _dep_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_external_local_include_cycle")

bazel_dep(name = "{module_name}")
local_path_override(
    module_name = "{module_name}",
    path = "{local_lib.as_posix()}",
)
""",
    )

    before = await _bzlmod_counters(buck)
    output, first = await _audit_cells_and_counters(buck)
    assert module_name in output
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    output, warm = await _audit_cells_and_counters(buck)
    assert module_name in output
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write(local_lib / "cycle.MODULE.bazel", 'include("//:cycle.MODULE.bazel")\n')
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell")
    failure_stderr = exc.value.stderr
    assert "Failed to parse MODULE.bazel for local module" in failure_stderr
    assert "cyclic include" in failure_stderr

    _write(
        local_lib / "cycle.MODULE.bazel",
        """dep = use_extension("//:dep_ext.bzl", "dep_ext")
use_repo(dep, "repaired_repo")
""",
    )
    output, _recovered = await _audit_cells_and_counters(buck)
    assert module_name in output
    assert "repaired_repo" in output


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_cached_git_override_module_edit_invalidates_bzlmod_resolution(
    buck: Buck,
) -> None:
    """Bazel anchor: git_override fetched MODULE.bazel is a module-resolution input."""
    module_name = "git_override_lib"
    commit = "abcdef1234567890"
    remote = f"https://example.invalid/{module_name}.git"
    cache_home = buck.cwd.parent / f"{buck.cwd.name}_git_override_cache_home"
    override_dir = _git_override_cache_dir(cache_home, module_name, remote, commit)
    override_dir.mkdir(parents=True)
    _write(override_dir / ".complete", "")
    _write(
        override_dir / "MODULE.bazel",
        f"""module(name = "{module_name}", version = "1.0")
include("//:cycle.MODULE.bazel")
""",
    )
    _write(override_dir / "cycle.MODULE.bazel", "# initially valid included segment\n")
    _write(
        override_dir / "dep_ext.bzl",
        """def _dep_ext_impl(module_ctx):
    pass

dep_ext = module_extension(
    implementation = _dep_ext_impl,
)
""",
    )
    _write(override_dir / "BUILD.bazel", 'filegroup(name = "ok", srcs = [])\n')
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_git_override_input")

bazel_dep(name = "{module_name}")
git_override(
    module_name = "{module_name}",
    remote = "{remote}",
    commit = "{commit}",
)
""",
    )

    env = {"XDG_CACHE_HOME": str(cache_home)}
    before = await _bzlmod_counters(buck, env=env)
    output, first = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    output, warm = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write(override_dir / "MODULE.bazel", f'module(name = "{module_name}", version = "2.0")\n')

    output, second = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert second["bzlmod_resolution_compute"] > warm["bzlmod_resolution_compute"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_cached_archive_override_module_edit_invalidates_bzlmod_resolution(
    buck: Buck,
) -> None:
    """Bazel anchor: archive_override extracted MODULE.bazel is a module-resolution input."""
    module_name = "archive_override_lib"
    urls = [f"https://example.invalid/{module_name}.tar.gz"]
    cache_home = buck.cwd.parent / f"{buck.cwd.name}_archive_override_cache_home"
    override_dir = _archive_override_cache_dir(cache_home, module_name, urls)
    override_dir.mkdir(parents=True)
    _write(override_dir / ".complete", "")
    _write(override_dir / "MODULE.bazel", f'module(name = "{module_name}", version = "1.0")\n')
    _write(
        override_dir / "dep_ext.bzl",
        """def _dep_ext_impl(module_ctx):
    pass

dep_ext = module_extension(
    implementation = _dep_ext_impl,
)
""",
    )
    _write(override_dir / "BUILD.bazel", 'filegroup(name = "ok", srcs = [])\n')
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_archive_override_input")

bazel_dep(name = "{module_name}")
archive_override(
    module_name = "{module_name}",
    urls = {urls!r},
)
""",
    )

    env = {"XDG_CACHE_HOME": str(cache_home)}
    before = await _bzlmod_counters(buck, env=env)
    output, first = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    output, warm = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write(override_dir / "MODULE.bazel", f'module(name = "{module_name}", version = "2.0")\n')

    output, second = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert second["bzlmod_resolution_compute"] > warm["bzlmod_resolution_compute"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_cached_git_override_module_creation_invalidates_bzlmod_resolution(
    buck: Buck,
) -> None:
    """Bazel anchor: git_override missing-to-present MODULE.bazel transitions are inputs."""
    module_name = "git_override_created_module"
    commit = "abcdef1234567890"
    remote = f"https://example.invalid/{module_name}.git"
    cache_home = buck.cwd.parent / f"{buck.cwd.name}_git_override_create_cache_home"
    override_dir = _git_override_cache_dir(cache_home, module_name, remote, commit)
    override_dir.mkdir(parents=True)
    _write(override_dir / ".complete", "")
    _write(override_dir / "BUILD.bazel", 'filegroup(name = "ok", srcs = [])\n')
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_git_override_create_input")

bazel_dep(name = "{module_name}")
git_override(
    module_name = "{module_name}",
    remote = "{remote}",
    commit = "{commit}",
)
""",
    )

    env = {"XDG_CACHE_HOME": str(cache_home)}
    before = await _bzlmod_counters(buck, env=env)
    output, first = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    output, warm = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write(override_dir / "MODULE.bazel", f'module(name = "{module_name}", version = "1.0")\n')

    output, second = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert second["bzlmod_resolution_compute"] > warm["bzlmod_resolution_compute"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_cached_archive_override_module_deletion_invalidates_bzlmod_resolution(
    buck: Buck,
) -> None:
    """Bazel anchor: archive_override present-to-missing MODULE.bazel transitions are inputs."""
    module_name = "archive_override_deleted_module"
    urls = [f"https://example.invalid/{module_name}.tar.gz"]
    cache_home = buck.cwd.parent / f"{buck.cwd.name}_archive_override_delete_cache_home"
    override_dir = _archive_override_cache_dir(cache_home, module_name, urls)
    override_dir.mkdir(parents=True)
    _write(override_dir / ".complete", "")
    module_file = override_dir / "MODULE.bazel"
    _write(module_file, f'module(name = "{module_name}", version = "1.0")\n')
    _write(override_dir / "BUILD.bazel", 'filegroup(name = "ok", srcs = [])\n')
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_archive_override_delete_input")

bazel_dep(name = "{module_name}")
archive_override(
    module_name = "{module_name}",
    urls = {urls!r},
)
""",
    )

    env = {"XDG_CACHE_HOME": str(cache_home)}
    before = await _bzlmod_counters(buck, env=env)
    output, first = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    output, warm = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    module_file.unlink()

    output, second = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert second["bzlmod_resolution_compute"] > warm["bzlmod_resolution_compute"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_cached_git_override_module_parse_failure_invalidates_bzlmod_resolution(
    buck: Buck,
) -> None:
    """Bazel anchor: git_override fetched MODULE.bazel parse errors are resolution inputs."""
    module_name = "git_override_parse_failure"
    commit = "abcdef1234567890"
    remote = f"https://example.invalid/{module_name}.git"
    cache_home = buck.cwd.parent / f"{buck.cwd.name}_git_override_parse_failure_cache_home"
    override_dir = _git_override_cache_dir(cache_home, module_name, remote, commit)
    override_dir.mkdir(parents=True)
    _write(override_dir / ".complete", "")
    _write(
        override_dir / "MODULE.bazel",
        f"""module(name = "{module_name}", version = "1.0")
include("//:cycle.MODULE.bazel")
""",
    )
    _write(override_dir / "cycle.MODULE.bazel", "# initially valid included segment\n")
    _write(
        override_dir / "dep_ext.bzl",
        """def _dep_ext_impl(module_ctx):
    pass

dep_ext = module_extension(
    implementation = _dep_ext_impl,
)
""",
    )
    _write(override_dir / "BUILD.bazel", 'filegroup(name = "ok", srcs = [])\n')
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_git_override_parse_failure")

bazel_dep(name = "{module_name}")
git_override(
    module_name = "{module_name}",
    remote = "{remote}",
    commit = "{commit}",
)
""",
    )

    env = {"XDG_CACHE_HOME": str(cache_home)}
    before = await _bzlmod_counters(buck, env=env)
    output, first = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    output, warm = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write(override_dir / "MODULE.bazel", f'module(name = "{module_name}", version = )\n')
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell", env=env)
    failure_stderr = exc.value.stderr
    assert "Failed to parse MODULE.bazel for git override" in failure_stderr
    assert module_name in failure_stderr

    _write(
        override_dir / "MODULE.bazel",
        f"""module(name = "{module_name}", version = "2.0")
dep = use_extension("//:dep_ext.bzl", "dep_ext")
use_repo(dep, "repaired_repo")
""",
    )
    output, _recovered = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert "repaired_repo" in output


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_cached_archive_override_module_utf8_failure_invalidates_bzlmod_resolution(
    buck: Buck,
) -> None:
    """Bazel anchor: archive_override extracted MODULE.bazel UTF-8 errors are inputs."""
    module_name = "archive_override_utf8_failure"
    urls = [f"https://example.invalid/{module_name}.tar.gz"]
    cache_home = buck.cwd.parent / f"{buck.cwd.name}_archive_override_utf8_failure_cache_home"
    override_dir = _archive_override_cache_dir(cache_home, module_name, urls)
    override_dir.mkdir(parents=True)
    _write(override_dir / ".complete", "")
    _write(override_dir / "MODULE.bazel", f'module(name = "{module_name}", version = "1.0")\n')
    _write(
        override_dir / "dep_ext.bzl",
        """def _dep_ext_impl(module_ctx):
    pass

dep_ext = module_extension(
    implementation = _dep_ext_impl,
)
""",
    )
    _write(override_dir / "BUILD.bazel", 'filegroup(name = "ok", srcs = [])\n')
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_archive_override_utf8_failure")

bazel_dep(name = "{module_name}")
archive_override(
    module_name = "{module_name}",
    urls = {urls!r},
)
""",
    )

    env = {"XDG_CACHE_HOME": str(cache_home)}
    before = await _bzlmod_counters(buck, env=env)
    output, first = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    output, warm = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write_bytes(override_dir / "MODULE.bazel", b"\xff\xfeinvalid module file\n")
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell", env=env)
    failure_stderr = exc.value.stderr
    assert "Failed to parse MODULE.bazel for archive override" in failure_stderr
    assert "valid UTF-8" in failure_stderr
    assert str(override_dir / "MODULE.bazel") in failure_stderr

    _write(
        override_dir / "MODULE.bazel",
        f"""module(name = "{module_name}", version = "2.0")
dep = use_extension("//:dep_ext.bzl", "dep_ext")
use_repo(dep, "repaired_repo")
""",
    )
    output, _recovered = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert "repaired_repo" in output


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_cached_archive_override_module_parse_failure_invalidates_bzlmod_resolution(
    buck: Buck,
) -> None:
    """Bazel anchor: archive_override extracted MODULE.bazel parse errors are inputs."""
    module_name = "archive_override_parse_failure"
    urls = [f"https://example.invalid/{module_name}.tar.gz"]
    cache_home = buck.cwd.parent / f"{buck.cwd.name}_archive_override_parse_failure_cache_home"
    override_dir = _archive_override_cache_dir(cache_home, module_name, urls)
    override_dir.mkdir(parents=True)
    _write(override_dir / ".complete", "")
    _write(override_dir / "MODULE.bazel", f'module(name = "{module_name}", version = "1.0")\n')
    _write(
        override_dir / "dep_ext.bzl",
        """def _dep_ext_impl(module_ctx):
    pass

dep_ext = module_extension(
    implementation = _dep_ext_impl,
)
""",
    )
    _write(override_dir / "BUILD.bazel", 'filegroup(name = "ok", srcs = [])\n')
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_archive_override_parse_failure")

bazel_dep(name = "{module_name}")
archive_override(
    module_name = "{module_name}",
    urls = {urls!r},
)
""",
    )

    env = {"XDG_CACHE_HOME": str(cache_home)}
    before = await _bzlmod_counters(buck, env=env)
    output, first = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    output, warm = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write(override_dir / "MODULE.bazel", f'module(name = "{module_name}", version = )\n')
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell", env=env)
    failure_stderr = exc.value.stderr
    assert "Failed to parse MODULE.bazel for archive override" in failure_stderr
    assert module_name in failure_stderr

    _write(
        override_dir / "MODULE.bazel",
        f"""module(name = "{module_name}", version = "2.0")
dep = use_extension("//:dep_ext.bzl", "dep_ext")
use_repo(dep, "repaired_repo")
""",
    )
    output, _recovered = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert "repaired_repo" in output


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_cached_git_override_module_utf8_failure_invalidates_bzlmod_resolution(
    buck: Buck,
) -> None:
    """Bazel anchor: git_override fetched MODULE.bazel UTF-8 errors are inputs."""
    module_name = "git_override_utf8_failure"
    commit = "abcdef1234567890"
    remote = f"https://example.invalid/{module_name}.git"
    cache_home = buck.cwd.parent / f"{buck.cwd.name}_git_override_utf8_failure_cache_home"
    override_dir = _git_override_cache_dir(cache_home, module_name, remote, commit)
    override_dir.mkdir(parents=True)
    _write(override_dir / ".complete", "")
    _write(override_dir / "MODULE.bazel", f'module(name = "{module_name}", version = "1.0")\n')
    _write(
        override_dir / "dep_ext.bzl",
        """def _dep_ext_impl(module_ctx):
    pass

dep_ext = module_extension(
    implementation = _dep_ext_impl,
)
""",
    )
    _write(override_dir / "BUILD.bazel", 'filegroup(name = "ok", srcs = [])\n')
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_git_override_utf8_failure")

bazel_dep(name = "{module_name}")
git_override(
    module_name = "{module_name}",
    remote = "{remote}",
    commit = "{commit}",
)
""",
    )

    env = {"XDG_CACHE_HOME": str(cache_home)}
    before = await _bzlmod_counters(buck, env=env)
    output, first = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    output, warm = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write_bytes(override_dir / "MODULE.bazel", b"\xff\xfeinvalid module file\n")
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell", env=env)
    failure_stderr = exc.value.stderr
    assert "Failed to parse MODULE.bazel for git override" in failure_stderr
    assert "valid UTF-8" in failure_stderr
    assert str(override_dir / "MODULE.bazel") in failure_stderr

    _write(
        override_dir / "MODULE.bazel",
        f"""module(name = "{module_name}", version = "2.0")
dep = use_extension("//:dep_ext.bzl", "dep_ext")
use_repo(dep, "repaired_repo")
""",
    )
    output, _recovered = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert "repaired_repo" in output


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_cached_git_override_module_include_cycle_invalidates_bzlmod_resolution(
    buck: Buck,
) -> None:
    """Bazel anchor: git_override fetched MODULE.bazel include cycles are inputs."""
    module_name = "git_override_include_cycle"
    commit = "abcdef1234567890"
    remote = f"https://example.invalid/{module_name}.git"
    cache_home = buck.cwd.parent / f"{buck.cwd.name}_git_override_include_cycle_cache_home"
    override_dir = _git_override_cache_dir(cache_home, module_name, remote, commit)
    override_dir.mkdir(parents=True)
    _write(override_dir / ".complete", "")
    _write(
        override_dir / "MODULE.bazel",
        f"""module(name = "{module_name}", version = "1.0")
include("//:cycle.MODULE.bazel")
""",
    )
    _write(override_dir / "cycle.MODULE.bazel", "# initially valid included segment\n")
    _write(
        override_dir / "dep_ext.bzl",
        """def _dep_ext_impl(module_ctx):
    pass

dep_ext = module_extension(
    implementation = _dep_ext_impl,
)
""",
    )
    _write(override_dir / "BUILD.bazel", 'filegroup(name = "ok", srcs = [])\n')
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_git_override_include_cycle")

bazel_dep(name = "{module_name}")
git_override(
    module_name = "{module_name}",
    remote = "{remote}",
    commit = "{commit}",
)
""",
    )

    env = {"XDG_CACHE_HOME": str(cache_home)}
    before = await _bzlmod_counters(buck, env=env)
    output, first = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    output, warm = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write(override_dir / "cycle.MODULE.bazel", 'include("//:cycle.MODULE.bazel")\n')
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell", env=env)
    failure_stderr = exc.value.stderr
    assert "Failed to parse MODULE.bazel for git override" in failure_stderr
    assert "cyclic include" in failure_stderr

    _write(
        override_dir / "cycle.MODULE.bazel",
        """dep = use_extension("//:dep_ext.bzl", "dep_ext")
use_repo(dep, "repaired_repo")
""",
    )
    output, _recovered = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert "repaired_repo" in output


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_cached_archive_override_module_include_cycle_invalidates_bzlmod_resolution(
    buck: Buck,
) -> None:
    """Bazel anchor: archive_override extracted MODULE.bazel include cycles are inputs."""
    module_name = "archive_override_include_cycle"
    urls = [f"https://example.invalid/{module_name}.tar.gz"]
    cache_home = buck.cwd.parent / f"{buck.cwd.name}_archive_override_include_cycle_cache_home"
    override_dir = _archive_override_cache_dir(cache_home, module_name, urls)
    override_dir.mkdir(parents=True)
    _write(override_dir / ".complete", "")
    _write(
        override_dir / "MODULE.bazel",
        f"""module(name = "{module_name}", version = "1.0")
include("//:cycle.MODULE.bazel")
""",
    )
    _write(override_dir / "cycle.MODULE.bazel", "# initially valid included segment\n")
    _write(
        override_dir / "dep_ext.bzl",
        """def _dep_ext_impl(module_ctx):
    pass

dep_ext = module_extension(
    implementation = _dep_ext_impl,
)
""",
    )
    _write(override_dir / "BUILD.bazel", 'filegroup(name = "ok", srcs = [])\n')
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_archive_override_include_cycle")

bazel_dep(name = "{module_name}")
archive_override(
    module_name = "{module_name}",
    urls = {urls!r},
)
""",
    )

    env = {"XDG_CACHE_HOME": str(cache_home)}
    before = await _bzlmod_counters(buck, env=env)
    output, first = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    output, warm = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write(override_dir / "cycle.MODULE.bazel", 'include("//:cycle.MODULE.bazel")\n')
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell", env=env)
    failure_stderr = exc.value.stderr
    assert "Failed to parse MODULE.bazel for archive override" in failure_stderr
    assert "cyclic include" in failure_stderr

    _write(
        override_dir / "cycle.MODULE.bazel",
        """dep = use_extension("//:dep_ext.bzl", "dep_ext")
use_repo(dep, "repaired_repo")
""",
    )
    output, _recovered = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert "repaired_repo" in output


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_warm_noop_locked_registry_dep_reuses_bzlmod_resolution(
    buck: Buck,
) -> None:
    """Bazel anchor: selected registry modules are in BazelModuleResolutionValue."""
    module_name = "remote_lib"
    module_version = "1.0.0"
    cache_home = buck.cwd / "cache_home"
    module_cache = (
        cache_home
        / "slug"
        / "registry"
        / "bcr.bazel.build"
        / "modules"
        / module_name
        / module_version
    )
    registry_cache = cache_home / "slug" / "registry" / "bcr.bazel.build"
    source_dir = module_cache / "source"
    source_dir.mkdir(parents=True)
    _write(registry_cache / "bazel_registry.json", "{}\n")
    _write(
        module_cache / "MODULE.bazel",
        f'module(name = "{module_name}", version = "{module_version}")\n',
    )
    _write(module_cache / "source.json", "{}\n")
    _write(source_dir / ".complete", "")
    _write(source_dir / "BUILD.bazel", 'filegroup(name = "ok", srcs = [])\n')
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_registry_warm")

bazel_dep(name = "{module_name}", version = "{module_version}")
""",
    )

    module_url = (
        f"https://bcr.bazel.build/modules/{module_name}/{module_version}/MODULE.bazel"
    )
    source_url = (
        f"https://bcr.bazel.build/modules/{module_name}/{module_version}/source.json"
    )
    registry_url = "https://bcr.bazel.build/bazel_registry.json"
    _write(
        buck.cwd / "MODULE.bazel.lock",
        json.dumps(
            {
                "lockFileVersion": 26,
                "registryFileHashes": {
                    registry_url: _sha256(registry_cache / "bazel_registry.json"),
                    module_url: _sha256(module_cache / "MODULE.bazel"),
                    source_url: _sha256(module_cache / "source.json"),
                },
                "selectedYankedVersions": {},
                "moduleExtensions": {},
                "facts": {},
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
    )

    env = {"XDG_CACHE_HOME": str(cache_home)}
    before = await _bzlmod_counters(buck, env=env)
    output, first = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    output, second = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert second["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write(
        module_cache / "MODULE.bazel",
        f'module(name = "{module_name}", version = "2.0.0")\n',
    )
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell", env=env)
    assert "Registry file checksum mismatch" in str(exc.value)


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_warm_noop_out_of_project_registry_cache_reuses_polled_dice_input(
    buck: Buck,
) -> None:
    """Bazel anchor: registry cache files are Skyframe module-resolution inputs."""
    module_name = "external_cache_lib"
    module_version = "1.0.0"
    cache_home = buck.cwd.parent / f"{buck.cwd.name}_cache_home"
    module_cache = _write_cached_registry_module(
        cache_home,
        "bcr.bazel.build",
        module_name,
        module_version,
        f'module(name = "{module_name}", version = "{module_version}")\n',
    )
    registry_cache = cache_home / "slug" / "registry" / "bcr.bazel.build"
    _write(registry_cache / "bazel_registry.json", "{}\n")
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_external_registry_cache")

bazel_dep(name = "{module_name}", version = "{module_version}")
""",
    )

    module_url = (
        f"https://bcr.bazel.build/modules/{module_name}/{module_version}/MODULE.bazel"
    )
    source_url = (
        f"https://bcr.bazel.build/modules/{module_name}/{module_version}/source.json"
    )
    registry_url = "https://bcr.bazel.build/bazel_registry.json"
    _write(
        buck.cwd / "MODULE.bazel.lock",
        json.dumps(
            {
                "lockFileVersion": 26,
                "registryFileHashes": {
                    registry_url: _sha256(registry_cache / "bazel_registry.json"),
                    module_url: _sha256(module_cache / "MODULE.bazel"),
                    source_url: _sha256(module_cache / "source.json"),
                },
                "selectedYankedVersions": {},
                "moduleExtensions": {},
                "facts": {},
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
    )

    env = {"XDG_CACHE_HOME": str(cache_home)}
    before = await _bzlmod_counters(buck, env=env)
    output, first = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    output, second = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert second["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write(registry_cache / "bazel_registry.json", '{"mirrors": []}\n')
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell", env=env)
    assert "Registry file checksum mismatch" in str(exc.value)
    assert registry_url in str(exc.value)


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_single_version_override_registry_uses_override_registry(
    buck: Buck,
) -> None:
    """Bazel anchor: ModuleFileFunction uses RegistryOverride.getRegistry()."""
    cache_home = buck.cwd / "cache_home"
    default_registry = cache_home / "slug" / "registry" / "bcr.bazel.build"
    override_registry = cache_home / "slug" / "registry" / "override.example"
    default_registry.mkdir(parents=True)
    override_registry.mkdir(parents=True)
    _write(default_registry / "bazel_registry.json", "{}\n")
    _write(override_registry / "bazel_registry.json", "{}\n")

    bbb = _write_cached_registry_module(
        cache_home,
        "bcr.bazel.build",
        "bbb",
        "1.0.0",
        'module(name = "bbb", version = "1.0.0")\n'
        'bazel_dep(name = "ccc", version = "1.0.0")\n',
    )
    _write_cached_registry_module(
        cache_home,
        "bcr.bazel.build",
        "ccc",
        "1.0.0",
        'module(name = "ccc", version = "1.0.0")\n',
    )
    ccc_override = _write_cached_registry_module(
        cache_home,
        "override.example",
        "ccc",
        "1.0.0",
        'module(name = "ccc", version = "1.0.0")\n',
        """filegroup(
    name = "alt_only",
    srcs = [],
    visibility = ["//visibility:public"],
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_single_override_registry")

bazel_dep(name = "bbb", version = "1.0.0")
single_version_override(
    module_name = "ccc",
    registry = "https://override.example",
)
""",
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_override_ccc",
    srcs = ["@ccc//:alt_only"],
)
""",
    )

    _write(
        buck.cwd / "MODULE.bazel.lock",
        json.dumps(
            {
                "lockFileVersion": 26,
                "registryFileHashes": {
                    "https://bcr.bazel.build/bazel_registry.json": _sha256(
                        default_registry / "bazel_registry.json"
                    ),
                    "https://override.example/bazel_registry.json": _sha256(
                        override_registry / "bazel_registry.json"
                    ),
                    "https://bcr.bazel.build/modules/bbb/1.0.0/MODULE.bazel": _sha256(
                        bbb / "MODULE.bazel"
                    ),
                    "https://bcr.bazel.build/modules/bbb/1.0.0/source.json": _sha256(
                        bbb / "source.json"
                    ),
                    "https://override.example/modules/ccc/1.0.0/MODULE.bazel": _sha256(
                        ccc_override / "MODULE.bazel"
                    ),
                    "https://override.example/modules/ccc/1.0.0/source.json": _sha256(
                        ccc_override / "source.json"
                    ),
                },
                "selectedYankedVersions": {},
                "moduleExtensions": {},
                "facts": {},
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
    )

    await buck.build("//:uses_override_ccc", env={"XDG_CACHE_HOME": str(cache_home)})


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_multiple_version_override_registry_uses_override_registry(
    buck: Buck,
) -> None:
    """Bazel anchor: multiple_version_override is a RegistryOverride."""
    cache_home = buck.cwd / "cache_home"
    default_registry = cache_home / "slug" / "registry" / "bcr.bazel.build"
    override_registry = cache_home / "slug" / "registry" / "override.example"
    default_registry.mkdir(parents=True)
    override_registry.mkdir(parents=True)
    _write(default_registry / "bazel_registry.json", "{}\n")
    _write(override_registry / "bazel_registry.json", "{}\n")

    bbb = _write_cached_registry_module(
        cache_home,
        "bcr.bazel.build",
        "bbb",
        "1.0.0",
        'module(name = "bbb", version = "1.0.0")\n'
        'bazel_dep(name = "ccc", version = "1.0.0")\n',
    )
    _write_cached_registry_module(
        cache_home,
        "bcr.bazel.build",
        "ccc",
        "1.0.0",
        'module(name = "ccc", version = "1.0.0")\n',
    )
    ccc_override = _write_cached_registry_module(
        cache_home,
        "override.example",
        "ccc",
        "1.0.0",
        'module(name = "ccc", version = "1.0.0")\n',
        """filegroup(
    name = "alt_only",
    srcs = [],
    visibility = ["//visibility:public"],
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_multiple_override_registry")

bazel_dep(name = "bbb", version = "1.0.0")
multiple_version_override(
    module_name = "ccc",
    versions = ["1.0.0"],
    registry = "https://override.example",
)
""",
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_override_ccc",
    srcs = ["@ccc//:alt_only"],
)
""",
    )

    _write(
        buck.cwd / "MODULE.bazel.lock",
        json.dumps(
            {
                "lockFileVersion": 26,
                "registryFileHashes": {
                    "https://bcr.bazel.build/bazel_registry.json": _sha256(
                        default_registry / "bazel_registry.json"
                    ),
                    "https://override.example/bazel_registry.json": _sha256(
                        override_registry / "bazel_registry.json"
                    ),
                    "https://bcr.bazel.build/modules/bbb/1.0.0/MODULE.bazel": _sha256(
                        bbb / "MODULE.bazel"
                    ),
                    "https://bcr.bazel.build/modules/bbb/1.0.0/source.json": _sha256(
                        bbb / "source.json"
                    ),
                    "https://override.example/modules/ccc/1.0.0/MODULE.bazel": _sha256(
                        ccc_override / "MODULE.bazel"
                    ),
                    "https://override.example/modules/ccc/1.0.0/source.json": _sha256(
                        ccc_override / "source.json"
                    ),
                },
                "selectedYankedVersions": {},
                "moduleExtensions": {},
                "facts": {},
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
    )

    await buck.build("//:uses_override_ccc", env={"XDG_CACHE_HOME": str(cache_home)})


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_locked_registry_source_json_and_registry_metadata_are_bridge_inputs(
    buck: Buck,
) -> None:
    """Bazel anchor: RepoSpec reads source.json and top-level registry metadata."""
    module_name = "remote_meta"
    module_version = "1.0.0"
    cache_home = buck.cwd / "cache_home"
    registry_cache = cache_home / "slug" / "registry" / "bcr.bazel.build"
    module_cache = registry_cache / "modules" / module_name / module_version
    source_dir = module_cache / "source"
    source_dir.mkdir(parents=True)
    _write(registry_cache / "bazel_registry.json", "{}\n")
    _write(
        module_cache / "MODULE.bazel",
        f'module(name = "{module_name}", version = "{module_version}")\n',
    )
    _write(module_cache / "source.json", "{}\n")
    _write(source_dir / ".complete", "")
    _write(source_dir / "BUILD.bazel", 'filegroup(name = "ok", srcs = [])\n')
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_registry_metadata")

bazel_dep(name = "{module_name}", version = "{module_version}")
""",
    )

    registry_url = "https://bcr.bazel.build/bazel_registry.json"
    module_url = (
        f"https://bcr.bazel.build/modules/{module_name}/{module_version}/MODULE.bazel"
    )
    source_url = (
        f"https://bcr.bazel.build/modules/{module_name}/{module_version}/source.json"
    )
    _write(
        buck.cwd / "MODULE.bazel.lock",
        json.dumps(
            {
                "lockFileVersion": 26,
                "registryFileHashes": {
                    registry_url: _sha256(registry_cache / "bazel_registry.json"),
                    module_url: _sha256(module_cache / "MODULE.bazel"),
                    source_url: _sha256(module_cache / "source.json"),
                },
                "selectedYankedVersions": {},
                "moduleExtensions": {},
                "facts": {},
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
    )

    env = {"XDG_CACHE_HOME": str(cache_home)}
    output, first = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output

    output, second = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert second["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write(module_cache / "source.json", '{"type": "archive"}\n')
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell", env=env)
    assert "Registry file checksum mismatch" in str(exc.value)
    assert source_url in str(exc.value)

    _write(module_cache / "source.json", "{}\n")
    _write(registry_cache / "bazel_registry.json", '{"mirrors": []}\n')
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell", env=env)
    assert "Registry file checksum mismatch" in str(exc.value)
    assert registry_url in str(exc.value)


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_locked_registry_source_json_parse_failure_invalidates_bzlmod_resolution(
    buck: Buck,
) -> None:
    """Bazel anchor: RepoSpec source.json parse failures are resolution inputs."""
    module_name = "remote_source_parse_failure"
    module_version = "1.0.0"
    cache_home = buck.cwd / "cache_home"
    registry_cache = cache_home / "slug" / "registry" / "bcr.bazel.build"
    module_cache = registry_cache / "modules" / module_name / module_version
    source_dir = module_cache / "source"
    source_dir.mkdir(parents=True)
    _write(registry_cache / "bazel_registry.json", "{}\n")
    _write(
        module_cache / "MODULE.bazel",
        f'module(name = "{module_name}", version = "{module_version}")\n',
    )
    source_json = module_cache / "source.json"
    _write(source_json, "{}\n")
    _write(source_dir / ".complete", "")
    _write(source_dir / "BUILD.bazel", 'filegroup(name = "ok", srcs = [])\n')
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_registry_source_parse_failure")

bazel_dep(name = "{module_name}", version = "{module_version}")
""",
    )

    registry_url = "https://bcr.bazel.build/bazel_registry.json"
    module_url = (
        f"https://bcr.bazel.build/modules/{module_name}/{module_version}/MODULE.bazel"
    )
    source_url = (
        f"https://bcr.bazel.build/modules/{module_name}/{module_version}/source.json"
    )

    def write_lockfile() -> None:
        _write(
            buck.cwd / "MODULE.bazel.lock",
            json.dumps(
                {
                    "lockFileVersion": 26,
                    "registryFileHashes": {
                        registry_url: _sha256(registry_cache / "bazel_registry.json"),
                        module_url: _sha256(module_cache / "MODULE.bazel"),
                        source_url: _sha256(source_json),
                    },
                    "selectedYankedVersions": {},
                    "moduleExtensions": {},
                    "facts": {},
                },
                indent=2,
                sort_keys=True,
            )
            + "\n",
        )

    write_lockfile()
    env = {"XDG_CACHE_HOME": str(cache_home)}
    output, first = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output

    output, warm = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write(source_json, "{not json}\n")
    write_lockfile()
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell", env=env)
    failure_stderr = exc.value.stderr
    assert "Failed to parse source.json" in failure_stderr
    assert module_name in failure_stderr

    _write(source_json, "{}\n")
    write_lockfile()
    output, _recovered = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_locked_registry_module_parse_failure_invalidates_bzlmod_resolution(
    buck: Buck,
) -> None:
    """Bazel anchor: registry MODULE.bazel parse failures are resolution inputs."""
    module_name = "remote_module_parse_failure"
    module_version = "1.0.0"
    repaired_module_name = "remote_module_parse_repaired_dep"
    cache_home = buck.cwd / "cache_home"
    registry_cache = cache_home / "slug" / "registry" / "bcr.bazel.build"
    module_cache = registry_cache / "modules" / module_name / module_version
    source_dir = module_cache / "source"
    source_dir.mkdir(parents=True)
    _write(registry_cache / "bazel_registry.json", "{}\n")
    module_file = module_cache / "MODULE.bazel"
    _write(module_file, f'module(name = "{module_name}", version = "{module_version}")\n')
    _write(module_cache / "source.json", "{}\n")
    _write(source_dir / ".complete", "")
    _write(source_dir / "BUILD.bazel", 'filegroup(name = "ok", srcs = [])\n')
    repaired_module_cache = _write_cached_registry_module(
        cache_home,
        "bcr.bazel.build",
        repaired_module_name,
        module_version,
        f'module(name = "{repaired_module_name}", version = "{module_version}")\n',
    )
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_registry_module_parse_failure")

bazel_dep(name = "{module_name}", version = "{module_version}")
""",
    )

    registry_url = "https://bcr.bazel.build/bazel_registry.json"
    module_url = (
        f"https://bcr.bazel.build/modules/{module_name}/{module_version}/MODULE.bazel"
    )
    source_url = (
        f"https://bcr.bazel.build/modules/{module_name}/{module_version}/source.json"
    )
    repaired_module_url = (
        f"https://bcr.bazel.build/modules/{repaired_module_name}/{module_version}/MODULE.bazel"
    )
    repaired_source_url = (
        f"https://bcr.bazel.build/modules/{repaired_module_name}/{module_version}/source.json"
    )

    def write_lockfile() -> None:
        _write(
            buck.cwd / "MODULE.bazel.lock",
            json.dumps(
                {
                    "lockFileVersion": 26,
                    "registryFileHashes": {
                        registry_url: _sha256(registry_cache / "bazel_registry.json"),
                        module_url: _sha256(module_file),
                        source_url: _sha256(module_cache / "source.json"),
                        repaired_module_url: _sha256(repaired_module_cache / "MODULE.bazel"),
                        repaired_source_url: _sha256(repaired_module_cache / "source.json"),
                    },
                    "selectedYankedVersions": {},
                    "moduleExtensions": {},
                    "facts": {},
                },
                indent=2,
                sort_keys=True,
            )
            + "\n",
        )

    write_lockfile()
    env = {"XDG_CACHE_HOME": str(cache_home)}
    output, first = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output

    output, warm = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write(module_file, f'module(name = "{module_name}", version = )\n')
    write_lockfile()
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell", env=env)
    failure_stderr = exc.value.stderr
    assert "Failed to parse MODULE.bazel" in failure_stderr
    assert module_name in failure_stderr

    _write(
        module_file,
        f"""module(name = "{module_name}", version = "{module_version}")
bazel_dep(name = "{repaired_module_name}", version = "{module_version}")
""",
    )
    write_lockfile()
    output, _recovered = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert repaired_module_name in output


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_locked_registry_module_utf8_failure_invalidates_bzlmod_resolution(
    buck: Buck,
) -> None:
    """Bazel anchor: registry MODULE.bazel UTF-8 failures are resolution inputs."""
    module_name = "remote_module_utf8_failure"
    module_version = "1.0.0"
    repaired_module_name = "remote_module_utf8_repaired_dep"
    cache_home = buck.cwd / "cache_home"
    registry_cache = cache_home / "slug" / "registry" / "bcr.bazel.build"
    module_cache = registry_cache / "modules" / module_name / module_version
    source_dir = module_cache / "source"
    source_dir.mkdir(parents=True)
    _write(registry_cache / "bazel_registry.json", "{}\n")
    module_file = module_cache / "MODULE.bazel"
    _write(module_file, f'module(name = "{module_name}", version = "{module_version}")\n')
    _write(module_cache / "source.json", "{}\n")
    _write(source_dir / ".complete", "")
    _write(source_dir / "BUILD.bazel", 'filegroup(name = "ok", srcs = [])\n')
    repaired_module_cache = _write_cached_registry_module(
        cache_home,
        "bcr.bazel.build",
        repaired_module_name,
        module_version,
        f'module(name = "{repaired_module_name}", version = "{module_version}")\n',
    )
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_registry_module_utf8_failure")

bazel_dep(name = "{module_name}", version = "{module_version}")
""",
    )

    registry_url = "https://bcr.bazel.build/bazel_registry.json"
    module_url = (
        f"https://bcr.bazel.build/modules/{module_name}/{module_version}/MODULE.bazel"
    )
    source_url = (
        f"https://bcr.bazel.build/modules/{module_name}/{module_version}/source.json"
    )
    repaired_module_url = (
        f"https://bcr.bazel.build/modules/{repaired_module_name}/{module_version}/MODULE.bazel"
    )
    repaired_source_url = (
        f"https://bcr.bazel.build/modules/{repaired_module_name}/{module_version}/source.json"
    )

    def write_lockfile() -> None:
        _write(
            buck.cwd / "MODULE.bazel.lock",
            json.dumps(
                {
                    "lockFileVersion": 26,
                    "registryFileHashes": {
                        registry_url: _sha256(registry_cache / "bazel_registry.json"),
                        module_url: _sha256(module_file),
                        source_url: _sha256(module_cache / "source.json"),
                        repaired_module_url: _sha256(repaired_module_cache / "MODULE.bazel"),
                        repaired_source_url: _sha256(repaired_module_cache / "source.json"),
                    },
                    "selectedYankedVersions": {},
                    "moduleExtensions": {},
                    "facts": {},
                },
                indent=2,
                sort_keys=True,
            )
            + "\n",
        )

    write_lockfile()
    env = {"XDG_CACHE_HOME": str(cache_home)}
    output, first = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output

    output, warm = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write_bytes(module_file, b"\xff\xfeinvalid module file\n")
    write_lockfile()
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell", env=env)
    failure_stderr = exc.value.stderr
    assert "Failed to fetch MODULE.bazel" in failure_stderr
    assert "Failed to read cached MODULE.bazel" in failure_stderr

    _write(
        module_file,
        f"""module(name = "{module_name}", version = "{module_version}")
bazel_dep(name = "{repaired_module_name}", version = "{module_version}")
""",
    )
    write_lockfile()
    output, _recovered = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert repaired_module_name in output


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_lockfile_selected_yanked_version_edit_invalidates_bzlmod_resolution(
    buck: Buck,
) -> None:
    """Bazel anchor: selectedYankedVersions is a BazelLockFileValue input."""
    module_name = "remote_selected_yanked"
    module_version = "1.0.0"
    selected_key = f"{module_name}@{module_version}"
    cache_home = buck.cwd / "cache_home"
    module_cache = _write_cached_registry_module(
        cache_home,
        "bcr.bazel.build",
        module_name,
        module_version,
        f'module(name = "{module_name}", version = "{module_version}")\n',
    )
    registry_cache = cache_home / "slug" / "registry" / "bcr.bazel.build"
    _write(registry_cache / "bazel_registry.json", "{}\n")
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_selected_yanked")

bazel_dep(name = "{module_name}", version = "{module_version}")
""",
    )

    registry_url = "https://bcr.bazel.build/bazel_registry.json"
    module_url = (
        f"https://bcr.bazel.build/modules/{module_name}/{module_version}/MODULE.bazel"
    )
    source_url = (
        f"https://bcr.bazel.build/modules/{module_name}/{module_version}/source.json"
    )

    def write_lockfile(selected_yanked_versions: dict[str, str]) -> None:
        _write(
            buck.cwd / "MODULE.bazel.lock",
            json.dumps(
                {
                    "lockFileVersion": 26,
                    "registryFileHashes": {
                        registry_url: _sha256(registry_cache / "bazel_registry.json"),
                        module_url: _sha256(module_cache / "MODULE.bazel"),
                        source_url: _sha256(module_cache / "source.json"),
                    },
                    "selectedYankedVersions": selected_yanked_versions,
                    "moduleExtensions": {},
                    "facts": {},
                },
                indent=2,
                sort_keys=True,
            )
            + "\n",
        )

    write_lockfile({})
    env = {"XDG_CACHE_HOME": str(cache_home)}
    output, first = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output

    output, warm = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    write_lockfile({selected_key: "security issue"})
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell", env=env)
    failure_stderr = exc.value.stderr
    assert selected_key in failure_stderr
    assert "security issue" in failure_stderr

    write_lockfile({})
    output, _recovered = await _audit_cells_and_counters(buck, env=env)
    assert module_name in output


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_warm_noop_extension_replay_audit_cell_reuses_bzlmod_resolution(
    buck: Buck,
) -> None:
    """Bazel anchor: valid extension replay is a Skyframe cut-off input."""
    module_name = "plan61_replay_warm"
    extension_id = f"@{module_name}//:replay_ext.bzl%replay_ext"
    repo_path = buck.cwd / "replayed_repo"
    repo_path.mkdir()
    _write(repo_path / "MODULE.bazel", 'module(name = "replayed_repo")\n')
    _write(repo_path / "BUILD.bazel", 'filegroup(name = "ok", srcs = [])\n')
    _write(
        buck.cwd / "replay_ext.bzl",
        """def _impl(module_ctx):
    fail("extension should replay from lockfile")

replay_ext = module_extension(
    implementation = _impl,
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
        repo_path=repo_path,
    )

    before = await _bzlmod_counters(buck)
    output, first = await _audit_cells_and_counters(buck)
    assert module_name in output
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]
    assert first["extension_replay_hit"] > before["extension_replay_hit"]
    assert first["lockfile_read"] == before["lockfile_read"] + 1

    output, second = await _audit_cells_and_counters(buck)
    assert module_name in output
    assert second["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_root_module_bazel_edit_invalidates_bzlmod_graph(buck: Buck) -> None:
    """Bazel anchors: ModuleFileFunction.java and BazelModuleResolutionFunction.java."""
    before = await _bzlmod_counters(buck)
    output, first = await _audit_cells_and_counters(buck)
    assert "plan61_guardrails" in output
    assert first["module_file_parse"] > before["module_file_parse"]
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    output, warm = await _audit_cells_and_counters(buck)
    assert "plan61_guardrails" in output
    assert warm["module_file_parse"] == first["module_file_parse"]
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_guardrails_edited")\n""",
    )

    output, second = await _audit_cells_and_counters(buck)
    assert "plan61_guardrails_edited" in output
    assert second["module_file_parse"] > warm["module_file_parse"]
    assert second["bzlmod_resolution_compute"] > warm["bzlmod_resolution_compute"]


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

    output, warm = await _audit_cells_and_counters(buck)
    assert "local_lib" in output
    assert warm["module_file_parse"] == first["module_file_parse"]
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write(
        buck.cwd / "libs/local_lib/MODULE.bazel",
        """module(name = "local_lib", version = "1.1")\n""",
    )

    output, second = await _audit_cells_and_counters(buck)
    assert "local_lib" in output
    assert second["module_file_parse"] > warm["module_file_parse"]
    assert second["bzlmod_resolution_compute"] > warm["bzlmod_resolution_compute"]


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

    output, warm = await _audit_cells_and_counters(buck)
    assert "plan61_guardrails" in output
    assert "included_lib" not in output
    assert warm["module_file_parse"] == first["module_file_parse"]
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

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
    assert second["module_file_parse"] > warm["module_file_parse"]
    assert second["bzlmod_resolution_compute"] > warm["bzlmod_resolution_compute"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_root_module_parse_failure_invalidates_bzlmod_graph(
    buck: Buck,
) -> None:
    """Bazel anchor: root MODULE.bazel parse errors are module-resolution inputs."""
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_root_parse_failure")
""",
    )

    output, first = await _audit_cells_and_counters(buck)
    assert "plan61_root_parse_failure" in output

    output, warm = await _audit_cells_and_counters(buck)
    assert "plan61_root_parse_failure" in output
    assert warm["module_file_parse"] == first["module_file_parse"]
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write(buck.cwd / "MODULE.bazel", 'module(name = "plan61_root_parse_failure", version = )\n')
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell")
    assert "Failed to parse root MODULE.bazel" in exc.value.stderr

    repaired_lib = buck.cwd / "libs/repaired_root_parse_lib"
    repaired_lib.mkdir(parents=True)
    _write(
        repaired_lib / "MODULE.bazel",
        'module(name = "repaired_root_parse_lib", version = "1.0")\n',
    )
    _write(repaired_lib / "BUILD.bazel", 'filegroup(name = "ok", srcs = [])\n')
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_root_parse_failure")
bazel_dep(name = "repaired_root_parse_lib")
local_path_override(
    module_name = "repaired_root_parse_lib",
    path = "libs/repaired_root_parse_lib",
)
""",
    )
    output, _recovered = await _audit_cells_and_counters(buck)
    assert "plan61_root_parse_failure" in output
    assert "repaired_root_parse_lib" in output


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_root_module_utf8_failure_invalidates_bzlmod_graph(
    buck: Buck,
) -> None:
    """Bazel anchor: root MODULE.bazel UTF-8 errors are module-resolution inputs."""
    module_file = buck.cwd / "MODULE.bazel"
    _write(
        module_file,
        """module(name = "plan61_root_utf8_failure")
""",
    )

    output, first = await _audit_cells_and_counters(buck)
    assert "plan61_root_utf8_failure" in output

    output, warm = await _audit_cells_and_counters(buck)
    assert "plan61_root_utf8_failure" in output
    assert warm["module_file_parse"] == first["module_file_parse"]
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write_bytes(module_file, b"\xff\xfeinvalid root module\n")
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell")
    assert "Failed to parse root MODULE.bazel" in exc.value.stderr
    assert "valid UTF-8" in exc.value.stderr

    repaired_lib = buck.cwd / "libs/repaired_root_utf8_lib"
    repaired_lib.mkdir(parents=True)
    _write(
        repaired_lib / "MODULE.bazel",
        'module(name = "repaired_root_utf8_lib", version = "1.0")\n',
    )
    _write(repaired_lib / "BUILD.bazel", 'filegroup(name = "ok", srcs = [])\n')
    _write(
        module_file,
        """module(name = "plan61_root_utf8_failure")
bazel_dep(name = "repaired_root_utf8_lib")
local_path_override(
    module_name = "repaired_root_utf8_lib",
    path = "libs/repaired_root_utf8_lib",
)
""",
    )
    output, _recovered = await _audit_cells_and_counters(buck)
    assert "plan61_root_utf8_failure" in output
    assert "repaired_root_utf8_lib" in output


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_non_root_included_module_segment_edit_invalidates_extension_graph(
    buck: Buck,
) -> None:
    """Bazel anchor: non-root ModuleFileValue include inputs feed extension aggregation."""
    dep = buck.cwd / "libs/dep_with_included_extension"
    dep.mkdir(parents=True)
    _write(
        dep / "MODULE.bazel",
        """module(name = "dep_with_included_extension", version = "1.0")
include("//:ext.MODULE.bazel")
""",
    )
    _write(dep / "ext.MODULE.bazel", "# initially no extension usage\n")
    _write(
        dep / "dep_ext.bzl",
        """def _dep_ext_impl(module_ctx):
    pass

dep_ext = module_extension(
    implementation = _dep_ext_impl,
)
""",
    )
    _write(dep / "BUILD.bazel", 'filegroup(name = "dep", srcs = [])\n')
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_non_root_include")
bazel_dep(name = "dep_with_included_extension", version = "1.0")
local_path_override(
    module_name = "dep_with_included_extension",
    path = "libs/dep_with_included_extension",
)
""",
    )

    before = await _bzlmod_counters(buck)
    output, first = await _audit_cells_and_counters(buck)
    assert "dep_with_included_extension" in output
    assert "dep_generated_repo" not in output
    assert first["module_file_parse"] > before["module_file_parse"]
    assert first["bzlmod_resolution_compute"] > before["bzlmod_resolution_compute"]

    output, warm = await _audit_cells_and_counters(buck)
    assert "dep_generated_repo" not in output
    assert warm["module_file_parse"] == first["module_file_parse"]
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write(
        dep / "ext.MODULE.bazel",
        """dep = use_extension("//:dep_ext.bzl", "dep_ext")
use_repo(dep, "dep_generated_repo")
""",
    )

    output, second = await _audit_cells_and_counters(buck)
    assert "dep_generated_repo" in output
    assert second["module_file_parse"] > warm["module_file_parse"]
    assert second["bzlmod_resolution_compute"] > warm["bzlmod_resolution_compute"]


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
async def test_included_module_segment_parse_failure_invalidates_bzlmod_graph(
    buck: Buck,
) -> None:
    """Bazel anchor: included MODULE.bazel segment parse errors are inputs."""
    _write(buck.cwd / "deps.MODULE.bazel", "# initially empty included segment\n")
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_include_parse_failure")
include("//:deps.MODULE.bazel")
""",
    )

    output, first = await _audit_cells_and_counters(buck)
    assert "plan61_include_parse_failure" in output

    output, warm = await _audit_cells_and_counters(buck)
    assert "plan61_include_parse_failure" in output
    assert warm["module_file_parse"] == first["module_file_parse"]
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write(buck.cwd / "deps.MODULE.bazel", "bazel_dep(name = )\n")
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell")
    failure_stderr = exc.value.stderr
    assert "deps.MODULE.bazel" in failure_stderr

    repaired_lib = buck.cwd / "libs/repaired_include_parse_lib"
    repaired_lib.mkdir(parents=True)
    _write(
        repaired_lib / "MODULE.bazel",
        'module(name = "repaired_include_parse_lib", version = "1.0")\n',
    )
    _write(repaired_lib / "BUILD.bazel", 'filegroup(name = "ok", srcs = [])\n')
    _write(
        buck.cwd / "deps.MODULE.bazel",
        """bazel_dep(name = "repaired_include_parse_lib")
local_path_override(
    module_name = "repaired_include_parse_lib",
    path = "libs/repaired_include_parse_lib",
)
""",
    )
    output, _recovered = await _audit_cells_and_counters(buck)
    assert "plan61_include_parse_failure" in output
    assert "repaired_include_parse_lib" in output


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_included_module_segment_utf8_failure_invalidates_bzlmod_graph(
    buck: Buck,
) -> None:
    """Bazel anchor: included MODULE.bazel segment UTF-8 errors are inputs."""
    included = buck.cwd / "deps.MODULE.bazel"
    _write(included, "# initially empty included segment\n")
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_include_utf8_failure")
include("//:deps.MODULE.bazel")
""",
    )

    output, first = await _audit_cells_and_counters(buck)
    assert "plan61_include_utf8_failure" in output

    output, warm = await _audit_cells_and_counters(buck)
    assert "plan61_include_utf8_failure" in output
    assert warm["module_file_parse"] == first["module_file_parse"]
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write_bytes(included, b"\xff\xfeinvalid included segment\n")
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell")
    failure_stderr = exc.value.stderr
    assert "deps.MODULE.bazel" in failure_stderr
    assert "not UTF-8" in failure_stderr

    repaired_lib = buck.cwd / "libs/repaired_include_utf8_lib"
    repaired_lib.mkdir(parents=True)
    _write(
        repaired_lib / "MODULE.bazel",
        'module(name = "repaired_include_utf8_lib", version = "1.0")\n',
    )
    _write(repaired_lib / "BUILD.bazel", 'filegroup(name = "ok", srcs = [])\n')
    _write(
        included,
        """bazel_dep(name = "repaired_include_utf8_lib")
local_path_override(
    module_name = "repaired_include_utf8_lib",
    path = "libs/repaired_include_utf8_lib",
)
""",
    )
    output, _recovered = await _audit_cells_and_counters(buck)
    assert "plan61_include_utf8_failure" in output
    assert "repaired_include_utf8_lib" in output


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_included_module_segment_include_cycle_invalidates_bzlmod_graph(
    buck: Buck,
) -> None:
    """Bazel anchor: included MODULE.bazel segment include cycles are inputs."""
    included = buck.cwd / "deps.MODULE.bazel"
    _write(included, "# initially empty included segment\n")
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_include_cycle_failure")
include("//:deps.MODULE.bazel")
""",
    )

    output, first = await _audit_cells_and_counters(buck)
    assert "plan61_include_cycle_failure" in output

    output, warm = await _audit_cells_and_counters(buck)
    assert "plan61_include_cycle_failure" in output
    assert warm["module_file_parse"] == first["module_file_parse"]
    assert warm["bzlmod_resolution_compute"] == first["bzlmod_resolution_compute"]

    _write(included, 'include("//:deps.MODULE.bazel")\n')
    with pytest.raises(BuckException) as exc:
        await buck.audit("cell")
    failure_stderr = exc.value.stderr
    assert "cyclic include" in failure_stderr

    repaired_lib = buck.cwd / "libs/repaired_include_cycle_lib"
    repaired_lib.mkdir(parents=True)
    _write(
        repaired_lib / "MODULE.bazel",
        'module(name = "repaired_include_cycle_lib", version = "1.0")\n',
    )
    _write(repaired_lib / "BUILD.bazel", 'filegroup(name = "ok", srcs = [])\n')
    _write(
        included,
        """bazel_dep(name = "repaired_include_cycle_lib")
local_path_override(
    module_name = "repaired_include_cycle_lib",
    path = "libs/repaired_include_cycle_lib",
)
""",
    )
    output, _recovered = await _audit_cells_and_counters(buck)
    assert "plan61_include_cycle_failure" in output
    assert "repaired_include_cycle_lib" in output


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_bazel_compatibility_incompatible_version_fails(
    buck: Buck,
) -> None:
    """Bazel anchor: incompatible module(bazel_compatibility) fails resolution."""
    _write(
        buck.cwd / "MODULE.bazel",
        """module(
    name = "plan61_bazel_compatibility",
    version = "1.0",
    bazel_compatibility = [">=99.0.0"],
)
""",
    )
    _write(buck.cwd / "BUILD.bazel", 'filegroup(name = "x")\n')

    with pytest.raises(BuckException) as exc:
        await buck.build("//:x")

    assert "Bazel version 9.0.1 is not compatible" in str(exc.value)
    assert "bazel_compatibility: [>=99.0.0]" in str(exc.value)


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_max_compatibility_level_is_bazel9_noop(
    buck: Buck,
) -> None:
    """Bazel anchor: bazel_dep(max_compatibility_level) is warning-only no-op."""
    dep = buck.cwd / "dep"
    dep.mkdir()
    _write(dep / "MODULE.bazel", 'module(name = "dep", version = "1.0")\n')
    _write(
        dep / "BUILD.bazel",
        'filegroup(name = "x", visibility = ["//visibility:public"])\n',
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_max_compatibility")
bazel_dep(name = "dep", version = "1.0", max_compatibility_level = 1)
local_path_override(module_name = "dep", path = "dep")
""",
    )
    _write(buck.cwd / "BUILD.bazel", 'filegroup(name = "x", srcs = ["@dep//:x"])\n')

    await buck.build("//:x")


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_isolated_extension_usage_fails_until_supported(
    buck: Buck,
) -> None:
    """Bazel anchor: isolate=True requires experimental isolated-extension semantics."""
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_isolate_unsupported")
ext = use_extension("//:ext.bzl", "ext", isolate = True)
use_repo(ext, "generated")
""",
    )
    _write(
        buck.cwd / "ext.bzl",
        """def _ext_impl(module_ctx):
    pass

ext = module_extension(implementation = _ext_impl)
""",
    )
    _write(buck.cwd / "BUILD.bazel", 'filegroup(name = "x")\n')

    with pytest.raises(BuckException) as exc:
        await buck.build("//:x")

    assert "use_extension(isolate = True)" in str(exc.value)
    assert "experimental_isolated_extension_usages" in str(exc.value)


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_single_version_override_patches_fail_until_supported(
    buck: Buck,
) -> None:
    """Bazel anchor: SVO patches affect both MODULE.bazel and final RepoSpec."""
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_svo_patches_unsupported")
bazel_dep(name = "dep", version = "1.0.0")
single_version_override(
    module_name = "dep",
    patches = ["//:fix.patch"],
)
""",
    )
    _write(buck.cwd / "BUILD.bazel", 'filegroup(name = "x")\n')
    _write(buck.cwd / "fix.patch", "")

    with pytest.raises(BuckException) as exc:
        await buck.build("//:x")

    assert "single_version_override(patches = ...)" in str(exc.value)
    assert "MODULE.bazel discovery" in str(exc.value)
    assert "repository materialization" in str(exc.value)


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_archive_override_patches_fail_until_supported(
    buck: Buck,
) -> None:
    """Bazel anchor: archive_override patches affect final RepoSpec materialization."""
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_archive_patches_unsupported")
bazel_dep(name = "dep", version = "1.0.0")
archive_override(
    module_name = "dep",
    urls = ["file:///does/not/matter.tar.gz"],
    patches = ["//:fix.patch"],
)
""",
    )
    _write(buck.cwd / "BUILD.bazel", 'filegroup(name = "x")\n')
    _write(buck.cwd / "fix.patch", "")

    with pytest.raises(BuckException) as exc:
        await buck.build("//:x")

    assert "archive_override(patches = ...)" in str(exc.value)
    assert "MODULE.bazel discovery" in str(exc.value)
    assert "repository materialization" in str(exc.value)


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_git_override_patches_fail_until_supported(
    buck: Buck,
) -> None:
    """Bazel anchor: git_override patches affect final RepoSpec materialization."""
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_git_patches_unsupported")
bazel_dep(name = "dep", version = "1.0.0")
git_override(
    module_name = "dep",
    remote = "file:///does/not/matter.git",
    commit = "0000000000000000000000000000000000000000",
    patches = ["//:fix.patch"],
)
""",
    )
    _write(buck.cwd / "BUILD.bazel", 'filegroup(name = "x")\n')
    _write(buck.cwd / "fix.patch", "")

    with pytest.raises(BuckException) as exc:
        await buck.build("//:x")

    assert "git_override(patches = ...)" in str(exc.value)
    assert "MODULE.bazel discovery" in str(exc.value)
    assert "repository materialization" in str(exc.value)


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
async def test_root_bazel_dep_dev_dependency_is_available_by_default(
    buck: Buck,
) -> None:
    """Bazel anchor: --ignore_dev_dependency defaults false for root MODULE.bazel."""
    dev_lib = buck.cwd / "libs/root_dev_dep_lib"
    dev_lib.mkdir(parents=True)
    _write(
        dev_lib / "MODULE.bazel",
        """module(name = "root_dev_dep_lib", version = "1.0")
""",
    )
    _write(
        dev_lib / "BUILD.bazel",
        """filegroup(
    name = "lib",
    srcs = [],
    visibility = ["//visibility:public"],
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_guardrails")
bazel_dep(name = "root_dev_dep_lib", version = "1.0", dev_dependency = True)
local_path_override(
    module_name = "root_dev_dep_lib",
    path = "libs/root_dev_dep_lib",
)
""",
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_root_dev_dep",
    srcs = ["@root_dev_dep_lib//:lib"],
)
""",
    )

    await buck.build("//:uses_root_dev_dep")
    with pytest.raises(BuckException) as exc:
        await buck.build("//:uses_root_dev_dep", "--ignore_dev_dependency")
    assert "root_dev_dep_lib" in str(exc.value)


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_root_registry_bazel_dep_dev_dependency_is_resolved_by_default(
    buck: Buck,
) -> None:
    """Bazel anchor: root bazel_dep(dev_dependency=True) participates by default."""
    module_name = "root_registry_dev_dep_lib"
    module_version = "1.0.0"
    cache_home = buck.cwd / "cache_home"
    module_cache = (
        cache_home
        / "slug"
        / "registry"
        / "bcr.bazel.build"
        / "modules"
        / module_name
        / module_version
    )
    registry_cache = cache_home / "slug" / "registry" / "bcr.bazel.build"
    source_dir = module_cache / "source"
    source_dir.mkdir(parents=True)
    _write(registry_cache / "bazel_registry.json", "{}\n")
    _write(
        module_cache / "MODULE.bazel",
        f'module(name = "{module_name}", version = "{module_version}")\n',
    )
    _write(module_cache / "source.json", "{}\n")
    _write(source_dir / ".complete", "")
    _write(
        source_dir / "BUILD.bazel",
        """filegroup(
    name = "lib",
    srcs = [],
    visibility = ["//visibility:public"],
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "plan61_guardrails")
bazel_dep(name = "{module_name}", version = "{module_version}", dev_dependency = True)
""",
    )
    _write(
        buck.cwd / "BUILD.bazel",
        f"""filegroup(
    name = "uses_root_registry_dev_dep",
    srcs = ["@{module_name}//:lib"],
)
""",
    )

    module_url = (
        f"https://bcr.bazel.build/modules/{module_name}/{module_version}/MODULE.bazel"
    )
    source_url = (
        f"https://bcr.bazel.build/modules/{module_name}/{module_version}/source.json"
    )
    registry_url = "https://bcr.bazel.build/bazel_registry.json"
    _write(
        buck.cwd / "MODULE.bazel.lock",
        json.dumps(
            {
                "lockFileVersion": 26,
                "registryFileHashes": {
                    registry_url: _sha256(registry_cache / "bazel_registry.json"),
                    module_url: _sha256(module_cache / "MODULE.bazel"),
                    source_url: _sha256(module_cache / "source.json"),
                },
                "selectedYankedVersions": {},
                "moduleExtensions": {},
                "facts": {},
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
    )

    await buck.build("//:uses_root_registry_dev_dep", env={"XDG_CACHE_HOME": str(cache_home)})
    with pytest.raises(BuckException) as exc:
        await buck.build(
            "//:uses_root_registry_dev_dep",
            "--ignore_dev_dependency",
            env={"XDG_CACHE_HOME": str(cache_home)},
        )
    assert module_name in str(exc.value)


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_deferred_toolchain_retry_recomputes_target_settings(
    buck: Buck,
) -> None:
    """Regression: deferred toolchain retry must recompute target_settings."""
    dep = buck.cwd / "deferred_toolchain_dep"
    dep.mkdir()
    _write(
        dep / "MODULE.bazel",
        """module(name = "deferred_toolchain_dep", version = "1.0")
register_toolchains("@deferred_toolchain_dep//:tc")
""",
    )
    _write(
        dep / "defs.bzl",
        """def _tc_impl(ctx):
    return [platform_common.ToolchainInfo(message = "retry-target-settings-ok")]

tc_impl = rule(implementation = _tc_impl)
""",
    )
    _write(
        dep / "BUILD.bazel",
        """load(":defs.bzl", "tc_impl")

toolchain_type(name = "type")

config_setting(
    name = "setting",
    values = {"compilation_mode": "fastbuild"},
)

tc_impl(name = "impl")

toolchain(
    name = "tc",
    toolchain_type = ":type",
    toolchain = ":impl",
    target_settings = [":setting"],
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_toolchain_retry")

bazel_dep(name = "deferred_toolchain_dep", version = "1.0")
local_path_override(
    module_name = "deferred_toolchain_dep",
    path = "deferred_toolchain_dep",
)
""",
    )
    _write(
        buck.cwd / "defs.bzl",
        """def _consumer_impl(ctx):
    toolchain = ctx.toolchains["@deferred_toolchain_dep//:type"]
    out = ctx.actions.declare_file("retry_target_settings.txt")
    ctx.actions.write(out, toolchain.message + "\\n")
    return [DefaultInfo(files = depset([out]))]

consumer = rule(
    implementation = _consumer_impl,
    toolchains = ["@deferred_toolchain_dep//:type"],
)
""",
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """load(":defs.bzl", "consumer")

consumer(name = "uses_deferred_toolchain")
""",
    )

    result = await buck.build("//:uses_deferred_toolchain")
    output = result.get_build_report().output_for_target("//:uses_deferred_toolchain")
    assert output.read_text().strip() == "retry-target-settings-ok"


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_root_use_repo_rule_dev_dependency_follows_ignore_policy(
    buck: Buck,
) -> None:
    """Bazel anchor: use_repo_rule(dev_dependency=True) is root-only unless ignored."""
    dev_repo = buck.cwd / "repo_rule_dev_repo"
    dev_repo.mkdir()
    _write(dev_repo / ".buckroot", "")
    _write(
        dev_repo / "BUILD.bazel",
        """filegroup(
    name = "lib",
    srcs = [],
    visibility = ["//visibility:public"],
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_guardrails")
repo = use_repo_rule("@@bazel_tools//tools/build_defs/repo:local.bzl", "local_repository", dev_dependency = True)
repo(name = "repo_rule_dev_repo", path = "repo_rule_dev_repo")
""",
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_repo_rule_dev_dep",
    srcs = ["@repo_rule_dev_repo//:lib"],
)
""",
    )

    await buck.build("//:uses_repo_rule_dev_dep")
    with pytest.raises(BuckException) as exc:
        await buck.build("//:uses_repo_rule_dev_dep", "--ignore_dev_dependency")
    assert "repo_rule_dev_repo" in str(exc.value)


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_non_root_use_repo_rule_dev_dependency_is_always_ignored(
    buck: Buck,
) -> None:
    """Bazel anchor: non-root use_repo_rule(dev_dependency=True) is ignored."""
    dep = buck.cwd / "libs/dep_with_dev_repo_rule"
    dev_repo = buck.cwd / "repo_rule_non_root_dev_repo"
    dep.mkdir(parents=True)
    dev_repo.mkdir()
    _write(dev_repo / ".buckroot", "")
    _write(
        dev_repo / "BUILD.bazel",
        """filegroup(
    name = "lib",
    srcs = [],
    visibility = ["//visibility:public"],
)
""",
    )
    _write(
        dep / "MODULE.bazel",
        """module(name = "dep_with_dev_repo_rule", version = "1.0")
repo = use_repo_rule("@@bazel_tools//tools/build_defs/repo:local.bzl", "local_repository", dev_dependency = True)
repo(name = "repo_rule_non_root_dev_repo", path = "../../repo_rule_non_root_dev_repo")
""",
    )
    _write(dep / "BUILD.bazel", 'filegroup(name = "dep", srcs = [])\n')
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_guardrails")
bazel_dep(name = "dep_with_dev_repo_rule", version = "1.0")
local_path_override(
    module_name = "dep_with_dev_repo_rule",
    path = "libs/dep_with_dev_repo_rule",
)
""",
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_non_root_repo_rule_dev_dep",
    srcs = ["@repo_rule_non_root_dev_repo//:lib"],
)
""",
    )

    with pytest.raises(BuckException) as exc:
        await buck.build("//:uses_non_root_repo_rule_dev_dep")
    assert "repo_rule_non_root_dev_repo" in str(exc.value)
    with pytest.raises(BuckException) as exc:
        await buck.build(
            "//:uses_non_root_repo_rule_dev_dep",
            "--ignore_dev_dependency",
        )
    assert "repo_rule_non_root_dev_repo" in str(exc.value)


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_custom_use_repo_rule_local_definition_reexecutes_after_input_edit(
    buck: Buck,
) -> None:
    """Bazel anchors: RepoDefinitionFunction loads RepoRule; RepositoryFetchFunction skips local cache."""
    repo_dir = buck.cwd / "bazel-external" / "+custom_local_repository+custom_local"
    payload = repo_dir / "payload.txt"
    _write(buck.cwd / "source.txt", "first\n")
    _write(buck.cwd / "helper.bzl", "LOCAL_REPOSITORY_RULE = True\n")
    _write(
        buck.cwd / "repo.bzl",
        """load("//:helper.bzl", "LOCAL_REPOSITORY_RULE")

def _custom_local_impl(repository_ctx):
    payload = repository_ctx.read(Label("//:source.txt"), watch = "no")
    repository_ctx.file("payload.txt", payload)
    repository_ctx.file("BUILD.bazel", "exports_files([\\"payload.txt\\"])\\nfilegroup(name = \\"payload\\", srcs = [\\"payload.txt\\"])\\n")

custom_local_repository = repository_rule(
    implementation = _custom_local_impl,
    local = LOCAL_REPOSITORY_RULE,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_custom_use_repo_rule_local")

repo = use_repo_rule("//:repo.bzl", "custom_local_repository")
repo(name = "custom_local")
""",
    )
    _write(buck.cwd / "BUILD.bazel", 'filegroup(name = "root")\n')

    await buck.build("@custom_local//:payload")
    assert payload.read_text() == "first\n"

    _write(buck.cwd / "source.txt", "second\n")
    await buck.build("@custom_local//:payload")
    assert payload.read_text() == "second\n"


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_custom_use_repo_rule_local_probe_failure_does_not_block_execution(
    buck: Buck,
) -> None:
    """Bazel anchor: custom repository_rule modules load through the normal bzl loader."""
    repo_dir = buck.cwd / "bazel-external" / "+custom_local_repository+custom_local"
    payload = repo_dir / "payload.txt"
    _write(buck.cwd / "source.txt", "first\n")
    _write(
        buck.cwd / "repo.bzl",
        """def _unused_rule_impl(ctx):
    return []

unused_rule = rule(
    implementation = _unused_rule_impl,
    attrs = {"tool": attr.label(default = "//:tool")},
)

def _custom_local_impl(repository_ctx):
    payload = repository_ctx.read(Label("//:source.txt"), watch = "no")
    repository_ctx.file("payload.txt", payload)
    repository_ctx.file("BUILD.bazel", "exports_files([\\"payload.txt\\"])\\nfilegroup(name = \\"payload\\", srcs = [\\"payload.txt\\"])\\n")

custom_local_repository = repository_rule(
    implementation = _custom_local_impl,
    local = True,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_custom_use_repo_rule_probe_failure")

repo = use_repo_rule("//:repo.bzl", "custom_local_repository")
repo(name = "custom_local")
""",
    )
    _write(buck.cwd / "BUILD.bazel", 'filegroup(name = "tool")\n')

    await buck.build("@custom_local//:payload")
    assert payload.read_text() == "first\n"

    _write(buck.cwd / "source.txt", "second\n")
    await buck.build("@custom_local//:payload")
    assert payload.read_text() == "second\n"


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_external_use_repo_rule_local_definition_reexecutes_after_input_edit(
    buck: Buck,
) -> None:
    """Bazel anchor: external use_repo_rule() local bits load after cell graph install."""
    owner = buck.cwd / "owner"
    owner.mkdir()
    _write(owner / "MODULE.bazel", 'module(name = "repo_rule_owner", version = "1.0")\n')
    _write(
        owner / "repo.bzl",
        """def _external_local_impl(repository_ctx):
    payload = repository_ctx.read(Label("@@//:source.txt"), watch = "no")
    repository_ctx.file("payload.txt", payload)
    repository_ctx.file("BUILD.bazel", "exports_files([\\"payload.txt\\"])\\nfilegroup(name = \\"payload\\", srcs = [\\"payload.txt\\"])\\n")

external_local_repository = repository_rule(
    implementation = _external_local_impl,
    local = True,
)
""",
    )
    repo_dir = buck.cwd / "bazel-external" / "+external_local_repository+external_local"
    payload = repo_dir / "payload.txt"
    _write(buck.cwd / "source.txt", "first\n")
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_external_use_repo_rule_local")

bazel_dep(name = "repo_rule_owner", version = "1.0")
local_path_override(module_name = "repo_rule_owner", path = "owner")

repo = use_repo_rule("@repo_rule_owner//:repo.bzl", "external_local_repository")
repo(name = "external_local")
""",
    )

    await buck.build("@external_local//:payload")
    assert payload.read_text() == "first\n"

    _write(buck.cwd / "source.txt", "second\n")
    await buck.build("@external_local//:payload")
    assert payload.read_text() == "second\n"


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_root_use_extension_dev_dependency_follows_ignore_policy(
    buck: Buck,
) -> None:
    """Bazel anchor: use_extension(dev_dependency=True) is root-only unless ignored."""
    module_name = "plan61_root_dev_extension"
    extension_id = "@plan61_root_dev_extension//:dev_ext.bzl%dev_ext"
    dev_repo = buck.cwd / "dev_extension_repo"
    dev_repo.mkdir()
    _write(dev_repo / "data.txt", "dev extension payload\n")
    _write(
        dev_repo / "BUILD.bazel",
        """exports_files(["data.txt"])
filegroup(name = "data", srcs = ["data.txt"], visibility = ["//visibility:public"])
""",
    )
    _write(
        buck.cwd / "dev_ext.bzl",
        """def _dev_ext_impl(module_ctx):
    fail("dev extension should replay from the lockfile")

dev_ext = module_extension(
    implementation = _dev_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "{module_name}")

dev = use_extension("//:dev_ext.bzl", "dev_ext", dev_dependency = True)
use_repo(dev, "dev_extension_repo")
""",
    )
    _write_replay_lockfile(
        buck.cwd / "MODULE.bazel.lock",
        extension_id=extension_id,
        module_name=module_name,
        project_root=buck.cwd,
        repo_path=dev_repo,
        repo_paths={"dev_extension_repo": dev_repo},
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_dev_extension_repo",
    srcs = ["@dev_extension_repo//:data"],
)
""",
    )

    await buck.build("//:uses_dev_extension_repo")
    with pytest.raises(BuckException) as exc:
        await buck.build("//:uses_dev_extension_repo", "--ignore_dev_dependency")
    assert "dev_extension_repo" in str(exc.value)


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_non_root_use_extension_dev_dependency_is_always_ignored(
    buck: Buck,
) -> None:
    """Bazel anchor: non-root use_extension(dev_dependency=True) is ignored."""
    dep = buck.cwd / "libs/dep_with_dev_extension"
    dep.mkdir(parents=True)
    dev_repo = buck.cwd / "non_root_dev_extension_repo"
    dev_repo.mkdir()
    _write(dev_repo / "data.txt", "non-root dev extension payload\n")
    _write(
        dev_repo / "BUILD.bazel",
        """exports_files(["data.txt"])
filegroup(name = "data", srcs = ["data.txt"], visibility = ["//visibility:public"])
""",
    )
    _write(
        dep / "dev_ext.bzl",
        """def _dev_ext_impl(module_ctx):
    fail("non-root dev extension should be ignored")

dev_ext = module_extension(
    implementation = _dev_ext_impl,
)
""",
    )
    _write(
        dep / "MODULE.bazel",
        """module(name = "dep_with_dev_extension", version = "1.0")
dev = use_extension("//:dev_ext.bzl", "dev_ext", dev_dependency = True)
use_repo(dev, "non_root_dev_extension_repo")
""",
    )
    _write(dep / "BUILD.bazel", 'filegroup(name = "dep", srcs = [])\n')
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_guardrails")
bazel_dep(name = "dep_with_dev_extension", version = "1.0")
local_path_override(
    module_name = "dep_with_dev_extension",
    path = "libs/dep_with_dev_extension",
)
""",
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_non_root_dev_extension_repo",
    srcs = ["@non_root_dev_extension_repo//:data"],
)
""",
    )

    with pytest.raises(BuckException) as exc:
        await buck.build("//:uses_non_root_dev_extension_repo")
    assert "non_root_dev_extension_repo" in str(exc.value)
    with pytest.raises(BuckException) as exc:
        await buck.build(
            "//:uses_non_root_dev_extension_repo",
            "--ignore_dev_dependency",
        )
    assert "non_root_dev_extension_repo" in str(exc.value)


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
    first = await _bzlmod_counters(buck, "--lockfile_mode=off")

    assert first["lockfile_read"] > before["lockfile_read"]

    await buck.audit("cell")
    warm = await _bzlmod_counters(buck, "--lockfile_mode=off")

    assert warm["lockfile_read"] == first["lockfile_read"]


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
async def test_hidden_lockfile_edit_invalidates_replay_in_same_daemon(
    buck: Buck,
) -> None:
    """Bazel anchor: HIDDEN_KEY is an input to SingleExtensionEvalValue."""
    module_name = "plan61_hidden_replay_materialization"
    extension_id = f"@{module_name}//:replay_ext.bzl%replay_ext"
    replayed_repo = buck.cwd / "replayed_repo"
    replayed_repo.mkdir()
    _write(replayed_repo / "data.txt", "hidden replay payload\n")
    _write(
        replayed_repo / "BUILD.bazel",
        """exports_files(["data.txt"])
filegroup(name = "data", srcs = ["data.txt"])
""",
    )
    _write(
        buck.cwd / "replay_ext.bzl",
        """def _replay_ext_impl(module_ctx):
    fail("hidden lockfile replay should have been used")

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
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_hidden_replay",
    srcs = ["@replayed_repo//:data"],
)
""",
    )

    daemon_dir = Path((await buck.debug("daemon-dir")).stdout.strip())
    hidden_lockfile = daemon_dir / "MODULE.bazel.lock"
    # `debug daemon-dir` starts the daemon before the hidden lockfile exists.
    # Switch modes so the replay command must reload default-mode bzlmod state.
    await _bzlmod_counters(buck, "--lockfile_mode=off")
    hidden_lockfile.parent.mkdir(parents=True, exist_ok=True)
    _write_replay_lockfile(
        hidden_lockfile,
        extension_id=extension_id,
        module_name=module_name,
        project_root=buck.cwd,
        repo_path=replayed_repo,
    )

    before = await _bzlmod_counters(buck)
    await buck.build("//:uses_hidden_replay")
    first = await _bzlmod_counters(buck)
    assert first["extension_replay_hit"] > before["extension_replay_hit"]
    assert first["extension_eval"] == before["extension_eval"]

    _write_minimal_lockfile(hidden_lockfile)

    failure_stderr: str | None = None
    try:
        await buck.build("//:uses_hidden_replay")
    except BuckException as e:
        failure_stderr = e.stderr
    after = await _bzlmod_counters(buck)

    if failure_stderr is None:
        pytest.fail("hidden lockfile replay stayed cached after hidden lockfile edit")

    assert "hidden lockfile replay should have been used" in failure_stderr
    assert after["extension_eval"] > first["extension_eval"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_hidden_lockfile_facts_create_edit_delete_are_observed(
    buck: Buck,
) -> None:
    """Bazel anchor: hidden lockfile facts feed module_ctx.facts before eval."""
    module_name = "plan61_hidden_facts"
    extension_id = f"@{module_name}//:hidden_facts_ext.bzl%hidden_facts_ext"
    _write(
        buck.cwd / "hidden_facts_ext.bzl",
        """def _repo_impl(repository_ctx):
    repository_ctx.file("data.txt", "hidden facts payload\\n")
    repository_ctx.file("BUILD.bazel", "exports_files([\\"data.txt\\"])\\nfilegroup(name = \\"data\\", srcs = [\\"data.txt\\"])\\n")

hidden_facts_repo_rule = repository_rule(
    implementation = _repo_impl,
)

def _hidden_facts_ext_impl(module_ctx):
    resource = module_ctx.facts.get("resource")
    if resource != "ok":
        fail("hidden facts missing or stale: %s" % resource)
    hidden_facts_repo_rule(name = "hidden_facts_repo")

hidden_facts_ext = module_extension(
    implementation = _hidden_facts_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "{module_name}")

hidden_facts = use_extension("//:hidden_facts_ext.bzl", "hidden_facts_ext")
use_repo(hidden_facts, "hidden_facts_repo")
""",
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_hidden_facts",
    srcs = ["@hidden_facts_repo//:data"],
)
""",
    )

    daemon_dir = Path((await buck.debug("daemon-dir")).stdout.strip())
    hidden_lockfile = daemon_dir / "MODULE.bazel.lock"
    await _bzlmod_counters(buck, "--lockfile_mode=off")
    hidden_lockfile.parent.mkdir(parents=True, exist_ok=True)

    with pytest.raises(BuckException) as absent_failure:
        await buck.build("//:uses_hidden_facts")
    assert "hidden facts missing or stale" in absent_failure.value.stderr

    _write_minimal_lockfile_with_facts(
        hidden_lockfile,
        extension_id=extension_id,
        facts={"resource": "ok"},
    )
    before = await _bzlmod_counters(buck)
    await buck.build("//:uses_hidden_facts")
    first = await _bzlmod_counters(buck)
    assert first["extension_eval"] > before["extension_eval"]

    _write_minimal_lockfile_with_facts(
        hidden_lockfile,
        extension_id=extension_id,
        facts={"resource": "stale"},
    )
    with pytest.raises(BuckException) as edited_failure:
        await buck.build("//:uses_hidden_facts")
    edited = await _bzlmod_counters(buck)
    assert "hidden facts missing or stale: stale" in edited_failure.value.stderr
    assert edited["extension_eval"] > first["extension_eval"]

    _write_minimal_lockfile_with_facts(
        hidden_lockfile,
        extension_id=extension_id,
        facts={"resource": "ok"},
    )
    await buck.build("//:uses_hidden_facts")
    restored = await _bzlmod_counters(buck)
    assert restored["extension_eval"] > edited["extension_eval"]

    hidden_lockfile.unlink()
    with pytest.raises(BuckException) as deleted_failure:
        await buck.build("//:uses_hidden_facts")
    deleted = await _bzlmod_counters(buck)
    assert "hidden facts missing or stale" in deleted_failure.value.stderr
    assert deleted["extension_eval"] > restored["extension_eval"]


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
async def test_visible_lockfile_edit_is_observed_in_same_daemon(
    buck: Buck,
) -> None:
    """Bazel anchor: BazelLockFileValue.KEY is a graph input to resolution."""
    lockfile = buck.cwd / "MODULE.bazel.lock"
    _write_minimal_lockfile(lockfile)

    before = await _bzlmod_counters(buck, "--lockfile_mode=error")
    await buck.audit("cell", "--lockfile_mode=error")
    first = await _bzlmod_counters(buck, "--lockfile_mode=error")
    assert first["lockfile_read"] > before["lockfile_read"]

    await buck.audit("cell", "--lockfile_mode=error")
    warm = await _bzlmod_counters(buck, "--lockfile_mode=error")
    assert warm["lockfile_read"] == first["lockfile_read"]

    _write(lockfile, "{ this is not json }\n")
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
async def test_missing_transitive_extension_bzl_load_creation_rejects_replay(
    buck: Buck,
) -> None:
    """Bazel anchor: SingleExtensionEvalFunction uses the loaded .bzl graph digest."""
    module_name = "plan61_missing_transitive_load"
    extension_id = "@plan61_missing_transitive_load//:replay_ext.bzl%replay_ext"
    replayed_repo = buck.cwd / "replayed_repo"
    replayed_repo.mkdir(exist_ok=True)
    _write(replayed_repo / "BUILD.bazel", "filegroup(name = \"data\")\n")
    _write(
        buck.cwd / "replay_ext.bzl",
        """load("//tools:helper.bzl", "HELPER_SENTINEL")

def _replay_ext_impl(module_ctx):
    fail("missing helper creation should make replay stale: %s" % HELPER_SENTINEL)

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
    first = await _bzlmod_counters(buck)
    assert first["extension_replay_hit"] > before["extension_replay_hit"]
    assert first["extension_eval"] == before["extension_eval"]

    helper_dir = buck.cwd / "tools"
    helper_dir.mkdir(exist_ok=True)
    _write(helper_dir / "helper.bzl", 'HELPER_SENTINEL = "created helper"\n')

    with pytest.raises(BuckException) as exc:
        await buck.build("//:uses_replayed_repo")
    second = await _bzlmod_counters(buck)

    assert "missing helper creation should make replay stale: created helper" in str(exc.value)
    assert second["extension_replay_miss_reason"] > first["extension_replay_miss_reason"]
    assert second["extension_replay_hit"] == first["extension_replay_hit"]


def _write_mapped_external_extension_audit_cell_workspace(
    buck: Buck,
    *,
    suffix: str,
    helper_content: str | None,
    include_uncached_extension: bool = False,
) -> Path:
    owner_module = f"plan61_rules_owner_audit_{suffix}"
    helper_module = f"plan61_real_helper_audit_{suffix}"
    helper_alias = f"plan61_helper_alias_audit_{suffix}"
    root_module = f"plan61_mapped_load_audit_{suffix}"
    extension_id = f"@{owner_module}//:replay_ext.bzl%replay_ext"
    replayed_repo = buck.cwd / "replayed_repo"
    owner_dir = buck.cwd.parent / f"{buck.cwd.name}_rules_owner_audit_{suffix}"
    helper_dir = buck.cwd.parent / f"{buck.cwd.name}_real_helper_audit_{suffix}"
    external_dir = buck.cwd / "bazel-external"
    helper_path = helper_dir / "helper.bzl"

    replayed_repo.mkdir(exist_ok=True)
    owner_dir.mkdir(parents=True, exist_ok=True)
    helper_dir.mkdir(parents=True, exist_ok=True)
    external_dir.mkdir(exist_ok=True)

    _write(replayed_repo / "BUILD.bazel", "filegroup(name = \"data\")\n")
    _write(
        owner_dir / "MODULE.bazel",
        f"""module(name = "{owner_module}", version = "1.0")
bazel_dep(name = "{helper_module}", repo_name = "{helper_alias}")
""",
    )
    _write(
        owner_dir / "replay_ext.bzl",
        f"""load("@{helper_alias}//:helper.bzl", "HELPER_SENTINEL")

def _replay_ext_impl(module_ctx):
    fail("audit cell mapped helper change should make replay stale: %s" % HELPER_SENTINEL)

replay_ext = module_extension(
    implementation = _replay_ext_impl,
)
""",
    )
    _write(helper_dir / "MODULE.bazel", f'module(name = "{helper_module}", version = "1.0")\n')
    if helper_content is not None:
        _write(helper_path, helper_content)
    if include_uncached_extension:
        _write(
            buck.cwd / "uncached_ext.bzl",
            """def _uncached_ext_impl(module_ctx):
    pass

uncached_ext = module_extension(
    implementation = _uncached_ext_impl,
)
""",
        )

    (external_dir / f"{owner_module}+").symlink_to(owner_dir, target_is_directory=True)
    (external_dir / f"{helper_module}+").symlink_to(helper_dir, target_is_directory=True)

    root_module_file = f"""module(name = "{root_module}")
bazel_dep(name = "{owner_module}", version = "1.0")
bazel_dep(name = "{helper_module}", version = "1.0")
local_path_override(module_name = "{owner_module}", path = "{owner_dir.as_posix()}")
local_path_override(module_name = "{helper_module}", path = "{helper_dir.as_posix()}")

replay = use_extension("@{owner_module}//:replay_ext.bzl", "replay_ext")
use_repo(replay, "replayed_repo")
"""
    if include_uncached_extension:
        root_module_file += 'uncached = use_extension("//:uncached_ext.bzl", "uncached_ext")\n'
    _write(buck.cwd / "MODULE.bazel", root_module_file)
    _write_replay_lockfile(
        buck.cwd / "MODULE.bazel.lock",
        extension_id=extension_id,
        module_name=root_module,
        project_root=buck.cwd,
        repo_path=replayed_repo,
        repo_mappings={owner_module: {helper_alias: helper_module}},
    )

    return helper_path


async def _assert_mapped_external_audit_cell_replay_misses_after_change(
    buck: Buck,
    first: BzlmodCounters,
) -> BzlmodCounters:
    await buck.audit("cell")
    second = await _bzlmod_counters(buck)

    assert second["extension_replay_miss_reason"] > first["extension_replay_miss_reason"]
    assert second["extension_replay_hit"] == first["extension_replay_hit"]
    return second


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_mapped_external_extension_bzl_load_edit_rejects_audit_cell_replay(
    buck: Buck,
) -> None:
    """Bazel anchor: SingleExtensionEvalFunction loaded .bzl digest includes edits."""
    helper_path = _write_mapped_external_extension_audit_cell_workspace(
        buck,
        suffix="edit",
        helper_content='HELPER_SENTINEL = "initial mapped helper"\n',
    )

    before = await _bzlmod_counters(buck)
    await buck.audit("cell")
    first = await _bzlmod_counters(buck)
    assert first["extension_replay_hit"] > before["extension_replay_hit"]
    assert first["extension_eval"] == before["extension_eval"]

    _write(helper_path, 'HELPER_SENTINEL = "edited mapped helper"\n')

    await _assert_mapped_external_audit_cell_replay_misses_after_change(buck, first)


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_mapped_external_extension_bzl_load_edit_with_uncached_extension_rejects_audit_cell_replay(
    buck: Buck,
) -> None:
    """Bazel anchor: uncached extensions do not hide loaded .bzl digest inputs."""
    helper_path = _write_mapped_external_extension_audit_cell_workspace(
        buck,
        suffix="mixed",
        helper_content='HELPER_SENTINEL = "initial mapped helper"\n',
        include_uncached_extension=True,
    )

    before = await _bzlmod_counters(buck)
    await buck.audit("cell")
    first = await _bzlmod_counters(buck)
    assert first["extension_replay_hit"] > before["extension_replay_hit"]

    _write(helper_path, 'HELPER_SENTINEL = "edited mapped helper with uncached extension"\n')

    await _assert_mapped_external_audit_cell_replay_misses_after_change(buck, first)


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_missing_mapped_external_extension_bzl_load_creation_rejects_audit_cell_replay(
    buck: Buck,
) -> None:
    """Bazel anchor: SingleExtensionEvalFunction loaded .bzl digest includes creates."""
    helper_path = _write_mapped_external_extension_audit_cell_workspace(
        buck,
        suffix="create",
        helper_content=None,
    )

    before = await _bzlmod_counters(buck)
    await buck.audit("cell")
    first = await _bzlmod_counters(buck)
    assert first["extension_replay_hit"] > before["extension_replay_hit"]
    assert first["extension_eval"] == before["extension_eval"]

    _write(helper_path, 'HELPER_SENTINEL = "created mapped helper"\n')

    await _assert_mapped_external_audit_cell_replay_misses_after_change(buck, first)


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_mapped_external_extension_bzl_load_deletion_rejects_audit_cell_replay(
    buck: Buck,
) -> None:
    """Bazel anchor: SingleExtensionEvalFunction loaded .bzl digest includes deletes."""
    helper_path = _write_mapped_external_extension_audit_cell_workspace(
        buck,
        suffix="delete",
        helper_content='HELPER_SENTINEL = "initial mapped helper"\n',
    )

    before = await _bzlmod_counters(buck)
    await buck.audit("cell")
    first = await _bzlmod_counters(buck)
    assert first["extension_replay_hit"] > before["extension_replay_hit"]
    assert first["extension_eval"] == before["extension_eval"]

    helper_path.unlink()

    await _assert_mapped_external_audit_cell_replay_misses_after_change(buck, first)


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_mapped_external_extension_bzl_load_edit_rejects_replay(
    buck: Buck,
) -> None:
    """Bazel anchor: extension loads are resolved through the source repo mapping."""
    owner_module = "plan61_rules_owner"
    helper_module = "plan61_real_helper"
    helper_alias = "plan61_helper_alias"
    root_module = "plan61_mapped_load_replay"
    extension_id = f"@{owner_module}//:replay_ext.bzl%replay_ext"
    replayed_repo = buck.cwd / "replayed_repo"
    owner_dir = buck.cwd.parent / f"{buck.cwd.name}_rules_owner"
    helper_dir = buck.cwd.parent / f"{buck.cwd.name}_real_helper"
    external_dir = buck.cwd / "bazel-external"
    wrong_alias_dir = external_dir / helper_alias

    replayed_repo.mkdir(exist_ok=True)
    owner_dir.mkdir(parents=True, exist_ok=True)
    helper_dir.mkdir(parents=True, exist_ok=True)
    external_dir.mkdir(exist_ok=True)
    wrong_alias_dir.mkdir(exist_ok=True)

    _write(replayed_repo / "BUILD.bazel", "filegroup(name = \"data\")\n")
    _write(
        owner_dir / "MODULE.bazel",
        f"""module(name = "{owner_module}", version = "1.0")
bazel_dep(name = "{helper_module}", repo_name = "{helper_alias}")
""",
    )
    _write(
        owner_dir / "replay_ext.bzl",
        f"""load("@{helper_alias}//:helper.bzl", "HELPER_SENTINEL")

def _replay_ext_impl(module_ctx):
    fail("mapped helper edit should make replay stale: %s" % HELPER_SENTINEL)

replay_ext = module_extension(
    implementation = _replay_ext_impl,
)
""",
    )
    _write(helper_dir / "MODULE.bazel", f'module(name = "{helper_module}", version = "1.0")\n')
    _write(helper_dir / "helper.bzl", 'HELPER_SENTINEL = "initial mapped helper"\n')
    _write(wrong_alias_dir / "helper.bzl", 'HELPER_SENTINEL = "wrong apparent helper"\n')

    (external_dir / f"{owner_module}+").symlink_to(owner_dir, target_is_directory=True)
    (external_dir / f"{helper_module}+").symlink_to(helper_dir, target_is_directory=True)

    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "{root_module}")
bazel_dep(name = "{owner_module}", version = "1.0")
bazel_dep(name = "{helper_module}", version = "1.0")
local_path_override(module_name = "{owner_module}", path = "{owner_dir.as_posix()}")
local_path_override(module_name = "{helper_module}", path = "{helper_dir.as_posix()}")

replay = use_extension("@{owner_module}//:replay_ext.bzl", "replay_ext")
use_repo(replay, "replayed_repo")
""",
    )
    _write_replay_lockfile(
        buck.cwd / "MODULE.bazel.lock",
        extension_id=extension_id,
        module_name=root_module,
        project_root=buck.cwd,
        repo_path=replayed_repo,
        repo_mappings={owner_module: {helper_alias: helper_module}},
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
    first = await _bzlmod_counters(buck)
    assert first["extension_replay_hit"] > before["extension_replay_hit"]
    assert first["extension_eval"] == before["extension_eval"]

    _write(
        external_dir / f"{helper_module}+" / "helper.bzl",
        'HELPER_SENTINEL = "edited mapped helper"\n',
    )

    with pytest.raises(BuckException) as exc:
        await buck.build("//:uses_replayed_repo")
    second = await _bzlmod_counters(buck)

    assert "mapped helper edit should make replay stale: edited mapped helper" in str(exc.value)
    assert second["extension_replay_miss_reason"] > first["extension_replay_miss_reason"]
    assert second["extension_replay_hit"] == first["extension_replay_hit"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_missing_mapped_external_extension_bzl_load_creation_rejects_replay(
    buck: Buck,
) -> None:
    """Bazel anchor: SingleExtensionEvalFunction loaded .bzl digest includes creates."""
    owner_module = "plan61_rules_owner_missing"
    helper_module = "plan61_real_helper_missing"
    helper_alias = "plan61_helper_alias_missing"
    root_module = "plan61_mapped_load_missing_replay"
    extension_id = f"@{owner_module}//:replay_ext.bzl%replay_ext"
    replayed_repo = buck.cwd / "replayed_repo"
    owner_dir = buck.cwd.parent / f"{buck.cwd.name}_rules_owner_missing"
    helper_dir = buck.cwd.parent / f"{buck.cwd.name}_real_helper_missing"
    external_dir = buck.cwd / "bazel-external"

    replayed_repo.mkdir(exist_ok=True)
    owner_dir.mkdir(parents=True, exist_ok=True)
    helper_dir.mkdir(parents=True, exist_ok=True)
    external_dir.mkdir(exist_ok=True)

    _write(replayed_repo / "BUILD.bazel", "filegroup(name = \"data\")\n")
    _write(
        owner_dir / "MODULE.bazel",
        f"""module(name = "{owner_module}", version = "1.0")
bazel_dep(name = "{helper_module}", repo_name = "{helper_alias}")
""",
    )
    _write(
        owner_dir / "replay_ext.bzl",
        f"""load("@{helper_alias}//:helper.bzl", "HELPER_SENTINEL")

def _replay_ext_impl(module_ctx):
    fail("missing mapped helper creation should make replay stale: %s" % HELPER_SENTINEL)

replay_ext = module_extension(
    implementation = _replay_ext_impl,
)
""",
    )
    _write(helper_dir / "MODULE.bazel", f'module(name = "{helper_module}", version = "1.0")\n')

    (external_dir / f"{owner_module}+").symlink_to(owner_dir, target_is_directory=True)
    (external_dir / f"{helper_module}+").symlink_to(helper_dir, target_is_directory=True)

    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "{root_module}")
bazel_dep(name = "{owner_module}", version = "1.0")
bazel_dep(name = "{helper_module}", version = "1.0")
local_path_override(module_name = "{owner_module}", path = "{owner_dir.as_posix()}")
local_path_override(module_name = "{helper_module}", path = "{helper_dir.as_posix()}")

replay = use_extension("@{owner_module}//:replay_ext.bzl", "replay_ext")
use_repo(replay, "replayed_repo")
""",
    )
    _write_replay_lockfile(
        buck.cwd / "MODULE.bazel.lock",
        extension_id=extension_id,
        module_name=root_module,
        project_root=buck.cwd,
        repo_path=replayed_repo,
        repo_mappings={owner_module: {helper_alias: helper_module}},
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
    first = await _bzlmod_counters(buck)
    assert first["extension_replay_hit"] > before["extension_replay_hit"]
    assert first["extension_eval"] == before["extension_eval"]

    _write(helper_dir / "helper.bzl", 'HELPER_SENTINEL = "created mapped helper"\n')

    with pytest.raises(BuckException) as exc:
        await buck.build("//:uses_replayed_repo")
    second = await _bzlmod_counters(buck)

    assert "missing mapped helper creation should make replay stale" in str(exc.value)
    assert "created mapped helper" in str(exc.value)
    assert second["extension_replay_miss_reason"] > first["extension_replay_miss_reason"]
    assert second["extension_replay_hit"] == first["extension_replay_hit"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_mapped_external_extension_bzl_load_deletion_rejects_replay(
    buck: Buck,
) -> None:
    """Bazel anchor: SingleExtensionEvalFunction loaded .bzl digest includes deletes."""
    owner_module = "plan61_rules_owner_delete"
    helper_module = "plan61_real_helper_delete"
    helper_alias = "plan61_helper_alias_delete"
    root_module = "plan61_mapped_load_delete_replay"
    extension_id = f"@{owner_module}//:replay_ext.bzl%replay_ext"
    replayed_repo = buck.cwd / "replayed_repo"
    owner_dir = buck.cwd.parent / f"{buck.cwd.name}_rules_owner_delete"
    helper_dir = buck.cwd.parent / f"{buck.cwd.name}_real_helper_delete"
    external_dir = buck.cwd / "bazel-external"

    replayed_repo.mkdir(exist_ok=True)
    owner_dir.mkdir(parents=True, exist_ok=True)
    helper_dir.mkdir(parents=True, exist_ok=True)
    external_dir.mkdir(exist_ok=True)

    _write(replayed_repo / "BUILD.bazel", "filegroup(name = \"data\")\n")
    _write(
        owner_dir / "MODULE.bazel",
        f"""module(name = "{owner_module}", version = "1.0")
bazel_dep(name = "{helper_module}", repo_name = "{helper_alias}")
""",
    )
    _write(
        owner_dir / "replay_ext.bzl",
        f"""load("@{helper_alias}//:helper.bzl", "HELPER_SENTINEL")

def _replay_ext_impl(module_ctx):
    fail("external helper deletion should make replay stale: %s" % HELPER_SENTINEL)

replay_ext = module_extension(
    implementation = _replay_ext_impl,
)
""",
    )
    _write(helper_dir / "MODULE.bazel", f'module(name = "{helper_module}", version = "1.0")\n')
    helper_path = helper_dir / "helper.bzl"
    _write(helper_path, 'HELPER_SENTINEL = "initial mapped helper"\n')

    (external_dir / f"{owner_module}+").symlink_to(owner_dir, target_is_directory=True)
    (external_dir / f"{helper_module}+").symlink_to(helper_dir, target_is_directory=True)

    _write(
        buck.cwd / "MODULE.bazel",
        f"""module(name = "{root_module}")
bazel_dep(name = "{owner_module}", version = "1.0")
bazel_dep(name = "{helper_module}", version = "1.0")
local_path_override(module_name = "{owner_module}", path = "{owner_dir.as_posix()}")
local_path_override(module_name = "{helper_module}", path = "{helper_dir.as_posix()}")

replay = use_extension("@{owner_module}//:replay_ext.bzl", "replay_ext")
use_repo(replay, "replayed_repo")
""",
    )
    _write_replay_lockfile(
        buck.cwd / "MODULE.bazel.lock",
        extension_id=extension_id,
        module_name=root_module,
        project_root=buck.cwd,
        repo_path=replayed_repo,
        repo_mappings={owner_module: {helper_alias: helper_module}},
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
    first = await _bzlmod_counters(buck)
    assert first["extension_replay_hit"] > before["extension_replay_hit"]
    assert first["extension_eval"] == before["extension_eval"]

    helper_path.unlink()

    with pytest.raises(BuckException) as exc:
        await buck.build("//:uses_replayed_repo")
    second = await _bzlmod_counters(buck)

    assert "helper.bzl" in str(exc.value)
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
    assert after["extension_spokes_compute"] > before["extension_spokes_compute"]
    assert after["extension_eval"] == before["extension_eval"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_missing_lockfile_extension_executes_once_then_reuses_dice_state(
    buck: Buck,
) -> None:
    """Bazel anchor: lockfile miss executes extension and then Skyframe reuses it."""
    _write(
        buck.cwd / "live_ext.bzl",
        """def _repo_impl(repository_ctx):
    repository_ctx.file("data.txt", "live payload\\n")
    repository_ctx.file("BUILD.bazel", "exports_files([\\"data.txt\\"])\\nfilegroup(name = \\"data\\", srcs = [\\"data.txt\\"])\\n")

live_repo_rule = repository_rule(
    implementation = _repo_impl,
)

def _live_ext_impl(module_ctx):
    live_repo_rule(name = "live_repo")

live_ext = module_extension(
    implementation = _live_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_missing_lockfile_reuse")

live = use_extension("//:live_ext.bzl", "live_ext")
use_repo(live, "live_repo")
""",
    )
    _write_minimal_lockfile(buck.cwd / "MODULE.bazel.lock")
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_live_repo",
    srcs = ["@live_repo//:data"],
)
""",
    )

    before = await _bzlmod_counters(buck)
    await buck.build("//:uses_live_repo")
    first = await _bzlmod_counters(buck)
    assert first["extension_eval"] > before["extension_eval"]
    assert first["extension_replay_hit"] == before["extension_replay_hit"]

    await buck.build("//:uses_live_repo")
    second = await _bzlmod_counters(buck)
    assert second["extension_eval"] == first["extension_eval"]
    assert second["extension_replay_hit"] == first["extension_replay_hit"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_module_ctx_repo_env_uses_command_key_input(buck: Buck) -> None:
    """Bazel anchors: ModuleExtensionContext.getenv and repository_os.environ."""
    _write(
        buck.cwd / "env_ext.bzl",
        """def _repo_impl(repository_ctx):
    repository_ctx.file("data.txt", "env payload\\n")
    repository_ctx.file("BUILD.bazel", "exports_files([\\"data.txt\\"])\\nfilegroup(name = \\"data\\", srcs = [\\"data.txt\\"])\\n")

env_repo_rule = repository_rule(
    implementation = _repo_impl,
)

def _env_ext_impl(module_ctx):
    if module_ctx.getenv("PLAN61_REPO_ENV") != "from-flag":
        fail("PLAN61_MODULE_CTX_GETENV_NOT_FROM_COMMAND")
    if module_ctx.getenv("PLAN61_REPO_ENV_MISSING", "fallback") != "fallback":
        fail("PLAN61_MODULE_CTX_GETENV_DEFAULT_NOT_USED")
    if module_ctx.os.environ.get("PLAN61_REPO_ENV") != "from-flag":
        fail("PLAN61_MODULE_CTX_OS_ENVIRON_NOT_FROM_COMMAND")
    env_repo_rule(name = "env_repo")

env_ext = module_extension(
    implementation = _env_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_module_ctx_repo_env")

env = use_extension("//:env_ext.bzl", "env_ext")
use_repo(env, "env_repo")
""",
    )
    _write_minimal_lockfile(buck.cwd / "MODULE.bazel.lock")
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_env_repo",
    srcs = ["@env_repo//:data"],
)
""",
    )

    await buck.build("//:uses_env_repo", "--repo_env=PLAN61_REPO_ENV=from-flag")


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_repository_ctx_repo_env_uses_command_key_input(buck: Buck) -> None:
    """Bazel anchors: RepositoryContext.getenv and RepositoryFunction."""
    repo_dir = buck.cwd / "bazel-external" / "_main+repo_env_ext+env_repo"
    _write(
        buck.cwd / "repo_env_ext.bzl",
        """def _repo_impl(repository_ctx):
    value = repository_ctx.getenv("PLAN61_REPO_ENV")
    if value == None:
        fail("PLAN61_REPOSITORY_CTX_GETENV_NOT_FROM_COMMAND")
    if repository_ctx.getenv("PLAN61_REPO_ENV_MISSING", "fallback") != "fallback":
        fail("PLAN61_REPOSITORY_CTX_GETENV_DEFAULT_NOT_USED")
    if repository_ctx.os.environ.get("PLAN61_REPO_ENV") != value:
        fail("PLAN61_REPOSITORY_CTX_OS_ENVIRON_NOT_FROM_COMMAND")
    repository_ctx.file("data.txt", value + "\\n")
    repository_ctx.file("BUILD.bazel", "exports_files([\\"data.txt\\"])\\nfilegroup(name = \\"data\\", srcs = [\\"data.txt\\"])\\n")

env_repo_rule = repository_rule(
    implementation = _repo_impl,
)

def _repo_env_ext_impl(module_ctx):
    env_repo_rule(name = "env_repo")

repo_env_ext = module_extension(
    implementation = _repo_env_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_repository_ctx_repo_env")

env = use_extension("//:repo_env_ext.bzl", "repo_env_ext")
use_repo(env, "env_repo")
""",
    )
    _write_minimal_lockfile(buck.cwd / "MODULE.bazel.lock")
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_env_repo",
    srcs = ["@env_repo//:data"],
)
""",
    )

    await buck.build("//:uses_env_repo", "--repo_env=PLAN61_REPO_ENV=first")
    first = await _bzlmod_counters(buck, "--repo_env=PLAN61_REPO_ENV=first")
    assert (repo_dir / "data.txt").read_text() == "first\n"

    await buck.build("//:uses_env_repo", "--repo_env=PLAN61_REPO_ENV=second")
    second = await _bzlmod_counters(buck, "--repo_env=PLAN61_REPO_ENV=second")

    assert (repo_dir / "data.txt").read_text() == "second\n"
    assert second["repo_materialization_miss_reason"] > first[
        "repo_materialization_miss_reason"
    ]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_module_ctx_label_taking_operations_materialize_or_fail_directly(
    buck: Buck,
) -> None:
    """Bazel anchors: StarlarkBaseExternalContext and StarlarkRepositoryContext."""
    _write(buck.cwd / "tool.txt", "tool payload\n")
    _write(buck.cwd / "template.txt", "before\n")
    _write(
        buck.cwd / "change.patch",
        """--- generated.txt
+++ generated.txt
@@ -1 +1 @@
-before
+after
""",
    )
    _write(
        buck.cwd / "label_ops_ext.bzl",
        """def _repo_impl(repository_ctx):
    repository_ctx.file("data.txt", "label ops payload\\n")
    repository_ctx.file("BUILD.bazel", "exports_files([\\"data.txt\\"])\\nfilegroup(name = \\"data\\", srcs = [\\"data.txt\\"])\\n")

label_ops_repo = repository_rule(
    implementation = _repo_impl,
)

def _label_ops_ext_impl(module_ctx):
    module_ctx.watch(Label("//:template.txt"))
    module_ctx.template("generated.txt", Label("//:template.txt"), substitutions = {})
    module_ctx.patch(Label("//:change.patch"), strip = 0)
    if module_ctx.read("generated.txt") != "after\\n":
        fail("PLAN61_MODULE_CTX_PATCH_LABEL_NOT_APPLIED")
    module_ctx.symlink(Label("//:tool.txt"), "linked.txt")
    if module_ctx.read("linked.txt") != "tool payload\\n":
        fail("PLAN61_MODULE_CTX_SYMLINK_LABEL_NOT_MATERIALIZED")
    label_ops_repo(name = "label_ops_repo")

label_ops_ext = module_extension(
    implementation = _label_ops_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_label_ops")

label_ops = use_extension("//:label_ops_ext.bzl", "label_ops_ext")
use_repo(label_ops, "label_ops_repo")
""",
    )
    _write_minimal_lockfile(buck.cwd / "MODULE.bazel.lock")
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_label_ops_repo",
    srcs = ["@label_ops_repo//:data"],
)
""",
    )

    before = await _bzlmod_counters(buck)
    await buck.build("//:uses_label_ops_repo")
    after = await _bzlmod_counters(buck)

    assert after["extension_eval"] > before["extension_eval"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_repository_ctx_watch_tree_label_fails_directly_for_non_directory(
    buck: Buck,
) -> None:
    """Bazel anchors: StarlarkBaseExternalContext.watchTree and getPathFromLabel."""
    repo_dir = buck.cwd / "bazel-external" / "_main+watch_ext+watch_repo"
    _write(buck.cwd / "watched.txt", "not a directory\n")
    _write(
        buck.cwd / "watch_ext.bzl",
        """def _repo_impl(repository_ctx):
    repository_ctx.watch_tree(Label("//:watched.txt"))
    repository_ctx.file("data.txt", "watch payload\\n")
    repository_ctx.file("BUILD.bazel", "exports_files([\\"data.txt\\"])\\nfilegroup(name = \\"data\\", srcs = [\\"data.txt\\"])\\n")

watch_repo_rule = repository_rule(
    implementation = _repo_impl,
)

def _watch_ext_impl(module_ctx):
    watch_repo_rule(name = "watch_repo")

watch_ext = module_extension(
    implementation = _watch_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_repo_ctx_watch")

watch = use_extension("//:watch_ext.bzl", "watch_ext")
use_repo(watch, "watch_repo")
""",
    )
    _write_minimal_lockfile(buck.cwd / "MODULE.bazel.lock")
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_watch_repo",
    srcs = ["@watch_repo//:data"],
)
""",
    )

    failure_stderr: str | None = None
    try:
        await buck.build("//:uses_watch_repo")
    except BuckException as e:
        failure_stderr = e.stderr

    if failure_stderr is None:
        pytest.fail("repository_ctx.watch_tree(Label(file)) unexpectedly succeeded")

    assert "watch_tree" in failure_stderr
    assert "non-directory" in failure_stderr
    assert not (repo_dir / ".slug_repo_complete").exists()


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_repository_ctx_watch_label_edit_reexecutes_materialized_repo(
    buck: Buck,
) -> None:
    """Bazel anchors: RepoRecordedInput.File and RepositoryFetchFunction."""
    repo_dir = buck.cwd / "bazel-external" / "_main+watch_ext+watch_repo"
    recorded_inputs = repo_dir / ".slug_repo_recorded_inputs"
    _write(buck.cwd / "watched.txt", "first\n")
    _write(
        buck.cwd / "watch_ext.bzl",
        """def _repo_impl(repository_ctx):
    repository_ctx.watch(Label("//:watched.txt"))
    repository_ctx.file("data.txt", repository_ctx.read(Label("//:watched.txt")))
    repository_ctx.file("BUILD.bazel", "exports_files([\\"data.txt\\"])\\nfilegroup(name = \\"data\\", srcs = [\\"data.txt\\"])\\n")

watch_repo_rule = repository_rule(
    implementation = _repo_impl,
)

def _watch_ext_impl(module_ctx):
    watch_repo_rule(name = "watch_repo")

watch_ext = module_extension(
    implementation = _watch_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_repo_ctx_watch_input")

watch = use_extension("//:watch_ext.bzl", "watch_ext")
use_repo(watch, "watch_repo")
""",
    )
    _write_minimal_lockfile(buck.cwd / "MODULE.bazel.lock")
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_watch_repo",
    srcs = ["@watch_repo//:data"],
)
""",
    )

    await buck.build("//:uses_watch_repo")
    first = await _bzlmod_counters(buck)
    assert (repo_dir / "data.txt").read_text() == "first\n"
    assert recorded_inputs.exists()
    assert "FILE:" in recorded_inputs.read_text()

    _write(buck.cwd / "watched.txt", "second\n")

    await buck.build("//:uses_watch_repo")
    second = await _bzlmod_counters(buck)

    assert (repo_dir / "data.txt").read_text() == "second\n"
    assert second["repo_materialization_miss_reason"] > first[
        "repo_materialization_miss_reason"
    ]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_repository_ctx_watch_tree_nested_edit_reexecutes_materialized_repo(
    buck: Buck,
) -> None:
    """Bazel anchors: RepoRecordedInput.DirTree and RepositoryFetchFunction."""
    repo_dir = buck.cwd / "bazel-external" / "_main+watch_tree_ext+watch_tree_repo"
    recorded_inputs = repo_dir / ".slug_repo_recorded_inputs"
    watched_leaf = buck.cwd / "watched_tree" / "sub" / "value.txt"
    watched_leaf.parent.mkdir(parents=True)
    _write(watched_leaf, "first\n")
    _write(
        buck.cwd / "watch_tree_ext.bzl",
        """def _repo_impl(repository_ctx):
    repository_ctx.watch_tree(Label("//:watched_tree"))
    repository_ctx.file("data.txt", repository_ctx.read(Label("//:watched_tree/sub/value.txt")))
    repository_ctx.file("BUILD.bazel", "exports_files([\\"data.txt\\"])\\nfilegroup(name = \\"data\\", srcs = [\\"data.txt\\"])\\n")

watch_tree_repo_rule = repository_rule(
    implementation = _repo_impl,
)

def _watch_tree_ext_impl(module_ctx):
    watch_tree_repo_rule(name = "watch_tree_repo")

watch_tree_ext = module_extension(
    implementation = _watch_tree_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_repo_ctx_watch_tree_input")

watch_tree = use_extension("//:watch_tree_ext.bzl", "watch_tree_ext")
use_repo(watch_tree, "watch_tree_repo")
""",
    )
    _write_minimal_lockfile(buck.cwd / "MODULE.bazel.lock")
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_watch_tree_repo",
    srcs = ["@watch_tree_repo//:data"],
)
""",
    )

    await buck.build("//:uses_watch_tree_repo")
    first = await _bzlmod_counters(buck)
    assert (repo_dir / "data.txt").read_text() == "first\n"
    assert recorded_inputs.exists()
    assert "DIRTREE:" in recorded_inputs.read_text()

    _write(watched_leaf, "second\n")

    await buck.build("//:uses_watch_tree_repo")
    second = await _bzlmod_counters(buck)

    assert (repo_dir / "data.txt").read_text() == "second\n"
    assert second["repo_materialization_miss_reason"] > first[
        "repo_materialization_miss_reason"
    ]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_module_ctx_wasm_methods_are_disabled_by_default(
    buck: Buck,
) -> None:
    """Bazel anchors: BuildLanguageOptions and StarlarkBaseExternalContext."""
    repo_dir = buck.cwd / "bazel-external" / "_main+wasm_ext+wasm_repo"
    (buck.cwd / "probe.wasm").write_bytes(b"\0asm\x01\0\0\0")
    _write(
        buck.cwd / "wasm_ext.bzl",
        """def _repo_impl(repository_ctx):
    repository_ctx.file("data.txt", "wasm payload\\n")
    repository_ctx.file("BUILD.bazel", "exports_files([\\"data.txt\\"])\\nfilegroup(name = \\"data\\", srcs = [\\"data.txt\\"])\\n")

wasm_repo_rule = repository_rule(
    implementation = _repo_impl,
)

def _wasm_ext_impl(module_ctx):
    module_ctx.load_wasm(Label("//:probe.wasm"))
    wasm_repo_rule(name = "wasm_repo")

wasm_ext = module_extension(
    implementation = _wasm_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_wasm_default_disabled")

wasm = use_extension("//:wasm_ext.bzl", "wasm_ext")
use_repo(wasm, "wasm_repo")
""",
    )
    _write_minimal_lockfile(buck.cwd / "MODULE.bazel.lock")
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_wasm_repo",
    srcs = ["@wasm_repo//:data"],
)
""",
    )

    failure_stderr: str | None = None
    try:
        await buck.build("//:uses_wasm_repo")
    except BuckException as e:
        failure_stderr = e.stderr

    if failure_stderr is None:
        pytest.fail("module_ctx.load_wasm unexpectedly succeeded by default")

    assert "load_wasm" in failure_stderr
    assert not (repo_dir / ".slug_repo_complete").exists()


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_lockfile_replay_unsupported_recorded_input_rejects_cache(
    buck: Buck,
) -> None:
    """Bazel anchor: unsupported RepoRecordedInput data is not replayable."""
    module_name = "plan61_unsupported_recorded_input"
    extension_id = "@plan61_unsupported_recorded_input//:replay_ext.bzl%replay_ext"
    watched = buck.cwd / "watched.txt"
    _write(watched, "unchanged\n")
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
    fail("unsupported recorded input replay rejected")

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
        recorded_inputs=[f"FILE:watched.txt {_sha256(watched)}"],
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
    failure_stderr: str | None = None
    try:
        await buck.audit("cell")
    except BuckException as e:
        failure_stderr = e.stderr
    after = await _bzlmod_counters(buck)

    if failure_stderr is not None:
        assert "unsupported recorded input replay rejected" in failure_stderr
    assert after["extension_replay_miss_reason"] > before["extension_replay_miss_reason"]
    assert after["extension_replay_hit"] == before["extension_replay_hit"]


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
async def test_lockfile_replay_recorded_env_input_change_rejects_mixed_graph_cache(
    buck: Buck,
) -> None:
    """Bazel anchors: RepoRecordedInput.EnvVar and SingleExtensionEvalFunction."""
    module_name = "plan61_recorded_env_mixed"
    dep_module_name = "plan61_recorded_env_dep"
    extension_id = f"@{module_name}//:replay_ext.bzl%replay_ext"
    dep = buck.cwd / "dep"
    dep.mkdir(exist_ok=True)
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

bazel_dep(name = "{dep_module_name}", version = "1.0")
local_path_override(module_name = "{dep_module_name}", path = "dep")

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
async def test_transitive_repo_name_aliases_are_scoped_to_declaring_module(
    buck: Buck,
) -> None:
    """Bazel anchor: apparent repo names are module-scoped repository mappings."""

    def write_module(name: str, module_bazel: str, build_bazel: str) -> None:
        module_dir = buck.cwd / name
        module_dir.mkdir()
        _write(module_dir / "MODULE.bazel", module_bazel)
        _write(module_dir / "BUILD.bazel", build_bazel)

    write_module(
        "a",
        """module(name = "a", version = "1.0")
bazel_dep(name = "c", version = "1.0", repo_name = "shared")
""",
        """filegroup(
    name = "uses_shared",
    srcs = ["@shared//:c_only"],
    visibility = ["//visibility:public"],
)
""",
    )
    write_module(
        "b",
        """module(name = "b", version = "1.0")
bazel_dep(name = "d", version = "1.0", repo_name = "shared")
""",
        """filegroup(
    name = "uses_shared",
    srcs = ["@shared//:d_only"],
    visibility = ["//visibility:public"],
)
""",
    )
    write_module(
        "c",
        """module(name = "c", version = "1.0")
""",
        """filegroup(name = "c_only", visibility = ["//visibility:public"])
""",
    )
    write_module(
        "d",
        """module(name = "d", version = "1.0")
""",
        """filegroup(name = "d_only", visibility = ["//visibility:public"])
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_scoped_repo_names")
bazel_dep(name = "a", version = "1.0")
bazel_dep(name = "b", version = "1.0")
bazel_dep(name = "c", version = "1.0")
bazel_dep(name = "d", version = "1.0")
local_path_override(module_name = "a", path = "a")
local_path_override(module_name = "b", path = "b")
local_path_override(module_name = "c", path = "c")
local_path_override(module_name = "d", path = "d")
""",
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_transitive_scoped_aliases",
    srcs = [
        "@a//:uses_shared",
        "@b//:uses_shared",
    ],
)
""",
    )

    await buck.build("//:uses_transitive_scoped_aliases")

    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "root_cannot_see_transitive_alias",
    srcs = ["@shared//:c_only"],
)
""",
    )
    with pytest.raises(BuckException) as exc:
        await buck.build("//:root_cannot_see_transitive_alias")
    assert "unknown cell name: `shared`" in str(exc.value)


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_root_repo_name_alias_does_not_leak_to_transitive_module(
    buck: Buck,
) -> None:
    """Bazel anchor: BazelDepGraphValue.getFullRepoMapping(ModuleKey)."""
    a = buck.cwd / "a"
    b = buck.cwd / "b"
    a.mkdir()
    b.mkdir()
    _write(a / "MODULE.bazel", 'module(name = "a", version = "1.0")\n')
    _write(
        a / "BUILD.bazel",
        """filegroup(
    name = "uses_root_alias",
    srcs = ["@root_b_alias//:b_only"],
    visibility = ["//visibility:public"],
)
""",
    )
    _write(b / "MODULE.bazel", 'module(name = "b", version = "1.0")\n')
    _write(
        b / "BUILD.bazel",
        """filegroup(name = "b_only", visibility = ["//visibility:public"])
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_root_alias_scope")
bazel_dep(name = "a", version = "1.0")
bazel_dep(name = "b", version = "1.0", repo_name = "root_b_alias")
local_path_override(module_name = "a", path = "a")
local_path_override(module_name = "b", path = "b")
""",
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "root_uses_alias",
    srcs = ["@root_b_alias//:b_only"],
)
""",
    )

    await buck.build("//:root_uses_alias")

    with pytest.raises(BuckException) as exc:
        await buck.build("@a//:uses_root_alias")

    assert "root_b_alias" in str(exc.value)


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_root_use_repo_alias_does_not_leak_to_transitive_module(
    buck: Buck,
) -> None:
    """Bazel anchor: module-scoped use_repo imports in BazelDepGraphValue."""
    a = buck.cwd / "a"
    tool_repo = buck.cwd / "tool_repo"
    a.mkdir()
    tool_repo.mkdir()
    _write(a / "MODULE.bazel", 'module(name = "a", version = "1.0")\n')
    _write(
        a / "BUILD.bazel",
        """filegroup(
    name = "uses_tool",
    srcs = ["@tool_alias//:tool"],
    visibility = ["//visibility:public"],
)
""",
    )
    _write(
        tool_repo / "BUILD.bazel",
        """filegroup(name = "tool", visibility = ["//visibility:public"])
""",
    )
    _write(
        buck.cwd / "root_ext.bzl",
        """load("@bazel_tools//tools/build_defs/repo:local.bzl", "local_repository")

def _impl(module_ctx):
    local_repository(name = "tool_repo", path = "tool_repo")

root_ext = module_extension(implementation = _impl)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_root_use_repo_scope")
bazel_dep(name = "a", version = "1.0")
local_path_override(module_name = "a", path = "a")
root = use_extension("//:root_ext.bzl", "root_ext")
use_repo(root, tool_alias = "tool_repo")
""",
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "root_uses_tool",
    srcs = ["@tool_alias//:tool"],
)
""",
    )

    await buck.build("//:root_uses_tool")

    with pytest.raises(BuckException) as exc:
        await buck.build("@a//:uses_tool")

    assert "tool_alias" in str(exc.value)


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_inject_repo_keyword_alias_maps_generated_repo_and_replays(
    buck: Buck,
) -> None:
    """Bazel anchor: inject_repo() adds root-visible repos to generated repo mappings."""
    helper_a = buck.cwd / "helper_a"
    helper_b = buck.cwd / "helper_b"
    helper_a.mkdir()
    helper_b.mkdir()
    _write(helper_a / "MODULE.bazel", 'module(name = "helper_a", version = "1.0")\n')
    _write(
        helper_a / "BUILD.bazel",
        """exports_files(["payload.txt"])
""",
    )
    _write(helper_a / "payload.txt", "payload from helper_a\n")
    _write(helper_b / "MODULE.bazel", 'module(name = "helper_b", version = "1.0")\n')
    _write(helper_b / "BUILD.bazel", "# helper_b intentionally has no payload.txt\n")
    _write(
        buck.cwd / "ext.bzl",
        """def _made_impl(ctx):
    ctx.file(
        "BUILD.bazel",
        "filegroup(name = \\"from_injected\\", srcs = [\\"@injected_helper//:payload.txt\\"])\\n",
    )

made = repository_rule(implementation = _made_impl)

def _ext_impl(module_ctx):
    made(name = "generated")

ext = module_extension(implementation = _ext_impl)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_inject_repo")
bazel_dep(name = "helper_a", version = "1.0")
bazel_dep(name = "helper_b", version = "1.0")
local_path_override(module_name = "helper_a", path = "helper_a")
local_path_override(module_name = "helper_b", path = "helper_b")
ext = use_extension("//:ext.bzl", "ext")
inject_repo(ext, injected_helper = "helper_a")
use_repo(ext, "generated")
""",
    )
    _write(buck.cwd / "BUILD.bazel", 'filegroup(name = "root")\n')

    before = await _bzlmod_counters(buck)
    await buck.build("@generated//:from_injected")
    after_first = await _bzlmod_counters(buck)

    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_inject_repo")
bazel_dep(name = "helper_a", version = "1.0")
bazel_dep(name = "helper_b", version = "1.0")
local_path_override(module_name = "helper_a", path = "helper_a")
local_path_override(module_name = "helper_b", path = "helper_b")
ext = use_extension("//:ext.bzl", "ext")
inject_repo(ext, injected_helper = "helper_b")
use_repo(ext, "generated")
""",
    )

    with pytest.raises(BuckException) as exc:
        await buck.build("@generated//:from_injected")
    after_second = await _bzlmod_counters(buck)

    assert "payload.txt" in str(exc.value)
    assert after_first["extension_eval"] > before["extension_eval"]
    assert after_second["extension_eval"] > after_first["extension_eval"]


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_override_repo_positional_maps_same_named_generated_repo(
    buck: Buck,
) -> None:
    """Bazel anchor: override_repo(ext, "repo") maps repo to same-named root repo."""
    replacement = buck.cwd / "generated"
    replacement.mkdir()
    _write(replacement / "MODULE.bazel", 'module(name = "generated", version = "1.0")\n')
    _write(replacement / "BUILD.bazel", 'exports_files(["payload.txt"])\n')
    _write(replacement / "payload.txt", "payload from root replacement\n")
    _write(
        buck.cwd / "ext.bzl",
        """def _repo_impl(ctx):
    ctx.file("BUILD.bazel", "filegroup(name = \\"empty\\", srcs = [])\\n")

def _consumer_impl(ctx):
    ctx.file(
        "BUILD.bazel",
        "filegroup(name = \\"from_override\\", srcs = [\\"@generated//:payload.txt\\"])\\n",
    )

repo = repository_rule(implementation = _repo_impl)
consumer = repository_rule(implementation = _consumer_impl)

def _ext_impl(module_ctx):
    repo(name = "generated")
    consumer(name = "consumer")

ext = module_extension(implementation = _ext_impl)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_override_repo_positional")
bazel_dep(name = "generated", version = "1.0")
local_path_override(module_name = "generated", path = "generated")
ext = use_extension("//:ext.bzl", "ext")
override_repo(ext, "generated")
use_repo(ext, "consumer")
""",
    )
    _write(buck.cwd / "BUILD.bazel", 'filegroup(name = "root")\n')

    await buck.build("@consumer//:from_override")


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_inject_repo_is_ignored_under_ignore_dev_dependency(
    buck: Buck,
) -> None:
    """Bazel anchor: inject_repo() is ignored by --ignore_dev_dependency."""
    helper = buck.cwd / "helper"
    helper.mkdir()
    _write(helper / "MODULE.bazel", 'module(name = "helper", version = "1.0")\n')
    _write(helper / "BUILD.bazel", 'exports_files(["payload.txt"])\n')
    _write(helper / "payload.txt", "payload from helper\n")
    _write(
        buck.cwd / "ext.bzl",
        """def _made_impl(ctx):
    ctx.file(
        "BUILD.bazel",
        "filegroup(name = \\"from_injected\\", srcs = [\\"@injected_helper//:payload.txt\\"])\\n",
    )

made = repository_rule(implementation = _made_impl)

def _ext_impl(module_ctx):
    made(name = "generated")

ext = module_extension(implementation = _ext_impl)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_inject_repo_ignore_dev")
bazel_dep(name = "helper", version = "1.0")
local_path_override(module_name = "helper", path = "helper")
ext = use_extension("//:ext.bzl", "ext")
inject_repo(ext, injected_helper = "helper")
use_repo(ext, "generated")
""",
    )
    _write(buck.cwd / "BUILD.bazel", 'filegroup(name = "root")\n')

    await buck.build("@generated//:from_injected")
    with pytest.raises(BuckException) as exc:
        await buck.build("@generated//:from_injected", "--ignore_dev_dependency")

    assert "injected_helper" in str(exc.value)


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
async def test_materialized_repo_marker_revalidates_corrupted_local_repo_layout(
    buck: Buck,
) -> None:
    """Bazel anchors: DigestWriter, RepoRecordedInput, and RepositoryFetchFunction."""
    canonical_repo = "+new_local_repository+corrupt_repo"
    repo_dir = buck.cwd / "bazel-external" / canonical_repo
    marker = repo_dir / ".slug_repo_complete"
    repo_src = buck.cwd / "repo_src"

    repo_src.mkdir()
    _write(repo_src / "fresh.txt", "fresh output from current repo spec\n")
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_corrupt_marker")

repo = use_repo_rule("@@bazel_tools//tools/build_defs/repo:local.bzl", "new_local_repository")
repo(
    name = "corrupt_repo",
    path = "repo_src",
    build_file_content = \"\"\"exports_files(["fresh.txt"])
\"\"\",
)
""",
    )
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_corrupt_repo",
    srcs = ["@corrupt_repo//:fresh.txt"],
)
""",
    )

    await buck.build("//:uses_corrupt_repo")
    assert marker.exists()

    materialized_file = repo_dir / "fresh.txt"
    assert materialized_file.is_symlink()
    materialized_file.unlink()
    _write(materialized_file, "corrupted materialized output\n")

    await buck.kill()
    await buck.build("//:uses_corrupt_repo")

    assert materialized_file.is_symlink()
    assert materialized_file.read_text() == "fresh output from current repo spec\n"


@buck_test(data_dir="test_plan61_guardrails_data")
async def test_materialized_repo_marker_revalidates_corrupted_output_digest(
    buck: Buck,
) -> None:
    """Bazel anchors: RepositoryDirectoryValue and RepoRecordedInput marker checks."""
    canonical_repo = "_main+output_digest_ext+output_digest_repo"
    repo_dir = buck.cwd / "bazel-external" / canonical_repo
    marker = repo_dir / ".slug_repo_complete"
    _write(
        buck.cwd / "output_digest_ext.bzl",
        """def _repo_impl(repository_ctx):
    repository_ctx.file("data.txt", "fresh output from repo rule\\n")
    repository_ctx.file("BUILD.bazel", "exports_files([\\"data.txt\\"])\\nfilegroup(name = \\"data\\", srcs = [\\"data.txt\\"])\\n")

output_digest_repo_rule = repository_rule(
    implementation = _repo_impl,
)

def _output_digest_ext_impl(module_ctx):
    output_digest_repo_rule(name = "output_digest_repo")

output_digest_ext = module_extension(
    implementation = _output_digest_ext_impl,
)
""",
    )
    _write(
        buck.cwd / "MODULE.bazel",
        """module(name = "plan61_output_digest_marker")

ext = use_extension("//:output_digest_ext.bzl", "output_digest_ext")
use_repo(ext, "output_digest_repo")
""",
    )
    _write_minimal_lockfile(buck.cwd / "MODULE.bazel.lock")
    _write(
        buck.cwd / "BUILD.bazel",
        """filegroup(
    name = "uses_output_digest_repo",
    srcs = ["@output_digest_repo//:data"],
)
""",
    )

    await buck.build("//:uses_output_digest_repo")
    assert marker.exists()
    assert ":output:" in marker.read_text()
    materialized_file = repo_dir / "data.txt"
    assert materialized_file.read_text() == "fresh output from repo rule\n"

    _write(materialized_file, "corrupted materialized output\n")

    await buck.kill()
    before_refetch = await _bzlmod_counters(buck)
    await buck.build("//:uses_output_digest_repo")
    after_refetch = await _bzlmod_counters(buck)

    assert materialized_file.read_text() == "fresh output from repo rule\n"
    assert after_refetch["repo_materialization_miss_reason"] > before_refetch[
        "repo_materialization_miss_reason"
    ]


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
