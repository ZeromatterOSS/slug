from __future__ import annotations

import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "v2_packet_validate.py"
SPEC = importlib.util.spec_from_file_location("v2_packet_validate", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
packet_validate = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = packet_validate
SPEC.loader.exec_module(packet_validate)


class PacketValidateTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.fixtures = self.root / "fixtures"
        self.fixtures.mkdir()
        self.constants = mock.patch.multiple(
            packet_validate, REPO_ROOT=self.root, FIXTURES_ROOT=self.fixtures
        )
        self.constants.start()
        self.addCleanup(self.constants.stop)
        self.addCleanup(self.tmp.cleanup)

    def fixture(self, name: str, *, generated: bool = True, daemon: bool = False) -> None:
        fixture_root = self.fixtures / name
        (fixture_root / "expected").mkdir(parents=True)
        (fixture_root / "workspace").mkdir()
        (fixture_root / "fixture.toml").write_text(
            f'[fixture]\nname = "{name}"\ndaemon = {str(daemon).lower()}\n\n[[commands]]\nargv = ["version"]\n',
            encoding="utf-8",
        )
        (fixture_root / "expected" / "oracle.json").write_text(
            json.dumps({"generated": generated}), encoding="utf-8"
        )

    def binary(self, target: Path | None = None, *, executable: bool = True) -> Path:
        binary = (target or self.root / "target") / "debug" / "slug"
        binary.parent.mkdir(parents=True)
        binary.write_text("#!/bin/sh\n", encoding="utf-8")
        binary.chmod(0o755 if executable else 0o644)
        return binary

    @staticmethod
    def completed(code: int = 0) -> subprocess.CompletedProcess[object]:
        return subprocess.CompletedProcess([], code)

    def test_selection_errors_happen_before_build(self) -> None:
        self.fixture("known")
        with mock.patch.object(packet_validate.subprocess, "run") as run:
            result = packet_validate.main(["--fixture", "missing", "--fixture", "missing"])
        self.assertEqual(2, result)
        run.assert_not_called()

    def test_generated_false_happens_before_build(self) -> None:
        self.fixture("not-generated", generated=False)
        with mock.patch.object(packet_validate.subprocess, "run") as run:
            result = packet_validate.main(["--fixture", "not-generated"])
        self.assertEqual(2, result)
        run.assert_not_called()

    def test_late_invalid_selection_keeps_validation_atomic_before_build(self) -> None:
        self.fixture("valid")
        self.fixture("stale", generated=False)
        with mock.patch.object(packet_validate.subprocess, "run") as run:
            result = packet_validate.main(
                ["--fixture", "valid", "--fixture", "stale"]
            )
        self.assertEqual(2, result)
        run.assert_not_called()

    def test_build_once_and_oracle_argv_preserves_fixture_order(self) -> None:
        self.fixture("first")
        self.fixture("second")
        slug = self.binary()
        with mock.patch.object(
            packet_validate.subprocess,
            "run",
            side_effect=[self.completed(), self.completed(), self.completed()],
        ) as run:
            result = packet_validate.main(
                ["--fixture", "second", "--fixture", "first", "--timeout", "9"]
            )
        self.assertEqual(0, result)
        self.assertEqual(["cargo", "build", "-p", "slug_cli_v2"], run.call_args_list[0].args[0])
        oracle_calls = [call.args[0] for call in run.call_args_list[1:]]
        self.assertEqual(["second", "first"], [call[-1] for call in oracle_calls])
        for argv in oracle_calls:
            self.assertEqual(
                [
                    sys.executable,
                    "-B",
                    "-m",
                    "tools.v2_oracle",
                    "run",
                    "--tool",
                    "slug",
                    "--slug",
                    str(slug),
                    "--run-root",
                ],
                argv[:10],
            )
            self.assertEqual(["--timeout", "9", "--fixture"], argv[-4:-1])
        self.assertEqual("1", run.call_args_list[0].kwargs["env"]["CARGO_BUILD_JOBS"])

    def test_target_dir_derivation_supports_relative_and_absolute_paths(self) -> None:
        with mock.patch.dict(os.environ, {"CARGO_TARGET_DIR": "relative-target"}, clear=False):
            self.assertEqual(self.root / "relative-target" / "debug" / "slug", packet_validate._slug_binary())
        absolute = self.root / "absolute-target"
        with mock.patch.dict(os.environ, {"CARGO_TARGET_DIR": str(absolute)}, clear=False):
            self.assertEqual(absolute / "debug" / "slug", packet_validate._slug_binary())

    def test_lock_contention_fails_before_build(self) -> None:
        self.fixture("one")
        lock_path = self.root / "target" / "v2_packet_validate" / "packet.lock"
        lock_path.parent.mkdir(parents=True)
        with lock_path.open("a+", encoding="utf-8") as lock_file:
            import fcntl

            fcntl.flock(lock_file.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
            with mock.patch.object(packet_validate.subprocess, "run") as run:
                result = packet_validate.main(["--fixture", "one"])
        self.assertEqual(2, result)
        run.assert_not_called()

    def test_overlong_daemon_socket_path_fails_before_build(self) -> None:
        self.fixture("x" * 80, daemon=True)
        with mock.patch.object(packet_validate.subprocess, "run") as run:
            result = packet_validate.main(["--fixture", "x" * 80])
        self.assertEqual(1, result)
        run.assert_not_called()

    def test_failure_does_not_prevent_later_fixture_execution(self) -> None:
        self.fixture("first")
        self.fixture("second")
        self.binary()
        with mock.patch.object(
            packet_validate.subprocess,
            "run",
            side_effect=[self.completed(), self.completed(1), self.completed()],
        ) as run:
            result = packet_validate.main(["--fixture", "first", "--fixture", "second"])
        self.assertEqual(1, result)
        self.assertEqual(3, run.call_count)
        self.assertEqual("second", run.call_args_list[-1].args[0][-1])

    def test_missing_or_nonexecutable_binary_fails_before_fixtures(self) -> None:
        self.fixture("one")
        with mock.patch.object(packet_validate.subprocess, "run", return_value=self.completed()) as run:
            self.assertEqual(1, packet_validate.main(["--fixture", "one"]))
        self.assertEqual(1, run.call_count)

        self.binary(executable=False)
        with mock.patch.object(packet_validate.subprocess, "run", return_value=self.completed()) as run:
            self.assertEqual(1, packet_validate.main(["--fixture", "one"]))
        self.assertEqual(1, run.call_count)

    def test_leftover_daemon_marker_fails_packet_without_stopping_oracle(self) -> None:
        self.fixture("one")
        self.binary()

        def run(argv: list[str], **_kwargs: object) -> subprocess.CompletedProcess[object]:
            if argv[0] != "cargo":
                run_root = Path(argv[argv.index("--run-root") + 1])
                marker = run_root / "ob" / "one" / "slug" / "slugd.sock"
                marker.parent.mkdir(parents=True)
                marker.touch()
            return self.completed()

        with mock.patch.object(packet_validate.subprocess, "run", side_effect=run):
            self.assertEqual(1, packet_validate.main(["--fixture", "one"]))


if __name__ == "__main__":
    unittest.main()
