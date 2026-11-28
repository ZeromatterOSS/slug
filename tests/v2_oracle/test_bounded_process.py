from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from tools.v2_oracle_lib.bounded_process import capture


class BoundedProcessTest(unittest.TestCase):
    def test_preserves_bytes_and_status_at_the_limit(self) -> None:
        code = "import os, sys; os.write(1, b'abc'); os.write(2, b'\\xff12'); sys.exit(3)"
        with tempfile.TemporaryDirectory() as root:
            result = capture([sys.executable, "-c", code], cwd=Path(root), timeout=5, max_bytes=3)
        self.assertEqual((result.returncode, result.stdout, result.stderr), (3, b"abc", b"\xff12"))

    def test_refuses_one_more_byte_in_either_stream(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            for fd, stream in [(1, "stdout"), (2, "stderr")]:
                prefix = Path(root) / stream
                with self.subTest(stream=stream), self.assertRaisesRegex(RuntimeError, f"{stream} exceeded 3 bytes"):
                    capture(
                        [sys.executable, "-c", f"import os; os.write({fd}, b'abcd')"],
                        cwd=Path(root),
                        timeout=5,
                        max_bytes=3,
                        log_prefix=prefix,
                    )
                self.assertEqual(Path(f"{prefix}.{stream}").read_bytes(), b"abc")

    def test_deadline_covers_a_command_after_both_streams_close(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            prefix = Path(root) / "deadline"
            source = "import os, time; os.write(2, b'warn'); os.close(1); os.close(2); time.sleep(60)"
            with self.assertRaises(subprocess.TimeoutExpired):
                capture(
                    [sys.executable, "-c", source],
                    cwd=Path(root),
                    timeout=0.5,
                    log_prefix=prefix,
                )
            self.assertEqual(Path(f"{prefix}.stderr").read_bytes(), b"warn")
