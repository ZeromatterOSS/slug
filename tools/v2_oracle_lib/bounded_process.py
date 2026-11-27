"""Run a POSIX fixture command with a deadline and bounded output streams."""

from __future__ import annotations

import os
from pathlib import Path
import selectors
import signal
import subprocess
import time


def _save_logs(prefix: Path | None, output: dict[str, bytearray]) -> None:
    if prefix is not None:
        for stream, data in output.items():
            Path(f"{prefix}.{stream}").write_bytes(data)


def capture(
    argv: list[str],
    *,
    cwd: Path,
    timeout: float,
    max_bytes: int = 64 * 1024,
    log_prefix: Path | None = None,
) -> subprocess.CompletedProcess[bytes]:
    """Retain up to max_bytes per stream, including in optional failure logs."""
    if timeout <= 0 or max_bytes < 0:
        raise ValueError("invalid fixture command bounds")
    deadline = time.monotonic() + timeout
    with subprocess.Popen(
        argv,
        cwd=cwd,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    ) as process:
        output = {"stdout": bytearray(), "stderr": bytearray()}
        try:
            with selectors.DefaultSelector() as reads:
                reads.register(process.stdout, selectors.EVENT_READ, "stdout")
                reads.register(process.stderr, selectors.EVENT_READ, "stderr")
                while reads.get_map():
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        raise subprocess.TimeoutExpired(argv, timeout)
                    for key, _ in reads.select(remaining):
                        stream = output[key.data]
                        available = max_bytes - len(stream)
                        chunk = os.read(key.fd, min(64 * 1024, available + 1))
                        if not chunk:
                            reads.unregister(key.fileobj)
                        elif len(chunk) > available:
                            stream.extend(chunk[:available])
                            raise RuntimeError(f"fixture command {key.data} exceeded {max_bytes} bytes")
                        else:
                            stream.extend(chunk)
            stdout = bytes(output["stdout"])
            stderr = bytes(output["stderr"])
            code = process.wait(timeout=max(0, deadline - time.monotonic()))
        except BaseException as error:
            # The unreaped child still owns its process group identity.
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            try:
                _save_logs(log_prefix, output)
            except OSError as log_error:
                error.add_note(f"saving fixture logs: {log_error}")
            raise
        _save_logs(log_prefix, output)
    return subprocess.CompletedProcess(argv, code, stdout, stderr)
