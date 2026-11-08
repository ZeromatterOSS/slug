#!/usr/bin/env python3
"""Stage pinned inputs for the unchanged Rust toolchain owner fixture.

This tool authors test inputs only. It performs no network access, archive
extraction, module substitution, Slug invocation or semantic result injection.
The existing configured fixture keeps its own independent byte limits.
"""
from __future__ import annotations

import argparse
from dataclasses import replace
import hashlib
import json
import os
import shutil
import sys
import tomllib
from pathlib import Path
from urllib.parse import urlsplit

if __package__:
    from . import configured_cli_fixture as configured
else:
    import configured_cli_fixture as configured

REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE_ROOT = REPO_ROOT / "tests/v2_oracle/fixtures/rules-rust-073-toolchain-owner"
INVENTORY = FIXTURE_ROOT / "native-inputs.json"
CHUNK_BYTES = 64 << 10
MAX_INVENTORY_BYTES = 64 << 10
MAX_CACHE_ROWS = 32
MAX_WORKSPACE_ROWS = 32


def _integer(value: object, field: str) -> int:
    if type(value) is not int or value < 0:
        raise configured.FixtureError(f"{field} must be a nonnegative integer")
    return value


def _digest(value: object) -> str:
    if not isinstance(value, str) or not configured.SHA256_RE.fullmatch(value):
        raise configured.FixtureError("inventory SHA-256 is invalid")
    return value


def _load_inventory() -> tuple[dict, tuple[configured.Object, ...], tuple[configured.Object, ...]]:
    # The small checked-in inventory is control data, never a payload buffer.
    with INVENTORY.open("rb") as stream:
        encoded = stream.read(MAX_INVENTORY_BYTES + 1)
    if len(encoded) > MAX_INVENTORY_BYTES:
        raise configured.FixtureError("Rust inventory exceeds its byte bound")
    raw = json.loads(encoded)
    if raw.get("version") != 1 or raw.get("fixture") != FIXTURE_ROOT.name:
        raise configured.FixtureError("unsupported Rust input inventory")
    rows = raw.get("objects")
    workspace = raw.get("workspace")
    if not isinstance(rows, list) or not 1 <= len(rows) <= MAX_CACHE_ROWS:
        raise configured.FixtureError("Rust cache inventory count refused")
    if not isinstance(workspace, list) or not 1 <= len(workspace) <= MAX_WORKSPACE_ROWS:
        raise configured.FixtureError("Rust workspace inventory count refused")
    objects: list[configured.Object] = []
    destinations: set[str] = set()
    archive_names: set[str] = set()
    for row in rows:
        destination = configured._relative(row["destination"], "destination")
        digest = _digest(row["sha256"])
        size = _integer(row["size"], "size")
        url = row["source_url"]
        parsed = urlsplit(url)
        if parsed.scheme != "https" or not parsed.netloc or parsed.query or parsed.fragment:
            raise configured.FixtureError("Rust inventory requires an exact HTTPS source URL")
        kind = row["kind"]
        if kind in {"registry", "patch"}:
            expected = "registry" + parsed.path
            if parsed.netloc != "bcr.bazel.build":
                raise configured.FixtureError("registry object has a foreign origin")
        elif kind == "archive":
            expected = "mirror/" + parsed.netloc + parsed.path
        elif kind == "tool_archive":
            expected = "distdir/rust/" + Path(parsed.path).name
        else:
            raise configured.FixtureError("unknown Rust input kind")
        if destination != expected or destination in destinations:
            raise configured.FixtureError("Rust inventory destination association refused")
        destinations.add(destination)
        if kind in {"archive", "tool_archive"}:
            basename = Path(destination).name
            if basename in archive_names:
                raise configured.FixtureError("ambiguous archive basename in Rust inventory")
            archive_names.add(basename)
        objects.append(configured.Object(
            source=f"{digest}/file", destination=destination, sha256=digest,
            size=size, source_url=url, kind=kind,
        ))
    files: list[configured.Object] = []
    for row in workspace:
        relative = configured._relative(row["path"], "workspace path")
        destination = "workspace/" + relative
        if destination in destinations:
            raise configured.FixtureError("duplicate Rust workspace path")
        destinations.add(destination)
        files.append(configured.Object(
            source=destination, destination=destination, sha256=_digest(row["sha256"]),
            size=_integer(row["size"], "workspace size"),
            source_url="repository-owned unchanged Rust owner fixture", kind="root",
        ))
    if (sum(item.size for item in objects) != _integer(raw["aggregate_cache_bytes"], "cache aggregate")
            or sum(item.size for item in files) != _integer(raw["aggregate_workspace_bytes"], "workspace aggregate")):
        raise configured.FixtureError("Rust inventory aggregate differs from its declared bound")
    raw["inventory_sha256"] = hashlib.sha256(encoded).hexdigest()
    return raw, tuple(objects), tuple(files)


def _copy(source_root: Path, item: configured.Object, destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    # The inherited helper opens every ancestor and the leaf O_NOFOLLOW,
    # correlates name/FD identity and rechecks them after full EOF consumption.
    # It has no large-buffer allocation or configured-fixture size-cap override.
    with configured._held_archive(source_root, item) as (source, _):
        with destination.open("xb", buffering=0) as output:
            digest = hashlib.sha256()
            remaining = item.size
            while remaining:
                block = source.read(min(CHUNK_BYTES, remaining))
                if not block:
                    raise configured.FixtureError(f"short staged input: {item.destination}")
                digest.update(block)
                remaining -= len(block)
                if output.write(block) != len(block):
                    raise configured.FixtureError(f"short staged write: {item.destination}")
            if source.read(1) or digest.hexdigest() != item.sha256:
                raise configured.FixtureError(f"staged input size/SHA mismatch: {item.destination}")
    destination.chmod(0o644)


def _archive_objects(base_manifest: dict, objects: tuple[configured.Object, ...]) -> tuple[configured.Object, ...]:
    archives = [configured.Object(**row) for row in base_manifest["objects"] if row["kind"] == "archive"]
    archives.extend(item for item in objects if item.kind in {"archive", "tool_archive"})
    basenames: dict[str, tuple[str, int]] = {}
    for item in archives:
        configured._relative(item.destination, "archive destination")
        name = Path(item.destination).name
        association = (item.sha256, item.size)
        if name in basenames and basenames[name] != association:
            raise configured.FixtureError(f"conflicting flat archive basename: {name}")
        basenames[name] = association
    return tuple(archives)


def _flat_archive_links(destination: Path, archives: tuple[configured.Object, ...]) -> dict:
    flags = os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_DIRECTORY
    root_fd = os.open(destination, flags)
    flat_fd = -1
    representatives: dict[str, configured.Object] = {}
    try:
        root_identity = configured._identity(os.fstat(root_fd), directory=True)
        if root_identity != configured._identity(destination.lstat(), directory=True):
            raise configured.FixtureError("staging root identity changed")
        os.mkdir("flat-distdir", dir_fd=root_fd)
        named_flat = os.stat("flat-distdir", dir_fd=root_fd, follow_symlinks=False)
        flat_fd = os.open("flat-distdir", flags, dir_fd=root_fd)
        flat_identity = configured._identity(os.fstat(flat_fd), directory=True)
        if flat_identity != configured._identity(named_flat, directory=True):
            raise configured.FixtureError("flat directory identity changed")
        for item in archives:
            representatives.setdefault(Path(item.destination).name, item)
        for name, item in sorted(representatives.items()):
            source_item = replace(item, source=item.destination)
            source_path = destination / item.destination
            # Link side effects use pinned directories, never an ambient
            # absolute source/destination traversal. Only fresh exclusively
            # owned staging copies are eligible, never the external cache.
            source_parent = os.open(source_path.parent, flags)
            try:
                with configured._held_archive(destination, source_item) as (stream, before):
                    if (configured._identity(os.fstat(source_parent), directory=True)
                            != before.ancestors[-1]
                            or before.ancestors[len(destination.parts) - 1] != root_identity):
                        raise configured.FixtureError(f"staged archive parent changed: {name}")
                    configured._stream_archive(stream, source_item)
                # Creating a hardlink changes ctime. Close the file verification
                # context first; keep both correlated directory FDs armed.
                named_source = os.stat(source_path.name, dir_fd=source_parent, follow_symlinks=False)
                if (configured._identity(named_source) != before.identity
                        or configured._identity(os.fstat(root_fd), directory=True) != root_identity
                        or configured._identity(destination.lstat(), directory=True) != root_identity
                        or configured._identity(os.fstat(flat_fd), directory=True) != flat_identity
                        or configured._identity(os.stat("flat-distdir", dir_fd=root_fd,
                                                       follow_symlinks=False), directory=True) != flat_identity):
                    raise configured.FixtureError(f"flat archive pre-link identity changed: {name}")
                os.link(source_path.name, name, src_dir_fd=source_parent,
                        dst_dir_fd=flat_fd, follow_symlinks=False)
                flat_item = replace(item, source="flat-distdir/" + name)
                with configured._held_archive(destination, source_item) as (source, current):
                    with configured._held_archive(destination, flat_item) as (linked, linked_current):
                        if (current.identity[:-1] != before.identity[:-1]
                                or current.ancestors != before.ancestors
                                or current.identity[:2] != linked_current.identity[:2]
                                or current.ancestors[-1] != configured._identity(
                                    os.fstat(source_parent), directory=True)
                                or linked_current.ancestors[-1] != flat_identity):
                            raise configured.FixtureError(f"flat archive identity changed: {name}")
                        configured._stream_archive(source, source_item)
                        configured._stream_archive(linked, flat_item)
            finally:
                os.close(source_parent)
    finally:
        if flat_fd != -1:
            os.close(flat_fd)
        os.close(root_fd)
    # Inventory bytes count consumed inputs. Report archive inode storage and
    # logical pathname bytes separately: links add names, never another payload.
    unique: dict[tuple[int, int], int] = {}
    logical_bytes = 0
    for relative in [item.destination for item in archives] + [
            "flat-distdir/" + name for name in representatives]:
        metadata = (destination / relative).lstat()
        unique[(metadata.st_dev, metadata.st_ino)] = metadata.st_size
        logical_bytes += metadata.st_size
    return {
        "source_paths": len(archives), "flat_paths": len(representatives),
        "unique_inode_bytes": sum(unique.values()), "logical_path_bytes": logical_bytes,
        "additional_payload_copies": 0,
        "classification": "File-length accounting, not kernel metadata allocation or RSS",
    }


def assemble(cache_root: Path, destination: Path) -> dict:
    raw, objects, workspace = _load_inventory()
    cache_root = cache_root.absolute()
    destination = destination.absolute()
    if destination.exists() or destination.is_symlink():
        raise configured.FixtureError("Rust staging destination already exists")
    # Refuse a changed base manifest before consuming any large external input.
    manifest = configured.DEFAULT_FIXTURE_ROOT / "fixture.toml"
    if hashlib.sha256(manifest.read_bytes()).hexdigest() != raw["provenance"]["configured_fixture_manifest_sha256"]:
        raise configured.FixtureError("configured fixture manifest differs from Rust inventory provenance")
    with manifest.open("rb") as stream:
        base_manifest = tomllib.load(stream)
    # Validate the WHOLE base+extension set before creating any destination.
    archives = _archive_objects(base_manifest, objects)
    base = configured.assemble(destination)
    try:
        # Base assembly's local registry/mirror paths remain correct. Its tiny
        # conflicts workspace is replaced by the six byte-exact owner inputs;
        # no released Starlark body or generated repository output is authored.
        shutil.rmtree(destination / "workspace")
        for item in workspace:
            _copy(FIXTURE_ROOT, item, destination / item.destination)
        for item in objects:
            _copy(cache_root, item, destination / item.destination)
        base_inventory = destination / "inventory.json"
        base_inventory.rename(destination / "configured-inventory.json")
        archive_storage = _flat_archive_links(destination, archives)
        result = {
            "fixture": FIXTURE_ROOT.name,
            "inventory_sha256": raw["inventory_sha256"],
            "configured_inventory_sha256": base["inventory_sha256"],
            "workspace": str(destination / "workspace"),
            "registry_uri": (destination / "registry").as_uri().rstrip("/") + "/",
            "files": [
                {"relative": item.source.removeprefix("workspace/"),
                 "bytes": item.size, "sha256": item.sha256}
                for item in workspace
            ],
            "distdirs": [str(destination / "flat-distdir")],
            "archive_storage": archive_storage,
            "cache_object_count": len(objects),
            "objects": raw["objects"],
            "cache_source_bytes": raw["aggregate_cache_bytes"],
            "workspace_source_bytes": raw["aggregate_workspace_bytes"],
            "configured_source_bytes": base["source_bytes"],
            "aggregate_source_bytes": base["source_bytes"] + raw["aggregate_cache_bytes"] + raw["aggregate_workspace_bytes"],
            "classification": raw["classification"],
        }
        (destination / "inventory.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        return result
    except BaseException:
        # Only this newly-created, exclusively owned staging destination retires.
        shutil.rmtree(destination, ignore_errors=True)
        raise


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache-root", type=Path, required=True,
                        help="Bazel content_addressable/sha256 directory (not extracted repository cache)")
    parser.add_argument("--output", type=Path, required=True,
                        help="fresh destination; never an existing workspace")
    args = parser.parse_args()
    try:
        print(json.dumps(assemble(args.cache_root, args.output), sort_keys=True))
        return 0
    except (configured.FixtureError, OSError, ValueError, KeyError, TypeError) as error:
        print(f"Rust fixture staging refused: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
