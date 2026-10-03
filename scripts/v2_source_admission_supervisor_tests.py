#!/usr/bin/env python3
"""Small actual Linux supervision sanity gates; no root/network/source workload.

Root serializes these local /tmp fixtures with Rust native fixtures. Every
supervise call is tighter than the production27+3/terminal3 envelope. Do not
launch this file concurrently with another native fixture owner.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

import v2_source_admission_supervisor as s


@unittest.skipUnless(sys.platform == "linux" and hasattr(os, "pidfd_open"),
                     "Linux pidfd supervisor admission")
class SupervisorTests(unittest.TestCase):
    def run_child(self, code: str, *, limits: s.Limits | None = None):
        limits = limits or s.Limits(address_bytes=64 * 1024 ** 2,
                                  rss_bytes=64 * 1024 ** 2,
                                  work_seconds=2, cleanup_seconds=0.4,
                                  terminal_seconds=0.5, poll_seconds=0.005)
        with tempfile.TemporaryDirectory(prefix="slug-supervisor-sanity-") as temp:
            parent = Path(temp)
            log = parent / "preserved.log"
            result = s.supervise([sys.executable, "-I", "-c", code],
                                 stage_parent=parent, log=log, limits=limits)
            output = log.read_text() if log.exists() else ""
            # Actual joins must leave no adopted child to pollute the next gate.
            self.assertTrue(result["joined"], result)
            self.assertEqual(s.children(os.getpid()), set())
            self.assertTrue(result["stage_removed"], result)
            self.assertFalse(any(p.is_dir() for p in parent.iterdir()))
            return result, output

    def test_hard_address_readback_enomem_and_shared_clock(self):
        before = time.monotonic_ns()
        result, output = self.run_child(r'''
import json, os, resource, time
limit = resource.getrlimit(resource.RLIMIT_AS)
refused = False
try:
    payload = bytearray(96 * 1024 ** 2)
except MemoryError:
    refused = True
work = int(os.environ["SLUG_SOURCE_ADMISSION_WORK_DEADLINE_MONOTONIC_NS"])
cleanup = int(os.environ["SLUG_SOURCE_ADMISSION_CLEANUP_DEADLINE_MONOTONIC_NS"])
assert os.environ["TMPDIR"] == os.environ["SLUG_SOURCE_ADMISSION_STAGE_ROOT"]
assert os.environ["PATH"]  # inherited environment is preserved
print(json.dumps({"limit": limit, "enomem": refused, "work": work,
                  "cleanup": cleanup, "now": time.monotonic_ns()}), flush=True)
time.sleep(.1)
''')
        values = json.loads(output)
        self.assertEqual(values["limit"], [64 * 1024 ** 2] * 2)
        self.assertTrue(values["enomem"])
        self.assertLess(before, values["now"])
        self.assertLess(values["now"], values["work"])
        self.assertEqual(values["cleanup"] - values["work"], 400_000_000)
        self.assertTrue(result["supervision_ok"], result)
        self.assertTrue(result["address_space_limit_verified"])
        self.assertEqual(result["limit_kind"], "RLIMIT_AS")
        self.assertFalse(result["source_gate_verified"])
        self.assertFalse(result["kernel_rss_quota"])
        self.assertFalse(result["swap_disabled"])
        self.assertFalse(result["disk_quota"])
        self.assertEqual(result["sampled_swap_peak_bytes"], 0)

    def test_actual_unlinked_fd_hardlink_dedup_and_equality_refusal(self):
        with tempfile.TemporaryDirectory(prefix="slug-supervisor-disk-") as temp:
            root = Path(temp)
            regular = root / "regular"
            regular.write_bytes(b"a" * 8192)
            os.link(regular, root / "hardlink")
            os.symlink("/outside-never-traversed", root / "link")
            erased = root / "erased"
            with erased.open("w+b") as held:
                held.write(b"b" * 4096)
                held.flush()
                erased.unlink()
                deadline = time.monotonic_ns() + 2_000_000_000
                # A dedicated owned quiet process avoids measuring an unrelated
                # test-runner's externally redirected output files.
                with subprocess.Popen([sys.executable, "-I", "-c",
                                       "import sys; sys.stdin.buffer.read(1)"],
                                      stdin=subprocess.PIPE,
                                      stdout=subprocess.DEVNULL,
                                      stderr=subprocess.DEVNULL,
                                      pass_fds=(held.fileno(),)) as probe:
                    try:
                        actual = s.disk_sample(root, [probe.pid], deadline)
                    finally:
                        probe.communicate(b"x", timeout=2)
                # Include root directory and symlink allocated blocks, count
                # regular/hardlink once and held-unlinked inode once.
                expected = sum(p.lstat().st_blocks * 512
                               for p in [root, regular, root / "link"])
                expected += os.fstat(held.fileno()).st_blocks * 512
                self.assertEqual(actual.allocated_bytes, expected)
                self.assertEqual(actual.logical_bytes, 8192 + 4096)
                self.assertEqual(actual.inodes, 4)
                equal = s.Limits(disk_bytes=actual.allocated_bytes)
                self.assertEqual(s.resource_refusal(0, 0, actual, equal),
                                 "sampled_disk_limit")
                roomy = s.Limits(disk_bytes=actual.allocated_bytes + 1)
                self.assertIsNone(s.resource_refusal(0, 0, actual, roomy))
            gone = s.disk_sample(root, [],
                                 time.monotonic_ns() + 2_000_000_000)
            self.assertEqual(gone.logical_bytes, 8192)

    def test_accounting_failure_is_not_a_partial_disk_success(self):
        with tempfile.TemporaryDirectory(prefix="slug-supervisor-account-") as temp:
            root = Path(temp)
            missing = root / "missing"
            with self.assertRaises(s.AccountingError):
                s.disk_sample(missing, [], time.monotonic_ns() + 1_000_000_000)
            with self.assertRaises(s.AccountingError):
                s.disk_sample(root, [2 ** 30], time.monotonic_ns() + 1_000_000_000)
            # Directories cannot be swapped for symlinks as the root capability.
            link = root / "root-link"
            os.symlink(root, link)
            with self.assertRaises(s.AccountingError):
                s.disk_sample(link, [], time.monotonic_ns() + 1_000_000_000)

    def test_expired_cleanup_keeps_owned_stage_and_never_follows_link(self):
        with tempfile.TemporaryDirectory(prefix="slug-supervisor-cleanup-") as temp:
            parent = Path(temp)
            root = parent / "stage"
            root.mkdir()
            outside = parent / "outside"
            outside.write_bytes(b"untouched")
            os.symlink(outside, root / "link")
            identity = (root.stat().st_dev, root.stat().st_ino)
            with self.assertRaisesRegex(s.AccountingError, "terminal_cleanup_deadline"):
                s.remove_owned_stage(root, identity, time.monotonic_ns() - 1)
            self.assertTrue(root.exists())
            self.assertTrue((root / "link").is_symlink())
            s.remove_owned_stage(root, identity, time.monotonic_ns() + 1_000_000_000)
            self.assertFalse(root.exists())
            self.assertEqual(outside.read_bytes(), b"untouched")

    def test_actual_disk_equality_latches_and_joins(self):
        result, _ = self.run_child(r'''
import os, time
p = os.path.join(os.environ["SLUG_SOURCE_ADMISSION_STAGE_ROOT"], "bytes")
with open(p, "wb") as f:
    f.write(b"x" * 65536)
    f.flush()
    time.sleep(1)
''', limits=s.Limits(address_bytes=64 * 1024 ** 2,
                    rss_bytes=64 * 1024 ** 2, disk_bytes=4096,
                    work_seconds=2, cleanup_seconds=.3,
                    terminal_seconds=.5, poll_seconds=.005))
        self.assertEqual(result["refusal"], "sampled_disk_limit")
        self.assertFalse(result["supervision_ok"])
        self.assertTrue(result["term_sent"])
        self.assertGreaterEqual(result["sampled_disk_peak_bytes"], 4096)

    def test_escaped_descendant_and_root_exit_adoption_are_refused_and_joined(self):
        for root_exits in (False, True):
            with self.subTest(root_exits=root_exits):
                code = r'''
import os, time
pid = os.fork()
if pid == 0:
    os.setsid()
    time.sleep(20)
    os._exit(0)
'''
                code += "os._exit(0)\n" if root_exits else "time.sleep(20)\n"
                result, _ = self.run_child(code)
                self.assertEqual(result["refusal"], "unexpected_descendant")
                self.assertGreaterEqual(result["unexpected_descendants"], 1)
                self.assertFalse(result["supervision_ok"])
                self.assertTrue(result["term_sent"])

    def test_term_ignoring_root_kill_and_actual_join_with_one_clock(self):
        result, _ = self.run_child(r'''
import signal, time
signal.signal(signal.SIGTERM, signal.SIG_IGN)
time.sleep(20)
''', limits=s.Limits(address_bytes=64 * 1024 ** 2,
                    rss_bytes=64 * 1024 ** 2, work_seconds=.3,
                    cleanup_seconds=.1, terminal_seconds=.4,
                    poll_seconds=.005))
        self.assertEqual(result["refusal"], "outer_deadline")
        self.assertTrue(result["term_sent"])
        self.assertTrue(result["kill_sent"])
        self.assertFalse(result["supervision_ok"])
        self.assertGreaterEqual(result["elapsed_seconds"], .4)
        self.assertLess(result["elapsed_seconds"], 2)

    def test_no_success_after_nonzero_child_and_no_environment_argv_receipt(self):
        result, _ = self.run_child("raise SystemExit(7)")
        self.assertEqual(result["refusal"], "child_exit_nonzero")
        self.assertEqual(result["child_exit_code"], 7)
        self.assertFalse(result["supervision_ok"])
        self.assertNotIn("command", result)
        self.assertNotIn("environment", result)

    def test_root_pin_setup_failure_still_kills_joins_and_cleans(self):
        with patch.object(s.OwnedProcess, "pin",
                          side_effect=s.AccountingError("forced_pin_refusal")):
            result, _ = self.run_child("import time; time.sleep(20)")
        self.assertEqual(result["refusal"], "forced_pin_refusal")
        self.assertTrue(result["kill_sent"])
        self.assertFalse(result["supervision_ok"])

    def test_oversize_log_refuses_before_external_copy_without_truncation(self):
        result, output = self.run_child("import sys; sys.stdout.write('x' * (1024**2+1))")
        self.assertEqual(result["refusal"], "diagnostic_log_limit")
        self.assertEqual(result["diagnostic_log_bytes"], 1024 ** 2 + 1)
        self.assertFalse(result["diagnostic_log_preserved"])
        self.assertEqual(output, "")
        self.assertFalse(result["supervision_ok"])

    def test_actual_external_writable_accounting_loss_refuses_and_cleans(self):
        with tempfile.TemporaryDirectory(prefix="slug-supervisor-external-") as temp:
            outside = Path(temp) / "outside"
            code = ("import time\n"
                    f"f=open({str(outside)!r},'wb')\n"
                    "f.write(b'x'); f.flush(); time.sleep(20)\n")
            result, _ = self.run_child(code)
        self.assertEqual(result["refusal"], "external_writable_file")
        self.assertFalse(result["supervision_ok"])

    def test_actual_supervisor_term_latches_and_joins_root_exit_escapee(self):
        with tempfile.TemporaryDirectory(prefix="slug-supervisor-term-") as temp:
            parent = Path(temp)
            receipt = parent / "receipt.json"
            log = parent / "child.log"
            # On the supervisor's owned TERM, the root exits and leaves a
            # session-escaped child. Adoption must still be tracked and joined.
            code = r'''
import os, signal, time
def ended(sig, frame):
    child = os.fork()
    if child == 0:
        os.setsid()
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        time.sleep(20)
        os._exit(0)
    os._exit(0)
signal.signal(signal.SIGTERM, ended)
root = os.environ["SLUG_SOURCE_ADMISSION_STAGE_ROOT"]
open(os.path.join(root, "ready"), "wb").close()
time.sleep(20)
'''
            command = [sys.executable, "-I", str(Path(s.__file__).resolve()),
                       "--stage-parent", str(parent), "--receipt", str(receipt),
                       "--log", str(log), "--", sys.executable, "-I", "-c", code]
            with subprocess.Popen(command, stdout=subprocess.DEVNULL,
                                  stderr=subprocess.DEVNULL) as supervisor:
                pidfd = os.pidfd_open(supervisor.pid)
                try:
                    until = time.monotonic() + 4
                    while not any(parent.glob("slug-source-admission-*/ready")):
                        self.assertIsNone(supervisor.poll())
                        self.assertLess(time.monotonic(), until, "ready timeout")
                        time.sleep(.005)
                    signal.pidfd_send_signal(pidfd, signal.SIGTERM)
                    self.assertEqual(supervisor.wait(timeout=5), 1)
                finally:
                    os.close(pidfd)
            result = json.loads(receipt.read_text())
            self.assertEqual(result["refusal"], "supervisor_interrupted")
            self.assertTrue(result["joined"], result)
            self.assertTrue(result["stage_removed"], result)
            self.assertGreaterEqual(result["unexpected_descendants"], 1)
            self.assertFalse(result["supervision_ok"])
            self.assertFalse(any(p.is_dir() for p in parent.iterdir()))
            self.assertEqual(s.children(os.getpid()), set())

    def test_deadline_and_resource_policy_cannot_expand_or_reset(self):
        limits = s.Limits()
        limits.validate()
        d = s.Deadlines.from_start(1_000_000_000, limits)
        self.assertEqual((d.work_ns, d.cleanup_ns, d.terminal_ns),
                         (28_000_000_000, 31_000_000_000, 34_000_000_000))
        self.assertEqual(s.resource_refusal(limits.rss_bytes, 0,
                                           s.DiskSample(0, 0, 0), limits),
                         "sampled_rss_limit")
        self.assertEqual(s.resource_refusal(0, 1, s.DiskSample(0, 0, 0), limits),
                         "sampled_swap_observed")
        for too_big in [s.Limits(address_bytes=4 * s.GIB + 1),
                        s.Limits(disk_bytes=6 * s.GIB + 1),
                        s.Limits(work_seconds=28), s.Limits(cleanup_seconds=4),
                        s.Limits(terminal_seconds=4)]:
            with self.assertRaises(ValueError):
                too_big.validate()


if __name__ == "__main__":
    unittest.main()
