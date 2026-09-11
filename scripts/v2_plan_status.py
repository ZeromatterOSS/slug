#!/usr/bin/env python3
"""Check the explicit current packet markers in the two scheduling documents."""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


PLAN_DIR = Path(__file__).resolve().parents[1] / "thoughts/shared/plans"


def packet(path: Path) -> str:
    markers = [line for line in path.read_text(encoding="utf-8").splitlines()
               if line.startswith("Packet:")]
    if len(markers) != 1:
        raise ValueError(f"{path}: expected exactly one Packet: line, found {len(markers)}")
    match = re.fullmatch(r"Packet: (WP-[A-Za-z0-9][A-Za-z0-9-]*)", markers[0])
    if match is None:
        raise ValueError(f"{path}: malformed Packet: line")
    return match[1]


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
    except (OSError, UnicodeError, ValueError) as exc:
        print(f"plan status: {exc}", file=sys.stderr)
        return 1
    print(f"Packet: {canonical}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
