#!/usr/bin/env python3
from __future__ import annotations
import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
if str(REPO_ROOT) not in sys.path: sys.path.insert(0, str(REPO_ROOT))
from tools.v2_oracle_lib.buildbuddy_build_rbe import normalize, record, run_gate

def main(argv: list[str] | None = None) -> int:
    try: result = normalize(run_gate()) if not (sys.argv[1:] if argv is None else argv) else record()
    except Exception: result = record()
    sys.stdout.write(json.dumps(result, sort_keys=True, separators=(",", ":")) + "\n")
    return 0 if result["classification"] == "PROVED_BUILD_RBE" else 1

if __name__ == "__main__": raise SystemExit(main())
