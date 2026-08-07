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

from tools.v2_oracle import buildbuddy_build_cache_prime_output_semantics_probe as cli
from tools.v2_oracle_lib import buildbuddy_build_cache as cache
from tools.v2_oracle_lib import buildbuddy_build_cache_prime_output_semantics_probe as probe

BEP = (b'{"id":{"targetCompleted":{"label":"//app/slug_cli_v2:slug"}},"completed":{"success":true}}\n'
       b'{"id":{"buildFinished":{}},"finished":{"exitCode":{"name":"SUCCESS","code":0}}}\n')
SPAWN = b'{"spawn":{"cacheable":true,"remote_cacheable":true,"runner":"local","action_digest":{"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","sizeBytes":1},"cache_hit":false,"status":"","exit_code":0}}\n'


class OutputSemanticsProbeTest(unittest.TestCase):
    def _run(self, change: str = "ready", code: int = 0) -> tuple[dict[str, object], list[list[str]], list[Path]]:
        calls, roots = [], []
        def executable(output: Path, name: str = "slug", mode: int = 0o700) -> Path:
            path = output / "execroot/bin/app/slug_cli_v2" / name; path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(b"x"); path.chmod(mode); return path
        def runner(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]:
            calls.append(argv)
            if argv[-1] == "shutdown":
                output = Path(argv[2].split("=", 1)[1])
                if change == "shutdown_output_swap": output.rename(output.with_name("old-output")); output.mkdir()
                elif change == "shutdown_root_swap": output.parents[1].rename(output.parents[1].with_name("old-root")); output.parents[1].mkdir()
                if change == "shutdown_exception": raise OSError
                return subprocess.CompletedProcess(argv, 9 if change == "shutdown_nonzero" else 0)
            output = Path(argv[1].split("=", 1)[1]); bep = Path(next(x.split("=", 1)[1] for x in argv if x.startswith("--build_event_json_file="))); execution = Path(next(x.split("=", 1)[1] for x in argv if x.startswith("--execution_log_json_file="))); self._bep, self._execution = bep, execution; roots.append(output.parent.parent)
            for artifact in (bep, execution):
                item = artifact.lstat(); self.assertTrue(stat.S_ISREG(item.st_mode)); self.assertEqual(0o600, stat.S_IMODE(item.st_mode)); self.assertEqual(1, item.st_nlink)
            if change == "bep_missing": bep.unlink()
            elif change == "bep_bad": bep.write_bytes(b"{")
            else: bep.write_bytes(BEP)
            if change == "execution_missing": execution.unlink()
            elif change == "execution_bad": execution.write_bytes(b"{")
            elif change == "execution_replace": execution.unlink(); execution.write_bytes(SPAWN)
            else: execution.write_bytes(SPAWN if change != "execution_empty" else b"")
            if change != "output_missing":
                binary = executable(output, mode=0o600 if change == "output_nonexecutable" else 0o700)
                if change == "output_link": binary.unlink(); binary.symlink_to("/dev/null")
                elif change == "output_multiple": executable(output, "slug", 0o700); executable(output / "execroot/other", "slug", 0o700)
            if change == "phase_swap": execution.parent.rename(execution.parent.with_name("old")); execution.parent.mkdir()
            elif change == "root_swap": output.parents[1].rename(output.parents[1].with_name("old")); output.parents[1].mkdir()
            elif change == "output_swap": output.rename(output.with_name("old-output")); output.mkdir()
            return subprocess.CompletedProcess(argv, code)
        with mock.patch.object(probe.cleanup, "_clean_git", return_value=True), mock.patch.object(probe.cleanup, "_no_slugd", return_value=True): result = probe.run_probe(runner=runner)
        return result, calls, roots

    def test_exact_prime_command_and_ready(self) -> None:
        result, calls, roots = self._run()
        self.assertEqual(("STAGE_RECORDED", "ZERO", "PRIME_READY"), (result["classification"], result["process"], result["stage"])); self.assertFalse(roots[0].exists()); self.assertEqual(2, len(calls))
        prime = calls[0]; nonce = next(x.rsplit("=", 1)[1] for x in prime if "NONCE=" in x); output = Path(prime[1].split("=", 1)[1]); bep = Path(next(x.split("=", 1)[1] for x in prime if x.startswith("--build_event_json_file="))); execution = Path(next(x.split("=", 1)[1] for x in prime if x.startswith("--execution_log_json_file=")))
        self.assertEqual(cache.command("bazel", output, bep, execution, nonce), prime); self.assertEqual(["bazel", "--ignore_all_rc_files", f"--output_base={output}", "shutdown"], calls[1]); self.assertEqual("a52c29d027050f07a5c51142e18554ec73443cfaa23b717b332d8d9e8ebdf4ba", hashlib.sha256(Path(cache.__file__).read_bytes()).hexdigest())

    def test_output_stages_precede_all_private_readers(self) -> None:
        for change in ("output_missing", "output_nonexecutable", "output_link", "output_multiple"):
            with self.subTest(change=change), mock.patch.object(probe.cache, "_private_bytes", side_effect=AssertionError), mock.patch.object(probe.cache, "_execution_bytes", side_effect=AssertionError): self.assertEqual("OUTPUT_MATERIALIZATION_REJECTED", self._run(change)[0]["stage"])
        with mock.patch.object(probe.cache, "_outputs", side_effect=OSError), mock.patch.object(probe.cache, "_private_bytes", side_effect=AssertionError), mock.patch.object(probe.cache, "_execution_bytes", side_effect=AssertionError): self.assertEqual("OUTPUT_SCAN_REJECTED", self._run()[0]["stage"])
        with mock.patch.object(probe.cache, "_anchored", side_effect=(True, False, True, True)): self.assertEqual("POST_OUTPUT_ANCHOR_REJECTED", self._run()[0]["stage"])

    def test_process_and_reader_parse_stages(self) -> None:
        cases = (("ready", 7, "PROCESS_NONZERO"), ("bep_missing", 0, "BEP_DESCRIPTOR_REJECTED"), ("bep_bad", 0, "BEP_PHASE_REJECTED"), ("execution_missing", 0, "EXECUTION_DESCRIPTOR_REJECTED"), ("execution_bad", 0, "EXECUTION_SPAWN_REJECTED"), ("execution_empty", 0, "PRIME_SEMANTICS_REJECTED"))
        for change, code, stage in cases:
            with self.subTest(stage=stage): self.assertEqual(stage, self._run(change, code)[0]["stage"])
        self.assertEqual("PRIME_READY", self._run("execution_replace")[0]["stage"])
        with mock.patch.object(probe.cache, "_anchored", side_effect=(False, True, True)): self.assertEqual("POST_RUN_ANCHOR_REJECTED", self._run()[0]["stage"])
        with mock.patch.object(probe.cache, "_anchored", side_effect=(True, True, False, True, True)): self.assertEqual("POST_PARSE_ANCHOR_REJECTED", self._run()[0]["stage"])

    def test_every_aggregate_prime_predicate_input_runs_through_imported_predicate(self) -> None:
        phase = {"_outcome": "success", "process_success_count": 1, "build_finished_success_count": 1, "target_success_count": 1, "output_count": 0, "persistent_action_cache_hit_count": 0}
        spawns = {"count": 1, "cache_error_count": 0, "status_error_count": 0, "exit_error_count": 0, "local": 1, "worker": 0, "linux_sandbox": 0, "remote_cache_hit": 0, "other": 0}
        cases: list[tuple[dict[str, object], dict[str, int], str]] = []
        for outcome in ("remote", "command", "target"): cases.append(({"_outcome": outcome}, {}, "PRIME_SEMANTICS_REJECTED"))
        for key in ("process_success_count", "build_finished_success_count", "target_success_count"): cases.append(({key: 0}, {}, "PRIME_SEMANTICS_REJECTED"))
        cases.append(({"persistent_action_cache_hit_count": 1}, {}, "PRIME_SEMANTICS_REJECTED"))
        cases.append(({}, {"count": 0, "local": 0}, "PRIME_SEMANTICS_REJECTED"))
        for key in ("cache_error_count", "status_error_count", "exit_error_count"): cases.append(({}, {key: 1}, "PRIME_SEMANTICS_REJECTED"))
        for runner in ("local", "worker", "linux_sandbox"):
            values = {key: 0 for key in ("local", "worker", "linux_sandbox")}; values[runner] = 1; cases.append(({}, values, "PRIME_READY"))
        for runner in ("remote_cache_hit", "other"): cases.append(({}, {"local": 0, runner: 1}, "PRIME_SEMANTICS_REJECTED"))
        for phase_change, spawn_change, expected in cases:
            current_phase, current_spawns = {**phase, **phase_change}, {**spawns, **spawn_change}
            current_phase["output_count"] = 1
            self.assertEqual(expected == "PRIME_READY", probe.prime_stage._ready(current_phase, current_spawns))
            current_phase["output_count"] = 0
            with self.subTest(phase=phase_change, spawn=spawn_change), mock.patch.object(probe.cache, "phase_record", return_value=current_phase), mock.patch.object(probe.cache, "spawns", return_value=current_spawns): self.assertEqual(expected, self._run()[0]["stage"])

    def test_root_phase_output_read_and_shutdown_swaps_fail_closed(self) -> None:
        for change in ("phase_swap", "root_swap", "output_swap", "shutdown_output_swap", "shutdown_root_swap"):
            result, calls, roots = self._run(change); self.assertEqual(probe.record(), result); self.assertFalse(roots[0].exists())
            if change.startswith("shutdown_"): self.assertEqual(1, len([call for call in calls if call[-1] == "shutdown"]))
        original, changed, bep_done = cache.os.read, [], []
        private = cache._private_bytes
        def read_private(*args: object) -> bytes:
            value = private(*args); bep_done.append(True); return value
        def read(fd: int, size: int) -> bytes:
            if bep_done and not changed: changed.append(True); self._execution.rename(self._execution.with_name("old-execution")); self._execution.write_bytes(SPAWN)
            return original(fd, size)
        with mock.patch.object(probe.cache, "_private_bytes", side_effect=read_private), mock.patch.object(cache.os, "read", side_effect=read): self.assertEqual("EXECUTION_DESCRIPTOR_REJECTED", self._run()[0]["stage"])
        changed.clear()
        def bep_read(fd: int, size: int) -> bytes:
            if not changed: changed.append(True); self._bep.rename(self._bep.with_name("old-bep")); self._bep.write_bytes(BEP)
            return original(fd, size)
        with mock.patch.object(cache.os, "read", side_effect=bep_read): self.assertEqual("BEP_DESCRIPTOR_REJECTED", self._run()[0]["stage"])

    def test_setup_cleanup_schema_cli_and_privacy_are_closed(self) -> None:
        with mock.patch.object(probe.cleanup, "_clean_git", side_effect=(False, True)), mock.patch.object(probe.cleanup, "_no_slugd", return_value=True): self.assertEqual("PRECHECK_REJECTED", probe.run_probe()["stage"])
        with mock.patch.object(probe.cleanup, "_clean_git", return_value=True), mock.patch.object(probe.cleanup, "_no_slugd", return_value=True), mock.patch.object(probe.tempfile, "mkdtemp", side_effect=OSError): self.assertEqual("SETUP_REJECTED", probe.run_probe()["stage"])
        actual = probe.lifecycle._remove_original
        def failed(*args: object) -> bool: actual(*args); return False
        with mock.patch.object(probe.lifecycle, "_remove_original", side_effect=failed): self.assertEqual(probe.record(), self._run()[0])
        for change in ("shutdown_nonzero", "shutdown_exception"):
            with self.subTest(change=change): self.assertEqual(probe.record(), self._run(change)[0])
        hostile = {**probe.record("STAGE_RECORDED", "ZERO", "PRIME_READY"), "secret": "/private"}
        class DictSubclass(dict): pass
        class StringSubclass(str): pass
        self.assertEqual(probe.record(), probe.normalize(hostile)); self.assertEqual(probe.record(), probe.normalize(DictSubclass(probe.record("STAGE_RECORDED", "ZERO", "PRIME_READY")))); self.assertEqual(probe.record(), probe.normalize({**probe.record("STAGE_RECORDED", "ZERO", "PRIME_READY"), "stage": StringSubclass("/private")}))
        out, err = StringIO(), StringIO()
        with mock.patch.object(cli, "run_probe", return_value=hostile), redirect_stdout(out), redirect_stderr(err): self.assertEqual(1, cli.main([]))
        self.assertEqual("", err.getvalue()); self.assertNotIn("private", out.getvalue()); self.assertEqual(probe.record(), json.loads(out.getvalue()))


if __name__ == "__main__": unittest.main()
