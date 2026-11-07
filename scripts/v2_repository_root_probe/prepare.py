#!/usr/bin/env python3
"""Freeze this fixed trusted Linux x86_64 experiment without launching it."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import sys


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for part in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(part)
    return digest.hexdigest()


def regular(path):
    resolved = path.resolve(strict=True)
    if not resolved.is_file():
        raise RuntimeError("expected a regular file: " + str(resolved))
    return resolved


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bwrap", type=Path, required=True)
    parser.add_argument("--python", type=Path, required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--plan", type=Path,
                        help="fresh plan file under output root; default: output-root/plan.json")
    parser.add_argument("--include-root", type=Path, default=Path("/usr/include"),
                        help="installed Linux x86_64 glibc header root")
    args = parser.parse_args()
    if sys.platform != "linux" or os.uname().machine != "x86_64":
        raise RuntimeError("fixed experiment requires Linux x86_64")
    if os.sysconf("SC_PAGE_SIZE") != 4096:
        raise RuntimeError("fixed experiment requires actual 4096-byte pages")
    interpreter = regular(args.python)
    if Path(sys.executable).resolve() != interpreter:
        raise RuntimeError("run prepare with the Python interpreter being frozen")
    bwrap = regular(args.bwrap)
    if not os.access(bwrap, os.X_OK):
        raise RuntimeError("bwrap is not executable")
    directory = Path(__file__).resolve().parent
    parent = regular(directory / "parent.py")
    helper = regular(directory / "trusted_helper.py")
    output_root = args.output_root.resolve()
    if output_root == Path("/"):
        raise RuntimeError("output root cannot be filesystem root")
    plan_path = (args.plan or output_root / "plan.json").resolve()
    if not plan_path.is_relative_to(output_root) or plan_path == output_root:
        raise RuntimeError("plan must be a fresh file below output root")
    headers = [regular(args.include_root / name) for name in (
        "linux/mount.h", "linux/nsfs.h", "x86_64-linux-gnu/asm/unistd_64.h",
        "x86_64-linux-gnu/bits/statfs.h")]
    pins = {str(path): sha(path) for path in (
        bwrap, interpreter, parent, helper, regular(Path(__file__)), *headers)}
    plan = {
        "format": "slug-repository-root-rolling-lease-probe-v1",
        "status": "FROZEN_UNEXECUTED_REQUIRES_INDEPENDENT_REVIEW",
        "revision": "portable packaging of fixed v5 rolling detached lease experiment",
        "bwrap": str(bwrap), "python": str(interpreter), "parent": str(parent),
        "helper": str(helper), "parent_only_output_root": str(output_root),
        "pins": pins, "uapi_and_abi_headers": [str(path) for path in headers],
        "bounds": {
            "initial_data_quota_bytes": 131072, "initial_inode_quota": 128,
            "data_quota_bytes": 65536, "inode_quota": 64, "page_bytes": 4096,
            "max_empty_inode_creations": 64, "max_extra_quota_page_attempts": 32,
            "phases": 3, "no_new_launch_after_seconds": 55,
            "whole_seconds": 60, "per_phase_live_seconds": 17,
            "per_phase_cleanup_seconds": 2.5, "per_phase_ceiling_seconds": 20,
            "protocol_bytes_each": 8192, "stderr_bytes_each": 65536},
        "expected_outputs": ["receipt.json", "first.stderr", "first.protocol.bin",
                             "second.stderr", "second.protocol.bin",
                             "third.stderr", "third.protocol.bin"],
        "scope": {"trusted_fixed_helper_only": True, "rootfd_lifetime": True,
                  "rolling_detached_lease": True, "same_superblock_quota_shrink": True,
                  "original_execution": False, "production_activation": False,
                  "confinement_claim": False, "repeated_anchor_cloning": False},
        "protocol": {
            "first": "own tmpfs; send root,detached clone,actual owning userns",
            "second": "reuse owner; receive root+lease after READY; attach directly; "
                      "shrink quota; mint next clone from attached root; export root+next lease",
            "third": "receive root+next lease; attach directly; verify quota and overwrite sentinel",
            "environment": "empty child exec env; clearenv; helper records/clears runtime startup keys",
            "socket": "AF_UNIX SOCK_SEQPACKET FD1; exact nonce-bound SCM_RIGHTS/ACK",
            "capabilities": "trusted fixed helper retains CAP_SYS_ADMIN inside owned userns only",
            "no_fallback": True},
        "qualification": "Page-aligned fixed data bytes and inode counters are separate; "
                         "not untrusted execution, confinement, source ownership or workload memory proof.",
        "future_run_requires_explicit_plan_sha": True,
    }
    plan_path.parent.mkdir(parents=True, exist_ok=True)
    with plan_path.open("x", encoding="utf-8") as destination:
        destination.write(json.dumps(plan, sort_keys=True, indent=2) + "\n")
    print(json.dumps({"plan": str(plan_path), "sha256": sha(plan_path)}, sort_keys=True))


if __name__ == "__main__":
    main()
