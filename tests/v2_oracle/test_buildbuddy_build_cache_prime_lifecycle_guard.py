from __future__ import annotations

import json
import os
import subprocess
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from io import StringIO
from pathlib import Path
from unittest import mock

from tools.v2_oracle import buildbuddy_build_cache_prime_lifecycle_guard as cli
from tools.v2_oracle_lib import buildbuddy_build_cache_prime_lifecycle_guard as guard
from tools.v2_oracle_lib import buildbuddy_build_cache_prime_output_semantics_probe as child


class LifecycleGuardTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.roots = mock.patch.object(guard.tempfile, "gettempdir", return_value=self.temp.name)
        self.roots.start()
        self.addCleanup(self.roots.stop)
        self.addCleanup(self.temp.cleanup)

    @staticmethod
    def _json(value: object) -> bytes:
        return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()

    def _child(self, value: object = None, code: int = 0, stderr: bytes = b"", **expected: object) -> dict[str, object]:
        calls: list[tuple[list[str], dict[str, object]]] = []
        payload = child.record("STAGE_RECORDED", "ZERO", "PRIME_READY") if value is None else value
        def runner(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            calls.append((argv, kwargs))
            kwargs["stdout"].write(self._json(payload))
            kwargs["stderr"].write(stderr)
            return subprocess.CompletedProcess(argv, code)
        with mock.patch.object(guard.cleanup, "_clean_git", return_value=True), mock.patch.object(guard.cleanup, "_no_slugd", return_value=True):
            result = guard.run_guard(runner=runner)
        self.assertEqual(expected.get("lifecycle", "LIFECYCLE_CLEAN"), result["lifecycle"])
        return {"result": result, "calls": calls}

    def test_clean_invokes_exactly_one_anonymous_child_with_inherited_environment(self) -> None:
        run = self._child()
        result, calls = run["result"], run["calls"]
        self.assertEqual("STAGE_RECORDED", result["child"]["classification"])
        self.assertEqual(1, len(calls))
        argv, kwargs = calls[0]
        self.assertEqual(["python3", str(guard.REPO_ROOT / "tools/v2_oracle/buildbuddy_build_cache_prime_output_semantics_probe.py")], argv)
        self.assertEqual(guard.REPO_ROOT, kwargs["cwd"]); self.assertFalse(kwargs["shell"]); self.assertFalse(kwargs["check"])
        self.assertNotIn("env", kwargs); self.assertFalse(hasattr(kwargs["stdout"], "name") and isinstance(kwargs["stdout"].name, str))

    def test_every_valid_child_stage_process_pair_is_clean(self) -> None:
        for process, stages in (("NONZERO", child.NONZERO_STAGES), ("ZERO", child.ZERO_STAGES)):
            for stage in stages:
                value = child.record("STAGE_RECORDED", process, stage)
                with self.subTest(process=process, stage=stage):
                    self.assertEqual("LIFECYCLE_CLEAN", self._child(value)["result"]["lifecycle"])

    def test_preexisting_root_and_precheck_short_circuit(self) -> None:
        (self.root / "slug-buildbuddy-prime-old").mkdir()
        with mock.patch.object(guard.cleanup, "_clean_git", return_value=True), mock.patch.object(guard.cleanup, "_no_slugd", return_value=True), mock.patch.object(guard, "subprocess") as process:
            result = guard.run_guard()
        self.assertEqual("PRECHECK_REJECTED", result["lifecycle"]); process.run.assert_not_called()
        for git, daemon in ((False, True), (True, False)):
            git_values = (git, True) if not git else (True, True)
            daemon_values = (daemon, True) if not daemon else (True, True)
            with self.subTest(git=git), mock.patch.object(guard.cleanup, "_clean_git", side_effect=git_values), mock.patch.object(guard.cleanup, "_no_slugd", side_effect=daemon_values):
                self.assertEqual("PRECHECK_REJECTED", guard.run_guard()["lifecycle"])

    def test_child_rejections_suppress_child_stage(self) -> None:
        cases = ((1, b"", None), (0, b"x", None), (0, b"", b"{"), (0, b"", {"secret": "/private"}), (0, b"", b"x" * 2049))
        for code, stderr, payload in cases:
            with self.subTest(code=code, stderr=stderr, payload=payload):
                if isinstance(payload, bytes):
                    def runner(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
                        kwargs["stdout"].write(payload); kwargs["stderr"].write(stderr); return subprocess.CompletedProcess(argv, code)
                    with mock.patch.object(guard.cleanup, "_clean_git", return_value=True), mock.patch.object(guard.cleanup, "_no_slugd", return_value=True): result = guard.run_guard(runner=runner)
                else:
                    result = self._child(payload, code, stderr, lifecycle="CHILD_REJECTED")["result"]
                self.assertEqual("CHILD_REJECTED", result["lifecycle"]); self.assertEqual(child.record(), result["child"])
        with mock.patch.object(guard.cleanup, "_clean_git", return_value=True), mock.patch.object(guard.cleanup, "_no_slugd", return_value=True):
            self.assertEqual("CHILD_REJECTED", guard.run_guard(runner=mock.Mock(side_effect=OSError))["lifecycle"])

    def test_child_requires_exact_cli_json_bytes(self) -> None:
        valid = child.record("STAGE_RECORDED", "ZERO", "PRIME_READY")
        payloads = (
            json.dumps(valid).encode() + b"\n", self._json(valid).rstrip(), b" " + self._json(valid), self._json(valid) + b"\n",
        )
        for payload in payloads:
            def runner(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
                kwargs["stdout"].write(payload); return subprocess.CompletedProcess(argv, 0)
            with self.subTest(payload=payload[:1]), mock.patch.object(guard.cleanup, "_clean_git", return_value=True), mock.patch.object(guard.cleanup, "_no_slugd", return_value=True):
                self.assertEqual("CHILD_REJECTED", guard.run_guard(runner=runner)["lifecycle"])

    def test_single_residue_is_removed_and_multiple_or_hostile_roots_reject(self) -> None:
        def residue(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            kwargs["stdout"].write(self._json(child.record("STAGE_RECORDED", "ZERO", "PRIME_READY")))
            (self.root / "slug-buildbuddy-prime-one").mkdir()
            return subprocess.CompletedProcess(argv, 0)
        with mock.patch.object(guard.cleanup, "_clean_git", return_value=True), mock.patch.object(guard.cleanup, "_no_slugd", return_value=True):
            result = guard.run_guard(runner=residue)
        self.assertEqual("ROOT_RESIDUE_REMOVED", result["lifecycle"]); self.assertEqual(child.record(), result["child"])
        for name in ("slug-buildbuddy-prime-a", "slug-buildbuddy-prime-b"):
            (self.root / name).mkdir()
        with mock.patch.object(guard.cleanup, "_clean_git", return_value=True), mock.patch.object(guard.cleanup, "_no_slugd", return_value=True):
            self.assertEqual("PRECHECK_REJECTED", guard.run_guard()["lifecycle"])

    def test_new_multiple_hostile_and_child_failure_residue_reject(self) -> None:
        def roots(kind: str, code: int = 0) -> subprocess.CompletedProcess[bytes]:
            def runner(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
                kwargs["stdout"].write(self._json(child.record("STAGE_RECORDED", "ZERO", "PRIME_READY")))
                if kind == "multiple":
                    (self.root / "slug-buildbuddy-prime-a").mkdir(); (self.root / "slug-buildbuddy-prime-b").mkdir()
                elif kind == "hostile":
                    (self.root / "slug-buildbuddy-prime-link").symlink_to("/dev/null")
                else:
                    (self.root / "slug-buildbuddy-prime-child").mkdir()
                return subprocess.CompletedProcess(argv, code)
            return runner
        for kind, code in (("multiple", 0), ("hostile", 0)):
            with self.subTest(kind=kind), mock.patch.object(guard.cleanup, "_clean_git", return_value=True), mock.patch.object(guard.cleanup, "_no_slugd", return_value=True):
                self.assertEqual("ROOT_RESIDUE_REJECTED", guard.run_guard(runner=roots(kind, code))["lifecycle"])
            for path in self.root.iterdir():
                path.unlink() if path.is_symlink() else path.rmdir()
        with mock.patch.object(guard.cleanup, "_clean_git", return_value=True), mock.patch.object(guard.cleanup, "_no_slugd", return_value=True):
            self.assertEqual("ROOT_RESIDUE_REJECTED", guard.run_guard(runner=roots("child", 7))["lifecycle"])
        for path in self.root.iterdir(): path.rmdir()
        (self.root / "slug-buildbuddy-prime-link").symlink_to("/dev/null")
        with mock.patch.object(guard.cleanup, "_clean_git", return_value=True), mock.patch.object(guard.cleanup, "_no_slugd", return_value=True):
            self.assertEqual("PRECHECK_REJECTED", guard.run_guard()["lifecycle"])

    def test_cleanup_failure_false_success_and_postcheck_drift_fail_closed(self) -> None:
        def residue(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            kwargs["stdout"].write(self._json(child.record("STAGE_RECORDED", "ZERO", "PRIME_READY"))); (self.root / "slug-buildbuddy-prime-one").mkdir(); return subprocess.CompletedProcess(argv, 0)
        with mock.patch.object(guard.cleanup, "_clean_git", return_value=True), mock.patch.object(guard.cleanup, "_no_slugd", return_value=True), mock.patch.object(guard, "_remove_original", return_value=False):
            self.assertEqual("ROOT_RESIDUE_REJECTED", guard.run_guard(runner=residue)["lifecycle"])
        (self.root / "slug-buildbuddy-prime-one").rmdir()
        with mock.patch.object(guard.cleanup, "_clean_git", return_value=True), mock.patch.object(guard.cleanup, "_no_slugd", return_value=True), mock.patch.object(guard, "_remove_original", side_effect=OSError):
            self.assertEqual("ROOT_RESIDUE_REJECTED", guard.run_guard(runner=residue)["lifecycle"])
        (self.root / "slug-buildbuddy-prime-one").rmdir()
        def false_success(path: Path, _: tuple[int, int]) -> bool:
            path.rename(path.with_name("old-root")); path.mkdir(); return True
        with mock.patch.object(guard.cleanup, "_clean_git", return_value=True), mock.patch.object(guard.cleanup, "_no_slugd", return_value=True), mock.patch.object(guard, "_remove_original", side_effect=false_success):
            self.assertEqual("ROOT_RESIDUE_REJECTED", guard.run_guard(runner=residue)["lifecycle"])
        for path in self.root.iterdir():
            path.rmdir()
        def clean_child(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            kwargs["stdout"].write(self._json(child.record("STAGE_RECORDED", "ZERO", "PRIME_READY")))
            return subprocess.CompletedProcess(argv, 0)
        with mock.patch.object(guard.cleanup, "_clean_git", side_effect=(True, False)), mock.patch.object(guard.cleanup, "_no_slugd", return_value=True):
            self.assertEqual("POSTCHECK_REJECTED", guard.run_guard(runner=clean_child)["lifecycle"])
        with mock.patch.object(guard.cleanup, "_clean_git", return_value=True), mock.patch.object(guard.cleanup, "_no_slugd", side_effect=(True, False)):
            self.assertEqual("POSTCHECK_REJECTED", guard.run_guard(runner=clean_child)["lifecycle"])

    def test_scan_to_cleanup_replacement_rejects_without_deleting_replacement(self) -> None:
        def residue(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            kwargs["stdout"].write(self._json(child.record("STAGE_RECORDED", "ZERO", "PRIME_READY")))
            (self.root / "slug-buildbuddy-prime-one").mkdir()
            return subprocess.CompletedProcess(argv, 0)
        actual = guard._remove_original
        def replace(path: Path, identity: tuple[int, int]) -> bool:
            path.rename(path.with_name("original")); path.mkdir()
            return actual(path, identity)
        with mock.patch.object(guard.cleanup, "_clean_git", return_value=True), mock.patch.object(guard.cleanup, "_no_slugd", return_value=True), mock.patch.object(guard, "_remove_original", side_effect=replace):
            self.assertEqual("ROOT_RESIDUE_REJECTED", guard.run_guard(runner=residue)["lifecycle"])
        self.assertTrue((self.root / "slug-buildbuddy-prime-one").is_dir())
        self.assertTrue((self.root / "original").is_dir())
        (self.root / "slug-buildbuddy-prime-one").rmdir(); (self.root / "original").rmdir()

    def test_schema_subclasses_cli_and_secret_suppression(self) -> None:
        class DictSubclass(dict): pass
        class StringSubclass(str): pass
        hostile = {**guard.record("LIFECYCLE_RECORDED", "LIFECYCLE_CLEAN", child.record("STAGE_RECORDED", "ZERO", "PRIME_READY")), "secret": "/private"}
        self.assertEqual(guard.record(), guard.normalize(hostile)); self.assertEqual(guard.record(), guard.normalize(DictSubclass(guard.record())))
        self.assertEqual(guard.record(), guard.normalize({**guard.record(), "schema_version": True}))
        self.assertEqual(guard.record(), guard.normalize({**guard.record(), "classification": StringSubclass("SANITIZER_REJECTED")}))
        self.assertEqual(guard.record(), guard.record(StringSubclass("LIFECYCLE_RECORDED"), "LIFECYCLE_CLEAN", child.record("STAGE_RECORDED", "ZERO", "PRIME_READY")))
        self.assertEqual(guard.record(), guard.record("LIFECYCLE_RECORDED", StringSubclass("LIFECYCLE_CLEAN"), child.record("STAGE_RECORDED", "ZERO", "PRIME_READY")))
        out, err = StringIO(), StringIO()
        with mock.patch.object(cli, "run_guard", return_value=hostile), redirect_stdout(out), redirect_stderr(err): self.assertEqual(1, cli.main([]))
        self.assertEqual("", err.getvalue()); self.assertNotIn("private", out.getvalue()); self.assertEqual(guard.record(), json.loads(out.getvalue()))
        clean = guard.record("LIFECYCLE_RECORDED", "LIFECYCLE_CLEAN", child.record("STAGE_RECORDED", "ZERO", "PRIME_READY"))
        out, err = StringIO(), StringIO()
        with mock.patch.object(cli, "run_guard", return_value=clean), redirect_stdout(out), redirect_stderr(err): self.assertEqual(0, cli.main([]))
        self.assertEqual("", err.getvalue()); self.assertEqual(clean, json.loads(out.getvalue()))
        with redirect_stdout(StringIO()), redirect_stderr(StringIO()): self.assertEqual(1, cli.main(["unexpected"]))

    def test_fixed_lifecycle_schema_suppresses_all_nonclean_child_records(self) -> None:
        staged = child.record("STAGE_RECORDED", "ZERO", "PRIME_READY")
        for lifecycle in guard.LIFECYCLES:
            value = guard.record("LIFECYCLE_RECORDED", lifecycle, staged)
            with self.subTest(lifecycle=lifecycle):
                self.assertEqual(staged if lifecycle == "LIFECYCLE_CLEAN" else child.record(), value["child"])
        self.assertEqual(guard.record(), guard.record("SANITIZER_REJECTED", "LIFECYCLE_CLEAN", staged))
        self.assertEqual(guard.record(), guard.record("LIFECYCLE_RECORDED", "NOT_RECORDED", staged))

    def test_observation_exceptions_fail_closed(self) -> None:
        with mock.patch.object(guard, "_roots", side_effect=RuntimeError), mock.patch.object(guard.cleanup, "_clean_git", return_value=True), mock.patch.object(guard.cleanup, "_no_slugd", return_value=True):
            self.assertEqual("PRECHECK_REJECTED", guard.run_guard()["lifecycle"])
        with mock.patch.object(guard.cleanup, "_clean_git", side_effect=RuntimeError):
            self.assertEqual("POSTCHECK_REJECTED", guard.run_guard()["lifecycle"])

    def test_clean_checks_git_and_daemon_independently(self) -> None:
        with mock.patch.object(guard.cleanup, "_clean_git", return_value=False) as git, mock.patch.object(guard.cleanup, "_no_slugd", return_value=True) as daemon:
            self.assertFalse(guard._clean()); git.assert_called_once_with(); daemon.assert_called_once_with()
        with mock.patch.object(guard.cleanup, "_clean_git", side_effect=RuntimeError) as git, mock.patch.object(guard.cleanup, "_no_slugd", side_effect=RuntimeError) as daemon:
            self.assertFalse(guard._clean()); git.assert_called_once_with(); daemon.assert_called_once_with()


if __name__ == "__main__":
    unittest.main()
