from __future__ import annotations
import hashlib
import json
import os
import subprocess
import unittest
from contextlib import redirect_stderr, redirect_stdout
from io import StringIO
from pathlib import Path
from unittest import mock

from tools.v2_oracle import buildbuddy_build_cache_prime_bep_stage_probe as cli
from tools.v2_oracle_lib import buildbuddy_build_cache as cache
from tools.v2_oracle_lib import buildbuddy_build_cache_prime_bep_stage_probe as probe

BEP = (b'{"id":{"targetCompleted":{"label":"//app/slug_cli_v2:slug"}},"completed":{"success":true}}\n'
       b'{"id":{"buildFinished":{}},"finished":{"exitCode":{"name":"SUCCESS"}}}\n')


def _payload(change: str) -> bytes:
    if change == "stream_bad": return b"{"
    if change == "event_bad": return b'{"id":[]}\n'
    if change == "nested_bad": return BEP + b'{"id":{},"buildMetrics":[]}\n'
    if change == "terminal_bad": return BEP.replace(b'"SUCCESS"', b'"FAILURE"')
    if change == "counter_bad": return BEP.replace(b'"success":true', b'"success":false')
    if change == "hits_bad": return BEP + b'{"id":{},"buildMetrics":{"actionSummary":{"actionCacheStatistics":{"hits":1}}}}\n'
    if change == "hits_invalid": return BEP + b'{"id":{},"buildMetrics":{"actionSummary":{"actionCacheStatistics":{"hits":"bad"}}}}\n'
    if change == "statistics_nondict": return BEP + b'{"id":{},"buildMetrics":{"actionSummary":{"actionCacheStatistics":[]}}}\n'
    if change == "exit_invalid": return BEP.replace(b'"name":"SUCCESS"', b'"name":"SUCCESS","code":"bad"')
    if change in ("exit_null", "exit_false", "exit_true", "exit_string", "exit_negative"):
        value = {"exit_null": b"null", "exit_false": b"false", "exit_true": b"true", "exit_string": b'"0"', "exit_negative": b"-1"}[change]
        return BEP.replace(b'"name":"SUCCESS"', b'"name":"SUCCESS","code":' + value)
    if change == "target_nondict": return BEP.replace(b'{"label":"//app/slug_cli_v2:slug"}', b'[]')
    if change == "completed_nondict": return BEP.replace(b'{"success":true}', b'[]')
    if change == "finished_invalid": return BEP.replace(b'{"exitCode":{"name":"SUCCESS"}}', b'[]')
    if change == "event_then_stream": return b'{"id":[]}\n{'
    if change == "finished_then_stream": return b'{"id":{"buildFinished":{}},"finished":[]}\n{'
    if change == "hits_then_stream": return b'{"id":{},"buildMetrics":{"actionSummary":{"actionCacheStatistics":{"hits":"bad"}}}}\n{'
    return BEP


class PrimeBepStageProbeTest(unittest.TestCase):
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
            else: bep.write_bytes(_payload(change))
            execution.write_bytes(b"never-read")
            if change == "phase_swap": bep.parent.rename(bep.parent.with_name("old")); bep.parent.mkdir()
            elif change == "root_swap":
                root = output.parents[1]; root.rename(root.with_name(root.name + "-old")); root.mkdir()
            elif change == "output_swap": output.rename(output.with_name("old-output")); output.mkdir()
            return subprocess.CompletedProcess(argv, code)
        with mock.patch.object(probe.cleanup, "_clean_git", return_value=True), mock.patch.object(probe.cleanup, "_no_slugd", return_value=True): result = probe.run_probe(runner=runner)
        return result, calls, roots

    def test_exact_command_bep_ready_and_no_execution_or_output_reader(self) -> None:
        with mock.patch.object(probe.cache, "_execution_bytes", side_effect=AssertionError), mock.patch.object(probe.cache, "_outputs", side_effect=AssertionError): result, calls, roots = self._run()
        self.assertEqual(("STAGE_RECORDED", "ZERO", "BEP_READY"), (result["classification"], result["process"], result["stage"]))
        self.assertFalse(roots[0].exists()); self.assertEqual(2, len(calls))
        prime = calls[0]; nonce = next(x.rsplit("=", 1)[1] for x in prime if "NONCE=" in x)
        bep = Path(next(x.split("=", 1)[1] for x in prime if x.startswith("--build_event_json_file="))); execution = Path(next(x.split("=", 1)[1] for x in prime if x.startswith("--execution_log_json_file=")))
        self.assertEqual(cache.command("bazel", Path(prime[1].split("=", 1)[1]), bep, execution, nonce), prime)
        self.assertEqual(["bazel", "--ignore_all_rc_files", f"--output_base={prime[1].split('=', 1)[1]}", "shutdown"], calls[1])
        self.assertEqual("641f76f3a272bb3914c825e7e351a5131855aedb576a8377e56c85e2840f8229", hashlib.sha256(Path(cache.__file__).read_bytes()).hexdigest())

    def test_process_descriptor_stream_event_terminal_and_counter_stages(self) -> None:
        cases = (("ready", 9, "PROCESS_NONZERO"), ("bep_missing", 0, "BEP_DESCRIPTOR_REJECTED"), ("bep_symlink", 0, "BEP_DESCRIPTOR_REJECTED"), ("bep_replace", 0, "BEP_DESCRIPTOR_REJECTED"), ("bep_hardlink", 0, "BEP_DESCRIPTOR_REJECTED"), ("bep_mode", 0, "BEP_DESCRIPTOR_REJECTED"), ("bep_directory", 0, "BEP_DESCRIPTOR_REJECTED"), ("stream_bad", 0, "BEP_STREAM_REJECTED"), ("event_bad", 0, "BEP_EVENT_REJECTED"), ("event_then_stream", 0, "BEP_EVENT_REJECTED"), ("nested_bad", 0, "BEP_EVENT_REJECTED"), ("completed_nondict", 0, "BEP_EVENT_REJECTED"), ("finished_invalid", 0, "BEP_TERMINAL_REJECTED"), ("finished_then_stream", 0, "BEP_STREAM_REJECTED"), ("hits_then_stream", 0, "BEP_COUNTER_REJECTED"), ("exit_invalid", 0, "BEP_COUNTER_REJECTED"), ("exit_null", 0, "BEP_COUNTER_REJECTED"), ("exit_false", 0, "BEP_COUNTER_REJECTED"), ("exit_true", 0, "BEP_COUNTER_REJECTED"), ("exit_string", 0, "BEP_COUNTER_REJECTED"), ("exit_negative", 0, "BEP_COUNTER_REJECTED"))
        for change, code, stage in cases:
            with self.subTest(stage=stage): self.assertEqual(stage, self._run(change, code)[0]["stage"])

    def test_phase_record_parser_parity(self) -> None:
        cases = (("ready", "BEP_READY"), ("terminal_bad", "BEP_READY"), ("counter_bad", "BEP_READY"), ("hits_bad", "BEP_READY"), ("statistics_nondict", "BEP_READY"), ("target_nondict", "BEP_READY"), ("event_bad", "BEP_EVENT_REJECTED"), ("nested_bad", "BEP_EVENT_REJECTED"), ("completed_nondict", "BEP_EVENT_REJECTED"), ("finished_invalid", "BEP_TERMINAL_REJECTED"), ("finished_then_stream", "BEP_STREAM_REJECTED"), ("hits_then_stream", "BEP_COUNTER_REJECTED"), ("exit_invalid", "BEP_COUNTER_REJECTED"), ("exit_null", "BEP_COUNTER_REJECTED"), ("exit_false", "BEP_COUNTER_REJECTED"), ("exit_true", "BEP_COUNTER_REJECTED"), ("exit_string", "BEP_COUNTER_REJECTED"), ("exit_negative", "BEP_COUNTER_REJECTED"))
        for change, stage in cases:
            with self.subTest(change=change):
                try: cache.phase_record(_payload(change), b"", 0, "prime"); gate_raises = False
                except Exception: gate_raises = True
                self.assertEqual(stage != "BEP_READY", gate_raises)
                self.assertEqual(stage, self._run(change)[0]["stage"])

    def test_anchor_shutdown_and_cleanup_fail_closed(self) -> None:
        with mock.patch.object(probe.cache, "_anchored", side_effect=(False, True, True)): self.assertEqual("POST_RUN_ANCHOR_REJECTED", self._run()[0]["stage"])
        with mock.patch.object(probe.cache, "_anchored", side_effect=(True, False, True, True)): self.assertEqual("POST_PARSE_ANCHOR_REJECTED", self._run()[0]["stage"])
        for change in ("phase_swap", "root_swap", "output_swap", "shutdown_output_swap", "shutdown_root_swap"):
            result, calls, roots = self._run(change)
            self.assertEqual(probe.record(), result); self.assertFalse(roots[0].exists())
            if change.startswith("shutdown_"): self.assertEqual(1, len([call for call in calls if call[-1] == "shutdown"]))
        actual = probe.lifecycle._remove_original
        def failed(*args: object) -> bool: actual(*args); return False
        with mock.patch.object(probe.lifecycle, "_remove_original", side_effect=failed): self.assertEqual(probe.record(), self._run()[0])

    def test_precheck_setup_and_schema_are_closed(self) -> None:
        with mock.patch.object(probe.cleanup, "_clean_git", side_effect=(False, True)), mock.patch.object(probe.cleanup, "_no_slugd", return_value=True): self.assertEqual("PRECHECK_REJECTED", probe.run_probe()["stage"])
        with mock.patch.object(probe.cleanup, "_clean_git", return_value=True), mock.patch.object(probe.cleanup, "_no_slugd", return_value=True), mock.patch.object(probe.tempfile, "mkdtemp", side_effect=OSError): self.assertEqual("SETUP_REJECTED", probe.run_probe()["stage"])
        class DictSubclass(dict): pass
        class StringSubclass(str): pass
        hostile = {**probe.record("STAGE_RECORDED", "ZERO", "BEP_READY"), "secret": "/private"}
        self.assertEqual(probe.record(), probe.normalize(hostile)); self.assertEqual(probe.record(), probe.normalize(DictSubclass(probe.record("STAGE_RECORDED", "ZERO", "BEP_READY"))))
        self.assertEqual(probe.record(), probe.normalize({**probe.record("STAGE_RECORDED", "ZERO", "BEP_READY"), "stage": StringSubclass("/private")}))
        out, err = StringIO(), StringIO()
        with mock.patch.object(cli, "run_probe", return_value=hostile), redirect_stdout(out), redirect_stderr(err): self.assertEqual(1, cli.main([]))
        self.assertEqual("", err.getvalue()); self.assertNotIn("private", out.getvalue()); self.assertEqual(probe.record(), json.loads(out.getvalue()))


if __name__ == "__main__": unittest.main()
