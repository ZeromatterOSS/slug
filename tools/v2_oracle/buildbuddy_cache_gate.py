#!/usr/bin/env python3
from __future__ import annotations

import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tools.v2_oracle_lib.buildbuddy_cache import GateError, REPO_ROOT, run_gate


def main(argv: list[str] | None = None) -> int:
    args = sys.argv[1:] if argv is None else argv
    manifest = Path(args[0]) if len(args) == 1 else REPO_ROOT / "tests/v2_oracle/buildbuddy_cache_targets.txt"
    try:
        result = run_gate(manifest)
    except GateError as error:
        result = {"schema_version": 1, "classification": error.classification}
    except Exception:
        result = {"schema_version": 1, "classification": "SANITIZER_REJECTED"}
    sys.stdout.write(json.dumps(result, sort_keys=True, separators=(",", ":")) + "\n")
    return 0 if result["classification"] == "PROVED_CACHE_ONLY" else 1


if __name__ == "__main__":
    raise SystemExit(main())
