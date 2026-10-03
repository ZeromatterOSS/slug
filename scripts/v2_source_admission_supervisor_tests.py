#!/usr/bin/env python3
"""Small actual Linux supervision sanity gates; no root/network/source workload.

Root serializes these local /tmp fixtures with Rust native fixtures. Every
supervise call is tighter than the production27+3/terminal3 envelope. Do not
launch this file concurrently with another native fixture owner.
"""
from __future__ import annotations

import errno
from contextlib import contextmanager
import json
import os
from pathlib import Path
import resource
import select
import signal
import subprocess
import sys
import tempfile
import time
from types import SimpleNamespace
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

    @contextmanager
    def census_child(self, churn=False):
        """Actual direct fresh-session child; every assertion path owns its join."""
        with tempfile.TemporaryDirectory(prefix="slug-stopped-census-") as temp:
            root = Path(temp)
            (root / "regular").write_bytes(b"a" * 8192)
            os.link(root / "regular", root / "hardlink")
            code = r'''
import os, sys, threading
root = sys.argv[1]
held = open(os.path.join(root, "erased"), "w+b")
held.write(b"b" * 4096); held.flush()
os.unlink(held.name)
running = threading.Event(); running.set()
count = [0]
def churn(index):
    path = os.path.join(root, "volatile" + str(index))
    while running.is_set():
        fd = os.open(path, os.O_CREAT | os.O_RDWR, 0o600)
        duplicate = os.dup(fd)
        os.unlink(path)
        os.close(duplicate); os.close(fd)
        count[0] += 1
threads = []
if sys.argv[2] == "1":
    threads = [threading.Thread(target=churn, args=(i,)) for i in range(2)]
    for thread in threads: thread.start()
print(held.fileno(), flush=True)
while True:
    command = sys.stdin.buffer.read(1)
    if command == b"P": print(count[0], flush=True)
    elif command == b"E": os._exit(0)
    else: break
running.clear()
for thread in threads: thread.join()
held.close()
'''
            parent_pid = os.getpid()
            probe = subprocess.Popen(
                [sys.executable, "-I", "-c", code, str(root), "1" if churn else "0"],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL, start_new_session=True,
                preexec_fn=lambda: s.parent_death_guard(parent_pid))
            owner = None
            try:
                self.assertTrue(select.select([probe.stdout], [], [], 2)[0])
                held_fd = int(probe.stdout.readline(32))
                owner = s.OwnedProcess.pin(probe.pid, {os.getpid()})
                yield probe, owner, root, held_fd
            finally:
                if owner is not None:
                    owner.signal(signal.SIGCONT)
                try:
                    probe.communicate(b"Q", timeout=2)
                except subprocess.TimeoutExpired:
                    probe.kill()  # Still-unreaped direct test child only.
                    probe.communicate(timeout=2)
                if owner is not None:
                    owner.close()
                self.assertEqual(s.children(os.getpid()), set())

    def test_stopped_multithread_census_fresh_cycles_exact_totals_and_progress(self):
        with self.census_child(True) as (probe, owner, root, held_fd):
            cycles = s.CensusCycles()
            def progress():
                probe.stdin.write(b"P"); probe.stdin.flush()
                self.assertTrue(select.select([probe.stdout], [], [], 2)[0])
                return int(probe.stdout.readline(32))
            previous = progress()
            for _ in range(2):
                scans = []
                def census(pids, deadline, *, deadline_reason):
                    self.assertEqual(s.process_stat(probe.pid).state, "T")
                    self.assertEqual(pids, [probe.pid])
                    self.assertIsNone(os.waitid(os.P_PIDFD, owner.pidfd,
                                               os.WSTOPPED | os.WNOHANG))
                    expected = (root.stat().st_blocks
                                + (root / "regular").stat().st_blocks
                                + os.stat(f"/proc/{probe.pid}/fd/{held_fd}").st_blocks) * 512
                    scans.append(deadline)
                    actual = s.disk_sample(root, pids, deadline,
                                           deadline_reason=deadline_reason)
                    self.assertEqual(actual.allocated_bytes, expected)
                    self.assertEqual(actual.logical_bytes, 8192 + 4096)
                    return actual
                deadline = time.monotonic_ns() + 100_000_000
                s.stopped_disk_census(owner, census,
                                      lambda: self.assertEqual(s.children(probe.pid), set()),
                                      deadline, lambda: False, cycles)
                self.assertEqual(scans, [deadline])
                until = time.monotonic_ns() + 2_000_000_000
                now = progress()
                while now <= previous:
                    self.assertLess(time.monotonic_ns(), until)
                    now = progress()
                previous = now
            self.assertEqual((cycles.attempts, cycles.completions, cycles.resume_failures),
                             (2, 2, 0))
            self.assertGreater(cycles.duration_ns, 0)
            self.assertLessEqual(cycles.max_duration_ns, cycles.duration_ns)

    def test_stopped_absent_wrong_prior_and_expired_ack_never_scan(self):
        for case in ("absent", "wrong", "prior", "expired"):
            with self.subTest(case=case), self.census_child() as (probe, owner, root, _):
                cycles = s.CensusCycles()
                calls = []
                real_wait = os.waitid
                def wait(kind, fd, flags):
                    result = real_wait(kind, fd, flags)
                    if flags & os.WSTOPPED:
                        if case == "absent": return None
                        if case == "wrong" and result is not None:
                            return SimpleNamespace(si_pid=probe.pid,
                                                   si_code=os.CLD_TRAPPED,
                                                   si_status=signal.SIGSTOP)
                    return result
                if case == "prior":
                    owner.signal(signal.SIGSTOP)
                    until = time.monotonic_ns() + 100_000_000
                    while real_wait(os.P_PIDFD, owner.pidfd,
                                    os.WSTOPPED | os.WNOHANG | os.WNOWAIT) is None:
                        self.assertLess(time.monotonic_ns(), until)
                deadline = time.monotonic_ns() + (0 if case == "expired" else 100_000_000)
                with patch.object(s.os, "waitid", side_effect=wait):
                    with self.assertRaises(s.AccountingError) as caught:
                        s.stopped_disk_census(owner,
                                              lambda *_: calls.append(1), lambda: None,
                                              deadline, lambda: False, cycles)
                self.assertIn(str(caught.exception),
                              ("census_prior_stop", "census_stop_unverified", "disk_census_bound"))
                self.assertEqual(calls, [])
                self.assertEqual(cycles.completions, 0)
                if case == "prior":
                    owner.signal(signal.SIGCONT)
                probe.stdin.write(b"P"); probe.stdin.flush()
                self.assertTrue(select.select([probe.stdout], [], [], 2)[0])
                self.assertEqual(probe.stdout.readline(32), b"0\n")

    def test_stopped_scan_failure_resume_failure_and_term_preserve_primary(self):
        first = s.DiskFailure(s.DiskSite.ROOT_OPEN, s.DiskOrigin.OS_ERROR, errno.ENOENT)
        for case in ("scan", "resume", "term", "late"):
            with self.subTest(case=case):
                real_disk = s.disk_sample
                real_signal = s.OwnedProcess.signal
                triggered = []
                def census(root, pids, deadline, expected, *, deadline_reason):
                    if pids and not triggered:
                        triggered.append(True)
                        self.assertEqual(s.process_stat(pids[0]).state, "T")
                        if case == "term":
                            os.kill(os.getpid(), signal.SIGTERM)
                        elif case == "late":
                            # A ready operation crossing the SAME bound cannot
                            # be admitted; do not reset it on return/resume.
                            while time.monotonic_ns() < deadline:
                                os.sched_yield()
                        else:
                            return real_disk(root / "missing", pids, deadline, expected,
                                             deadline_reason=deadline_reason)
                    return real_disk(root, pids, deadline, expected,
                                     deadline_reason=deadline_reason)
                def sent(owner, sig):
                    if case == "resume" and sig == signal.SIGCONT and triggered:
                        raise s.AccountingError("forced_resume_boundary")
                    return real_signal(owner, sig)
                with patch.object(s, "disk_sample", side_effect=census), \
                        patch.object(s.OwnedProcess, "signal", new=sent):
                    result, _ = self.run_child("import time; time.sleep(20)")
                self.assertEqual(len(triggered), 1)
                self.assertFalse(result["supervision_ok"])
                self.assertEqual(result["census_cycle_completions"], 0)
                if case in ("scan", "resume"):
                    self.assertEqual(result["refusal"], "disk_accounting_unavailable")
                    self.assertEqual((result["disk_failure_site"], result["disk_failure_origin"],
                                      result["disk_failure_errno"]),
                                     (int(first.site), int(first.origin), first.errno))
                else:
                    self.assertEqual(result["refusal"], "supervisor_interrupted" if
                                     case == "term" else "disk_census_bound")
                self.assertEqual(result["census_resume_failures"], int(case == "resume"))
                if case == "resume": self.assertTrue(result["kill_sent"])
                if case == "term":
                    self.assertTrue(result["term_sent"])
                    self.assertFalse(result["kill_sent"])
                    self.assertEqual(result["child_exit_code"], -signal.SIGTERM)

    def test_stopped_actual_exit_is_unreaped_complete_stage_only(self):
        with self.census_child() as (probe, owner, root, _):
            real_signal = owner.signal
            def exit_before_stop(sig):
                if sig == signal.SIGSTOP:
                    probe.stdin.write(b"E"); probe.stdin.flush()
                    self.assertTrue(select.select([owner.pidfd], [], [], .08)[0])
                return real_signal(sig)
            scans = []
            def census(pids, deadline, *, deadline_reason):
                scans.append(pids)
                self.assertIsNone(probe.returncode)
                return s.disk_sample(root, pids, deadline,
                                     deadline_reason=deadline_reason)
            cycles = s.CensusCycles()
            with patch.object(owner, "signal", side_effect=exit_before_stop):
                actual = s.stopped_disk_census(owner, census, lambda: None,
                                               time.monotonic_ns()+100_000_000,
                                               lambda: False, cycles)
            self.assertEqual(scans, [[]])
            self.assertEqual(actual.logical_bytes, 8192)
            self.assertEqual(cycles.completions, 1)
            self.assertIsNone(probe.returncode)

    def test_disk_leaf_deadline_producer_reason_and_invalid_code_before_io(self):
        with tempfile.TemporaryDirectory(prefix="slug-census-deadline-") as temp:
            root = Path(temp)
            for reason in (None, "disk_census_bound", "outer_deadline"):
                with self.subTest(reason=reason), self.assertRaises(s.AccountingError) as caught:
                    if reason is None:
                        s.disk_sample(root, [], 0)
                    else:
                        s.disk_sample(root, [], 0, deadline_reason=reason)
                self.assertEqual(str(caught.exception), reason or "disk_census_bound")
                self.assertIsNone(caught.exception.detail)
            # Preserve a primary native source failure; never remap it later
            # merely because the chosen clock is also past.
            with self.assertRaises(s.AccountingError) as caught:
                s.disk_sample(root / "missing", [], 0, deadline_reason="outer_deadline")
            self.assertEqual(caught.exception.detail,
                             s.DiskFailure(s.DiskSite.ROOT_OPEN,
                                           s.DiskOrigin.OS_ERROR, errno.ENOENT))
            for invalid in (None, True, 1, "wrong"):
                cycles = s.CensusCycles()
                with patch.object(s.os, "open", side_effect=AssertionError("unexpected IO")) as opened:
                    with self.assertRaises(ValueError):
                        s.disk_sample(root, [], 0, deadline_reason=invalid)
                    with self.assertRaises(ValueError):
                        s.stopped_disk_census(None, None, None, 0, None, cycles,
                                               deadline_reason=invalid)
                    opened.assert_not_called()
                self.assertEqual(cycles.attempts, 0)

    def test_guard_setup_and_ordinary_executable_boundaries_fail_closed(self):
        healthy, output = self.run_child(
            "import ctypes; value=ctypes.c_int(); "
            "assert ctypes.CDLL(None).prctl(2,ctypes.byref(value),0,0,0)==0; "
            "print(value.value, flush=True)")
        self.assertTrue(healthy["parent_death_signal_verified"])
        self.assertTrue(healthy["supervision_ok"], healthy)
        self.assertEqual(int(output), signal.SIGKILL)
        with tempfile.TemporaryDirectory(prefix="slug-exec-admission-") as temp:
            path = Path(temp) / "ordinary"
            path.write_bytes(b"not executed")
            path.chmod(0o4755)
            with self.assertRaises(s.AccountingError) as caught:
                s.ordinary_executable(str(path))
            self.assertEqual(str(caught.exception), "ordinary_executable_required")
            path.chmod(0o2755)
            with self.assertRaises(s.AccountingError): s.ordinary_executable(str(path))
            path.chmod(0o755)
            with patch.object(s.os, "getxattr", return_value=b"capability"):
                with self.assertRaises(s.AccountingError) as caught:
                    s.ordinary_executable(str(path))
            self.assertEqual(str(caught.exception), "privileged_executable_refused")
            with patch.object(s.os, "getxattr", side_effect=OSError(errno.EACCES, "secret")):
                with self.assertRaises(s.AccountingError) as caught:
                    s.ordinary_executable(str(path))
            self.assertEqual(str(caught.exception), "executable_capability_unavailable")
            with self.assertRaises(s.AccountingError):
                s.ordinary_executable(str(path / "absent"))
        parent = os.getpid()
        for case in ("install", "readback", "parent"):
            def prctl(option, argument, *_):
                if option == 1: return -1 if case == "install" else 0
                argument._obj.value = 0 if case == "readback" else signal.SIGKILL
                return 0
            with self.subTest(case=case), \
                    patch.object(s.ctypes, "CDLL", return_value=SimpleNamespace(prctl=prctl)), \
                    patch.object(s.os, "getppid", return_value=parent + int(case == "parent")):
                with self.assertRaises(s.AccountingError) as caught:
                    s.parent_death_guard(parent)
                self.assertEqual(str(caught.exception), "parent_death_guard_unverified")
        # A real preexec refusal must not launch a child workload or leave it.
        with patch.object(s, "parent_death_guard",
                          side_effect=s.AccountingError("forced_guard_setup")):
            result, output = self.run_child("print('must-not-execute', flush=True)")
        self.assertEqual(output, "")
        self.assertFalse(result["parent_death_signal_verified"])
        self.assertEqual(result["census_cycle_attempts"], 0)
        self.assertFalse(result["supervision_ok"])

    def test_first_resume_failure_refuses_then_actually_kills_and_joins(self):
        real_signal = s.OwnedProcess.signal
        def sent(owner, sig):
            if sig == signal.SIGCONT:
                raise s.AccountingError("forced_resume_boundary")
            return real_signal(owner, sig)
        with patch.object(s.OwnedProcess, "signal", new=sent):
            result, _ = self.run_child("import time; time.sleep(20)")
        self.assertEqual(result["refusal"], "census_resume_failed")
        self.assertEqual(result["census_resume_failures"], 1)
        self.assertEqual(result["census_cycle_completions"], 0)
        self.assertEqual(result["disk_failure_site"], 0)
        self.assertTrue(result["kill_sent"])
        self.assertTrue(result["joined"])

    def test_fatal_stopped_supervisor_parent_guard_outer_join_before_cleanup(self):
        # Dedicated outer subreaper joins both its supervisor and the adopted
        # stopped root. The ordinary supervisor cannot claim fatal cleanup.
        outer = r'''
import json, os, pathlib, select, signal, subprocess, sys, time
sys.path.insert(0, sys.argv[1])
import v2_source_admission_supervisor as s
s.install_subreaper()
parent = pathlib.Path(sys.argv[2])
outer_pid = os.getpid()
report_r, report_w = os.pipe2(os.O_CLOEXEC)
inner_code = r"""
import os, pathlib, sys
sys.path.insert(0, sys.argv[1])
import v2_source_admission_supervisor as s
parent = pathlib.Path(sys.argv[2]); report = int(sys.argv[3])
real = s.disk_sample
def scan(root, pids, deadline, expected, *, deadline_reason):
    if pids:
        os.write(report, str(pids[0]).encode() + b"\n")
        # Actual barrier was acknowledged before this hook. No fake stop.
        import select
        select.select([], [], [], 20)
    return real(root, pids, deadline, expected, deadline_reason=deadline_reason)
s.disk_sample = scan
result = s.supervise([sys.executable, '-I', '-c', 'import time; time.sleep(20)'],
                     stage_parent=parent, log=parent/'preserved.log')
(parent/'must-not-publish.json').write_text(str(result))
"""
inner = subprocess.Popen([sys.executable, '-I', '-c', inner_code,
                          sys.argv[1], str(parent), str(report_w)],
                         pass_fds=(report_w,), stdout=subprocess.DEVNULL,
                         stderr=subprocess.DEVNULL,
                         preexec_fn=lambda: s.parent_death_guard(outer_pid))
os.close(report_w)
inner_fd = os.pidfd_open(inner.pid)
root_owner = None
try:
    assert select.select([report_r], [], [], 3)[0]
    root_pid = int(os.read(report_r, 32))
    root_owner = s.OwnedProcess.pin(root_pid, {inner.pid})
    assert s.process_stat(root_pid).state == 'T'
    stages = list(parent.glob('slug-source-admission-*'))
    assert len(stages) == 1
    stage = stages[0]; metadata = stage.lstat()
    identity = (metadata.st_dev, metadata.st_ino)
    signal.pidfd_send_signal(inner_fd, signal.SIGKILL)
    assert inner.wait(timeout=2) == -signal.SIGKILL
    assert stage.exists()  # No fatal-finally stage-cleanup claim.
    assert select.select([root_owner.pidfd], [], [], 2)[0]
    joined, status = os.waitpid(root_pid, 0)
    assert joined == root_pid and os.WIFSIGNALED(status)
    assert os.WTERMSIG(status) == signal.SIGKILL
    assert not s.children(os.getpid())
    assert not (parent/'must-not-publish.json').exists()
    s.remove_owned_stage(stage, identity, time.monotonic_ns()+1_000_000_000)
    assert not stage.exists()
    print(json.dumps({'guard_killed': True, 'outer_joined': True,
                      'retained_until_join': True, 'test_stage_removed': True}))
finally:
    if inner.returncode is None:
        signal.pidfd_send_signal(inner_fd, signal.SIGKILL)
        inner.wait(timeout=2)
    if root_owner is not None:
        root_owner.signal(signal.SIGKILL)
        assert select.select([root_owner.pidfd], [], [], 2)[0]
        try: os.waitpid(root_owner.identity.pid, 0)
        except ChildProcessError: pass
        root_owner.close()
    # A setup/assertion failure before pinning may still have adopted a root.
    # Every such PID is now our actual child, not a numeric/user PID target.
    until = time.monotonic_ns()+2_000_000_000
    for pid in s.children(os.getpid()):
        owned = s.OwnedProcess.pin(pid, {os.getpid()})
        try:
            owned.signal(signal.SIGKILL)
            assert select.select([owned.pidfd], [], [],
                                 max(0, (until-time.monotonic_ns())/1e9))[0]
            os.waitpid(pid, 0)
        finally: owned.close()
    os.close(inner_fd); os.close(report_r)
'''
        with tempfile.TemporaryDirectory(prefix="slug-fatal-census-") as temp:
            self.assertEqual(s.children(os.getpid()), set())
            s.install_subreaper()
            caller = os.getpid()
            probe = subprocess.Popen([sys.executable, "-I", "-c", outer,
                                      str(Path(s.__file__).resolve().parent), temp],
                                     stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                     preexec_fn=lambda: s.parent_death_guard(caller))
            try:
                stdout, stderr = probe.communicate(timeout=8)
            except subprocess.TimeoutExpired:
                probe.kill(); probe.communicate(timeout=2)
                until = time.monotonic_ns()+2_000_000_000
                for pid in s.children(os.getpid()):
                    owned = s.OwnedProcess.pin(pid, {os.getpid()})
                    try:
                        owned.signal(signal.SIGKILL)
                        self.assertTrue(select.select([owned.pidfd], [], [],
                            max(0, (until-time.monotonic_ns())/1e9))[0])
                        os.waitpid(pid, 0)
                    finally:
                        owned.close()
                self.fail("fatal fixture exceeded its bounded ownership interval")
            self.assertEqual(probe.returncode, 0, stderr)
            self.assertEqual(json.loads(stdout),
                             {"guard_killed": True, "outer_joined": True,
                              "retained_until_join": True, "test_stage_removed": True})
            self.assertEqual(s.children(os.getpid()), set())

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

    def test_missing_root_and_redacted_projection_are_exact_and_closed(self):
        with tempfile.TemporaryDirectory(prefix="slug-disk-detail-") as temp:
            with self.assertRaises(s.AccountingError) as caught:
                s.disk_sample(Path(temp) / "missing-secret", [],
                              time.monotonic_ns() + 1_000_000_000)
            self.assertEqual(caught.exception.detail,
                             s.DiskFailure(s.DiskSite.ROOT_OPEN,
                                           s.DiskOrigin.OS_ERROR, errno.ENOENT))
            self.assertEqual(str(caught.exception), "disk_accounting_unavailable")
        for secret in ("credential-one", "credential-two"):
            def denied():
                raise PermissionError(errno.EACCES, secret, "/private-secret")
            with self.assertRaises(s.AccountingError) as caught:
                s._disk_call(s.DiskSite.PROC_FD_INFO, denied)
            self.assertEqual(caught.exception.detail,
                             s.DiskFailure(s.DiskSite.PROC_FD_INFO,
                                           s.DiskOrigin.OS_ERROR, errno.EACCES))
            self.assertIsNone(caught.exception.__cause__)
            self.assertIsNone(caught.exception.__context__)
            self.assertNotIn(secret, repr(caught.exception))
            self.assertNotIn("private", repr(caught.exception.detail))
        for flags in ("other: 1", "flags: not-octal-secret"):
            with self.assertRaises(s.AccountingError) as caught:
                s._disk_call(s.DiskSite.FLAGS_PARSE, s._disk_access, flags)
            self.assertEqual(caught.exception.detail,
                             s.DiskFailure(s.DiskSite.FLAGS_PARSE,
                                           s.DiskOrigin.INVALID_METADATA))
        # Keep the original parser domain: extra tokens are ignored and a
        # truncated operand retains its inherited unclassified IndexError.
        self.assertEqual(s._disk_access("flags: 0 extra"), os.O_RDONLY)
        with self.assertRaises(IndexError):
            s._disk_call(s.DiskSite.FLAGS_PARSE, s._disk_access, "flags:")
        with self.assertRaises(ValueError):
            s.DiskFailure(999, s.DiskOrigin.OS_ERROR, errno.EACCES)
        with self.assertRaises(ValueError):
            s.DiskFailure(s.DiskSite.ROOT_OPEN, s.DiskOrigin.OS_ERROR, True)
        with self.assertRaises(ValueError):
            s.DiskFailure(s.DiskSite.ROOT_OPEN, s.DiskOrigin.OS_ERROR, 2 ** 40)
        with self.assertRaises(AttributeError):
            caught.exception.detail.errno = 7

    def proc_close_fixture(self, exit_child: bool):
        # Dedicated actual child retains its regular FD until the census has
        # obtained that real proc entry. No replacement stat or synthetic errno.
        with tempfile.TemporaryDirectory(prefix="slug-disk-handoff-") as temp:
            root = Path(temp)
            leaf = root / "held"
            leaf.write_bytes(b"abc")
            code = r'''
import os, sys
held = open(sys.argv[1], "rb")
print(held.fileno(), flush=True)
command = sys.stdin.buffer.read(1)
if command == b"E":
    os._exit(0)
if command == b"C":
    held.close()
    print("closed", flush=True)
    sys.stdin.buffer.read(1)
'''
            probe = subprocess.Popen([sys.executable, "-I", "-c", code, str(leaf)],
                                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     stderr=subprocess.DEVNULL)
            owner = None
            try:
                self.assertTrue(select.select([probe.stdout], [], [], 2)[0])
                number = int(probe.stdout.readline(32))
                owner = s.OwnedProcess.pin(probe.pid, {os.getpid()})
                real_scandir = s.os.scandir
                handoffs = []
                observed_errors = []
                def scanned(path):
                    iterator = real_scandir(path)
                    if path != f"/proc/{probe.pid}/fd":
                        return iterator
                    class Gate:
                        def __enter__(self):
                            return self
                        def __exit__(self, *args):
                            iterator.close()
                        def __iter__(self):
                            return self
                        def __next__(self):
                            entry = next(iterator)
                            if entry.name == str(number):
                                handoffs.append(entry.name)
                                probe.stdin.write(b"E" if exit_child else b"C")
                                probe.stdin.flush()
                                if exit_child:
                                    self_outer.assertTrue(select.select([owner.pidfd], [], [], 2)[0])
                                    # Observe actual exit without reaping the PID.
                                    exited = os.waitid(os.P_PID, probe.pid,
                                                      os.WEXITED | os.WNOWAIT)
                                    self_outer.assertEqual(exited.si_pid, probe.pid)
                                    self_outer.assertIsNone(probe.returncode)
                                else:
                                    self_outer.assertTrue(select.select([probe.stdout], [], [], 2)[0])
                                    self_outer.assertEqual(probe.stdout.readline(32), b"closed\n")
                                    self_outer.assertFalse(owner.exited())
                            return entry
                    return Gate()
                self_outer = self
                def census(pids):
                    try:
                        return s.disk_sample(root, pids, time.monotonic_ns() + 1_000_000_000)
                    except s.AccountingError as error:
                        observed_errors.append(error.detail)
                        raise
                positive = census([probe.pid])
                self.assertEqual(positive.logical_bytes, 3)
                with patch.object(s.os, "scandir", side_effect=scanned):
                    if exit_child:
                        complete = s._complete_disk_census(census, [probe.pid],
                                                         lambda pid: owner.exited())
                        self.assertEqual(complete.logical_bytes, 3)
                        self.assertEqual(complete.allocated_bytes, positive.allocated_bytes)
                        # No diagnostic follows this genuine complete sample.
                        self.assertFalse(hasattr(complete, "detail"))
                    else:
                        with self.assertRaises(s.AccountingError) as caught:
                            s._complete_disk_census(census, [probe.pid],
                                                    lambda pid: owner.exited())
                        self.assertEqual(caught.exception.detail,
                                         s.DiskFailure(s.DiskSite.PROC_FD_STAT,
                                                       s.DiskOrigin.OS_ERROR, errno.ENOENT))
                self.assertEqual(len(handoffs), 1)
                self.assertEqual(observed_errors,
                                 [s.DiskFailure(s.DiskSite.PROC_FD_STAT,
                                                s.DiskOrigin.OS_ERROR, errno.ENOENT)])
            finally:
                try:
                    probe.communicate(b"Q", timeout=2)
                except subprocess.TimeoutExpired:
                    probe.kill()  # Only this still-unreaped direct test child.
                    probe.communicate(timeout=2)
                if owner is not None:
                    owner.close()
            self.assertEqual(probe.returncode, 0)
            self.assertEqual(s.children(os.getpid()), set())

    def test_actual_proc_fd_close_after_enumeration_refuses_no_partial_sample(self):
        self.proc_close_fixture(False)

    def test_actual_full_exit_discards_failed_census_detail(self):
        self.proc_close_fixture(True)

    def test_child_census_sites_close_precedence_and_redaction(self):
        context, owner = s.ChildContext.ACTIVE_DISCOVERY, s.ChildOwner.ROOT
        denied = PermissionError(errno.EACCES, "credential-secret", "/private-secret")
        missing = FileNotFoundError(errno.ENOENT, "read-secret", "/task-secret")

        class Tasks:
            def __init__(self, rows, close_error=None):
                self.rows, self.close_error, self.closed = iter(rows), close_error, False
            def __enter__(self): return self
            def __iter__(self): return self
            def __next__(self):
                value = next(self.rows)
                if isinstance(value, BaseException): raise value
                return SimpleNamespace(path=value)
            def __exit__(self, *_):
                self.closed = True
                if self.close_error is not None: raise self.close_error

        # Iterator loss after a valid row and parse loss after one valid PID
        # must still refuse instead of publishing any partially accumulated set.
        cases = [
            ("open", None, denied, [], s.ChildSite.TASK_OPEN, errno.EACCES),
            ("next", Tasks(["one", missing]), None, ["7"], s.ChildSite.TASK_NEXT, errno.ENOENT),
            ("read", Tasks(["one"]), None, [missing], s.ChildSite.CHILDREN_READ, errno.ENOENT),
            ("parse", Tasks(["one"]), None, ["7 invalid-secret"], s.ChildSite.PID_PARSE, None),
            ("close", Tasks(["one"], denied), None, ["7"], s.ChildSite.TASK_CLOSE, errno.EACCES),
            ("close-after-read", Tasks(["one"], denied), None, [missing], s.ChildSite.TASK_CLOSE, errno.EACCES),
            ("close-after-unrelated", Tasks(["one"], denied), None,
             [RuntimeError("unrelated-secret")], s.ChildSite.TASK_CLOSE, errno.EACCES),
        ]
        for name, tasks, opening, reads, site, number in cases:
            with self.subTest(case=name), \
                    patch.object(s.os, "scandir", return_value=tasks, side_effect=opening), \
                    patch.object(s.Path, "read_text", side_effect=reads):
                with self.assertRaises(s.AccountingError) as caught:
                    s.children(123, context=context, owner=owner)
            origin = s.ChildOrigin.INVALID_METADATA if number is None else s.ChildOrigin.OS_ERROR
            self.assertEqual(caught.exception.detail, s.ChildFailure(site, origin, context, owner, number))
            self.assertEqual(caught.exception.args, ("child_accounting_unavailable",))
            self.assertIsNone(caught.exception.__cause__)
            self.assertIsNone(caught.exception.__context__)
            self.assertNotIn("secret", repr(caught.exception) + repr(caught.exception.detail))
            if tasks is not None: self.assertTrue(tasks.closed)
        tasks = Tasks(["one", "two"])
        with patch.object(s.os, "scandir", return_value=tasks), \
                patch.object(s.Path, "read_text", side_effect=["+7 0008 7", "9 -3"]):
            self.assertEqual(s.children(123), {7, 8, 9, -3})  # Original int parser domain.
        self.assertTrue(tasks.closed)
        tasks = Tasks(["one"])
        unrelated = RuntimeError("original-domain")
        with patch.object(s.os, "scandir", return_value=tasks), \
                patch.object(s.Path, "read_text", side_effect=unrelated):
            with self.assertRaises(RuntimeError) as caught:
                s.children(123)
        self.assertIs(caught.exception, unrelated)
        self.assertTrue(tasks.closed)

    def test_child_diagnostic_scalar_and_caller_domains(self):
        values = [s.ChildSite.CHILDREN_READ, s.ChildOrigin.OS_ERROR,
                  s.ChildContext.ACTIVE_DISCOVERY, s.ChildOwner.ROOT, errno.ENOENT]
        for index, invalid in ((0, 3), (1, 1), (2, 2), (3, 2), (4, True), (4, 2 ** 40)):
            changed = values.copy(); changed[index] = invalid
            with self.subTest(index=index, invalid=invalid), self.assertRaises(ValueError):
                s.ChildFailure(*changed)
        for kwargs in ({"context": 2}, {"owner": 2}):
            with patch.object(s.os, "scandir") as scan, self.assertRaises(ValueError):
                s.children(123, **kwargs)
            scan.assert_not_called()
        with self.assertRaises(ValueError):
            s.AccountingError("child_accounting_unavailable", SimpleNamespace(**{}))

    def test_actual_thread_retirement_after_enumeration_refuses_no_partial_children(self):
        code = r'''
import sys, threading
release = threading.Event()
worker = threading.Thread(target=release.wait)
worker.start()
print(worker.native_id, flush=True)
while True:
    command = sys.stdin.buffer.read(1)
    if command in (b"R", b"E", b""):
        release.set(); worker.join(timeout=2)
        assert not worker.is_alive()
        if command == b"R": print("retired", flush=True)
        else: break
'''
        parent_pid = os.getpid()
        probe = subprocess.Popen([sys.executable, "-I", "-c", code],
                                 stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                 stderr=subprocess.DEVNULL, start_new_session=True,
                                 preexec_fn=lambda: s.parent_death_guard(parent_pid))
        pinned = None
        try:
            self.assertTrue(select.select([probe.stdout], [], [], 2)[0])
            tid = int(probe.stdout.readline(32))
            pinned = s.OwnedProcess.pin(probe.pid, {parent_pid})
            target = Path(f"/proc/{probe.pid}/task/{tid}/children")
            self.assertTrue(target.exists())
            real_read = s.Path.read_text
            retired = []
            def retire_then_read(path, *args, **kwargs):
                if path == target and not retired:
                    # children() obtained this real task entry before the hook.
                    probe.stdin.write(b"R"); probe.stdin.flush()
                    self.assertTrue(select.select([probe.stdout], [], [], 2)[0])
                    self.assertEqual(probe.stdout.readline(32), b"retired\n")
                    self.assertFalse(target.exists())  # Actual worker join, no fake ENOENT.
                    retired.append(True)
                return real_read(path, *args, **kwargs)
            with patch.object(s.Path, "read_text", autospec=True, side_effect=retire_then_read):
                with self.assertRaises(s.AccountingError) as caught:
                    s.children(probe.pid, context=s.ChildContext.ACTIVE_DISCOVERY,
                               owner=s.ChildOwner.ROOT)
            self.assertEqual(retired, [True])
            self.assertEqual(caught.exception.detail,
                             s.ChildFailure(s.ChildSite.CHILDREN_READ, s.ChildOrigin.OS_ERROR,
                                            s.ChildContext.ACTIVE_DISCOVERY, s.ChildOwner.ROOT,
                                            errno.ENOENT))
            self.assertIsNone(caught.exception.__cause__)
            self.assertIsNone(caught.exception.__context__)
        finally:
            # Every assertion path releases/joins the worker and the actual child.
            try:
                probe.communicate(b"E", timeout=2)
            except subprocess.TimeoutExpired:
                probe.kill()  # Still-unreaped direct fixture child only.
                probe.communicate(timeout=2)
            if pinned is not None: pinned.close()
            self.assertEqual(s.children(os.getpid()), set())
        self.assertEqual(probe.returncode, 0)

    def test_child_contexts_and_cross_kind_first_detail_survive_actual_cleanup(self):
        real_children, real_preserve = s.children, s.preserve_log
        calls = []
        def traverse(pid, **kwargs):
            calls.append((kwargs.get("context"), kwargs.get("owner")))
            return real_children(pid, **kwargs)
        with patch.object(s, "children", side_effect=traverse):
            healthy, _ = self.run_child("import time; time.sleep(.05)")
        self.assertTrue(healthy["supervision_ok"], healthy)
        self.assertIn((s.ChildContext.ISOLATION, s.ChildOwner.SUPERVISOR), calls)
        self.assertIn((s.ChildContext.ACTIVE_DISCOVERY, s.ChildOwner.ROOT), calls)
        self.assertIn((s.ChildContext.STOPPED_DISCOVERY, s.ChildOwner.ROOT), calls)
        self.assertIn((s.ChildContext.JOIN, s.ChildOwner.SUPERVISOR), calls)
        first = s.ChildFailure(s.ChildSite.CHILDREN_READ, s.ChildOrigin.OS_ERROR,
                               s.ChildContext.ACTIVE_DISCOVERY, s.ChildOwner.ROOT, errno.ENOENT)
        disk = s.DiskFailure(s.DiskSite.ROOT_OPEN, s.DiskOrigin.OS_ERROR, errno.EACCES)
        def cleanup_child(*args):
            real_preserve(*args)
            raise s.AccountingError("child_accounting_unavailable", first)
        def cleanup_disk(*args):
            real_preserve(*args)
            raise s.AccountingError("disk_accounting_unavailable", disk)
        injected = []
        def refuse_once(pid, **kwargs):
            calls.append((kwargs.get("context"), kwargs.get("owner")))
            if (kwargs.get("context") == s.ChildContext.ACTIVE_DISCOVERY
                    and kwargs.get("owner") == s.ChildOwner.ROOT and not injected):
                injected.append(True)
                raise s.AccountingError("child_accounting_unavailable", first)
            return real_children(pid, **kwargs)
        with patch.object(s, "children", side_effect=refuse_once), \
                patch.object(s, "preserve_log", side_effect=cleanup_disk):
            child_first, _ = self.run_child("import time; time.sleep(20)")
        self.assertEqual(injected, [True])
        self.assertIn((s.ChildContext.TERMINAL_DISCOVERY, s.ChildOwner.SUPERVISOR), calls)
        self.assertTrue(child_first["term_sent"])
        self.assertEqual(child_first["child_exit_code"], -signal.SIGTERM)
        self.assertEqual(child_first["refusal"], "child_accounting_unavailable")
        child_fields = ("child_failure_site", "child_failure_origin", "child_failure_context",
                        "child_failure_owner", "child_failure_errno")
        disk_fields = ("disk_failure_site", "disk_failure_origin", "disk_failure_errno")
        self.assertEqual(tuple(child_first[k] for k in child_fields), (3, 1, 2, 2, errno.ENOENT))
        self.assertEqual(tuple(child_first[k] for k in disk_fields), (0, 0, None))
        with patch.object(s, "disk_sample",
                          side_effect=s.AccountingError("disk_accounting_unavailable", disk)), \
                patch.object(s, "preserve_log", side_effect=cleanup_child):
            disk_first, _ = self.run_child("import time; time.sleep(20)")
        self.assertEqual(disk_first["refusal"], "disk_accounting_unavailable")
        self.assertEqual(tuple(disk_first[k] for k in disk_fields), (1, 1, errno.EACCES))
        self.assertEqual(tuple(disk_first[k] for k in child_fields), (0, 0, 0, 0, None))
        with patch.object(s.OwnedProcess, "pin", side_effect=s.AccountingError("first_setup")), \
                patch.object(s, "preserve_log", side_effect=cleanup_child):
            unrelated_first, _ = self.run_child("import time; time.sleep(20)")
        self.assertEqual(unrelated_first["refusal"], "first_setup")
        self.assertEqual(tuple(unrelated_first[k] for k in child_fields), (0, 0, 0, 0, None))
        for result in (healthy, child_first, disk_first, unrelated_first):
            self.assertNotIn("command", result)
            self.assertNotIn("environment", result)
            self.assertFalse(result["source_gate_verified"])

    def test_first_accounting_detail_survives_later_cleanup_and_unrelated_first(self):
        first = s.DiskFailure(s.DiskSite.PROC_FD_STAT, s.DiskOrigin.OS_ERROR, errno.ENOENT)
        later = s.DiskFailure(s.DiskSite.ROOT_OPEN, s.DiskOrigin.OS_ERROR, errno.EACCES)
        real_preserve = s.preserve_log
        def cleanup(*args):
            real_preserve(*args)
            raise s.AccountingError("later_cleanup", later)
        with patch.object(s, "disk_sample",
                          side_effect=s.AccountingError("disk_accounting_unavailable", first)), \
                patch.object(s, "preserve_log", side_effect=cleanup):
            result, _ = self.run_child("import time; time.sleep(20)")
        self.assertEqual(result["refusal"], "disk_accounting_unavailable")
        self.assertEqual((result["disk_failure_site"], result["disk_failure_origin"],
                          result["disk_failure_errno"]), (8, 1, errno.ENOENT))
        with patch.object(s.OwnedProcess, "pin",
                          side_effect=s.AccountingError("first_setup")), \
                patch.object(s, "preserve_log", side_effect=cleanup):
            result, _ = self.run_child("import time; time.sleep(20)")
        self.assertEqual(result["refusal"], "first_setup")
        self.assertEqual((result["disk_failure_site"], result["disk_failure_origin"],
                          result["disk_failure_errno"]), (0, 0, None))
        self.assertFalse(result["supervision_ok"])

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
