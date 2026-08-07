"""Fail-closed one-prime parser-stage discriminator."""
from __future__ import annotations

import os
import secrets
import stat
import subprocess
import tempfile
from pathlib import Path
from typing import Any, Callable

from tools.v2_oracle_lib import buildbuddy_build_cache as cache
from tools.v2_oracle_lib import buildbuddy_build_cache_artifact_probe as lifecycle
from tools.v2_oracle_lib import buildbuddy_prime_diagnostic as cleanup

REPO_ROOT = cache.REPO_ROOT
MODE = "buildbuddy-build-cache-prime-stage-probe"
CLASSES = frozenset(("STAGE_RECORDED", "SANITIZER_REJECTED"))
STAGES = frozenset(("NOT_RECORDED", "PRECHECK_REJECTED", "SETUP_REJECTED", "PROCESS_NONZERO", "POST_RUN_ANCHOR_REJECTED", "BEP_DESCRIPTOR_REJECTED", "BEP_PHASE_REJECTED", "EXECUTION_DESCRIPTOR_REJECTED", "EXECUTION_SPAWN_REJECTED", "OUTPUT_REJECTED", "POST_PARSE_ANCHOR_REJECTED", "PRIME_SEMANTICS_REJECTED", "PRIME_READY"))
NONZERO_STAGES = frozenset(("PRECHECK_REJECTED", "SETUP_REJECTED", "PROCESS_NONZERO"))
ZERO_STAGES = STAGES - NONZERO_STAGES - {"NOT_RECORDED"}


class ProbeError(Exception):
    def __init__(self, stage: str): self.stage = stage if stage in STAGES else "NOT_RECORDED"


def record(classification: str = "SANITIZER_REJECTED", process: str = "NONZERO", stage: str = "NOT_RECORDED") -> dict[str, object]:
    valid = type(classification) is str and type(process) is str and type(stage) is str and classification == "STAGE_RECORDED" and ((process == "NONZERO" and stage in NONZERO_STAGES) or (process == "ZERO" and stage in ZERO_STAGES))
    if not valid:
        classification, process, stage = "SANITIZER_REJECTED", "NONZERO", "NOT_RECORDED"
    return {"schema_version": 1, "mode": MODE, "classification": classification, "process": process, "stage": stage}


def normalize(value: object) -> dict[str, object]:
    if type(value) is not dict or set(value) != set(record()) or type(value.get("schema_version")) is not int or value.get("schema_version") != 1 or any(type(value.get(key)) is not str for key in ("mode", "classification", "process", "stage")):
        return record()
    result = record(value["classification"], value["process"], value["stage"])
    return result if result == value else record()


def _ready(phase: dict[str, Any], spawns: dict[str, Any]) -> bool:
    return _semantic_stage(phase, spawns) == "PRIME_READY"


def _semantic_stage(phase: dict[str, Any], spawns: dict[str, Any]) -> str:
    if phase.get("_outcome") != "success": return "PRIME_OUTCOME_REJECTED"
    if phase.get("process_success_count") != 1: return "PRIME_PROCESS_COUNTER_REJECTED"
    if phase.get("build_finished_success_count") != 1: return "PRIME_BUILD_FINISHED_COUNTER_REJECTED"
    if phase.get("target_success_count") != 1: return "PRIME_TARGET_COUNTER_REJECTED"
    if phase.get("output_count") != 1: return "PRIME_OUTPUT_COUNTER_REJECTED"
    if phase.get("persistent_action_cache_hit_count") != 0: return "PRIME_PERSISTENT_CACHE_REJECTED"
    try:
        if not spawns.get("count") > 0: return "PRIME_ELIGIBLE_SET_REJECTED"
    except TypeError: return "PRIME_ELIGIBLE_SET_REJECTED"
    if spawns.get("cache_error_count") != 0: return "PRIME_CACHE_EXPECTATION_REJECTED"
    if spawns.get("status_error_count") != 0: return "PRIME_STATUS_EXPECTATION_REJECTED"
    if spawns.get("exit_error_count") != 0: return "PRIME_EXIT_EXPECTATION_REJECTED"
    if spawns.get("remote_cache_hit") != 0: return "PRIME_REMOTE_HIT_CLASS_REJECTED"
    if spawns.get("other") != 0: return "PRIME_OTHER_RUNNER_CLASS_REJECTED"
    try: partitioned = spawns.get("local") + spawns.get("worker") + spawns.get("linux_sandbox") == spawns.get("count")
    except TypeError: partitioned = False
    return "PRIME_READY" if partitioned else "PRIME_RUNNER_PARTITION_REJECTED"


def _private(path: Path) -> tuple[int, int]:
    cleanup._private_file(path)
    item = path.lstat()
    if not stat.S_ISREG(item.st_mode) or stat.S_IMODE(item.st_mode) != 0o600 or item.st_nlink != 1: raise OSError
    return item.st_dev, item.st_ino


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
            bep, execution = phase / "bep.json", phase / "execution.json"; identities = {path.name: _private(path) for path in (bep, execution)}
        except ProbeError: raise
        except Exception: raise ProbeError("SETUP_REJECTED") from None
        started = True
        with (phase / "stdout").open("xb") as out, (phase / "stderr").open("xb") as err:
            done = runner(cache.command("prime", bazel, output, bep, execution, secrets.token_hex(32)), cwd=REPO_ROOT, stdout=out, stderr=err, check=False)
        process = "ZERO" if type(done.returncode) is int and done.returncode == 0 else "NONZERO"
        if process != "ZERO": result = record("STAGE_RECORDED", process, "PROCESS_NONZERO")
        elif not cache._anchored(root, root_id, root_fd, "prime", phase_id, output_fd, output_id): result = record("STAGE_RECORDED", process, "POST_RUN_ANCHOR_REJECTED")
        else:
            try: bep_bytes = cache._private_bytes(phase_fd, bep.name, identities[bep.name])
            except Exception: result = record("STAGE_RECORDED", process, "BEP_DESCRIPTOR_REJECTED")
            else:
                try: phase_record = cache.phase_record(bep_bytes, b"", 0, "prime")
                except Exception: result = record("STAGE_RECORDED", process, "BEP_PHASE_REJECTED")
                else:
                    try: execution_bytes = cache._execution_bytes(phase_fd, execution.name)
                    except Exception: result = record("STAGE_RECORDED", process, "EXECUTION_DESCRIPTOR_REJECTED")
                    else:
                        try: spawns = cache.spawns(cache.parsed.json_sequence(execution_bytes), "prime")
                        except Exception: result = record("STAGE_RECORDED", process, "EXECUTION_SPAWN_REJECTED")
                        else:
                            try: phase_record["output_count"] = cache._outputs(output)
                            except Exception: result = record("STAGE_RECORDED", process, "OUTPUT_REJECTED")
                            else:
                                if not cache._anchored(root, root_id, root_fd, "prime", phase_id, output_fd, output_id): result = record("STAGE_RECORDED", process, "POST_PARSE_ANCHOR_REJECTED")
                                else: result = record("STAGE_RECORDED", process, "PRIME_READY" if _ready(phase_record, spawns) else "PRIME_SEMANTICS_REJECTED")
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
