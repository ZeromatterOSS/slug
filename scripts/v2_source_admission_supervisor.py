#!/usr/bin/env python3
"""Supervise one source-admission diagnostic; no cgroup/RSS/swap/disk quota claim.

The child is the sole admitted process (threads are allowed). Any descendant is
a refusal, including adopted children after root exit. Only pinned owned PIDs
are signalled. This script never logs command/environment values or interprets
exit0 as source/root fit. Root must separately verify the genuine typed outcome.
"""
from __future__ import annotations

import argparse
import ctypes
from dataclasses import dataclass
from enum import IntEnum
import errno
import json
import os
from pathlib import Path
import re
import resource
import select
import signal
import stat
import struct
import subprocess
import sys
import tempfile
import time
from typing import Sequence


GIB = 1024 ** 3
MAX_LOG_BYTES = 1024 * 1024
WORK_ENV = "SLUG_SOURCE_ADMISSION_WORK_DEADLINE_MONOTONIC_NS"
CLEANUP_ENV = "SLUG_SOURCE_ADMISSION_CLEANUP_DEADLINE_MONOTONIC_NS"
STAGE_ENV = "SLUG_SOURCE_ADMISSION_STAGE_ROOT"


class DiskSite(IntEnum):
    UNKNOWN = 0
    ROOT_OPEN = 1
    DIRECTORY_OPEN = 2
    ROOT_STAT = 3
    TREE_STAT = 4
    CHILD_OPEN = 5
    CHILD_STAT = 6
    PROC_FD_OPEN = 7
    PROC_FD_STAT = 8
    PROC_FD_LINK = 9
    PROC_FD_INFO = 10
    FLAGS_PARSE = 11
    ROOT_REVALIDATE = 12
    DIRECTORY_NEXT = 13
    PROC_FD_NEXT = 14


class DiskOrigin(IntEnum):
    OS_ERROR = 1
    INVALID_METADATA = 2


@dataclass(frozen=True)
class DiskFailure:
    site: DiskSite
    origin: DiskOrigin
    errno: int | None = None

    def __post_init__(self) -> None:
        if (type(self.site) is not DiskSite or type(self.origin) is not DiskOrigin
                or (self.errno is not None and
                    (type(self.errno) is not int or not -(2 ** 31) <= self.errno < 2 ** 31))):
            raise ValueError("invalid disk diagnostic scalar")


class AccountingError(Exception):
    """Fixed reason and optional scalar detail; never retain exception text."""
    def __init__(self, reason: str, detail: DiskFailure | None = None):
        super().__init__(reason)
        if detail is not None and type(detail) is not DiskFailure:
            raise ValueError("invalid disk diagnostic detail")
        self.detail = detail


def _disk_detail(site: DiskSite, exc) -> DiskFailure:
    origin = DiskOrigin.OS_ERROR if isinstance(exc, OSError) else DiskOrigin.INVALID_METADATA
    number = exc.errno if isinstance(exc, OSError) else None
    # OS errno is a signed C int; unknown non-OS/future values stay absent.
    if type(number) is not int or not -(2 ** 31) <= number < 2 ** 31:
        number = None
    return DiskFailure(site, origin, number)


def _disk_call(site: DiskSite, operation, *args, **kwargs):
    try:
        return operation(*args, **kwargs)
    except (OSError, ValueError, StopIteration) as exc:
        detail = _disk_detail(site, exc)
    # Outside the handler: retain neither native exception nor its traceback.
    raise AccountingError("disk_accounting_unavailable", detail) from None


def _disk_entries(iterator, site: DiskSite):
    while True:
        try:
            entry = next(iterator)
        except StopIteration:
            return
        except (OSError, ValueError) as exc:
            detail = _disk_detail(site, exc)
        else:
            yield entry
            continue
        raise AccountingError("disk_accounting_unavailable", detail) from None


def _disk_access(flags: str) -> int:
    return next(int(row.split()[1], 8) & os.O_ACCMODE
                for row in flags.splitlines() if row.startswith("flags:"))


def _complete_disk_census(census, live, exited):
    try:
        return census(live)
    except AccountingError:
        if not all(exited(pid) for pid in live):
            raise
        # Discard the failed live census ONLY after confirmed full exit.
        return census([])


@dataclass(frozen=True)
class Limits:
    # Production CLI cannot raise/change these. Sanity callers may tighten only.
    address_bytes: int = 4 * GIB
    rss_bytes: int = 4 * GIB
    disk_bytes: int = 6 * GIB
    work_seconds: float = 27.0
    cleanup_seconds: float = 3.0
    terminal_seconds: float = 3.0
    poll_seconds: float = 0.02
    census_seconds: float = 0.1

    def validate(self) -> None:
        if not (0 < self.address_bytes <= 4 * GIB
                and 0 < self.rss_bytes <= 4 * GIB
                and 0 < self.disk_bytes <= 6 * GIB
                and 0 < self.work_seconds <= 27
                and 0 < self.cleanup_seconds <= 3
                and 0 < self.terminal_seconds <= 3
                and 0 < self.poll_seconds <= 0.1
                and 0 < self.census_seconds <= 0.1):
            raise ValueError("invalid supervisor limits")


@dataclass(frozen=True)
class Deadlines:
    start_ns: int
    work_ns: int
    cleanup_ns: int
    terminal_ns: int

    @classmethod
    def from_start(cls, start_ns: int, limits: Limits) -> Deadlines:
        work = start_ns + int(limits.work_seconds * 1e9)
        cleanup = work + int(limits.cleanup_seconds * 1e9)
        return cls(start_ns, work, cleanup,
                   cleanup + int(limits.terminal_seconds * 1e9))


@dataclass(frozen=True)
class ProcessStat:
    pid: int
    ppid: int
    pgid: int
    starttime: int
    state: str


def process_stat(pid: int) -> ProcessStat:
    try:
        line = Path(f"/proc/{pid}/stat").read_text()
        end = line.rfind(")")
        fields = line[end + 2:].split()
        return ProcessStat(int(line[:line.find(" ")]), int(fields[1]),
                           int(fields[2]), int(fields[19]), fields[0])
    except (OSError, ValueError, IndexError) as exc:
        raise AccountingError("process_identity_unavailable") from exc


def children(pid: int) -> set[int]:
    # Linux's per-thread children list avoids a system-wide PID/argv scan.
    result: set[int] = set()
    try:
        with os.scandir(f"/proc/{pid}/task") as tasks:
            for task in tasks:
                result.update(int(p) for p in
                              Path(task.path, "children").read_text().split())
    except (OSError, ValueError) as exc:
        raise AccountingError("child_accounting_unavailable") from exc
    return result


@dataclass
class OwnedProcess:
    identity: ProcessStat
    pidfd: int

    @classmethod
    def pin(cls, pid: int, allowed_parents: set[int]) -> OwnedProcess:
        before = process_stat(pid)
        if before.ppid not in allowed_parents:
            raise AccountingError("child_relationship_changed")
        fd = os.pidfd_open(pid, 0)
        try:
            after = process_stat(pid)
            if before.starttime != after.starttime or before.ppid != after.ppid:
                raise AccountingError("child_identity_changed")
            return cls(after, fd)
        except BaseException:
            os.close(fd)
            raise

    def exited(self) -> bool:
        return bool(select.select([self.pidfd], [], [], 0)[0])

    def signal(self, sig: int) -> None:
        try:
            signal.pidfd_send_signal(self.pidfd, sig, None, 0)
        except OSError as exc:
            if exc.errno != errno.ESRCH:
                raise AccountingError("owned_signal_failed") from exc

    def close(self) -> None:
        os.close(self.pidfd)


def memory_sample(owner: OwnedProcess, address_limit: int) -> tuple[int, int]:
    if owner.exited():
        return 0, 0
    pid = owner.identity.pid
    try:
        before = process_stat(pid)
        if before.starttime != owner.identity.starttime:
            raise AccountingError("process_identity_changed")
        values = {}
        for line in Path(f"/proc/{pid}/status").read_text().splitlines():
            if line.startswith(("VmRSS:", "VmSwap:")):
                key, value, unit = line.split()
                if unit != "kB":
                    raise AccountingError("memory_units_changed")
                values[key] = int(value) * 1024
        limits = Path(f"/proc/{pid}/limits").read_text()
        match = re.search(r"^Max address space\s+(\d+)\s+(\d+)\s+bytes[ \t]*$",
                          limits, re.MULTILINE)
        if not match or any(int(x) > address_limit for x in match.groups()):
            raise AccountingError("address_limit_changed")
        after = process_stat(pid)
        if after.starttime != before.starttime:
            raise AccountingError("process_identity_changed")
        if owner.exited() or after.state == "Z":
            return 0, 0
        return values["VmRSS:"], values["VmSwap:"]
    except (OSError, ValueError, KeyError) as exc:
        # During exit Linux can remove the mm fields shortly before pidfd exit
        # readiness. Confirm actual exit with a finite5ms wait; a still-live
        # inaccessible process remains a refusal, never a partial sample.
        if owner.exited() or select.select([owner.pidfd], [], [], 0.005)[0]:
            return 0, 0
        raise AccountingError("memory_accounting_unavailable") from exc


@dataclass(frozen=True)
class DiskSample:
    allocated_bytes: int
    logical_bytes: int
    inodes: int


def disk_sample(root: Path, pids: Sequence[int], deadline_ns: int,
                expected_root: tuple[int, int] | None = None) -> DiskSample:
    """Metadata-only, no-follow tree plus owned regular FDs; inode dedup.

    External read-only input files are not owned staged disk. External writable
    regular FDs refuse accounting rather than silently escaping the stage bank.
    Open-unlinked private files and anonymous memfds are included. A raced or
    inaccessible census refuses; it never reports a partial successful total.
    """
    seen: set[tuple[int, int]] = set()
    allocated = logical = 0
    visited = 0

    def account(info: os.stat_result) -> None:
        nonlocal allocated, logical, visited
        visited += 1
        if visited > 1_000_000 or time.monotonic_ns() >= deadline_ns:
            raise AccountingError("disk_census_bound")
        key = (info.st_dev, info.st_ino)
        if key not in seen:
            seen.add(key)
            allocated += info.st_blocks * 512
            if stat.S_ISREG(info.st_mode):
                logical += info.st_size

    stack: list[tuple[int, object]] = []
    def push_directory(fd: int) -> None:
        try:
            stack.append((fd, _disk_call(DiskSite.DIRECTORY_OPEN, os.scandir, fd)))
        except BaseException:
            os.close(fd)
            raise

    try:
        root_fd = _disk_call(DiskSite.ROOT_OPEN, os.open, root,
                             os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        push_directory(root_fd)
        root_info = _disk_call(DiskSite.ROOT_STAT, os.fstat, root_fd)
        if expected_root is not None and (root_info.st_dev, root_info.st_ino) != expected_root:
            raise AccountingError("stage_identity_changed")
        account(root_info)
        while stack:
            fd, iterator = stack[-1]
            try:
                entry = next(_disk_entries(iterator, DiskSite.DIRECTORY_NEXT))
            except StopIteration:
                iterator.close()
                os.close(fd)
                stack.pop()
                continue
            info = _disk_call(DiskSite.TREE_STAT, os.stat, entry.name,
                              dir_fd=fd, follow_symlinks=False)
            account(info)
            if stat.S_ISDIR(info.st_mode):
                if len(stack) >= 128:
                    raise AccountingError("disk_census_depth")
                child = _disk_call(DiskSite.CHILD_OPEN, os.open, entry.name,
                                   os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
                                   | os.O_CLOEXEC, dir_fd=fd)
                opened = _disk_call(DiskSite.CHILD_STAT, os.fstat, child)
                if (opened.st_dev, opened.st_ino) != (info.st_dev, info.st_ino):
                    os.close(child)
                    raise AccountingError("disk_directory_raced")
                push_directory(child)
        root_text = str(root)
        for pid in pids:
            with _disk_call(DiskSite.PROC_FD_OPEN, os.scandir, f"/proc/{pid}/fd") as fds:
                for entry in _disk_entries(fds, DiskSite.PROC_FD_NEXT):
                    if time.monotonic_ns() >= deadline_ns:
                        raise AccountingError("disk_census_bound")
                    info = _disk_call(DiskSite.PROC_FD_STAT, os.stat, entry.path)
                    if not stat.S_ISREG(info.st_mode):
                        continue
                    name = _disk_call(DiskSite.PROC_FD_LINK, os.readlink, entry.path)
                    original = name.removesuffix(" (deleted)")
                    private = original.startswith(root_text + "/")
                    anonymous = original.startswith(("/memfd:", "memfd:"))
                    if private or anonymous or (info.st_dev, info.st_ino) in seen:
                        account(info)
                        continue
                    flags = _disk_call(DiskSite.PROC_FD_INFO,
                                       Path(f"/proc/{pid}/fdinfo/{entry.name}").read_text)
                    access = _disk_call(
                        DiskSite.FLAGS_PARSE,
                        _disk_access, flags)
                    if access != os.O_RDONLY:
                        raise AccountingError("external_writable_file")
        after = _disk_call(DiskSite.ROOT_REVALIDATE, root.lstat)
        if (after.st_dev, after.st_ino) != (root_info.st_dev, root_info.st_ino):
            raise AccountingError("stage_identity_changed")
        return DiskSample(allocated, logical, len(seen))
    except (OSError, ValueError, StopIteration) as exc:
        # Unwrapped context operations have an honest unknown site.
        detail = _disk_detail(DiskSite.UNKNOWN, exc)
    finally:
        for fd, iterator in reversed(stack):
            iterator.close()
            os.close(fd)
    raise AccountingError("disk_accounting_unavailable", detail) from None


def resource_refusal(rss: int, swap: int, disk: DiskSample, limits: Limits) -> str | None:
    if rss >= limits.rss_bytes:
        return "sampled_rss_limit"
    if swap:
        return "sampled_swap_observed"
    if disk.allocated_bytes >= limits.disk_bytes:
        return "sampled_disk_limit"
    return None


def install_subreaper() -> None:
    libc = ctypes.CDLL(None, use_errno=True)
    value = ctypes.c_int()
    if libc.prctl(36, 1, 0, 0, 0) or libc.prctl(37, ctypes.byref(value), 0, 0, 0):
        raise AccountingError("subreaper_unavailable")
    if value.value != 1:
        raise AccountingError("subreaper_not_installed")


def preserve_log(source: Path, destination: Path, deadline_ns: int) -> int:
    # Only the child diagnostic log, never input/source file payloads.
    fd = os.open(source, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    created = False
    try:
        with os.fdopen(fd, "rb") as incoming:
            info = os.fstat(incoming.fileno())
            if not stat.S_ISREG(info.st_mode):
                raise AccountingError("diagnostic_log_not_regular")
            if info.st_size > MAX_LOG_BYTES:
                raise AccountingError("diagnostic_log_limit")
            with destination.open("xb") as outgoing:
                created = True
                copied = 0
                while True:
                    if time.monotonic_ns() >= deadline_ns:
                        raise AccountingError("terminal_cleanup_deadline")
                    block = incoming.read(min(64 * 1024, MAX_LOG_BYTES - copied + 1))
                    if not block:
                        if copied != info.st_size:
                            raise AccountingError("diagnostic_log_raced")
                        return copied
                    copied += len(block)
                    if copied > MAX_LOG_BYTES:
                        raise AccountingError("diagnostic_log_limit")
                    outgoing.write(block)
    except BaseException:
        if created:
            destination.unlink()
        raise


def remove_owned_stage(root: Path, identity: tuple[int, int], deadline_ns: int) -> None:
    """Post-join descriptor-relative deletion; no symlink traversal/new grace."""
    stack = []
    try:
        root_fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        info = os.fstat(root_fd)
        if (info.st_dev, info.st_ino) != identity:
            os.close(root_fd)
            raise AccountingError("stage_identity_changed")
        try:
            iterator = os.scandir(root_fd)
        except BaseException:
            os.close(root_fd)
            raise
        stack.append((root_fd, iterator, None))
        while stack:
            if time.monotonic_ns() >= deadline_ns:
                raise AccountingError("terminal_cleanup_deadline")
            fd, iterator, name = stack[-1]
            try:
                entry = next(iterator)
            except StopIteration:
                iterator.close()
                os.close(fd)
                stack.pop()
                if stack:
                    os.rmdir(name, dir_fd=stack[-1][0])
                continue
            info = os.stat(entry.name, dir_fd=fd, follow_symlinks=False)
            if stat.S_ISDIR(info.st_mode):
                if len(stack) >= 128:
                    raise AccountingError("stage_cleanup_depth")
                child = os.open(entry.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
                                | os.O_CLOEXEC, dir_fd=fd)
                opened = os.fstat(child)
                if (opened.st_dev, opened.st_ino) != (info.st_dev, info.st_ino):
                    os.close(child)
                    raise AccountingError("stage_identity_changed")
                try:
                    iterator = os.scandir(child)
                except BaseException:
                    os.close(child)
                    raise
                stack.append((child, iterator, entry.name))
            else:
                os.unlink(entry.name, dir_fd=fd)
        if time.monotonic_ns() >= deadline_ns:
            raise AccountingError("terminal_cleanup_deadline")
        info = root.lstat()
        if (info.st_dev, info.st_ino) != identity:
            raise AccountingError("stage_identity_changed")
        os.rmdir(root)
    finally:
        for fd, iterator, _ in reversed(stack):
            iterator.close()
            os.close(fd)


def supervise(command: Sequence[str], *, stage_parent: Path, log: Path,
              limits: Limits = Limits()) -> dict:
    """Fresh single-thread supervisor only; test callers may tighten limits.

    The returned receipt names controls and outcomes, not source semantic fit.
    No child is admitted after a refusal latch, even if it later exits0.
    """
    limits.validate()
    if sys.platform != "linux" or not command:
        raise ValueError("Linux and one explicit command required")
    if len(list(Path("/proc/self/task").iterdir())) != 1 or children(os.getpid()):
        raise AccountingError("supervisor_not_isolated")
    if not hasattr(os, "pidfd_open") or not hasattr(signal, "pidfd_send_signal"):
        raise AccountingError("pidfd_unavailable")
    install_subreaper()
    stage_parent = stage_parent.resolve(strict=True)
    stage = Path(tempfile.mkdtemp(prefix="slug-source-admission-", dir=stage_parent))
    root_info = stage.stat()
    stage_log = stage / "child.log"
    owners: dict[int, OwnedProcess] = {}
    process = None
    reason = None
    disk_failure = None
    joined = False
    removed = False
    descendants = 0
    term_sent = kill_sent = False
    peak_rss = peak_swap = peak_disk = peak_logical = 0
    samples = 0
    actual_limit = None
    log_preserved = False
    log_bytes = None
    deadline = Deadlines.from_start(time.monotonic_ns(), limits)
    # Failure gets at most the original terminal deadline, never a new phase.
    terminal_ns = deadline.terminal_ns
    kill_ns = deadline.terminal_ns
    handshake_r, handshake_w = os.pipe2(os.O_CLOEXEC)
    prior_handlers = {}

    def child_limit() -> None:
        resource.setrlimit(resource.RLIMIT_AS, (limits.address_bytes, limits.address_bytes))
        soft, hard = resource.getrlimit(resource.RLIMIT_AS)
        os.write(handshake_w, struct.pack("!QQ", soft, hard))
        os.close(handshake_w)

    def refuse(why: str, detail: DiskFailure | None = None) -> None:
        nonlocal reason, disk_failure, terminal_ns, kill_ns
        if reason is None:
            reason = why
            disk_failure = detail
            terminal_ns = min(terminal_ns, time.monotonic_ns()
                              + int(limits.terminal_seconds * 1e9))
            kill_ns = min(terminal_ns, time.monotonic_ns()
                          + int(limits.terminal_seconds * 0.5e9))

    def discover() -> None:
        nonlocal descendants
        parent_ids = {os.getpid(), *owners}
        pending = list(parent_ids)
        while pending:
            parent = pending.pop()
            if parent != os.getpid() and owners[parent].exited():
                continue
            for pid in children(parent):
                if pid in owners:
                    try:
                        listed = process_stat(pid)
                    except AccountingError:
                        if owners[pid].exited():
                            continue
                        raise
                    if listed.starttime == owners[pid].identity.starttime:
                        continue
                    # A non-root descendant can have been reaped by its parent
                    # and its PID reused. Never keep a numeric-only identity.
                    owners[pid].close()
                    del owners[pid]
                    refuse("owned_identity_changed")
                if process is not None and pid == process.pid and process.returncode is None:
                    continue
                refuse("unexpected_descendant")
                descendants += 1
                try:
                    owner = OwnedProcess.pin(pid, parent_ids)
                except (AccountingError, ProcessLookupError):
                    # A listed child can exit before pinning. Reaping below still
                    # detects adopted exits; no unstable raw PID is signalled.
                    try:
                        os.waitpid(pid, os.WNOHANG)
                    except ChildProcessError:
                        pass
                    continue
                owners[pid] = owner
                parent_ids.add(pid)
                pending.append(pid)

    def reap_descendants() -> None:
        for pid, owner in list(owners.items()):
            if process is not None and pid == process.pid:
                continue
            if owner.exited():
                try:
                    reaped, _ = os.waitpid(pid, os.WNOHANG)
                    if reaped == 0:
                        continue
                except ChildProcessError:
                    # Still parent-owned zombie must remain until its parent
                    # reaps it or exits and adoption permits our actual wait.
                    try:
                        if process_stat(pid).state == "Z":
                            continue
                    except AccountingError:
                        pass
                owner.close()
                del owners[pid]

    def join_if_complete() -> bool:
        if process is None:
            return True
        root_owner = owners.get(process.pid)
        if root_owner is not None and not root_owner.exited():
            return False
        if children(os.getpid()) - {process.pid}:
            return False
        if any(pid != process.pid for pid in owners):
            return False
        # Hold the exited root unreaped throughout descendant/adoption draining:
        # its numeric PID cannot be reused as a newly adopted descendant.
        if process.returncode is None:
            try:
                process.wait(timeout=0.01)
            except subprocess.TimeoutExpired:
                return False
        if process.returncode != 0:
            refuse("child_exit_nonzero")
        return True

    def census(pids: Sequence[int]) -> DiskSample:
        return disk_sample(stage, pids,
                           min(deadline.terminal_ns, time.monotonic_ns()
                               + int(limits.census_seconds * 1e9)),
                           (root_info.st_dev, root_info.st_ino))

    # External TERM/INT must latch refusal, not kill the supervisor and orphan
    # the child in its fresh session. A signal never refreshes a deadline.
    def interrupted(_signal, _frame) -> None:
        refuse("supervisor_interrupted")

    for sig in (signal.SIGTERM, signal.SIGINT):
        prior_handlers[sig] = signal.signal(sig, interrupted)

    try:
        environment = os.environ.copy()
        environment.update({WORK_ENV: str(deadline.work_ns),
                            CLEANUP_ENV: str(deadline.cleanup_ns),
                            STAGE_ENV: str(stage), "TMPDIR": str(stage)})
        with stage_log.open("wb") as output:
            process = subprocess.Popen(command, stdin=subprocess.DEVNULL,
                                       stdout=output, stderr=output,
                                       env=environment, start_new_session=True,
                                       preexec_fn=child_limit, pass_fds=(handshake_w,))
            # Pin while Popen still owns the unreaped direct child. On setup
            # failure the emergency path below also owns that reserved PID.
            owners[process.pid] = OwnedProcess.pin(process.pid, {os.getpid()})
            os.close(handshake_w)
            handshake_w = -1
            data = os.read(handshake_r, 16)
            if len(data) != 16 or struct.unpack("!QQ", data) != (limits.address_bytes,) * 2:
                refuse("address_limit_unverified")
            else:
                actual_limit = limits.address_bytes
            # Root is our known child even if it already became a zombie.
            if owners[process.pid].identity.pgid != process.pid:
                refuse("root_session_unverified")
            while True:
                try:
                    discover()
                    reap_descendants()
                    if not owners[process.pid].exited():
                        rss = swap = 0
                        live = []
                        for pid, owner in owners.items():
                            if owner.exited():
                                continue
                            r, s = memory_sample(owner, limits.address_bytes)
                            rss += r
                            swap += s
                            live.append(pid)
                        disk = _complete_disk_census(
                            census, live, lambda pid: owners[pid].exited())
                        samples += 1
                        peak_rss = max(peak_rss, rss)
                        peak_swap = max(peak_swap, swap)
                        peak_disk = max(peak_disk, disk.allocated_bytes)
                        peak_logical = max(peak_logical, disk.logical_bytes)
                        refusal = resource_refusal(rss, swap, disk, limits)
                        if refusal:
                            refuse(refusal)
                    else:
                        # Final disk census still occurs after child cleanup.
                        disk = census([])
                        peak_disk = max(peak_disk, disk.allocated_bytes)
                        peak_logical = max(peak_logical, disk.logical_bytes)
                        if disk.allocated_bytes >= limits.disk_bytes:
                            refuse("sampled_disk_limit")
                except AccountingError as exc:
                    refuse(str(exc), exc.detail)
                if owners[process.pid].exited():
                    # Adoption happens on root exit; never stop before checking.
                    try:
                        discover()
                        reap_descendants()
                        if join_if_complete():
                            joined = True
                            break
                    except AccountingError as exc:
                        refuse(str(exc), exc.detail)
                now = time.monotonic_ns()
                if now >= deadline.cleanup_ns:
                    refuse("outer_deadline")
                if reason is not None and not term_sent:
                    # Group signal only while the pinned root still owns its
                    # fresh group. Escaped/adopted children get pidfd signals.
                    root_owner = owners[process.pid]
                    if not root_owner.exited():
                        current = process_stat(process.pid)
                        if (current.starttime == root_owner.identity.starttime
                                and current.pgid == process.pid):
                            try:
                                os.killpg(process.pid, signal.SIGTERM)
                            except ProcessLookupError:
                                pass
                    for owner in owners.values():
                        owner.signal(signal.SIGTERM)
                    term_sent = True
                # Newly discovered escapees cannot wait until the next timeout.
                elif reason is not None:
                    for owner in owners.values():
                        owner.signal(signal.SIGKILL if kill_sent else signal.SIGTERM)
                if reason is not None and now >= kill_ns and not kill_sent:
                    for owner in owners.values():
                        owner.signal(signal.SIGKILL)
                    kill_sent = True
                if reason is not None and now >= terminal_ns:
                    # KILL was already sent within the original terminal budget;
                    # no new grace. D-state/unjoined children stay an explicit
                    # failure and owned storage is not claimed removed.
                    discover()
                    reap_descendants()
                    if join_if_complete():
                        joined = True
                    break
                time.sleep(limits.poll_seconds)
    except (OSError, AccountingError, subprocess.SubprocessError) as exc:
        refuse("supervisor_operation_failed" if not isinstance(exc, AccountingError) else str(exc),
               exc.detail if isinstance(exc, AccountingError) else None)
        # Unexpected supervisor failure grants no success. Keep actual bounded
        # joins after KILL; signal only pinned owners and adopted relationships.
        for owner in owners.values():
            owner.signal(signal.SIGKILL)
        kill_sent = bool(owners)
        if process is not None and process.pid not in owners and process.returncode is None:
            # Popen's direct child has never been waited/reaped here: its PID
            # cannot be reused. This fallback is limited to root pin/setup failure.
            try:
                os.kill(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            kill_sent = True
            try:
                process.wait(timeout=max(0, (terminal_ns - time.monotonic_ns()) / 1e9))
            except subprocess.TimeoutExpired:
                pass
        while process is not None and time.monotonic_ns() < terminal_ns:
            try:
                discover()
                for owner in owners.values():
                    owner.signal(signal.SIGKILL)
                reap_descendants()
                if join_if_complete():
                    joined = True
                    break
            except (OSError, AccountingError):
                pass
            time.sleep(limits.poll_seconds)
    finally:
        if process is None:
            joined = True
        if joined:
            # Common normal/refusal/setup-failure cleanup only AFTER actual
            # joins, with owned stage identity rechecked and no payload loading.
            try:
                if stage_log.exists():
                    log_bytes = stage_log.lstat().st_size
                    preserve_log(stage_log, log, terminal_ns)
                    log_preserved = True
            except AccountingError as exc:
                refuse(str(exc), exc.detail)
            except OSError:
                refuse("diagnostic_log_preservation_failed")
            try:
                current = stage.lstat()
                if (current.st_dev, current.st_ino) != (root_info.st_dev, root_info.st_ino):
                    refuse("stage_identity_changed")
                else:
                    remove_owned_stage(stage, (root_info.st_dev, root_info.st_ino), terminal_ns)
                    removed = not stage.exists()
            except AccountingError as exc:
                refuse(str(exc), exc.detail)
            except OSError:
                refuse("stage_cleanup_failed")
        os.close(handshake_r)
        if handshake_w >= 0:
            os.close(handshake_w)
        for owner in owners.values():
            owner.close()
        for sig, previous in prior_handlers.items():
            signal.signal(sig, previous)
    # A root exit/cleanup racing the last loop's clock check cannot manufacture
    # success after the outer cleanup bound, even when every join completed.
    if time.monotonic_ns() >= deadline.cleanup_ns:
        refuse("outer_deadline")
    return {"schema": 1, "limit_kind": "RLIMIT_AS",
            "address_space_limit_bytes": limits.address_bytes,
            "address_space_limit_verified": actual_limit == limits.address_bytes,
            "sampled_rss_limit_bytes": limits.rss_bytes,
            "sampled_disk_limit_bytes": limits.disk_bytes,
            "sampled_rss_peak_bytes": peak_rss, "sampled_swap_peak_bytes": peak_swap,
            "sampled_disk_peak_bytes": peak_disk,
            "sampled_logical_disk_peak_bytes": peak_logical,
            "samples": samples, "poll_interval_seconds": limits.poll_seconds,
            "census_deadline_seconds": limits.census_seconds,
            "elapsed_seconds": (time.monotonic_ns() - deadline.start_ns) / 1e9,
            "work_seconds": limits.work_seconds,
            "cleanup_seconds": limits.cleanup_seconds,
            "terminal_seconds": limits.terminal_seconds,
            "unexpected_descendants": descendants,
            "child_exit_code": None if process is None else process.returncode,
            "refusal": reason, "term_sent": term_sent, "kill_sent": kill_sent,
            "joined": joined, "stage_removed": removed,
            "diagnostic_log_bytes": log_bytes, "diagnostic_log_preserved": log_preserved,
            "diagnostic_log_limit_bytes": MAX_LOG_BYTES,
            "terminal_cleanup_exhausted": time.monotonic_ns() >= terminal_ns,
            "supervision_ok": reason is None and joined and removed,
            "source_gate_verified": False,
            "disk_failure_site": 0 if disk_failure is None else int(disk_failure.site),
            "disk_failure_origin": 0 if disk_failure is None else int(disk_failure.origin),
            "disk_failure_errno": None if disk_failure is None else disk_failure.errno,
            "kernel_rss_quota": False, "swap_disabled": False, "disk_quota": False}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage-parent", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--log", type=Path, required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    try:
        receipt = supervise(command, stage_parent=args.stage_parent, log=args.log)
    except (AccountingError, OSError, ValueError):
        receipt = {"schema": 1, "supervision_ok": False,
                   "source_gate_verified": False, "refusal": "supervisor_setup_failed"}
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n")
    return 0 if receipt["supervision_ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
