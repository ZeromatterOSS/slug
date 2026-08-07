from __future__ import annotations
import hashlib
import json
import os
import stat
import subprocess
import unittest
from contextlib import redirect_stderr, redirect_stdout
from io import StringIO
from pathlib import Path
from unittest import mock

from tools.v2_oracle import buildbuddy_build_cache_prime_execution_stage_probe as cli
from tools.v2_oracle_lib import buildbuddy_build_cache as cache
from tools.v2_oracle_lib import buildbuddy_build_cache_prime_execution_stage_probe as probe

SPAWN = b'{"spawn":{"cacheable":true,"remote_cacheable":true,"runner":"local","action_digest":{"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","sizeBytes":1},"cache_hit":false,"status":"","exit_code":0}}\n'


class ExecutionStageProbeTest(unittest.TestCase):
    def _run(self, change: str = "ready", code: int = 0) -> tuple[dict[str, object], list[list[str]], list[Path]]:
        calls, roots = [], []
        def runner(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]:
            calls.append(argv)
            if argv[-1] == "shutdown":
                output = Path(argv[2].split("=", 1)[1])
                if change == "shutdown_output_swap": output.rename(output.with_name("old-output")); output.mkdir()
                if change == "shutdown_root_swap": output.parents[1].rename(output.parents[1].with_name("old-root")); output.parents[1].mkdir()
                return subprocess.CompletedProcess(argv, 0)
            output = Path(argv[1].split("=", 1)[1]); bep = Path(next(x.split("=", 1)[1] for x in argv if x.startswith("--build_event_json_file="))); execution = Path(next(x.split("=", 1)[1] for x in argv if x.startswith("--execution_log_json_file="))); self._active_execution = execution; roots.append(output.parent.parent)
            bep_item = bep.lstat()
            self.assertTrue(stat.S_ISREG(bep_item.st_mode)); self.assertEqual(0o600, stat.S_IMODE(bep_item.st_mode)); self.assertEqual(1, bep_item.st_nlink)
            if change == "missing": execution.unlink()
            elif change == "symlink": execution.unlink(); execution.symlink_to("/dev/null")
            elif change == "hardlink": other = execution.with_name("other"); other.write_bytes(SPAWN); execution.unlink(); os.link(other, execution)
            elif change == "mode": execution.write_bytes(SPAWN); execution.chmod(0o644)
            elif change == "directory": execution.unlink(); execution.mkdir()
            elif change == "stream": execution.write_bytes(b"{")
            elif change == "spawn": execution.write_bytes(b'{"spawn":[]}\n')
            elif change == "event_then_stream": execution.write_bytes(b'{"spawn":[]}\n{')
            elif change == "stream_after": execution.write_bytes(SPAWN + b"{")
            elif change == "replace": execution.unlink(); execution.write_bytes(SPAWN)
            else: execution.write_bytes(SPAWN if change != "empty" else b"")
            if change == "phase_swap": execution.parent.rename(execution.parent.with_name("old")); execution.parent.mkdir()
            elif change == "root_swap": output.parents[1].rename(output.parents[1].with_name("old")); output.parents[1].mkdir()
            elif change == "output_swap": output.rename(output.with_name("old-output")); output.mkdir()
            return subprocess.CompletedProcess(argv, code)
        with mock.patch.object(probe.cleanup, "_clean_git", return_value=True), mock.patch.object(probe.cleanup, "_no_slugd", return_value=True): result = probe.run_probe(runner=runner)
        return result, calls, roots

    def test_command_ready_replacement_and_no_bep_or_output_readers(self) -> None:
        with mock.patch.object(probe.cache, "_private_bytes", side_effect=AssertionError), mock.patch.object(probe.cache, "_outputs", side_effect=AssertionError): result, calls, roots = self._run()
        self.assertEqual(("STAGE_RECORDED", "ZERO", "EXECUTION_READY"), (result["classification"], result["process"], result["stage"])); self.assertFalse(roots[0].exists()); self.assertEqual(2, len(calls))
        prime = calls[0]; nonce = next(x.rsplit("=", 1)[1] for x in prime if "NONCE=" in x); bep = Path(next(x.split("=", 1)[1] for x in prime if x.startswith("--build_event_json_file="))); execution = Path(next(x.split("=", 1)[1] for x in prime if x.startswith("--execution_log_json_file=")))
        self.assertEqual(cache.command("bazel", Path(prime[1].split("=", 1)[1]), bep, execution, nonce), prime); self.assertEqual(["bazel", "--ignore_all_rc_files", f"--output_base={prime[1].split('=', 1)[1]}", "shutdown"], calls[1]); self.assertEqual("641f76f3a272bb3914c825e7e351a5131855aedb576a8377e56c85e2840f8229", hashlib.sha256(Path(cache.__file__).read_bytes()).hexdigest())
        self.assertEqual("EXECUTION_READY", self._run("replace")[0]["stage"])

    def test_process_descriptor_stream_and_spawn_order(self) -> None:
        cases = (("ready", 7, "PROCESS_NONZERO"), ("missing", 0, "EXECUTION_DESCRIPTOR_REJECTED"), ("symlink", 0, "EXECUTION_DESCRIPTOR_REJECTED"), ("hardlink", 0, "EXECUTION_DESCRIPTOR_REJECTED"), ("mode", 0, "EXECUTION_DESCRIPTOR_REJECTED"), ("directory", 0, "EXECUTION_DESCRIPTOR_REJECTED"), ("stream", 0, "EXECUTION_STREAM_REJECTED"), ("spawn", 0, "EXECUTION_SPAWN_REJECTED"), ("event_then_stream", 0, "EXECUTION_SPAWN_REJECTED"), ("stream_after", 0, "EXECUTION_STREAM_REJECTED"), ("empty", 0, "EXECUTION_READY"))
        for change, code, stage in cases:
            with self.subTest(stage=stage): self.assertEqual(stage, self._run(change, code)[0]["stage"])

    def test_anchor_shutdown_and_cleanup_fail_closed(self) -> None:
        with mock.patch.object(probe.cache, "_anchored", side_effect=(False, True, True)): self.assertEqual("POST_RUN_ANCHOR_REJECTED", self._run()[0]["stage"])
        with mock.patch.object(probe.cache, "_anchored", side_effect=(True, False, True, True)): self.assertEqual("POST_PARSE_ANCHOR_REJECTED", self._run()[0]["stage"])
        for change in ("phase_swap", "root_swap", "output_swap", "shutdown_output_swap", "shutdown_root_swap"):
            result, calls, roots = self._run(change); self.assertEqual(probe.record(), result); self.assertFalse(roots[0].exists())
            if change.startswith("shutdown_"): self.assertEqual(1, len([call for call in calls if call[-1] == "shutdown"]))
        actual = probe.lifecycle._remove_original
        def failed(*args: object) -> bool: actual(*args); return False
        with mock.patch.object(probe.lifecycle, "_remove_original", side_effect=failed): self.assertEqual(probe.record(), self._run()[0])

    def test_read_swap_rejects_the_bound_execution_descriptor(self) -> None:
        original, swapped = cache.os.read, []
        def read(fd: int, size: int) -> bytes:
            if not swapped:
                swapped.append(True); path = self._active_execution
                path.rename(path.with_name("old-execution")); path.write_bytes(SPAWN)
            return original(fd, size)
        with mock.patch.object(cache.os, "read", side_effect=read): self.assertEqual("EXECUTION_DESCRIPTOR_REJECTED", self._run()[0]["stage"])

    def test_precheck_setup_read_shutdown_schema_cli_and_privacy(self) -> None:
        with mock.patch.object(probe.cleanup, "_clean_git", side_effect=(False, True)), mock.patch.object(probe.cleanup, "_no_slugd", return_value=True): self.assertEqual("PRECHECK_REJECTED", probe.run_probe()["stage"])
        with mock.patch.object(probe.cleanup, "_clean_git", return_value=True), mock.patch.object(probe.cleanup, "_no_slugd", return_value=True), mock.patch.object(probe.tempfile, "mkdtemp", side_effect=OSError): self.assertEqual("SETUP_REJECTED", probe.run_probe()["stage"])
        with mock.patch.object(probe.cache, "_execution_bytes", side_effect=OSError): self.assertEqual("EXECUTION_DESCRIPTOR_REJECTED", self._run()[0]["stage"])
        def bad(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]: return subprocess.CompletedProcess(argv, 1 if argv[-1] == "shutdown" else 0)
        with mock.patch.object(probe.cleanup, "_clean_git", return_value=True), mock.patch.object(probe.cleanup, "_no_slugd", return_value=True): self.assertEqual(probe.record(), probe.run_probe(runner=bad))
        def ready(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]:
            if argv[-1] != "shutdown": Path(next(x.split("=", 1)[1] for x in argv if x.startswith("--execution_log_json_file="))).write_bytes(SPAWN)
            return subprocess.CompletedProcess(argv, 0)
        with mock.patch.object(probe.cleanup, "_clean_git", side_effect=(True, False)), mock.patch.object(probe.cleanup, "_no_slugd", return_value=True): self.assertEqual(probe.record(), probe.run_probe(runner=ready))
        class DictSubclass(dict): pass
        class StringSubclass(str): pass
        hostile = {**probe.record("STAGE_RECORDED", "ZERO", "EXECUTION_READY"), "secret": "/private"}
        self.assertEqual(probe.record(), probe.normalize(hostile)); self.assertEqual(probe.record(), probe.normalize(DictSubclass(probe.record("STAGE_RECORDED", "ZERO", "EXECUTION_READY")))); self.assertEqual(probe.record(), probe.normalize({**probe.record("STAGE_RECORDED", "ZERO", "EXECUTION_READY"), "stage": StringSubclass("/private")}))
        out, err = StringIO(), StringIO()
        with mock.patch.object(cli, "run_probe", return_value=hostile), redirect_stdout(out), redirect_stderr(err): self.assertEqual(1, cli.main([]))
        self.assertEqual("", err.getvalue()); self.assertNotIn("private", out.getvalue()); self.assertEqual(probe.record(), json.loads(out.getvalue()))


if __name__ == "__main__": unittest.main()
