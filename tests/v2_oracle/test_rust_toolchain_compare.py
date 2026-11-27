from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from tools.v2_oracle import rust_toolchain_compare as compare


def _fixture_inputs(root: Path) -> dict:
    return {
        "workspace": str(compare.rust_toolchain_fixture.FIXTURE_ROOT / "workspace"),
        "registry_uri": (root / "registry").as_uri(),
        "distdirs": [str(root / "distdir")],
    }


class RustToolchainCompareTest(unittest.TestCase):
    def test_lane_replays_pinned_editions_and_samples_each_daemon_command(self) -> None:
        cases = compare._cases()
        expected = iter(stdout for _, stdout in cases)
        module_inputs = []
        samples = []

        def capture(argv, *, cwd, timeout, log_prefix):
            self.assertEqual(argv[:2], ["/test/slug", "--output_base=/test/output"])
            self.assertIn("--starlark:file=cquery_format.bzl", argv)
            self.assertEqual(argv[-2:], [
                f"--registry={(root / 'registry').as_uri()}",
                f"--distdir={root / 'distdir'}",
            ])
            self.assertEqual(log_prefix, output / cases[len(module_inputs)][0].name)
            module_inputs.append((cwd / "MODULE.bazel").read_bytes())
            return subprocess.CompletedProcess(argv, 0, next(expected), b"")

        def sample() -> int:
            samples.append(len(module_inputs))
            return 123

        with tempfile.TemporaryDirectory() as temporary, patch.object(compare, "capture", capture):
            root = Path(temporary)
            output = root / "lane"
            compare._run_lane(output, _fixture_inputs(root), cases,
                              ["/test/slug", "--output_base=/test/output"], 5, sample)
        initial = (compare.rust_toolchain_fixture.FIXTURE_ROOT / "workspace/MODULE.bazel").read_bytes()
        changed = initial.replace(b'edition = "2024"', b'edition = "2021"')
        self.assertEqual(module_inputs, [initial, initial, changed, initial])
        self.assertEqual(samples, [1, 2, 3, 4])

    def test_lane_rejects_wrong_bytes_at_exit_zero(self) -> None:
        cases = compare._cases()

        def capture(argv, **_):
            return subprocess.CompletedProcess(argv, 0, cases[0][1] + b"\n", b"")

        with tempfile.TemporaryDirectory() as temporary, patch.object(compare, "capture", capture):
            root = Path(temporary)
            with self.assertRaisesRegex(RuntimeError, "expected exit 0, exact CrateInfo stdout"):
                compare._run_lane(root / "lane", _fixture_inputs(root), cases, ["/test/slug"], 5)
