#!/usr/bin/env python3
from __future__ import annotations
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path: sys.path.insert(0, str(ROOT))
from tools.v2_oracle_lib import buildbuddy_cache as gate

def main(argv: list[str] | None = None) -> int:
    args = sys.argv[1:] if argv is None else argv
    try:
        if args: raise gate.GateError()
        result = gate.normalize(gate.run_gate())
    except Exception: result = gate.record()
    sys.stdout.write(json.dumps(result, sort_keys=True, separators=(",", ":")) + "\n")
    return 0 if result["classification"] == "PROVED_CACHE_ONLY" else 1

if __name__ == "__main__": raise SystemExit(main())
