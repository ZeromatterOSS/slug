"""Identify and retire the installed daemon at one fresh private output base."""

import fcntl
import os
from pathlib import Path
import select
import socket
import stat
import struct
import time


def _identity(info):
    return info.st_dev, info.st_ino


def _exited(pidfd, milliseconds=0):
    poll = select.poll()
    poll.register(pidfd, select.POLLIN)
    return bool(poll.poll(milliseconds))


class InstalledDaemon:
    def __init__(self, output_base: Path, binary: Path):
        self.base = output_base
        self.control = output_base / ".slugd-installed"
        self.live = self.control / "live"
        self.endpoint = self.live / "s"
        stage = self.control / "stage" / "s"
        if not output_base.is_absolute() or len(os.fsencode(stage)) >= 108:
            raise ValueError("installed daemon needs a short absolute output base")
        self.binary = _identity(binary.stat())
        self.generation = None
        self.pidfd = None

    def __enter__(self):
        self.base.mkdir(mode=0o700)
        return self

    def _peer(self):
        stream = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            stream.settimeout(10)
            stream.connect(os.fspath(self.endpoint))
            credentials = stream.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12)
            pid, uid, _ = struct.unpack("iII", credentials)
            if pid <= 0 or uid != os.geteuid():
                raise RuntimeError("unexpected installed daemon peer credentials")

            live = self.live.lstat()
            endpoint = self.endpoint.lstat()
            if (not stat.S_ISDIR(live.st_mode) or not stat.S_ISSOCK(endpoint.st_mode)
                    or live.st_uid != uid or endpoint.st_uid != uid):
                raise RuntimeError("unexpected installed daemon generation types")
            path = self.live / "pid"
            fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
            try:
                info = os.fstat(fd)
                if not stat.S_ISREG(info.st_mode) or info.st_uid != uid:
                    raise RuntimeError("unexpected installed daemon PID file")
                if os.read(fd, 32) != str(pid).encode():
                    raise RuntimeError("installed daemon PID differs from socket peer")
            finally:
                os.close(fd)
            if _identity(Path(f"/proc/{pid}/exe").stat()) != self.binary:
                raise RuntimeError("installed daemon peer is a different executable")
            return stream, (pid, _identity(live), _identity(endpoint))
        except BaseException:
            stream.close()
            raise

    def _pin(self, generation):
        fd = os.pidfd_open(generation[0])
        if _exited(fd):
            os.close(fd)
            raise RuntimeError("installed daemon already exited")
        return fd

    def sample(self) -> int:
        """Return the same live server PID on each successful observation."""
        stream, generation = self._peer()
        with stream:
            if self.generation is None:
                self.pidfd = self._pin(generation)
                self.generation = generation
            elif generation != self.generation or _exited(self.pidfd):
                raise RuntimeError("installed daemon generation changed between commands")
        return generation[0]

    def _lock_vacant(self):
        try:
            fd = os.open(self.control / "lock", os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        except FileNotFoundError:
            return True
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            return True
        except BlockingIOError:
            return False
        finally:
            os.close(fd)

    def _shutdown(self):
        deadline = time.monotonic() + 10
        # A failed client can leave a committed daemon. Wait for a publisher
        # holding the lifetime lock to publish or retire before inspecting paths.
        while not self.endpoint.exists() and not self._lock_vacant():
            if time.monotonic() >= deadline:
                raise RuntimeError("installed daemon retained its startup lock")
            time.sleep(0.02)

        if self.endpoint.exists():
            stream, generation = self._peer()
            with stream:
                if generation == self.generation:
                    fd = self.pidfd
                else:
                    # A failed first request or a replacement after a detected
                    # restart still belongs to this previously empty private base.
                    if self.pidfd is not None and not _exited(self.pidfd):
                        raise RuntimeError("a second installed daemon appeared while the first lives")
                    fd = self._pin(generation)
                try:
                    stream.sendall(b"shutdown\n")
                    if stream.recv(1):
                        raise RuntimeError("unexpected installed daemon shutdown response")
                    if not _exited(fd, 10_000):
                        raise RuntimeError("installed daemon did not exit after shutdown")
                finally:
                    if fd != self.pidfd:
                        os.close(fd)
        if self.pidfd is not None and not _exited(self.pidfd, 10_000):
            raise RuntimeError("sampled installed daemon is still alive")
        if os.path.lexists(self.live) or os.path.lexists(self.control / "stage") or not self._lock_vacant():
            raise RuntimeError("installed daemon did not retire its managed generation")

    def __exit__(self, _type, error, _traceback):
        try:
            self._shutdown()
        except Exception as cleanup:
            if error is not None:
                raise RuntimeError(f"{error}; daemon cleanup failed: {cleanup}") from error
            raise
        finally:
            if self.pidfd is not None:
                os.close(self.pidfd)
