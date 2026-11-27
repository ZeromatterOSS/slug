#!/usr/bin/env python3
"""Check current packet markers and the active routing log budget."""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
PLAN_DIR = REPO_ROOT / "thoughts/shared/plans"
ROUTING_LOG = REPO_ROOT / ".codex/skills/slug-agent-orchestration/references/routing-log.md"
ROUTING_LOG_BYTES = 16 * 1024
ROUTING_LOG_LINES = 250
ROUTING_LOG_ROWS = 20


def packet(path: Path) -> str:
    markers = [line for line in path.read_text(encoding="utf-8").splitlines()
               if line.startswith("Packet:")]
    if len(markers) != 1:
        raise ValueError(f"{path}: expected exactly one Packet: line, found {len(markers)}")
    match = re.fullmatch(r"Packet: (WP-[A-Za-z0-9][A-Za-z0-9-]*)", markers[0])
    if match is None:
        raise ValueError(f"{path}: malformed Packet: line")
    return match[1]


def check_routing_log(path: Path) -> None:
    with path.open("rb") as log:
        encoded = log.read(ROUTING_LOG_BYTES + 1)
    if len(encoded) > ROUTING_LOG_BYTES:
        raise ValueError(f"{path}: routing log exceeds {ROUTING_LOG_BYTES} bytes")
    lines = encoded.decode("utf-8").splitlines()
    if len(lines) > ROUTING_LOG_LINES:
        raise ValueError(f"{path}: routing log exceeds {ROUTING_LOG_LINES} lines")
    rows = sum(bool(re.match(r"\|\s*\d{4}-\d{2}-\d{2}\s*\|", line)) for line in lines)
    if rows > ROUTING_LOG_ROWS:
        raise ValueError(f"{path}: routing log exceeds {ROUTING_LOG_ROWS} packet rows")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--canonical", type=Path,
                        default=PLAN_DIR / "2026-06-26-slug-v2-clean-restart.md")
    parser.add_argument("--manifest", type=Path,
                        default=PLAN_DIR / "slug-v2-subplans/current-packet.md")
    args = parser.parse_args(argv)
    try:
        canonical, manifest = packet(args.canonical), packet(args.manifest)
        if canonical != manifest:
            raise ValueError(f"packet mismatch: canonical={canonical}; manifest={manifest}")
        check_routing_log(ROUTING_LOG)
    except (OSError, UnicodeError, ValueError) as exc:
        print(f"plan status: {exc}", file=sys.stderr)
        return 1
    print(f"Packet: {canonical}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
