#!/usr/bin/env python3
"""Verify and assemble the repository-owned configured CLI fixture."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import re
import shutil
import signal
import stat
import subprocess
import sys
import tarfile
import tempfile
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any

try:
    import tomllib
except ModuleNotFoundError as error:  # pragma: no cover - Python 3.11 is required.
    raise RuntimeError("configured_cli_fixture requires Python 3.11+") from error


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_FIXTURE_ROOT = (
    REPO_ROOT / "tests" / "v2_oracle" / "fixtures" / "configured-cli-authentic"
)
DRIVER = REPO_ROOT / "tools" / "v2_oracle" / "run_payload_demand_probe.sh"
SHA256_RE = re.compile(r"[0-9a-f]{64}")
HUNK_RE = re.compile(r"@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@")
MAX_OBJECT_BYTES = 16 << 20
MAX_FIXTURE_BYTES = 16 << 20


class FixtureError(ValueError):
    """The checked-in fixture is incomplete, corrupt, or internally inconsistent."""


@dataclass(frozen=True)
class Object:
    source: str
    destination: str
    sha256: str
    size: int
    source_url: str
    kind: str


@dataclass(frozen=True)
class VerifiedFixture:
    root: Path
    objects: tuple[Object, ...]
    bytes_by_source: dict[str, bytes]
    total_bytes: int
    inventory_sha256: str
    registry_entries: dict[str, bytes]


def _relative(value: Any, field: str) -> str:
    if not isinstance(value, str) or not value:
        raise FixtureError(f"{field} must be a non-empty relative path")
    path = Path(value)
    if path.is_absolute() or ".." in path.parts or path.as_posix() != value:
        raise FixtureError(f"{field} must be a canonical relative POSIX path")
    return value


def _load_manifest(root: Path) -> tuple[dict[str, Any], tuple[Object, ...]]:
    manifest = root / "fixture.toml"
    try:
        with manifest.open("rb") as stream:
            raw = tomllib.load(stream)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise FixtureError(f"cannot read fixture manifest: {error}") from error
    authentic = raw.get("authentic")
    if not isinstance(authentic, dict) or authentic.get("inventory_version") != 1:
        raise FixtureError("[authentic] inventory_version must be 1")
    rows = raw.get("objects")
    if not isinstance(rows, list) or not rows:
        raise FixtureError("fixture manifest must contain [[objects]] rows")
    objects: list[Object] = []
    sources: set[str] = set()
    destinations: set[str] = set()
    for index, row in enumerate(rows):
        if not isinstance(row, dict):
            raise FixtureError(f"objects[{index}] must be a table")
        source = _relative(row.get("source"), f"objects[{index}].source")
        destination = _relative(row.get("destination"), f"objects[{index}].destination")
        digest = row.get("sha256")
        size = row.get("size")
        source_url = row.get("source_url")
        kind = row.get("kind")
        if not isinstance(digest, str) or not SHA256_RE.fullmatch(digest):
            raise FixtureError(f"objects[{index}].sha256 must be lowercase SHA-256")
        if not isinstance(size, int) or isinstance(size, bool) or not 0 <= size <= MAX_OBJECT_BYTES:
            raise FixtureError(f"objects[{index}].size is outside the object limit")
        if not isinstance(source_url, str) or not source_url:
            raise FixtureError(f"objects[{index}].source_url must be non-empty")
        if kind not in {
            "root",
            "registry",
            "patch",
            "archive",
            "license",
            "registry_bundle",
            "registry_inventory",
        }:
            raise FixtureError(f"objects[{index}].kind is unsupported")
        if source in sources or destination in destinations:
            raise FixtureError("fixture source and destination paths must be unique")
        sources.add(source)
        destinations.add(destination)
        objects.append(Object(source, destination, digest, size, source_url, kind))
    return raw, tuple(objects)


def _read_object(root: Path, item: Object) -> bytes:
    path = root / item.source
    try:
        metadata = path.lstat()
    except OSError as error:
        raise FixtureError(f"missing fixture object {item.source}: {error}") from error
    if not stat.S_ISREG(metadata.st_mode):
        raise FixtureError(f"fixture object is not a regular file: {item.source}")
    if metadata.st_size != item.size:
        raise FixtureError(
            f"size mismatch for {item.source}: expected {item.size}, got {metadata.st_size}"
        )
    try:
        data = path.read_bytes()
    except OSError as error:
        raise FixtureError(f"cannot read fixture object {item.source}: {error}") from error
    digest = hashlib.sha256(data).hexdigest()
    if digest != item.sha256:
        raise FixtureError(
            f"hash mismatch for {item.source}: expected {item.sha256}, got {digest}"
        )
    return data


def _apply_unified_patch(original: bytes, patch: bytes) -> bytes:
    try:
        original_lines = original.decode("utf-8").splitlines(keepends=True)
        patch_lines = patch.decode("utf-8").splitlines(keepends=True)
    except UnicodeDecodeError as error:
        raise FixtureError(f"patch check requires UTF-8 text: {error}") from error
    result: list[str] = []
    cursor = 0
    index = 0
    hunks = 0
    while index < len(patch_lines):
        header = HUNK_RE.match(patch_lines[index].rstrip("\n"))
        if header is None:
            index += 1
            continue
        hunks += 1
        old_start = int(header.group(1)) - 1
        if old_start < cursor or old_start > len(original_lines):
            raise FixtureError("patch hunk has an invalid or overlapping source range")
        result.extend(original_lines[cursor:old_start])
        cursor = old_start
        index += 1
        while index < len(patch_lines) and not patch_lines[index].startswith("@@ "):
            line = patch_lines[index]
            if line.startswith(("--- ", "+++ ", "====")):
                break
            if line.startswith("\\ No newline at end of file"):
                index += 1
                continue
            if not line or line[0] not in " +-":
                break
            marker, content = line[0], line[1:]
            if marker in " -":
                if cursor >= len(original_lines) or original_lines[cursor] != content:
                    raise FixtureError("patch preimage does not match the archived file")
                cursor += 1
            if marker in " +":
                result.append(content)
            index += 1
    if hunks == 0:
        raise FixtureError("patch contains no unified diff hunks")
    result.extend(original_lines[cursor:])
    return "".join(result).encode("utf-8")


def _verify_patch(raw: dict[str, Any], verified: VerifiedFixture) -> None:
    checks = raw.get("patch_checks")
    if not isinstance(checks, list) or not checks:
        raise FixtureError("fixture manifest must contain [[patch_checks]] rows")
    for index, check in enumerate(checks):
        if not isinstance(check, dict):
            raise FixtureError(f"patch_checks[{index}] must be a table")
        prefix = f"patch_checks[{index}]"
        archive = _relative(check.get("archive"), f"{prefix}.archive")
        member = _relative(check.get("member"), f"{prefix}.member")
        patch = _relative(check.get("patch"), f"{prefix}.patch")
        expected = _relative(check.get("expected"), f"{prefix}.expected")
        preimage = check.get("preimage_sha256")
        if not isinstance(preimage, str) or not SHA256_RE.fullmatch(preimage):
            raise FixtureError(f"{prefix}.preimage_sha256 must be lowercase SHA-256")
        try:
            archive_path = verified.root / archive
            with tarfile.open(archive_path, mode="r:gz") as bundle:
                info = bundle.getmember(member)
                if not info.isfile() or info.size > MAX_OBJECT_BYTES:
                    raise FixtureError("patch archive member is not a bounded regular file")
                stream = bundle.extractfile(info)
                if stream is None:
                    raise FixtureError("patch archive member could not be opened")
                original = stream.read(MAX_OBJECT_BYTES + 1)
        except (OSError, KeyError, tarfile.TarError) as error:
            raise FixtureError(f"cannot verify patch archive member: {error}") from error
        if hashlib.sha256(original).hexdigest() != preimage:
            raise FixtureError("patch archive member preimage hash mismatch")
        try:
            patched = _apply_unified_patch(original, verified.bytes_by_source[patch])
            expected_bytes = verified.bytes_by_source.get(expected)
            if expected_bytes is None:
                expected_bytes = verified.registry_entries[expected]
        except KeyError as error:
            raise FixtureError(f"patch check references an unlisted object: {error}") from error
        if patched != expected_bytes:
            raise FixtureError("patched archive member does not equal pinned registry metadata")


def _verify_registry_bundle(
    raw: dict[str, Any], verified: VerifiedFixture
) -> dict[str, bytes]:
    config = raw.get("registry_bundle")
    if not isinstance(config, dict):
        raise FixtureError("fixture manifest must contain [registry_bundle]")
    archive_source = _relative(config.get("archive"), "registry_bundle.archive")
    inventory_source = _relative(config.get("inventory"), "registry_bundle.inventory")
    expected_entries = config.get("entry_count")
    if not isinstance(expected_entries, int) or isinstance(expected_entries, bool):
        raise FixtureError("registry_bundle.entry_count must be an integer")
    try:
        inventory_raw = json.loads(verified.bytes_by_source[inventory_source])
        archive_bytes = verified.bytes_by_source[archive_source]
    except (KeyError, json.JSONDecodeError, UnicodeDecodeError) as error:
        raise FixtureError(f"cannot read registry bundle inventory: {error}") from error
    if not isinstance(inventory_raw, list) or len(inventory_raw) != expected_entries:
        raise FixtureError("registry bundle inventory count mismatch")
    inventory: dict[str, dict[str, Any]] = {}
    demand_indices: set[int] = set()
    for index, row in enumerate(inventory_raw):
        if not isinstance(row, dict) or set(row) != {
            "demand_index",
            "path",
            "sha256",
            "size",
            "source_url",
        }:
            raise FixtureError(f"registry bundle inventory row {index} has invalid fields")
        path = _relative(row["path"], f"registry bundle row {index}.path")
        if not re.fullmatch(
            r"modules/[A-Za-z0-9_.+-]+/[A-Za-z0-9_.+-]+/(?:MODULE\.bazel|source\.json)",
            path,
        ):
            raise FixtureError(f"registry bundle inventory path is not BCR metadata: {path}")
        digest = row["sha256"]
        size = row["size"]
        source_url = row["source_url"]
        demand_index = row["demand_index"]
        if not isinstance(digest, str) or not SHA256_RE.fullmatch(digest):
            raise FixtureError(f"registry bundle row {index} has invalid SHA-256")
        if not isinstance(size, int) or isinstance(size, bool) or not 0 <= size <= MAX_OBJECT_BYTES:
            raise FixtureError(f"registry bundle row {index} has invalid size")
        if source_url != f"https://bcr.bazel.build/{path}":
            raise FixtureError(f"registry bundle row {index} has invalid source URL")
        if not isinstance(demand_index, int) or isinstance(demand_index, bool):
            raise FixtureError(f"registry bundle row {index} has invalid demand index")
        if path in inventory or demand_index in demand_indices:
            raise FixtureError("registry bundle inventory has duplicate path or demand index")
        inventory[path] = row
        demand_indices.add(demand_index)
    if demand_indices != set(range(1, expected_entries + 1)):
        raise FixtureError("registry bundle demand indices are not contiguous")

    entries: dict[str, bytes] = {}
    try:
        with tarfile.open(fileobj=io.BytesIO(archive_bytes), mode="r:") as bundle:
            members = bundle.getmembers()
            if len(members) != expected_entries:
                raise FixtureError("registry bundle archive count mismatch")
            for member in members:
                if (
                    not member.isfile()
                    or member.name not in inventory
                    or member.mode != 0o644
                    or member.uid != 0
                    or member.gid != 0
                    or member.mtime != 0
                ):
                    raise FixtureError(f"unsafe or unexpected registry bundle entry: {member.name}")
                stream = bundle.extractfile(member)
                if stream is None:
                    raise FixtureError(f"cannot open registry bundle entry: {member.name}")
                data = stream.read(MAX_OBJECT_BYTES + 1)
                row = inventory[member.name]
                if len(data) != row["size"] or hashlib.sha256(data).hexdigest() != row["sha256"]:
                    raise FixtureError(f"registry bundle entry mismatch: {member.name}")
                entries[member.name] = data
    except (OSError, tarfile.TarError) as error:
        raise FixtureError(f"cannot read registry bundle archive: {error}") from error
    if entries.keys() != inventory.keys():
        raise FixtureError("registry bundle archive and inventory paths differ")
    return entries


def verify_fixture(root: Path = DEFAULT_FIXTURE_ROOT) -> VerifiedFixture:
    root = root.resolve()
    raw, objects = _load_manifest(root)
    total = 0
    bytes_by_source: dict[str, bytes] = {}
    inventory = hashlib.sha256()
    for item in objects:
        data = _read_object(root, item)
        total += len(data)
        if total > MAX_FIXTURE_BYTES:
            raise FixtureError("fixture exceeds the total byte limit")
        bytes_by_source[item.source] = data
        inventory.update(
            (
                json.dumps(
                    {
                        "destination": item.destination,
                        "kind": item.kind,
                        "sha256": item.sha256,
                        "size": item.size,
                        "source": item.source,
                        "source_url": item.source_url,
                    },
                    sort_keys=True,
                    separators=(",", ":"),
                )
                + "\n"
            ).encode()
        )
    expected_total = raw["authentic"].get("total_source_bytes")
    if expected_total != total:
        raise FixtureError(
            f"fixture total mismatch: manifest declares {expected_total}, verified {total}"
        )
    verified = VerifiedFixture(
        root, objects, bytes_by_source, total, inventory.hexdigest(), {}
    )
    verified = VerifiedFixture(
        root,
        objects,
        bytes_by_source,
        total,
        inventory.hexdigest(),
        _verify_registry_bundle(raw, verified),
    )
    _verify_patch(raw, verified)
    return verified


def assemble(destination: Path, root: Path = DEFAULT_FIXTURE_ROOT) -> dict[str, Any]:
    verified = verify_fixture(root)
    destination = destination.absolute()
    if destination.exists() or destination.is_symlink():
        raise FixtureError(f"assembly destination already exists: {destination}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = destination.parent / f".{destination.name}.tmp-{uuid.uuid4().hex}"
    temporary.mkdir(mode=0o755)
    try:
        for item in verified.objects:
            if item.kind in {"registry_bundle", "registry_inventory"}:
                continue
            output = temporary / item.destination
            output.parent.mkdir(parents=True, exist_ok=True)
            with output.open("xb") as stream:
                stream.write(verified.bytes_by_source[item.source])
            output.chmod(0o644)
        for relative, data in verified.registry_entries.items():
            output = temporary / "registry" / relative
            if output.exists():
                raise FixtureError(f"registry bundle collides with an ordinary object: {relative}")
            output.parent.mkdir(parents=True, exist_ok=True)
            with output.open("xb") as stream:
                stream.write(data)
            output.chmod(0o644)
        registry = temporary / "registry" / "bazel_registry.json"
        registry.parent.mkdir(parents=True, exist_ok=True)
        registry.write_text(
            json.dumps(
                {"mirrors": [(destination / "mirror").as_uri()]},
                sort_keys=True,
                separators=(",", ":"),
            )
            + "\n",
            encoding="utf-8",
        )
        receipt = {
            "fixture": "configured-cli-authentic",
            "inventory_sha256": verified.inventory_sha256,
            "object_count": len(verified.objects),
            "registry_metadata_count": len(verified.registry_entries),
            "source_bytes": verified.total_bytes,
            "workspace": str(destination / "workspace"),
            "registry": (destination / "registry").as_uri(),
            "mirror": (destination / "mirror").as_uri(),
        }
        (temporary / "inventory.json").write_text(
            json.dumps(receipt, sort_keys=True, separators=(",", ":")) + "\n",
            encoding="utf-8",
        )
        temporary.rename(destination)
        return receipt
    except BaseException:
        shutil.rmtree(temporary, ignore_errors=True)
        raise


def _run_portable_proof(harness: Path, fixture_root: Path) -> int:
    harness = harness.resolve()
    if not harness.is_file() or not os.access(harness, os.X_OK):
        raise FixtureError(f"harness is not an executable file: {harness}")
    with tempfile.TemporaryDirectory(prefix="slug-configured-cli-", dir="/tmp") as scratch:
        assembled = Path(scratch) / "fixture"
        assembly = assemble(assembled, fixture_root)
        command = ["/bin/bash", str(DRIVER), "--portable-run", str(assembled), str(harness)]
        process = subprocess.Popen(
            command,
            cwd=REPO_ROOT,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            start_new_session=True,
        )
        try:
            stdout, stderr = process.communicate(timeout=15)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            stdout, stderr = process.communicate()
            raise FixtureError("portable proof exceeded the 15-second absolute ceiling")
        if stdout:
            sys.stdout.write(stdout)
        if stderr:
            sys.stderr.write(stderr)
        if process.returncode != 0:
            return process.returncode
        print(
            json.dumps(
                {"assembly": assembly, "proof": "F3-configured-source-closure"},
                sort_keys=True,
                separators=(",", ":"),
            )
        )
        return 0


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--fixture-root", type=Path, default=DEFAULT_FIXTURE_ROOT, help=argparse.SUPPRESS
    )
    subcommands = parser.add_subparsers(dest="command", required=True)
    subcommands.add_parser("verify", help="verify every checked-in object and patch")
    assemble_parser = subcommands.add_parser("assemble", help="assemble into a fresh directory")
    assemble_parser.add_argument("--output", type=Path, required=True)
    prove_parser = subcommands.add_parser("prove", help="assemble and run the bounded F3 proof")
    prove_parser.add_argument("--harness", type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.command == "verify":
            verified = verify_fixture(args.fixture_root)
            print(
                json.dumps(
                    {
                        "fixture": "configured-cli-authentic",
                        "inventory_sha256": verified.inventory_sha256,
                        "object_count": len(verified.objects),
                        "registry_metadata_count": len(verified.registry_entries),
                        "source_bytes": verified.total_bytes,
                    },
                    sort_keys=True,
                    separators=(",", ":"),
                )
            )
            return 0
        if args.command == "assemble":
            print(json.dumps(assemble(args.output, args.fixture_root), sort_keys=True))
            return 0
        return _run_portable_proof(args.harness, args.fixture_root)
    except FixtureError as error:
        print(f"configured CLI fixture error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
