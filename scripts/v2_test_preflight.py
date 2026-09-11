#!/usr/bin/env python3
"""Verify exact nonignored Rust test names without executing any tests."""
from __future__ import annotations

import argparse
from collections import Counter
from pathlib import Path
import subprocess
import sys
import tempfile
import time


TIMEOUT_SECONDS = 10
MAX_OUTPUT_BYTES = 1024 * 1024


def list_tests(harness: Path, *, ignored: bool, deadline: float) -> Counter[str]:
    command = [str(harness), "--list"]
    if ignored:
        command.append("--ignored")
    # Keep potentially large harness output off the Python heap. Both listings
    # share one deadline, and only bounded output is read back.
    with tempfile.TemporaryFile() as output:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise ValueError("test listing exceeded the 10-second deadline")
        try:
            result = subprocess.run(command, stdout=output, stderr=output,
                                    timeout=remaining, check=False)
        except subprocess.TimeoutExpired as exc:
            raise ValueError("test listing exceeded the 10-second deadline") from exc
        output.seek(0)
        data = output.read(MAX_OUTPUT_BYTES + 1)
    if len(data) > MAX_OUTPUT_BYTES:
        raise ValueError("test listing exceeded the 1 MiB output limit")
    if result.returncode:
        raise ValueError(f"test listing exited {result.returncode}: "
                         f"{data[:2000].decode('utf-8', errors='replace').strip()}")
    return Counter(line.removesuffix(": test") for line in data.decode("utf-8").splitlines()
                   if line.endswith(": test"))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("harness", type=Path, help="compiled Rust test executable")
    parser.add_argument("--exact", action="append", nargs="+", required=True,
                        metavar="FULL_TEST_NAME")
    args = parser.parse_args(argv)
    names = [name for group in args.exact for name in group]
    try:
        if len(names) != len(set(names)):
            raise ValueError("duplicate requested test name")
        harness = args.harness.resolve(strict=True)
        deadline = time.monotonic() + TIMEOUT_SECONDS
        available = list_tests(harness, ignored=False, deadline=deadline)
        ignored = list_tests(harness, ignored=True, deadline=deadline)
        failures = []
        for name in names:
            if available[name] != 1:
                failures.append(f"{name}: expected exactly one test, found {available[name]}")
            elif ignored[name]:
                failures.append(f"{name}: test is ignored")
        if failures:
            raise ValueError("\n".join(failures))
    except (OSError, UnicodeError, ValueError) as exc:
        print(f"test preflight: {exc}", file=sys.stderr)
        return 1
    print(f"Verified {len(names)} exact nonignored test(s); no tests executed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
