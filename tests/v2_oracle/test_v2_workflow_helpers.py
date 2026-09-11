from __future__ import annotations

import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]


def load_script(name: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / f"{name}.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


plan_status = load_script("v2_plan_status")
preflight = load_script("v2_test_preflight")


class WorkflowHelpersTest(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)

    def check_plan(self, canonical: str, manifest: str) -> int:
        plan = self.root / "plan.md"
        current = self.root / "current.md"
        plan.write_text(canonical, encoding="utf-8")
        current.write_text(manifest, encoding="utf-8")
        return plan_status.main(["--canonical", str(plan), "--manifest", str(current)])

    def test_matching_packets_ignore_historical_mentions(self) -> None:
        self.assertEqual(0, self.check_plan("Packet: WP-current-r1\nOld WP-other\n",
                                            "Packet: WP-current-r1\nStatus: ready\n"))

    def test_different_packets_fail(self) -> None:
        self.assertEqual(1, self.check_plan("Packet: WP-first\n", "Packet: WP-second\n"))

    def test_missing_duplicate_and_malformed_markers_fail_in_either_document(self) -> None:
        valid = "Packet: WP-first\n"
        for invalid in ("WP-first\n", valid * 2, "Packet: `WP-first`\n"):
            for canonical, manifest in ((invalid, valid), (valid, invalid)):
                with self.subTest(canonical=canonical, manifest=manifest):
                    self.assertEqual(1, self.check_plan(canonical, manifest))

    def harness(self, listed: str, ignored: str = "", exit_code: int = 0) -> Path:
        path = self.root / "harness"
        path.write_text(
            f"#!{sys.executable}\n"
            "import sys\n"
            f"with open({str(self.root / 'calls')!r}, 'a') as log:\n"
            "    log.write(repr(sys.argv[1:]) + '\\n')\n"
            "assert sys.argv[1:] in (['--list'], ['--list', '--ignored'])\n"
            f"print({ignored!r} if '--ignored' in sys.argv else {listed!r})\n"
            f"sys.exit({exit_code})\n", encoding="utf-8")
        path.chmod(0o755)
        return path

    def test_exact_selection_only_lists_tests(self) -> None:
        harness = self.harness("owner::one: test\nowner::two: test\n2 tests, 0 benchmarks")
        self.assertEqual(0, preflight.main([str(harness), "--exact", "owner::one",
                                             "--exact", "owner::two"]))
        self.assertEqual("['--list']\n['--list', '--ignored']\n",
                         (self.root / "calls").read_text())

    def test_missing_ambiguous_ignored_and_benchmark_names_fail(self) -> None:
        for listed, ignored in (("owner::other: test", ""),
                                ("owner::one: test\nowner::one: test", ""),
                                ("owner::one: test", "owner::one: test"),
                                ("owner::one: benchmark", "")):
            with self.subTest(listed=listed, ignored=ignored):
                harness = self.harness(listed, ignored)
                self.assertEqual(1, preflight.main([str(harness), "--exact", "owner::one"]))

    def test_duplicate_selection_fails_before_listing(self) -> None:
        harness = self.harness("owner::one: test")
        self.assertEqual(1, preflight.main([str(harness), "--exact", "owner::one", "owner::one"]))
        self.assertFalse((self.root / "calls").exists())

    def test_listing_failure_is_not_success(self) -> None:
        harness = self.harness("owner::one: test", exit_code=1)
        self.assertEqual(1, preflight.main([str(harness), "--exact", "owner::one"]))

    def test_timeout_fails_without_executing_tests(self) -> None:
        harness = self.harness("owner::one: test")
        with mock.patch.object(preflight.subprocess, "run",
                               side_effect=subprocess.TimeoutExpired("harness", 10)):
            self.assertEqual(1, preflight.main([str(harness), "--exact", "owner::one"]))
        self.assertFalse((self.root / "calls").exists())


if __name__ == "__main__":
    unittest.main()
