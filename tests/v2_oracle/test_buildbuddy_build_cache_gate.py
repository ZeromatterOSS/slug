from __future__ import annotations
import json
import os
import stat
import subprocess
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from io import StringIO
from pathlib import Path
from unittest import mock

from tools.v2_oracle import buildbuddy_build_cache_gate as cli
from tools.v2_oracle_lib import buildbuddy_build_cache as gate

ROOT = Path(__file__).resolve().parents[2]
DIGEST, OTHER = "d" * 64, "e" * 64

def seq(values: list[dict[str, object]]) -> bytes:
    return b"\n".join(json.dumps(value, indent=2).encode() for value in values)

class BuildBuddyBuildCacheGateTest(unittest.TestCase):
    def test_command_is_exact_minimal_build_and_nonce(self) -> None:
        argv = gate.command("bazel", Path("/p/out"), Path("/p/bep"), Path("/p/log"), "a" * 64)
        self.assertEqual(["bazel", "--output_base=/p/out", "build", "--config=buildbuddy-cache"], argv[:4])
        self.assertEqual(gate.LABEL, argv[-1]); self.assertEqual(1, argv.count("--@rules_rust//rust/toolchain/channel=nightly"))
        self.assertEqual({"--remote_executor=", "--bes_backend=", "--bes_results_url=", "--disk_cache="}, set(x for x in argv if x in {"--remote_executor=", "--bes_backend=", "--bes_results_url=", "--disk_cache="}))
        for forbidden in ("remote_cache", "remote_instance", "spawn_strategy", "test_", "publish_all", "remote_accept", "remote_upload", "cache_async", "test_env"):
            self.assertFalse(any(forbidden in x for x in argv))
        with self.assertRaises(gate.GateError): gate.command("bazel", Path("/p"), Path("/b"), Path("/e"), "secret")

    def test_json_parser_and_spawns_are_strict(self) -> None:
        event = {"runner": "local", "cacheable": True, "remoteCacheable": True, "digest": {"hash": DIGEST, "sizeBytes": 1}, "cacheHit": False, "status": "", "exitCode": 0}
        summary = gate.spawns([event], "prime")
        self.assertEqual(1, summary["local"]); self.assertEqual(0, summary["cache_error_count"])
        event["runner"] = "worker"; self.assertEqual(1, gate.spawns([event], "prime")["worker"])
        event["runner"] = "linux-sandbox"; self.assertEqual(1, gate.spawns([event], "prime")["linux_sandbox"])
        event["runner"] = "private-runner"; self.assertEqual(1, gate.spawns([event], "prime")["other"])
        with self.assertRaises(Exception): list(gate.parsed.json_sequence(b"[]"))

    def test_classification_eight_fixed_classes(self) -> None:
        prime, replay = self._records()
        self.assertEqual("PROVED_BUILD_CACHE", gate.classify(prime, replay))
        cases = (("remote", "REMOTE_UNAVAILABLE"), ("command", "COMMAND_LINE_FAILURE"), ("target", "TARGET_FAILURE"))
        for outcome, expected in cases:
            p, r = self._records(); p["_outcome"] = outcome; self.assertEqual(expected, gate.classify(p, r))
        p, r = self._records(); r["eligible_spawns"]["digest_multiset_sha256"] = "different"; self.assertEqual("CACHE_MISS_OR_MIXED_REPLAY", gate.classify(p, r))
        p, r = self._records(); p["eligible_spawns"]["count"] = 0; self.assertEqual("EVIDENCE_INCOMPLETE", gate.classify(p, r))
        self.assertEqual({"PROVED_BUILD_CACHE", "CONFIG_DRIFT", "REMOTE_UNAVAILABLE", "COMMAND_LINE_FAILURE", "TARGET_FAILURE", "CACHE_MISS_OR_MIXED_REPLAY", "EVIDENCE_INCOMPLETE", "SANITIZER_REJECTED"}, gate.CLASSES)

    def test_phase_record_counts_target_and_hides_private_bep(self) -> None:
        record = gate.phase_record(self._bep(), self._execution("prime"), 0, "prime")
        self.assertEqual(1, record["target_success_count"]); self.assertEqual("success", record["_outcome"])
        bad = seq([{"id": {"buildFinished": {}}, "finished": {"exitCode": {"name": "REMOTE_ERROR", "code": 1}, "failureDetail": {"message": "token=/private"}}}])
        self.assertEqual("remote", gate.phase_record(bad, b"", 1, "prime")["_outcome"])
        for value in (None, False, True, "0", -1):
            with self.subTest(value=value):
                invalid = seq([{"id": {"buildFinished": {}}, "finished": {"exitCode": {"name": "SUCCESS", "code": value}}}])
                with self.assertRaises(gate.GateError): gate.phase_record(invalid, b"", 0, "prime")
        class IntSubclass(int): pass
        with self.assertRaises(gate.GateError): gate._count(IntSubclass(0))
        self.assertNotIn("private", json.dumps(record))
        with self.assertRaises(gate.GateError): gate.phase_record(self._bep() + self._bep(), self._execution("prime"), 0, "prime")

    def test_outputs_require_one_regular_executable_suffix(self) -> None:
        root = Path(tempfile.mkdtemp()); self.addCleanup(lambda: gate.cleanup._remove_root(root) if root.exists() else None)
        output = root / "execroot" / "x" / "bin" / "app" / "slug_cli_v2"; output.mkdir(parents=True)
        target = output / "slug"; target.write_bytes(b"x"); target.chmod(0o700)
        self.assertEqual(1, gate._outputs(root)); target.chmod(0o600); self.assertEqual(0, gate._outputs(root))

    def test_run_is_private_shared_nonce_and_cleanup_safe(self) -> None:
        calls: list[list[str]] = []; roots: list[Path] = []
        def runner(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            calls.append(argv)
            if argv[-1] == "shutdown": return subprocess.CompletedProcess(argv, 0)
            self.assertEqual("build", argv[2]); phase = "prime" if len([x for x in calls if len(x) > 2 and x[2] == "build"]) == 1 else "replay"
            base = Path(argv[1].split("=", 1)[1]); roots.append(base.parents[1])
            bep = Path(next(x.split("=", 1)[1] for x in argv if x.startswith("--build_event_json_file=")))
            execution = Path(next(x.split("=", 1)[1] for x in argv if x.startswith("--execution_log_json_file=")))
            target = base / "execroot" / "x" / "bin" / "app" / "slug_cli_v2" / "slug"; target.parent.mkdir(parents=True); target.write_bytes(b"x"); target.chmod(0o700)
            bep.write_bytes(self._bep()); execution.write_bytes(self._execution(phase)); kwargs["stderr"].write(b"token=/private")
            self.assertEqual(0o600, stat.S_IMODE(os.fstat(kwargs["stderr"].fileno()).st_mode)); return subprocess.CompletedProcess(argv, 0)
        with mock.patch.object(gate, "_clean", return_value=True): result = gate.run_gate(runner=runner)
        builds = [x for x in calls if len(x) > 2 and x[2] == "build"]
        nonces = [next(x for x in argv if "NONCE=" in x) for argv in builds]
        self.assertEqual(2, len(builds)); self.assertEqual(nonces[0], nonces[1]); self.assertRegex(nonces[0], r"[0-9a-f]{64}$")
        self.assertEqual("PROVED_BUILD_CACHE", result["classification"]); self.assertTrue(all(not path.exists() for path in roots)); self.assertNotIn("private", json.dumps(result))
        self.assertEqual({"schema_version", "mode", "classification", "prime", "replay"}, set(result))
        self.assertEqual({"process_success_count", "build_finished_success_count", "target_success_count", "output_count", "persistent_action_cache_hit_count", "eligible_spawns"}, set(result["prime"]))
        self.assertEqual({"count", "digest_multiset_sha256", "cache_error_count", "status_error_count", "exit_error_count", "local", "worker", "linux_sandbox", "remote_cache_hit", "other"}, set(result["prime"]["eligible_spawns"]))

    def test_closed_record_cli_and_lifecycle_failures(self) -> None:
        fallback = gate.record("CONFIG_DRIFT")
        self.assertEqual("CONFIG_DRIFT", fallback["classification"])
        self.assertEqual({"schema_version", "mode", "classification", "prime", "replay"}, set(fallback))
        stdout, stderr = StringIO(), StringIO()
        with mock.patch.object(cli, "run_gate", side_effect=RuntimeError("token=/private")), redirect_stdout(stdout), redirect_stderr(stderr): self.assertEqual(1, cli.main([]))
        self.assertEqual("", stderr.getvalue()); self.assertEqual("SANITIZER_REJECTED", json.loads(stdout.getvalue())["classification"])
        hostile = gate.record("PROVED_BUILD_CACHE"); hostile["prime"]["eligible_spawns"] = {"private": "/private/token"}
        stdout, stderr = StringIO(), StringIO()
        with mock.patch.object(cli, "run_gate", return_value=hostile), redirect_stdout(stdout), redirect_stderr(stderr): self.assertEqual(1, cli.main([]))
        self.assertEqual("", stderr.getvalue()); self.assertNotIn("private", stdout.getvalue()); self.assertEqual("SANITIZER_REJECTED", json.loads(stdout.getvalue())["classification"])
        class StringSubclass(str):
            def __hash__(self) -> int: return hash("PROVED_BUILD_CACHE")
            def __eq__(self, other: object) -> bool: return other == "PROVED_BUILD_CACHE"
        hostile = gate.record("PROVED_BUILD_CACHE"); hostile["classification"] = StringSubclass("/private/class")
        stdout, stderr = StringIO(), StringIO()
        with mock.patch.object(cli, "run_gate", return_value=hostile), redirect_stdout(stdout), redirect_stderr(stderr): self.assertEqual(1, cli.main([]))
        self.assertEqual("", stderr.getvalue()); self.assertNotIn("private", stdout.getvalue()); self.assertEqual("SANITIZER_REJECTED", json.loads(stdout.getvalue())["classification"])
        with mock.patch.object(gate, "_clean", return_value=False): self.assertEqual("CONFIG_DRIFT", gate.run_gate(runner=lambda *a, **k: subprocess.CompletedProcess([], 0))["classification"])

    def test_evidence_symlink_and_swap_are_rejected_without_reads(self) -> None:
        descriptor, name = tempfile.mkstemp(); os.close(descriptor)
        outside = Path(name); self.addCleanup(outside.unlink); outside.write_text("token=/private")
        for attacked, method in (("bep.json", "symlink"), ("bep.json", "replace"), ("execution.json", "swap")):
            with self.subTest(attacked=attacked):
                def runner(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]:
                    if argv[-1] == "shutdown": return subprocess.CompletedProcess(argv, 0)
                    paths = {name: Path(next(x.split("=", 1)[1] for x in argv if x.startswith(flag))) for name, flag in (("bep.json", "--build_event_json_file="), ("execution.json", "--execution_log_json_file="))}
                    paths["bep.json"].write_bytes(self._bep()); paths["execution.json"].write_bytes(self._execution("prime"))
                    target = paths[attacked]; target.unlink()
                    if method == "symlink": target.symlink_to(outside)
                    elif method == "replace": target.write_bytes(self._bep()); target.chmod(0o600)
                    else: os.link(outside, target)
                    return subprocess.CompletedProcess(argv, 0)
                with mock.patch.object(gate, "_clean", return_value=True): result = gate.run_gate(runner=runner)
                self.assertEqual("EVIDENCE_INCOMPLETE", result["classification"]); self.assertNotIn("private", json.dumps(result))

    def test_phase_directory_swap_rejects_and_skips_replacement_shutdown(self) -> None:
        shutdowns: list[Path] = []
        def runner(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]:
            if argv[-1] == "shutdown": shutdowns.append(Path(argv[2].split("=", 1)[1])); return subprocess.CompletedProcess(argv, 0)
            base = Path(argv[1].split("=", 1)[1]); phase = base.parent
            bep = Path(next(x.split("=", 1)[1] for x in argv if x.startswith("--build_event_json_file=")))
            execution = Path(next(x.split("=", 1)[1] for x in argv if x.startswith("--execution_log_json_file=")))
            bep.write_bytes(self._bep()); execution.write_bytes(self._execution("prime"))
            original = phase.with_name(phase.name + "-original"); phase.rename(original); phase.mkdir(); (phase / "output").mkdir()
            return subprocess.CompletedProcess(argv, 0)
        with mock.patch.object(gate, "_clean", return_value=True): result = gate.run_gate(runner=runner)
        self.assertNotEqual("PROVED_BUILD_CACHE", result["classification"]); self.assertEqual([], shutdowns)

    def test_cleanup_failure_is_fail_closed(self) -> None:
        with mock.patch.object(gate, "_clean", return_value=True), mock.patch.object(gate, "_remove_original", return_value=False):
            result = gate.run_gate(runner=lambda argv, **_: subprocess.CompletedProcess(argv, 0) if argv[-1] == "shutdown" else (_ for _ in ()).throw(OSError()))
        self.assertEqual("SANITIZER_REJECTED", result["classification"])

    def test_retained_replaced_and_empty_execution_reach_the_existing_parser(self) -> None:
        for kind in ("retained", "replaced", "empty"):
            with self.subTest(kind=kind):
                def runner(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]:
                    if argv[-1] == "shutdown": return subprocess.CompletedProcess(argv, 0)
                    phase = "replay" if "replay" in argv[1] else "prime"
                    bep = Path(next(x.split("=", 1)[1] for x in argv if x.startswith("--build_event_json_file=")))
                    execution = Path(next(x.split("=", 1)[1] for x in argv if x.startswith("--execution_log_json_file=")))
                    target = Path(argv[1].split("=", 1)[1]) / "execroot/x/bin/app/slug_cli_v2/slug"; target.parent.mkdir(parents=True); target.write_bytes(b"x"); target.chmod(0o700)
                    bep.write_bytes(self._bep())
                    if kind == "replaced": execution.unlink()
                    execution.write_bytes(self._execution(phase) if kind != "empty" else b""); execution.chmod(0o600)
                    return subprocess.CompletedProcess(argv, 0)
                with mock.patch.object(gate, "_clean", return_value=True): result = gate.run_gate(runner=runner)
                self.assertEqual("EVIDENCE_INCOMPLETE" if kind == "empty" else "PROVED_BUILD_CACHE", result["classification"])
                self.assertNotIn("private", json.dumps(result))

    def test_execution_link_mode_and_directory_are_rejected(self) -> None:
        descriptor, name = tempfile.mkstemp(); os.close(descriptor)
        outside = Path(name); self.addCleanup(lambda: outside.unlink(missing_ok=True)); outside.write_bytes(b"token=/private")
        for attack in ("symlink", "hardlink", "mode", "directory"):
            with self.subTest(attack=attack):
                def runner(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]:
                    if argv[-1] == "shutdown": return subprocess.CompletedProcess(argv, 0)
                    bep = Path(next(x.split("=", 1)[1] for x in argv if x.startswith("--build_event_json_file=")))
                    execution = Path(next(x.split("=", 1)[1] for x in argv if x.startswith("--execution_log_json_file=")))
                    bep.write_bytes(self._bep()); execution.unlink()
                    if attack == "symlink": execution.symlink_to(outside)
                    elif attack == "hardlink": os.link(outside, execution)
                    elif attack == "mode": execution.write_bytes(self._execution("prime")); execution.chmod(0o640)
                    else: execution.mkdir()
                    return subprocess.CompletedProcess(argv, 0)
                with mock.patch.object(gate, "_clean", return_value=True): result = gate.run_gate(runner=runner)
                self.assertEqual("EVIDENCE_INCOMPLETE", result["classification"]); self.assertNotIn("private", json.dumps(result))

    def test_root_phase_output_and_read_swaps_fail_closed(self) -> None:
        for attack in ("root", "phase", "output", "read", "chmod_read", "hardlink_read", "shutdown"):
            with self.subTest(attack=attack):
                swapped = False
                execution_path: Path | None = None
                def runner(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]:
                    nonlocal swapped, execution_path
                    if argv[-1] == "shutdown":
                        if attack == "shutdown" and not swapped:
                            output = Path(argv[2].split("=", 1)[1]); output.rename(output.with_name("output-old")); output.mkdir(); swapped = True
                        return subprocess.CompletedProcess(argv, 0)
                    output = Path(argv[1].split("=", 1)[1]); phase, root = output.parent, output.parents[1]
                    bep = Path(next(x.split("=", 1)[1] for x in argv if x.startswith("--build_event_json_file="))); execution = Path(next(x.split("=", 1)[1] for x in argv if x.startswith("--execution_log_json_file="))); execution_path = execution
                    target = output / "execroot/x/bin/app/slug_cli_v2/slug"; target.parent.mkdir(parents=True); target.write_bytes(b"x"); target.chmod(0o700)
                    bep.write_bytes(self._bep()); execution.write_bytes(self._execution("replay" if "replay" in str(phase) else "prime"))
                    if not swapped and attack in ("root", "phase", "output"):
                        victim = {"root": root, "phase": phase, "output": output}[attack]; victim.rename(victim.with_name(victim.name + "-old")); victim.mkdir(); swapped = True
                    return subprocess.CompletedProcess(argv, 0)
                read = os.read
                def replace_after_read(fd: int, size: int) -> bytes:
                    nonlocal swapped
                    value = read(fd, size)
                    if attack in ("read", "chmod_read", "hardlink_read") and not swapped and execution_path is not None and os.fstat(fd).st_ino == execution_path.lstat().st_ino:
                        # The parser must reject a dirent that changes after its FD was opened.
                        if attack == "read": execution_path.rename(execution_path.with_name("execution-old.json")); execution_path.write_bytes(self._execution("prime"))
                        elif attack == "chmod_read": execution_path.chmod(0o640)
                        else: os.link(execution_path, execution_path.with_name("execution-link.json"))
                        swapped = True
                    return value
                with mock.patch.object(gate, "_clean", return_value=True), mock.patch.object(gate.os, "read", side_effect=replace_after_read):
                    result = gate.run_gate(runner=runner)
                self.assertNotEqual("PROVED_BUILD_CACHE", result["classification"])

    def test_setup_open_and_fstat_failures_close_temporary_descriptors(self) -> None:
        for stage in ("open", "fstat"):
            with self.subTest(stage=stage):
                before = len(os.listdir("/proc/self/fd")); opened_output: int | None = None; injected = False
                original_open, original_fstat = gate.os.open, gate.os.fstat
                def fail_open(name: object, flags: int, *args: object, **kwargs: object) -> int:
                    nonlocal opened_output, injected
                    if name == "output" and stage == "open" and not injected: injected = True; raise OSError
                    value = original_open(name, flags, *args, **kwargs)
                    if name == "output": opened_output = value
                    return value
                def fail_fstat(fd: int) -> os.stat_result:
                    nonlocal injected
                    if stage == "fstat" and fd == opened_output and not injected: injected = True; raise OSError
                    return original_fstat(fd)
                with mock.patch.object(gate, "_clean", return_value=True), mock.patch.object(gate.os, "open", side_effect=fail_open), mock.patch.object(gate.os, "fstat", side_effect=fail_fstat):
                    result = gate.run_gate(runner=lambda argv, **_: subprocess.CompletedProcess(argv, 0))
                self.assertNotEqual("PROVED_BUILD_CACHE", result["classification"]); self.assertEqual(before, len(os.listdir("/proc/self/fd")))

    @staticmethod
    def _execution(phase: str) -> bytes:
        return seq([{"runner": "remote cache hit" if phase == "replay" else "local", "cacheable": True, "remoteCacheable": True, "digest": {"hash": DIGEST, "sizeBytes": 1}, "cacheHit": phase == "replay", "status": "", "exitCode": 0}])
    @staticmethod
    def _bep() -> bytes:
        return seq([{"id": {"targetCompleted": {"label": gate.LABEL}}, "completed": {"success": True}}, {"id": {"buildFinished": {}}, "finished": {"exitCode": {"name": "SUCCESS"}}}])
    @staticmethod
    def _records() -> tuple[dict[str, object], dict[str, object]]:
        def record(remote: bool) -> dict[str, object]:
            return {"process_success_count": 1, "build_finished_success_count": 1, "target_success_count": 1, "output_count": 1, "persistent_action_cache_hit_count": 0, "_outcome": "success", "eligible_spawns": {"count": 1, "digest_multiset_sha256": "same", "cache_error_count": 0, "status_error_count": 0, "exit_error_count": 0, "local": 0 if remote else 1, "worker": 0, "linux_sandbox": 0, "remote_cache_hit": 1 if remote else 0, "other": 0}}
        return record(False), record(True)

if __name__ == "__main__": unittest.main()
