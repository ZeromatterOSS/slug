from __future__ import annotations

import re
from pathlib import Path
from typing import Mapping

ANSI_RE = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")
TIMING_RE = re.compile(r"\b\d+(?:\.\d+)?s\b")
UUID_RE = re.compile(r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b")


def normalize_text(text: str, replacements: Mapping[str, str] | None = None) -> str:
    normalized = ANSI_RE.sub("", text)
    normalized = normalized.replace("\r\n", "\n").replace("\r", "\n")
    normalized = normalized.replace("\\", "/")
    if replacements:
        for raw, token in sorted(replacements.items(), key=lambda item: len(item[0]), reverse=True):
            if raw:
                normalized = normalized.replace(raw.replace("\\", "/"), token)
                normalized = normalized.replace(raw, token)
    normalized = TIMING_RE.sub("<duration>", normalized)
    normalized = UUID_RE.sub("<uuid>", normalized)
    return normalized.strip()


def path_replacements(**paths: Path | str | None) -> dict[str, str]:
    replacements: dict[str, str] = {}
    for name, value in paths.items():
        if value is None:
            continue
        path = str(value)
        replacements[path] = f"<{name}>"
        replacements[path.replace("\\", "/")] = f"<{name}>"
    return replacements