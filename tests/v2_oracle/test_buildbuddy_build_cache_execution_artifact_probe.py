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

from tools.v2_oracle import buildbuddy_build_cache_execution_artifact_probe as cli
from tools.v2_oracle_lib import buildbuddy_build_cache as cache
from tools.v2_oracle_lib import buildbuddy_build_cache_execution_artifact_probe as probe


class ExecutionArtifactProbeTest(unittest.TestCase):
    def _run(self, change: str, code: int = 0) -> tuple[dict[str, object], list[list[str]], list[Path]]:
        calls, roots, saved_roots = [], [], []
        def runner(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]:
            calls.append(argv)
            if argv[-1] == "shutdown":
                output = Path(argv[2].split("=", 1)[1])
                if change == "shutdown_output_swap": output.rename(output.with_name("shutdown-saved-output")); output.mkdir()
                elif change == "shutdown_root_swap":
                    root = output.parents[1]; saved = root.with_name(root.name + "-shutdown-old"); root.rename(saved); root.mkdir(); saved_roots.append(saved)
                return subprocess.CompletedProcess(argv, 0)
            execution = Path(next(x.split("=", 1)[1] for x in argv if x.startswith("--execution_log_json_file=")))
            roots.append(execution.parents[1])
            if change == "replace_nonempty": execution.unlink(); execution.write_bytes(b"x"); execution.chmod(0o600)
            elif change == "replace_empty": execution.unlink(); execution.touch(); execution.chmod(0o600)
            elif change == "retained_nonempty": execution.write_bytes(b"x")
            elif change == "missing": execution.unlink()
            elif change == "symlink": execution.unlink(); execution.symlink_to("/dev/null")
            elif change == "hardlink":
                other = execution.with_name("other"); other.write_bytes(b"x"); other.chmod(0o600); execution.unlink(); os.link(other, execution)
            elif change == "mode": execution.chmod(0o644)
            elif change == "directory": execution.unlink(); execution.mkdir()
            elif change == "phase_swap":
                phase = execution.parent; phase.rename(phase.with_name("old")); phase.mkdir()
            elif change == "root_swap":
                root = execution.parents[1]; saved = root.with_name(root.name + "-old"); root.rename(saved); root.mkdir(); saved_roots.append(saved)
            elif change == "output_symlink":
                output = execution.parent / "output"; output.rmdir(); output.symlink_to(tempfile.gettempdir(), target_is_directory=True)
            elif change == "output_replace":
                output = execution.parent / "output"; output.rename(output.with_name("saved-output")); output.mkdir()
            return subprocess.CompletedProcess(argv, code)
        with mock.patch.object(probe.cleanup, "_clean_git", return_value=True), mock.patch.object(probe.cleanup, "_no_slugd", return_value=True): result = probe.run_probe(runner=runner)
        self.assertTrue(all(not path.exists() for path in saved_roots))
        return result, calls, roots

    def test_retained_and_replaced_regular_files(self) -> None:
        for change, expected in (("retained_empty", "ANCHORED_PRIVATE_EMPTY"), ("retained_nonempty", "ANCHORED_PRIVATE_NONEMPTY"), ("replace_nonempty", "ANCHORED_PRIVATE_NONEMPTY"), ("replace_empty", "ANCHORED_PRIVATE_EMPTY")):
            with self.subTest(change=change):
                result, calls, roots = self._run(change)
                self.assertEqual(("PROBE_RECORDED", "ZERO", expected), (result["classification"], result["process"], result["execution"]))
                self.assertFalse(roots[0].exists())
                prime = calls[0]; nonce = next(x.rsplit("=", 1)[1] for x in prime if "NONCE=" in x)
                execution = Path(next(x.split("=", 1)[1] for x in prime if x.startswith("--execution_log_json_file=")))
                bep = Path(next(x.split("=", 1)[1] for x in prime if x.startswith("--build_event_json_file=")))
                self.assertEqual(cache.command("prime", "bazel", Path(prime[1].split("=", 1)[1]), bep, execution, nonce), prime)

    def test_invalid_artifacts_and_nonzero_are_recorded_conservatively(self) -> None:
        for change in ("missing", "symlink", "hardlink", "mode", "directory"):
            with self.subTest(change=change): self.assertEqual("NOT_ANCHORED_PRIVATE", self._run(change)[0]["execution"])
        result, _, _ = self._run("replace_empty", 7)
        self.assertEqual(("PROBE_RECORDED", "NONZERO", "ANCHORED_PRIVATE_EMPTY"), (result["classification"], result["process"], result["execution"]))

    def test_root_and_phase_swaps_fail_closed_without_shutdown(self) -> None:
        for change in ("phase_swap", "root_swap"):
            result, calls, roots = self._run(change)
            self.assertEqual(probe.record(), result); self.assertEqual([], [x for x in calls if x[-1] == "shutdown"])
            self.assertTrue(all(not root.exists() for root in roots))

    def test_output_swaps_fail_closed_without_shutdown(self) -> None:
        for change in ("output_symlink", "output_replace"):
            result, calls, roots = self._run(change)
            self.assertEqual(probe.record(), result); self.assertEqual([], [x for x in calls if x[-1] == "shutdown"]); self.assertFalse(roots[0].exists())

    def test_shutdown_time_swaps_reject_and_clean_both_roots(self) -> None:
        for change in ("shutdown_output_swap", "shutdown_root_swap"):
            result, calls, roots = self._run(change)
            self.assertEqual(probe.record(), result); self.assertEqual(1, len([x for x in calls if x[-1] == "shutdown"])); self.assertFalse(roots[0].exists())

    def test_never_opens_or_reads_execution_content(self) -> None:
        root = Path(tempfile.mkdtemp()); self.addCleanup(lambda: probe.cleanup._remove_root(root) if root.exists() else None)
        artifact = root / "execution.json"; artifact.write_bytes(b"private bytes"); artifact.chmod(0o600)
        fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY)
        try:
            with mock.patch.object(os, "open", side_effect=AssertionError("open")), mock.patch.object(os, "read", side_effect=AssertionError("read")):
                self.assertEqual("ANCHORED_PRIVATE_NONEMPTY", probe._execution(fd, artifact.name))
        finally: os.close(fd)

    def test_hostile_schema_cli_and_lifecycle_suppress_secrets(self) -> None:
        hostile = {**probe.record("PROBE_RECORDED", "ZERO", "ANCHORED_PRIVATE_EMPTY"), "secret": "/private"}
        self.assertEqual(probe.record(), probe.normalize(hostile))
        out, err = StringIO(), StringIO()
        with mock.patch.object(cli, "run_probe", return_value=hostile), redirect_stdout(out), redirect_stderr(err): self.assertEqual(1, cli.main([]))
        self.assertEqual("", err.getvalue()); self.assertNotIn("private", out.getvalue()); self.assertEqual(probe.record(), json.loads(out.getvalue()))
        class DictSubclass(dict): pass
        class StringSubclass(str):
            def __hash__(self) -> int: return hash("ZERO")
            def __eq__(self, other: object) -> bool: return other == "ZERO"
        for value in (DictSubclass(probe.record("PROBE_RECORDED", "ZERO", "ANCHORED_PRIVATE_EMPTY")), {**probe.record("PROBE_RECORDED", "ZERO", "ANCHORED_PRIVATE_EMPTY"), "process": StringSubclass("/private/process")}):
            out, err = StringIO(), StringIO()
            with mock.patch.object(cli, "run_probe", return_value=value), redirect_stdout(out), redirect_stderr(err): self.assertEqual(1, cli.main([]))
            self.assertEqual("", err.getvalue()); self.assertNotIn("private", out.getvalue()); self.assertEqual(probe.record(), json.loads(out.getvalue()))
        with mock.patch.object(probe.cleanup, "_clean_git", return_value=False): self.assertEqual(probe.record(), probe.run_probe())
        with mock.patch.object(probe.cleanup, "_clean_git", return_value=True), mock.patch.object(probe.cleanup, "_no_slugd", return_value=False): self.assertEqual(probe.record(), probe.run_probe())

    def test_shutdown_and_cleanup_failures_reject(self) -> None:
        def runner(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]: return subprocess.CompletedProcess(argv, 1 if argv[-1] == "shutdown" else 0)
        with mock.patch.object(probe.cleanup, "_clean_git", return_value=True), mock.patch.object(probe.cleanup, "_no_slugd", return_value=True): self.assertEqual(probe.record(), probe.run_probe(runner=runner))
        actual = probe.lifecycle._remove_original
        def failed_cleanup(*args: object) -> bool: actual(*args); return False
        with mock.patch.object(probe.cleanup, "_clean_git", return_value=True), mock.patch.object(probe.cleanup, "_no_slugd", return_value=True), mock.patch.object(probe.lifecycle, "_remove_original", side_effect=failed_cleanup):
            self.assertEqual(probe.record(), probe.run_probe(runner=lambda argv, **_: subprocess.CompletedProcess(argv, 0)))


if __name__ == "__main__": unittest.main()
