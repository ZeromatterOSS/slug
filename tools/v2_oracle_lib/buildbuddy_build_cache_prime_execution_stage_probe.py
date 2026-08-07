"""Fail-closed one-prime execution-log-only stage discriminator."""
from __future__ import annotations

import os
import secrets
import stat
import subprocess
import tempfile
from pathlib import Path
from typing import Any, Callable, Iterable

from tools.v2_oracle_lib import buildbuddy_build_cache as cache
from tools.v2_oracle_lib import buildbuddy_build_cache_artifact_probe as lifecycle
from tools.v2_oracle_lib import buildbuddy_prime_diagnostic as cleanup

REPO_ROOT = cache.REPO_ROOT
MODE = "buildbuddy-build-cache-prime-execution-stage-probe"
CLASSES = frozenset(("STAGE_RECORDED", "SANITIZER_REJECTED"))
STAGES = frozenset(("NOT_RECORDED", "PRECHECK_REJECTED", "SETUP_REJECTED", "PROCESS_NONZERO", "POST_RUN_ANCHOR_REJECTED", "EXECUTION_DESCRIPTOR_REJECTED", "EXECUTION_STREAM_REJECTED", "EXECUTION_SPAWN_REJECTED", "POST_PARSE_ANCHOR_REJECTED", "EXECUTION_READY"))
NONZERO_STAGES = frozenset(("PRECHECK_REJECTED", "SETUP_REJECTED", "PROCESS_NONZERO"))
ZERO_STAGES = STAGES - NONZERO_STAGES - {"NOT_RECORDED"}


class ProbeError(Exception):
    def __init__(self, stage: str): self.stage = stage if stage in STAGES else "NOT_RECORDED"


def record(classification: str = "SANITIZER_REJECTED", process: str = "NONZERO", stage: str = "NOT_RECORDED") -> dict[str, object]:
    valid = type(classification) is str and type(process) is str and type(stage) is str and classification == "STAGE_RECORDED" and ((process == "NONZERO" and stage in NONZERO_STAGES) or (process == "ZERO" and stage in ZERO_STAGES))
    if not valid: classification, process, stage = "SANITIZER_REJECTED", "NONZERO", "NOT_RECORDED"
    return {"schema_version": 1, "mode": MODE, "classification": classification, "process": process, "stage": stage}


def normalize(value: object) -> dict[str, object]:
    if type(value) is not dict or set(value) != set(record()) or type(value.get("schema_version")) is not int or value.get("schema_version") != 1 or any(type(value.get(key)) is not str for key in ("mode", "classification", "process", "stage")):
        return record()
    result = record(value["classification"], value["process"], value["stage"])
    return result if result == value else record()


def _private(path: Path) -> None:
    cleanup._private_file(path)
    item = path.lstat()
    if not stat.S_ISREG(item.st_mode) or stat.S_IMODE(item.st_mode) != 0o600 or item.st_nlink != 1: raise OSError


def _entries(data: bytes) -> Iterable[dict[str, Any]]:
    try: entries = iter(cache.parsed.json_sequence(data))
    except Exception: raise ProbeError("EXECUTION_STREAM_REJECTED") from None
    while True:
        try: yield next(entries)
        except StopIteration: return
        except Exception: raise ProbeError("EXECUTION_STREAM_REJECTED") from None


def run_probe(bazel: str = "bazel", runner: Callable[..., subprocess.CompletedProcess[bytes]] = subprocess.run) -> dict[str, object]:
    old, root, parent_fd, root_fd, phase_fd, output_fd, root_id, phase_id, output_id, result, started = os.umask(0o077), None, None, None, None, None, None, None, None, record(), False
    try:
        if not (cleanup._clean_git() and cleanup._no_slugd()): raise ProbeError("PRECHECK_REJECTED")
        try:
            root = Path(tempfile.mkdtemp(prefix="slug-buildbuddy-prime-"))
            if stat.S_IMODE(root.stat().st_mode) != 0o700: raise OSError
            try: root.resolve().relative_to(REPO_ROOT.resolve()); raise OSError
            except ValueError: pass
            parent_fd = os.open(root.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW); root_fd = os.open(root.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd)
            item = os.fstat(root_fd); root_id = item.st_dev, item.st_ino
            phase = root / "prime"; phase.mkdir(); phase_fd = os.open("prime", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=root_fd)
            item = os.fstat(phase_fd); phase_id = item.st_dev, item.st_ino
            output = phase / "output"; output.mkdir(); output_fd = os.open("output", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=phase_fd)
            item = os.fstat(output_fd); output_id = item.st_dev, item.st_ino
            bep, execution = phase / "bep.json", phase / "execution.json"; _private(bep); _private(execution)
        except ProbeError: raise
        except Exception: raise ProbeError("SETUP_REJECTED") from None
        started = True
        with (phase / "stdout").open("xb") as out, (phase / "stderr").open("xb") as err:
            done = runner(cache.command(bazel, output, bep, execution, secrets.token_hex(32)), cwd=REPO_ROOT, stdout=out, stderr=err, check=False)
        process = "ZERO" if type(done.returncode) is int and done.returncode == 0 else "NONZERO"
        if process != "ZERO": result = record("STAGE_RECORDED", process, "PROCESS_NONZERO")
        elif not cache._anchored(root, root_id, root_fd, "prime", phase_id, output_fd, output_id): result = record("STAGE_RECORDED", process, "POST_RUN_ANCHOR_REJECTED")
        else:
            try: execution = cache._execution_bytes(phase_fd, execution.name)
            except Exception: result = record("STAGE_RECORDED", process, "EXECUTION_DESCRIPTOR_REJECTED")
            else:
                try: cache.spawns(_entries(execution), "prime")
                except ProbeError as error: result = record("STAGE_RECORDED", process, error.stage)
                except Exception: result = record("STAGE_RECORDED", process, "EXECUTION_SPAWN_REJECTED")
                else: result = record("STAGE_RECORDED", process, "EXECUTION_READY") if cache._anchored(root, root_id, root_fd, "prime", phase_id, output_fd, output_id) else record("STAGE_RECORDED", process, "POST_PARSE_ANCHOR_REJECTED")
    except ProbeError as error: result = record("STAGE_RECORDED", "NONZERO", error.stage)
    except Exception: result = record()
    finally:
        os.umask(old); okay = True
        ready = all(value is not None for value in (root, parent_fd, root_fd, phase_fd, output_fd, root_id, phase_id, output_id))
        if started:
            if ready and cache._anchored(root, root_id, root_fd, "prime", phase_id, output_fd, output_id): okay = cache._shutdown(bazel, root / "prime" / "output", runner) and cache._anchored(root, root_id, root_fd, "prime", phase_id, output_fd, output_id)
            else: okay = False
        if output_fd is not None: os.close(output_fd)
        if phase_fd is not None: os.close(phase_fd)
        if root is not None:
            if parent_fd is not None and root_fd is not None and root_id is not None: okay &= lifecycle._remove_original(parent_fd, root_fd, root_id)
            else: okay = False
            if parent_fd is not None: okay &= cache._remove_reserved(root, parent_fd)
            else: okay = False
        if root_fd is not None: os.close(root_fd)
        if parent_fd is not None: os.close(parent_fd)
        if not (cleanup._clean_git() and cleanup._no_slugd()): okay = False
        if not okay: result = record()
    return result
