from __future__ import annotations
import json
import hashlib
import os
import subprocess
import unittest
from contextlib import redirect_stderr, redirect_stdout
from io import StringIO
from pathlib import Path
from unittest import mock

from tools.v2_oracle import buildbuddy_build_cache_prime_stage_probe as cli
from tools.v2_oracle_lib import buildbuddy_build_cache as cache
from tools.v2_oracle_lib import buildbuddy_build_cache_prime_stage_probe as probe

BEP = (b'{"id":{"targetCompleted":{"label":"//app/slug_cli_v2:slug"}},"completed":{"success":true}}\n'
       b'{"id":{"buildFinished":{}},"finished":{"exitCode":{"name":"SUCCESS","code":0}}}\n')
SPAWN = b'{"spawn":{"cacheable":true,"remote_cacheable":true,"runner":"local","action_digest":{"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","sizeBytes":1},"cache_hit":false,"status":"","exit_code":0}}\n'
REMOTE_BEP = BEP + b'{"id":{"aborted":{}},"aborted":{"reason":"REMOTE_ENVIRONMENT_FAILURE"}}\n'


class PrimeStageProbeTest(unittest.TestCase):
    def _run(self, change: str = "ready", code: int = 0) -> tuple[dict[str, object], list[list[str]], list[Path]]:
        calls, roots = [], []
        def runner(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]:
            calls.append(argv)
            if argv[-1] == "shutdown":
                output = Path(argv[2].split("=", 1)[1])
                if change == "shutdown_output_swap": output.rename(output.with_name("shutdown-old-output")); output.mkdir()
                elif change == "shutdown_root_swap":
                    root = output.parents[1]; root.rename(root.with_name(root.name + "-shutdown-old")); root.mkdir()
                return subprocess.CompletedProcess(argv, 0)
            output = Path(argv[1].split("=", 1)[1]); bep = Path(next(x.split("=", 1)[1] for x in argv if x.startswith("--build_event_json_file="))); execution = Path(next(x.split("=", 1)[1] for x in argv if x.startswith("--execution_log_json_file=")))
            roots.append(output.parent.parent)
            if change == "bep_missing": bep.unlink()
            elif change == "bep_symlink": bep.unlink(); bep.symlink_to("/dev/null")
            elif change == "bep_replace": bep.unlink(); bep.write_bytes(BEP); bep.chmod(0o600)
            elif change == "bep_hardlink":
                other = bep.with_name("bep-other"); other.write_bytes(BEP); bep.unlink(); os.link(other, bep)
            elif change == "bep_mode": bep.write_bytes(BEP); bep.chmod(0o644)
            elif change == "bep_directory": bep.unlink(); bep.mkdir()
            elif change == "bep_bad": bep.write_bytes(b"{")
            elif change == "bep_remote": bep.write_bytes(REMOTE_BEP)
            else: bep.write_bytes(BEP)
            if change == "execution_missing": execution.unlink()
            elif change == "execution_symlink": execution.unlink(); execution.symlink_to("/dev/null")
            elif change == "execution_hardlink":
                other = execution.with_name("execution-other"); other.write_bytes(SPAWN); execution.unlink(); os.link(other, execution)
            elif change == "execution_mode": execution.write_bytes(SPAWN); execution.chmod(0o644)
            elif change == "execution_directory": execution.unlink(); execution.mkdir()
            elif change == "execution_bad": execution.write_bytes(b"{")
            elif change == "execution_empty": execution.write_bytes(b"")
            elif change == "execution_replace": execution.unlink(); execution.write_bytes(SPAWN)
            else: execution.write_bytes(SPAWN)
            binary = output / "execroot/bin/app/slug_cli_v2/slug"; binary.parent.mkdir(parents=True); binary.write_bytes(b"x"); binary.chmod(0o700)
            if change == "phase_swap": execution.parent.rename(execution.parent.with_name("old")); execution.parent.mkdir()
            elif change == "root_swap":
                root = output.parents[1]; root.rename(root.with_name(root.name + "-old")); root.mkdir()
            elif change == "output_swap": output.rename(output.with_name("old-output")); output.mkdir()
            return subprocess.CompletedProcess(argv, code)
        with mock.patch.object(probe.cleanup, "_clean_git", return_value=True), mock.patch.object(probe.cleanup, "_no_slugd", return_value=True): result = probe.run_probe(runner=runner)
        return result, calls, roots

    def test_exact_one_prime_command_and_ready_semantics(self) -> None:
        result, calls, roots = self._run()
        self.assertEqual(("STAGE_RECORDED", "ZERO", "PRIME_READY"), (result["classification"], result["process"], result["stage"]))
        self.assertFalse(roots[0].exists()); self.assertEqual(2, len(calls))
        prime = calls[0]; nonce = next(x.rsplit("=", 1)[1] for x in prime if "NONCE=" in x)
        bep = Path(next(x.split("=", 1)[1] for x in prime if x.startswith("--build_event_json_file="))); execution = Path(next(x.split("=", 1)[1] for x in prime if x.startswith("--execution_log_json_file=")))
        self.assertEqual(cache.command("prime", "bazel", Path(prime[1].split("=", 1)[1]), bep, execution, nonce), prime)
        self.assertEqual(["bazel", "--ignore_all_rc_files", f"--output_base={prime[1].split('=', 1)[1]}", "shutdown"], calls[1])
        gate_source = Path(cache.__file__).read_bytes()
        self.assertEqual("ab285a31113a85f5a687e585088e596c552f29622b65fb991be4d591ab3886bc", hashlib.sha256(gate_source).hexdigest())

    def test_process_descriptor_phase_and_spawn_stages(self) -> None:
        cases = (("ready", 7, "PROCESS_NONZERO"), ("bep_missing", 0, "BEP_DESCRIPTOR_REJECTED"), ("bep_symlink", 0, "BEP_DESCRIPTOR_REJECTED"), ("bep_replace", 0, "BEP_DESCRIPTOR_REJECTED"), ("bep_hardlink", 0, "BEP_DESCRIPTOR_REJECTED"), ("bep_mode", 0, "BEP_DESCRIPTOR_REJECTED"), ("bep_directory", 0, "BEP_DESCRIPTOR_REJECTED"), ("bep_bad", 0, "BEP_PHASE_REJECTED"), ("execution_missing", 0, "EXECUTION_DESCRIPTOR_REJECTED"), ("execution_symlink", 0, "EXECUTION_DESCRIPTOR_REJECTED"), ("execution_hardlink", 0, "EXECUTION_DESCRIPTOR_REJECTED"), ("execution_mode", 0, "EXECUTION_DESCRIPTOR_REJECTED"), ("execution_directory", 0, "EXECUTION_DESCRIPTOR_REJECTED"), ("execution_bad", 0, "EXECUTION_SPAWN_REJECTED"))
        for change, code, stage in cases:
            with self.subTest(stage=stage): self.assertEqual(stage, self._run(change, code)[0]["stage"])
        self.assertEqual("PRIME_READY", self._run("execution_replace")[0]["stage"])

    def test_output_semantic_and_anchor_stages(self) -> None:
        with mock.patch.object(probe.cache, "_outputs", side_effect=OSError): self.assertEqual("OUTPUT_REJECTED", self._run()[0]["stage"])
        with mock.patch.object(probe.cache, "_outputs", return_value=0): self.assertEqual("PRIME_SEMANTICS_REJECTED", self._run()[0]["stage"])
        self.assertEqual("PRIME_SEMANTICS_REJECTED", self._run("execution_empty")[0]["stage"])
        self.assertEqual("PRIME_SEMANTICS_REJECTED", self._run("bep_remote")[0]["stage"])
        with mock.patch.object(probe.cache, "_anchored", side_effect=(True, False, True, True)):
            self.assertEqual("POST_PARSE_ANCHOR_REJECTED", self._run()[0]["stage"])
        with mock.patch.object(probe.cache, "_anchored", side_effect=(False, True, True)):
            self.assertEqual("POST_RUN_ANCHOR_REJECTED", self._run()[0]["stage"])
        for change in ("phase_swap", "root_swap", "output_swap"):
            result, calls, roots = self._run(change)
            self.assertEqual(probe.record(), result); self.assertEqual([], [x for x in calls if x[-1] == "shutdown"]); self.assertFalse(roots[0].exists())

    def test_shutdown_swaps_and_cleanup_failure_reject(self) -> None:
        for change in ("shutdown_output_swap", "shutdown_root_swap"):
            result, calls, roots = self._run(change)
            self.assertEqual(probe.record(), result); self.assertEqual(1, len([x for x in calls if x[-1] == "shutdown"])); self.assertFalse(roots[0].exists())
        actual = probe.lifecycle._remove_original
        def failed(*args: object) -> bool: actual(*args); return False
        with mock.patch.object(probe.lifecycle, "_remove_original", side_effect=failed): self.assertEqual(probe.record(), self._run()[0])

    def test_precheck_setup_cleanup_and_shutdown_fail_closed(self) -> None:
        self.assertEqual("NOT_RECORDED", probe.record()["stage"])
        with mock.patch.object(probe.cleanup, "_clean_git", side_effect=(False, True)), mock.patch.object(probe.cleanup, "_no_slugd", return_value=True): self.assertEqual("PRECHECK_REJECTED", probe.run_probe()["stage"])
        with mock.patch.object(probe.cleanup, "_clean_git", return_value=True), mock.patch.object(probe.cleanup, "_no_slugd", return_value=True), mock.patch.object(probe.tempfile, "mkdtemp", side_effect=OSError): self.assertEqual("SETUP_REJECTED", probe.run_probe()["stage"])
        with mock.patch.object(probe.cleanup, "_clean_git", return_value=True), mock.patch.object(probe.cleanup, "_no_slugd", return_value=True), mock.patch.object(probe, "_private", side_effect=OSError): self.assertEqual("SETUP_REJECTED", probe.run_probe()["stage"])
        def bad_shutdown(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]: return subprocess.CompletedProcess(argv, 1 if argv[-1] == "shutdown" else 0)
        with mock.patch.object(probe.cleanup, "_clean_git", return_value=True), mock.patch.object(probe.cleanup, "_no_slugd", return_value=True): self.assertEqual(probe.record(), probe.run_probe(runner=bad_shutdown))

    def test_schema_subclasses_cli_and_secret_suppression(self) -> None:
        hostile = {**probe.record("STAGE_RECORDED", "ZERO", "PRIME_READY"), "secret": "/private"}
        self.assertEqual(probe.record(), probe.normalize(hostile))
        class DictSubclass(dict): pass
        class StringSubclass(str): pass
        self.assertEqual(probe.record(), probe.normalize(DictSubclass(probe.record("STAGE_RECORDED", "ZERO", "PRIME_READY"))))
        self.assertEqual(probe.record(), probe.normalize({**probe.record("STAGE_RECORDED", "ZERO", "PRIME_READY"), "stage": StringSubclass("/private")}))
        self.assertEqual(probe.record(), probe.normalize({"schema_version": 1, "mode": probe.MODE, "classification": "STAGE_RECORDED", "process": "ZERO", "stage": "PROCESS_NONZERO"}))
        self.assertEqual(probe.record(), probe.normalize({"schema_version": 1, "mode": probe.MODE, "classification": "STAGE_RECORDED", "process": "NONZERO", "stage": "PRIME_READY"}))
        out, err = StringIO(), StringIO()
        with mock.patch.object(cli, "run_probe", return_value=hostile), redirect_stdout(out), redirect_stderr(err): self.assertEqual(1, cli.main([]))
        self.assertEqual("", err.getvalue()); self.assertNotIn("private", out.getvalue()); self.assertEqual(probe.record(), json.loads(out.getvalue()))
        def secret(_: list[str], **__: object) -> subprocess.CompletedProcess[bytes]: raise RuntimeError("/private/token")
        with mock.patch.object(probe.cleanup, "_clean_git", return_value=True), mock.patch.object(probe.cleanup, "_no_slugd", return_value=True): self.assertNotIn("private", json.dumps(probe.run_probe(runner=secret)))


if __name__ == "__main__": unittest.main()
