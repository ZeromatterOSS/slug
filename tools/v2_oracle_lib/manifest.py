from __future__ import annotations

import hashlib
import os
import stat
from pathlib import Path
from typing import Any


def _file_digest(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def _mode(path: Path, follow_symlinks: bool = False) -> str:
    mode = path.stat().st_mode if follow_symlinks else path.lstat().st_mode
    return oct(stat.S_IMODE(mode))


def _entry(root: Path, path: Path) -> dict[str, Any]:
    rel = path.relative_to(root).as_posix()
    if path.is_symlink():
        return {
            "path": rel,
            "type": "symlink",
            "mode": _mode(path),
            "symlink_target": os.readlink(path),
            "digest": None,
            "size": None,
        }
    if path.is_dir():
        return {
            "path": rel,
            "type": "directory",
            "mode": _mode(path),
            "symlink_target": None,
            "digest": None,
            "size": None,
        }
    return {
        "path": rel,
        "type": "file",
        "mode": _mode(path),
        "symlink_target": None,
        "digest": _file_digest(path),
        "size": path.stat().st_size,
    }


def collect_manifest(root: Path) -> list[dict[str, Any]]:
    if not root.exists():
        return []
    if root.is_file() or root.is_symlink():
        base = root.parent
        return [_entry(base, root)]
    entries = [_entry(root, path) for path in root.rglob("*")]
    return sorted(entries, key=lambda item: (item["path"], item["type"]))


def collect_manifest_roots(workspace: Path, roots: list[str] | tuple[str, ...]) -> list[dict[str, Any]]:
    merged: list[dict[str, Any]] = []
    for root_name in roots:
        root = Path(root_name)
        if not root.is_absolute():
            root = workspace / root
        for entry in collect_manifest(root):
            entry = dict(entry)
            entry["root"] = root_name
            merged.append(entry)
    return sorted(merged, key=lambda item: (item["root"], item["path"], item["type"]))