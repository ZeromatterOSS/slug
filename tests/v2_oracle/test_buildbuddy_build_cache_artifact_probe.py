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

from tools.v2_oracle import buildbuddy_build_cache_artifact_probe as cli
from tools.v2_oracle_lib import buildbuddy_build_cache as cache
from tools.v2_oracle_lib import buildbuddy_build_cache_artifact_probe as probe


class ArtifactProbeTest(unittest.TestCase):
    def test_all_eight_records_and_exact_command_reuse(self) -> None:
        for exit_code in (0, 7):
            for bep_good in (False, True):
                for execution_good in (False, True):
                    with self.subTest(exit_code=exit_code, bep_good=bep_good, execution_good=execution_good):
                        calls: list[list[str]] = []
                        def runner(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]:
                            calls.append(argv)
                            if argv[-1] == "shutdown": return subprocess.CompletedProcess(argv, 0)
                            paths = {flag: Path(next(x.split("=", 1)[1] for x in argv if x.startswith(flag))) for flag in ("--build_event_json_file=", "--execution_log_json_file=")}
                            if bep_good: paths["--build_event_json_file="].write_bytes(b"evidence")
                            if execution_good: paths["--execution_log_json_file="].write_bytes(b"evidence")
                            return subprocess.CompletedProcess(argv, exit_code)
                        with mock.patch.object(probe, "_clean", return_value=True): result = probe.run_probe(runner=runner)
                        prime = calls[0]
                        nonce = next(item.rsplit("=", 1)[1] for item in prime if "NONCE=" in item)
                        self.assertEqual(cache.command("prime", "bazel", Path(prime[1].split("=", 1)[1]), Path(next(x.split("=", 1)[1] for x in prime if x.startswith("--build_event_json_file="))), Path(next(x.split("=", 1)[1] for x in prime if x.startswith("--execution_log_json_file="))), nonce), prime)
                        self.assertEqual("PROBE_RECORDED", result["classification"])
                        self.assertEqual("ZERO" if exit_code == 0 else "NONZERO", result["process"])
                        self.assertEqual("PRIVATE_REGULAR" if bep_good else "NOT_PRIVATE_REGULAR", result["bep"])
                        self.assertEqual("PRIVATE_REGULAR" if execution_good else "NOT_PRIVATE_REGULAR", result["execution"])

    def test_hostile_schema_and_cli_suppress_secrets(self) -> None:
        hostile = {"schema_version": 1, "mode": probe.MODE, "classification": "PROBE_RECORDED", "process": "ZERO", "bep": "PRIVATE_REGULAR", "execution": "PRIVATE_REGULAR", "secret": "/private"}
        self.assertEqual(probe.record(), probe.normalize(hostile))
        out, err = StringIO(), StringIO()
        with mock.patch.object(cli, "run_probe", return_value=hostile), redirect_stdout(out), redirect_stderr(err): self.assertEqual(1, cli.main([]))
        self.assertEqual("", err.getvalue()); self.assertNotIn("private", out.getvalue()); self.assertEqual(probe.record(), json.loads(out.getvalue()))

    def test_symlink_hardlink_and_replacement_are_metadata_only(self) -> None:
        descriptor, name = tempfile.mkstemp(); os.close(descriptor)
        outside = Path(name); self.addCleanup(outside.unlink); outside.write_text("token=/private")
        for attack in ("symlink", "hardlink", "replacement"):
            with self.subTest(attack=attack):
                def runner(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]:
                    if argv[-1] == "shutdown": return subprocess.CompletedProcess(argv, 0)
                    path = Path(next(x.split("=", 1)[1] for x in argv if x.startswith("--build_event_json_file=")))
                    path.unlink()
                    if attack == "symlink": path.symlink_to(outside)
                    elif attack == "hardlink": os.link(outside, path)
                    else: path.write_text("token=/private")
                    return subprocess.CompletedProcess(argv, 0)
                with mock.patch.object(probe, "_clean", return_value=True): result = probe.run_probe(runner=runner)
                self.assertEqual("NOT_PRIVATE_REGULAR", result["bep"]); self.assertNotIn("private", json.dumps(result))

    def test_artifacts_are_never_opened_or_read(self) -> None:
        original = probe.os.open
        def guarded(name: object, flags: int, *args: object, **kwargs: object) -> int:
            if str(name) in {"bep.json", "execution.json"}: self.fail("artifact opened")
            return original(name, flags, *args, **kwargs)
        def runner(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]: return subprocess.CompletedProcess(argv, 0)
        with mock.patch.object(probe, "_clean", return_value=True), mock.patch.object(probe.os, "open", side_effect=guarded): self.assertEqual("PROBE_RECORDED", probe.run_probe(runner=runner)["classification"])

    def test_lifecycle_cleanup_git_daemon_and_shutdown_fail_closed(self) -> None:
        runner = lambda argv, **_: subprocess.CompletedProcess(argv, 1 if argv[-1] == "shutdown" else 0)
        with mock.patch.object(probe, "_clean", return_value=True): self.assertEqual(probe.record(), probe.run_probe(runner=runner))
        with mock.patch.object(probe, "_clean", return_value=False): self.assertEqual(probe.record(), probe.run_probe(runner=runner))
        actual = probe._remove_original
        def bad(*args: object) -> bool: actual(*args); return False
        with mock.patch.object(probe, "_clean", return_value=True), mock.patch.object(probe, "_remove_original", side_effect=bad): self.assertEqual(probe.record(), probe.run_probe(runner=lambda argv, **_: subprocess.CompletedProcess(argv, 0)))

    def test_phase_swap_skips_shutdown_and_rejects(self) -> None:
        shutdowns: list[list[str]] = []
        def runner(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]:
            if argv[-1] == "shutdown": shutdowns.append(argv); return subprocess.CompletedProcess(argv, 0)
            phase = Path(argv[1].split("=", 1)[1]).parent
            saved = phase.with_name("saved"); phase.rename(saved); phase.mkdir()
            return subprocess.CompletedProcess(argv, 0)
        with mock.patch.object(probe, "_clean", return_value=True): self.assertEqual(probe.record(), probe.run_probe(runner=runner))
        self.assertEqual([], shutdowns)

    def test_root_swap_removes_original_and_preserves_replacement(self) -> None:
        shutdowns: list[list[str]] = []; paths: list[tuple[Path, Path]] = []
        def runner(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]:
            if argv[-1] == "shutdown": shutdowns.append(argv); return subprocess.CompletedProcess(argv, 0)
            root = Path(argv[1].split("=", 1)[1]).parents[1]; saved = root.with_name(root.name + "-saved")
            root.rename(saved); root.mkdir(); paths.append((root, saved))
            return subprocess.CompletedProcess(argv, 0)
        with mock.patch.object(probe, "_clean", return_value=True): self.assertEqual(probe.record(), probe.run_probe(runner=runner))
        replacement, original = paths[0]
        self.addCleanup(replacement.rmdir); self.assertTrue(replacement.is_dir()); self.assertFalse(original.exists()); self.assertEqual([], shutdowns)


if __name__ == "__main__": unittest.main()
