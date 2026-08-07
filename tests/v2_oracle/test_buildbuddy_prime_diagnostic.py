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

from tools.v2_oracle import buildbuddy_prime_diagnostic as cli
from tools.v2_oracle_lib import buildbuddy_prime_diagnostic as diagnostic


ROOT = Path(__file__).resolve().parents[2]


class BuildBuddyPrimeDiagnosticTest(unittest.TestCase):
    def test_sanitize_accepts_only_five_exact_payloads(self) -> None:
        for flag, diagnosis in diagnostic.MAPPINGS.items():
            for suffix in (b"", b"\n", b"\r\n"):
                path = self._file(f"ERROR: {flag} :: Unrecognized option: {flag}".encode() + suffix)
                self.assertEqual(diagnosis, diagnostic.sanitize(path))
        for flag in ("--remote_cache=private", "--other", *diagnostic.MAPPINGS):
            self.assertEqual(diagnostic.NONE, diagnostic.sanitize(self._file(f"ERROR: {flag} :: Unrecognized option: {flag}=value".encode())))

    def test_sanitize_rejects_near_misses_and_private_data(self) -> None:
        flag = next(iter(diagnostic.MAPPINGS))
        payload = f"ERROR: {flag} :: Unrecognized option: {flag}".encode()
        cases = (payload + b"\nextra", payload + b"\n\n", payload.replace(flag.encode(), b"--other", 1),
                 payload + b" https://private/token", b"ERROR: /private/path :: Unrecognized option: /private/path",
                 b"ERROR: token=secret :: Unrecognized option: token=secret", b"\xff", b"x" * (diagnostic.MAX_STDERR_BYTES + 1))
        for value in cases:
            self.assertEqual(diagnostic.NONE, diagnostic.sanitize(self._file(value)))
        fifo = Path(tempfile.mkdtemp()) / "stderr"
        os.mkfifo(fifo)
        self.addCleanup(lambda: fifo.parent.rmdir())
        self.addCleanup(lambda: fifo.unlink(missing_ok=True))
        self.assertEqual(diagnostic.NONE, diagnostic.sanitize(fifo))
        target = self._file(payload)
        link = target.with_name(target.name + "-link")
        link.symlink_to(target)
        self.addCleanup(lambda: link.unlink(missing_ok=True))
        self.assertEqual(diagnostic.NONE, diagnostic.sanitize(link))

    def test_run_reuses_frozen_prime_argv_and_only_reads_stderr_on_exit_two(self) -> None:
        calls: list[list[str]] = []
        roots: list[Path] = []
        flag = "--noremote_accept_cached"

        def runner(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            calls.append(argv)
            if argv[-1] == "shutdown":
                return subprocess.CompletedProcess(argv, 0)
            stderr = kwargs["stderr"]
            stdout = kwargs["stdout"]
            self.assertEqual(0o600, stat.S_IMODE(os.fstat(stderr.fileno()).st_mode))
            self.assertEqual(0o600, stat.S_IMODE(os.fstat(stdout.fileno()).st_mode))
            stderr.write(f"ERROR: {flag} :: Unrecognized option: {flag}\n".encode())
            for option in ("--build_event_json_file=", "--execution_log_json_file="):
                path = Path(next(item.split("=", 1)[1] for item in argv if item.startswith(option)))
                self.assertEqual(0o600, stat.S_IMODE(path.stat().st_mode))
                path.write_bytes(b"token=private/unread")
            roots.append(Path(argv[1].split("=", 1)[1]).parent)
            return subprocess.CompletedProcess(argv, 2)

        with mock.patch.object(diagnostic, "_clean_git", return_value=True), mock.patch.object(diagnostic, "_no_slugd", return_value=True):
            result = diagnostic.run(runner=runner)
        self.assertEqual({"schema_version": 1, "classification": "NORMAL_RC_PRIME_DIAGNOSED", "diagnosis": diagnostic.MAPPINGS[flag]}, result)
        prime = calls[0]
        nonces = [item.split("=", 2)[2] for item in prime if "CACHE_GATE_NONCE=" in item]
        self.assertEqual(2, len(nonces))
        self.assertEqual(1, len(set(nonces)))
        self.assertRegex(nonces[0], r"^[0-9a-f]{64}$")
        self.assertEqual(diagnostic.buildbuddy_cache.command("prime", "bazel", Path(prime[1].split("=", 1)[1]), Path(next(x.split("=", 1)[1] for x in prime if x.startswith("--build_event_json_file="))), Path(next(x.split("=", 1)[1] for x in prime if x.startswith("--execution_log_json_file="))), nonces[0], diagnostic.LABELS), prime)
        self.assertEqual(["//app/slug_cli_v2:slug"], [item for item in prime if item.startswith("//")])
        self.assertNotIn("--ignore_all_rc_files", prime)
        self.assertTrue(roots and all(not root.exists() for root in roots))

    def test_every_frozen_non_mapping_prime_flag_is_rejected(self) -> None:
        argv = diagnostic.buildbuddy_cache.command("prime", "bazel", Path("/private/output"), Path("/private/bep"), Path("/private/execution"), "0" * 64, diagnostic.LABELS)
        for flag in (item for item in argv if item.startswith("--") and item not in diagnostic.MAPPINGS):
            self.assertEqual(diagnostic.NONE, diagnostic.sanitize(self._file(f"ERROR: {flag} :: Unrecognized option: {flag}".encode())))

    def test_non_exit_two_and_cleanup_fail_closed(self) -> None:
        def runner(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]:
            return subprocess.CompletedProcess(argv, 7 if argv[-1] != "shutdown" else 0)
        with mock.patch.object(diagnostic, "_clean_git", return_value=True), mock.patch.object(diagnostic, "_no_slugd", return_value=True):
            self.assertEqual(diagnostic.record("NORMAL_RC_PRIME_UNEXPLAINED"), diagnostic.run(runner=runner))
        def malformed(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            if argv[-1] == "shutdown":
                return subprocess.CompletedProcess(argv, 0)
            kwargs["stderr"].write(b"ERROR: token=private/path :: Unrecognized option: token=private/path")
            return subprocess.CompletedProcess(argv, 2)
        with mock.patch.object(diagnostic, "_clean_git", return_value=True), mock.patch.object(diagnostic, "_no_slugd", return_value=True):
            self.assertEqual(diagnostic.record("SANITIZER_REJECTED"), diagnostic.run(runner=malformed))
        def bad_shutdown(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]:
            return subprocess.CompletedProcess(argv, 1)
        with mock.patch.object(diagnostic, "_clean_git", return_value=True), mock.patch.object(diagnostic, "_no_slugd", return_value=True):
            self.assertEqual(diagnostic.record("SANITIZER_REJECTED"), diagnostic.run(runner=bad_shutdown))

    def test_read_only_cleanup_and_swapped_symlink_fail_closed(self) -> None:
        root = Path(tempfile.mkdtemp(prefix="slug-buildbuddy-prime-"))
        child = root / "child"
        child.write_bytes(b"private")
        child.chmod(0o400)
        root.chmod(0o500)
        self.assertTrue(diagnostic._remove_root(root))
        self.assertFalse(root.exists())
        outside = Path(tempfile.mkdtemp(prefix="prime-diagnostic-outside-"))
        marker = outside / "marker"
        marker.write_bytes(b"preserve")
        mode = stat.S_IMODE(outside.stat().st_mode)
        swapped = Path(tempfile.gettempdir()) / f"slug-buildbuddy-prime-swapped-{os.getpid()}"
        swapped.symlink_to(outside, target_is_directory=True)
        self.addCleanup(lambda: outside.rmdir())
        self.addCleanup(lambda: marker.unlink(missing_ok=True))
        self.addCleanup(lambda: swapped.unlink(missing_ok=True))
        self.assertFalse(diagnostic._remove_root(swapped))
        self.assertEqual(b"preserve", marker.read_bytes())
        self.assertEqual(mode, stat.S_IMODE(outside.stat().st_mode))

    def test_nested_directory_swap_is_preserved_and_fails_closed(self) -> None:
        root = Path(tempfile.mkdtemp(prefix="slug-buildbuddy-prime-"))
        child = root / "child"
        child.mkdir()
        saved = Path(tempfile.mkdtemp(prefix="prime-diagnostic-saved-"))
        saved.rmdir()
        outside = Path(tempfile.mkdtemp(prefix="prime-diagnostic-external-"))
        outside_inode = outside.stat().st_ino
        original_stat = diagnostic.os.stat
        swapped = False
        child_stats = 0

        def swap_before_check(name: object, *args: object, **kwargs: object) -> os.stat_result:
            nonlocal child_stats, swapped
            if name == "child" and kwargs.get("dir_fd") is not None:
                child_stats += 1
                if child_stats == 2:
                    child.rename(saved)
                    outside.rename(child)
                    swapped = True
            return original_stat(name, *args, **kwargs)

        with mock.patch.object(diagnostic.os, "stat", side_effect=swap_before_check):
            self.assertFalse(diagnostic._remove_root(root))
        self.assertTrue(swapped)
        self.assertEqual(outside_inode, child.stat().st_ino)
        child.rename(outside)
        saved.rename(child)
        self.assertTrue(diagnostic._remove_root(root))
        outside.rmdir()

    def test_closed_record_and_cli_hide_errors(self) -> None:
        for classification in ("NORMAL_RC_PRIME_DIAGNOSED", "NORMAL_RC_PRIME_UNEXPLAINED", "SANITIZER_REJECTED"):
            item = diagnostic.record(classification)
            self.assertEqual({"schema_version", "classification", "diagnosis"}, set(item))
            self.assertNotIn("secret", json.dumps(item))
        self.assertEqual(diagnostic.record("SANITIZER_REJECTED"), diagnostic.record("NORMAL_RC_PRIME_DIAGNOSED"))
        valid = next(iter(diagnostic.MAPPINGS.values()))
        self.assertEqual("NORMAL_RC_PRIME_DIAGNOSED", diagnostic.record("NORMAL_RC_PRIME_DIAGNOSED", valid)["classification"])
        self.assertEqual(diagnostic.record("SANITIZER_REJECTED"), diagnostic.record("token=private/path", "nonce"))
        stdout, stderr = StringIO(), StringIO()
        with mock.patch.object(cli, "run", side_effect=RuntimeError("token=private/path")), redirect_stdout(stdout), redirect_stderr(stderr):
            self.assertEqual(1, cli.main([]))
        self.assertEqual("", stderr.getvalue())
        self.assertEqual(diagnostic.record("SANITIZER_REJECTED"), json.loads(stdout.getvalue()))
        stdout, stderr = StringIO(), StringIO()
        malicious = {"schema_version": 1, "classification": "NORMAL_RC_PRIME_DIAGNOSED", "diagnosis": "token=/private/path", "path": "/private"}
        with mock.patch.object(cli, "run", return_value=malicious), redirect_stdout(stdout), redirect_stderr(stderr):
            self.assertEqual(1, cli.main([]))
        self.assertEqual("", stderr.getvalue())
        self.assertEqual(diagnostic.record("SANITIZER_REJECTED"), json.loads(stdout.getvalue()))

    def _file(self, value: bytes) -> Path:
        path = ROOT / "target" / f"prime-diagnostic-{len(self._cleanup)}"
        path.parent.mkdir(exist_ok=True)
        path.write_bytes(value)
        self._cleanup.append(path)
        self.addCleanup(lambda: path.unlink(missing_ok=True))
        return path

    @property
    def _cleanup(self) -> list[Path]:
        if not hasattr(self, "__cleanup"):
            self.__cleanup: list[Path] = []
        return self.__cleanup


if __name__ == "__main__":
    unittest.main()
