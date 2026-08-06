"""Canonical `slug-fixture-payload-v1` packing and extraction."""

from __future__ import annotations

import hashlib
import os
import re
import stat
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

MAGIC = b"slug-fixture-payload-v1\n"
PATH_RE = re.compile(r"(?:[A-Za-z0-9._-]+/)*[A-Za-z0-9._-]+\Z")
DEVICE_NAMES = {"con", "prn", "aux", "nul", *(f"com{n}" for n in range(1, 10)), *(f"lpt{n}" for n in range(1, 10))}


@dataclass(frozen=True)
class Entry:
    path: str
    directory: bool
    body: bytes = b""


def _path_error(path: str) -> None:
    if not PATH_RE.fullmatch(path):
        raise ValueError(f"invalid payload path: {path!r}")
    for component in path.split("/"):
        if component in {".", ".."} or component.endswith("."):
            raise ValueError(f"invalid payload path component: {component!r}")
        if component.split(".", 1)[0].lower() in DEVICE_NAMES:
            raise ValueError(f"Windows device path component: {component!r}")


def _read_line(data: bytes, offset: int) -> tuple[bytes, int]:
    end = data.find(b"\n", offset)
    if end < 0:
        raise ValueError("truncated payload record")
    return data[offset:end], end + 1


def parse(payload: bytes) -> tuple[Entry, ...]:
    if not payload.startswith(MAGIC):
        raise ValueError("invalid payload header")
    offset = len(MAGIC)
    entries: list[Entry] = []
    prior: bytes | None = None
    folded: set[str] = set()
    directories: set[str] = set()
    file_count = byte_count = 0
    while True:
        line, offset = _read_line(payload, offset)
        fields = line.split(b"\t")
        if fields[0] == b"E":
            if len(fields) != 4 or any(not item.isdigit() for item in fields[1:]):
                raise ValueError("invalid payload footer")
            if tuple(map(int, fields[1:])) != (len([e for e in entries if e.directory]), file_count, byte_count):
                raise ValueError("payload footer count mismatch")
            if offset != len(payload):
                raise ValueError("trailing payload bytes")
            return tuple(entries)
        if fields[0] == b"D":
            if len(fields) != 3 or fields[1] != b"0755":
                raise ValueError("invalid directory record")
            raw_path = fields[2]
            body = b""
            directory = True
        elif fields[0] == b"F":
            if len(fields) != 5 or fields[1] != b"0644" or not fields[2].isdigit():
                raise ValueError("invalid file record")
            raw_path = fields[4]
            length = int(fields[2])
            if len(fields[3]) != 64 or any(c not in b"0123456789abcdef" for c in fields[3]):
                raise ValueError("invalid file digest")
            if offset + length > len(payload):
                raise ValueError("truncated file body")
            body = payload[offset : offset + length]
            offset += length
            if offset == len(payload) or payload[offset : offset + 1] != b"\n":
                raise ValueError("missing file body terminator")
            offset += 1
            if hashlib.sha256(body).hexdigest().encode() != fields[3]:
                raise ValueError("file digest mismatch")
            directory = False
            file_count += 1
            byte_count += length
        else:
            raise ValueError("unknown payload record")
        try:
            path = raw_path.decode("ascii")
        except UnicodeDecodeError as error:
            raise ValueError("non-ASCII payload path") from error
        _path_error(path)
        if prior is not None and raw_path <= prior:
            raise ValueError("payload paths are not globally sorted")
        if path.lower() in folded:
            raise ValueError("payload path case-fold collision")
        parent = path.rpartition("/")[0]
        if parent and parent not in directories:
            raise ValueError(f"payload parent is not a prior directory: {path}")
        prior = raw_path
        folded.add(path.lower())
        if directory:
            directories.add(path)
        entries.append(Entry(path, directory, body))


def encode(entries: Iterable[Entry]) -> bytes:
    entries = list(entries)
    ordered = sorted(entries, key=lambda entry: entry.path.encode("ascii"))
    if entries != ordered:
        entries = ordered
    result = bytearray(MAGIC)
    directories = files = byte_count = 0
    for entry in entries:
        _path_error(entry.path)
        raw_path = entry.path.encode("ascii")
        if entry.directory:
            result.extend(b"D\t0755\t" + raw_path + b"\n")
            directories += 1
        else:
            digest = hashlib.sha256(entry.body).hexdigest().encode()
            result.extend(b"F\t0644\t%d\t%s\t%s\n" % (len(entry.body), digest, raw_path))
            result.extend(entry.body)
            result.extend(b"\n")
            files += 1
            byte_count += len(entry.body)
    result.extend(f"E\t{directories}\t{files}\t{byte_count}\n".encode())
    return bytes(result)


def projection(payload: bytes, workspace: str) -> bytes:
    _path_error(workspace)
    selected = [entry for entry in parse(payload) if entry.path == workspace or entry.path.startswith(workspace + "/")]
    if not selected:
        raise KeyError(workspace)
    return encode(selected)


def extract(payload: bytes, workspace: str, root: Path) -> Path:
    """Validate then materialize one projection into an absent root."""
    entries = [entry for entry in parse(payload) if entry.path == workspace or entry.path.startswith(workspace + "/")]
    if not entries or entries[0].path != workspace or not entries[0].directory:
        raise ValueError(f"missing workspace root in payload: {workspace}")
    if root.exists() or root.is_symlink():
        raise FileExistsError(f"refusing to reuse extraction root: {root}")
    root.mkdir(mode=0o755, parents=False)
    try:
        for entry in entries[1:]:
            relative = entry.path.removeprefix(workspace + "/")
            destination = root.joinpath(*relative.split("/"))
            if destination.exists() or destination.is_symlink():
                raise FileExistsError(f"pre-existing extraction component: {destination}")
            if entry.directory:
                destination.mkdir(mode=0o755)
            else:
                with destination.open("xb") as handle:
                    handle.write(entry.body)
                if os.name == "posix":
                    os.chmod(destination, 0o644)
        if os.name == "posix":
            for entry in entries:
                destination = root if entry.path == workspace else root.joinpath(*entry.path.removeprefix(workspace + "/").split("/"))
                if entry.directory:
                    os.chmod(destination, 0o755)
        return root
    except Exception:
        # The caller owns cleanup; leave the failed root visible for diagnosis.
        raise


def _source_identity(value: os.stat_result) -> tuple[int, ...]:
    return (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )


def _source_mode(value: os.stat_result, expected: int, path: Path) -> None:
    if stat.S_IMODE(value.st_mode) != expected:
        raise ValueError(f"source mode is not {expected:04o}: {path}")


def _read_regular_at(directory_fd: int, name: str, display: Path) -> bytes:
    before = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
    if not stat.S_ISREG(before.st_mode):
        raise ValueError(f"source is not a regular file: {display}")
    _source_mode(before, 0o644, display)
    descriptor = os.open(name, os.O_RDONLY | os.O_NOFOLLOW, dir_fd=directory_fd)
    try:
        opened = os.fstat(descriptor)
        if _source_identity(opened) != _source_identity(before):
            raise ValueError(f"source changed before packing: {display}")
        chunks: list[bytes] = []
        while chunk := os.read(descriptor, 1024 * 1024):
            chunks.append(chunk)
        after = os.fstat(descriptor)
        path_after = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
        if not (
            _source_identity(before)
            == _source_identity(opened)
            == _source_identity(after)
            == _source_identity(path_after)
        ):
            raise ValueError(f"source changed while packing: {display}")
        return b"".join(chunks)
    finally:
        os.close(descriptor)


def pack(fixture_roots: Iterable[tuple[str, Path]]) -> bytes:
    if os.name != "posix" or not hasattr(os, "O_NOFOLLOW") or not hasattr(os, "O_DIRECTORY"):
        raise OSError("fixture payload packing requires POSIX no-follow directory descriptors")
    entries: list[Entry] = []
    for name, root in fixture_roots:
        _path_error(name)
        before = os.stat(root, follow_symlinks=False)
        if not stat.S_ISDIR(before.st_mode):
            raise ValueError(f"fixture root is not a real directory: {root}")
        _source_mode(before, 0o755, root)
        root_fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        expected_directories = {"": _source_identity(before)}
        try:
            for current, directory_names, file_names, directory_fd in os.fwalk(
                ".", topdown=True, follow_symlinks=False, dir_fd=root_fd
            ):
                relative = "" if current == "." else current.removeprefix("./")
                display = root if not relative else root / relative
                opened = os.fstat(directory_fd)
                expected = expected_directories.pop(relative, None)
                if expected is None or _source_identity(opened) != expected:
                    raise ValueError(f"source directory changed while packing: {display}")
                if not stat.S_ISDIR(opened.st_mode):
                    raise ValueError(f"source is not a directory: {display}")
                _source_mode(opened, 0o755, display)
                path = name if not relative else f"{name}/{relative}"
                entries.append(Entry(path, True))
                directory_names.sort()
                file_names.sort()
                for directory in directory_names:
                    source = display / directory
                    value = os.stat(directory, dir_fd=directory_fd, follow_symlinks=False)
                    if not stat.S_ISDIR(value.st_mode):
                        raise ValueError(f"source link or non-directory: {source}")
                    _source_mode(value, 0o755, source)
                    child = directory if not relative else f"{relative}/{directory}"
                    expected_directories[child] = _source_identity(value)
                for filename in file_names:
                    source = display / filename
                    entries.append(
                        Entry(
                            f"{path}/{filename}",
                            False,
                            _read_regular_at(directory_fd, filename, source),
                        )
                    )
                if _source_identity(os.fstat(directory_fd)) != _source_identity(opened):
                    raise ValueError(f"source directory changed while packing: {display}")
            if expected_directories:
                raise ValueError("source directory traversal was incomplete")
            after = os.stat(root, follow_symlinks=False)
            if _source_identity(after) != _source_identity(before):
                raise ValueError(f"fixture root changed while packing: {root}")
        finally:
            os.close(root_fd)
    encoded = encode(entries)
    parse(encoded)
    return encoded


def write_payload(destination: Path, fixture_roots: Iterable[tuple[str, Path]]) -> bytes:
    """Pack to a same-directory temporary file, then atomically replace destination."""
    payload = pack(fixture_roots)
    temporary = destination.with_name(destination.name + ".tmp")
    if temporary.exists() or temporary.is_symlink():
        raise FileExistsError(f"refusing to reuse payload temporary: {temporary}")
    with temporary.open("xb") as handle:
        handle.write(payload)
    os.replace(temporary, destination)
    return payload


def _main() -> None:
    import argparse

    parser = argparse.ArgumentParser(description="pack canonical V2 fixture payload")
    parser.add_argument("destination", type=Path)
    parser.add_argument("fixtures", type=Path)
    parser.add_argument("names", nargs="+")
    args = parser.parse_args()
    write_payload(
        args.destination,
        ((name, args.fixtures / name / "workspace") for name in args.names),
    )


if __name__ == "__main__":
    _main()
