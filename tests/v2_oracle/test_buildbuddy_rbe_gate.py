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

from tools.v2_oracle import buildbuddy_rbe_gate as cli
from tools.v2_oracle_lib import buildbuddy_rbe as rbe

DIGEST = "a" * 64
HEAD = "b" * 40
TESTS = tuple(f"//tests:t{number:02d}" for number in range(43))
LABELS = (rbe.cache.BUILD_LABEL,) + TESTS


def sequence(events: list[dict[str, object]]) -> bytes:
    return b"\n".join(json.dumps(event, separators=(",", ":")).encode() for event in events) + b"\n"


def bep(change: str = "ready") -> bytes:
    events: list[dict[str, object]] = [
        {"id": {"targetCompleted": {"label": rbe.cache.BUILD_LABEL}}, "completed": {"success": True}},
        *({"id": {"targetCompleted": {"label": test}}, "completed": {"success": True}} for test in TESTS),
        *({"id": {"testSummary": {"label": test}}, "testSummary": {"overallStatus": "PASSED", "totalRunCount": 1}} for test in TESTS),
        {"id": {"buildFinished": {}}, "finished": {"exitCode": {"name": "SUCCESS"}}},
    ]
    if change == "duplicate_finished": events.append(events[-1])
    elif change == "missing_finished": events.pop()
    elif change == "duplicate_completion": events.insert(1, events[0])
    elif change == "missing_completion": events.pop(1)
    elif change == "foreign_completion": events.insert(1, {"id": {"targetCompleted": {"label": "//foreign:t"}}, "completed": {"success": True}})
    elif change == "failed_completion": events[1] = {**events[1], "completed": {"success": False}}
    elif change == "duplicate_summary": events.insert(-1, events[-2])
    elif change == "missing_summary": events.pop(-2)
    elif change == "foreign_summary": events.insert(-1, {"id": {"testSummary": {"label": "//foreign:t"}}, "testSummary": {"overallStatus": "PASSED", "totalRunCount": 1}})
    elif change == "failed_summary": events[-2] = {**events[-2], "testSummary": {"overallStatus": "FAILED", "totalRunCount": 1}}
    elif change == "bad_runs": events[-2] = {**events[-2], "testSummary": {"overallStatus": "PASSED", "totalRunCount": 2}}
    elif change == "cached": events[-2] = {**events[-2], "testSummary": {"overallStatus": "PASSED", "totalRunCount": 1, "totalNumCached": 1}}
    elif change.startswith("cached_"): events[-2]["testSummary"]["totalNumCached"] = {"cached_null": None, "cached_bool": False, "cached_string": "0"}[change]  # type: ignore[index,union-attr]
    elif change.startswith("exit_"): events[-1]["finished"]["exitCode"]["code"] = {"exit_null": None, "exit_bool": False, "exit_string": "0"}[change]  # type: ignore[index,union-attr]
    elif change == "remote": events.insert(-1, {"id": {"aborted": {}}, "aborted": {"reason": "REMOTE_ENVIRONMENT_FAILURE", "description": "/private/secret"}})
    elif change == "command": events[-1] = {"id": {"buildFinished": {}}, "finished": {"exitCode": {"name": "COMMAND_LINE_ERROR", "code": 2}}}
    elif change == "target": events[-1] = {"id": {"buildFinished": {}}, "finished": {"exitCode": {"name": "BUILD_FAILURE", "code": 1}}}
    elif change == "persistent": events.insert(-1, {"id": {"buildMetrics": {}}, "buildMetrics": {"actionSummary": {"actionCacheStatistics": {"hits": 1}}}})
    return sequence(events)


def execution(runner: str = "remote", **changes: object) -> bytes:
    event: dict[str, object] = {"remotable": True, "runner": runner, "action_digest": {"hash": DIGEST, "sizeBytes": 1}, "cache_hit": False, "status": "", "exit_code": 0}
    event.update(changes)
    return sequence([{"SpawnExec": event}])


class FullRbeGateTest(unittest.TestCase):
    def _phase(self) -> tuple[dict[str, object], str]:
        phase, outcome = rbe.phase_record(bep(), execution(), TESTS, 0); phase["output_count"] = 1
        return phase, outcome

    def test_exact_command_order_nonce_and_bans(self) -> None:
        output, event, log, nonce = Path("/p/output"), Path("/p/bep"), Path("/p/execution"), "f" * 64
        expected = ["bazel", f"--output_base={output}", "test", "--config=buildbuddy-rbe", "--@rules_rust//rust/toolchain/channel=nightly", "--noremote_accept_cached", "--noremote_upload_local_results", "--remote_download_outputs=toplevel", "--remote_timeout=900", "--jobs=4", "--remote_instance_name=", "--bes_backend=", "--bes_results_url=", "--disk_cache=", "--build_event_publish_all_actions", "--cache_test_results=yes", "--runs_per_test=1", "--test_sharding_strategy=disabled", f"--action_env=SLUG_BUILDBUDDY_RBE_GATE_NONCE={nonce}", f"--test_env=SLUG_BUILDBUDDY_RBE_GATE_NONCE={nonce}", f"--build_event_json_file={event}", f"--execution_log_json_file={log}", *LABELS]
        self.assertEqual(expected, rbe.command("bazel", output, event, log, nonce, LABELS))
        accepted = rbe.one.command("bazel", output, event, log, nonce); self.assertEqual(accepted[3:15], expected[3:15])
        for option in expected[15:22]: self.assertEqual(1, expected.count(option))
        for forbidden in ("--remote_accept_cached", "--remote_upload_local_results", "--remote_cache=", "--remote_executor=", "--remote_default_exec_properties", "--remote_local_fallback"):
            self.assertFalse(any(item.startswith(forbidden) for item in expected))
        for bad_nonce, labels in (("secret", LABELS), (nonce, LABELS[:-1]), (nonce, LABELS[:-2] + (LABELS[-1], LABELS[-2]))):
            with self.assertRaises(rbe.GateError): rbe.command("bazel", output, event, log, bad_nonce, labels)

    def test_all_spawn_runners_fields_and_digests(self) -> None:
        for runner, bucket in (("remote", "remote_execution"), ("remote cache hit", "remote_cache_hit"), ("local", "local"), ("worker", "worker"), ("linux-sandbox", "linux_sandbox"), ("unknown", "other")):
            summary = rbe.spawn_summary(rbe.cache.json_sequence(execution(runner))); self.assertEqual(1, summary["count"]); self.assertEqual(1, summary[bucket])
        missing = object()
        for field, accepted, rejected, key in (("remotable", True, (missing, None, "true", False, 1), "remotable_error_count"), ("cache_hit", False, (missing, None, "false", True, 0), "cache_hit_error_count"), ("status", "", (missing, None, "bad", False, 0), "status_error_count"), ("exit_code", 0, (missing, None, "0", False, 1), "exit_error_count")):
            for value, expected in ((accepted, 0), *((item, 1) for item in rejected)):
                event = json.loads(execution())["SpawnExec"]
                if value is missing: event.pop(field)
                else: event[field] = value
                self.assertEqual(expected, rbe.spawn_summary([{"SpawnExec": event}])[key])
        self.assertEqual(0, rbe.spawn_summary(rbe.cache.json_sequence(execution(action_digest={"hash": "bad", "sizeBytes": 1})))["valid_digest_count"])
        self.assertEqual(2, rbe.spawn_summary(rbe.cache.json_sequence(execution() + execution("worker")))["count"])

    def test_bep_defaults_singletons_and_classification(self) -> None:
        phase, outcome = self._phase(); self.assertEqual("PROVED_RBE", rbe.classify(phase, outcome)); self.assertEqual((1, 43, 43, 0), (phase["build_success_count"], phase["test_completion_count"], phase["passed_test_count"], phase["remotely_cached_test_count"]))
        for change in ("duplicate_finished", "missing_finished", "duplicate_completion", "missing_completion", "foreign_completion", "duplicate_summary", "missing_summary", "foreign_summary", "cached_null", "cached_bool", "cached_string", "exit_null", "exit_bool", "exit_string"):
            with self.subTest(change=change), self.assertRaises(rbe.GateError): rbe.phase_record(bep(change), execution(), TESTS, 0)
        for change, expected in (("failed_completion", "TARGET_FAILURE"), ("failed_summary", "TARGET_FAILURE"), ("bad_runs", "TARGET_FAILURE"), ("cached", "CACHE_HIT_OR_MIXED_EXECUTION"), ("remote", "REMOTE_UNAVAILABLE"), ("command", "COMMAND_LINE_FAILURE"), ("target", "TARGET_FAILURE"), ("persistent", "CACHE_HIT_OR_MIXED_EXECUTION")):
            current, current_outcome = rbe.phase_record(bep(change), execution(), TESTS, 0); current["output_count"] = 1
            self.assertEqual(expected, rbe.classify(current, current_outcome))
        invalid = {**phase, "spawns": {**phase["spawns"], "valid_digest_count": 0}}; self.assertEqual("EVIDENCE_INCOMPLETE", rbe.classify(invalid, "success"))
        for key in ("remotable_error_count", "cache_hit_error_count", "status_error_count", "exit_error_count", "remote_cache_hit", "local", "worker", "linux_sandbox", "other"):
            current = {**phase, "spawns": {**phase["spawns"], key: 1}}
            self.assertEqual("CACHE_HIT_OR_MIXED_EXECUTION", rbe.classify(current, "success"))

    def test_closed_schema_classes_platform_manifest_and_hostile_values(self) -> None:
        phase, _ = self._phase(); proved = rbe.record("PROVED_RBE", phase, HEAD, True)
        self.assertEqual(proved, rbe.normalize(proved)); self.assertEqual({"build": 1, "test": 43}, proved["target_counts"])
        self.assertEqual({"PROVED_RBE", "CONFIG_DRIFT", "REMOTE_UNAVAILABLE", "COMMAND_LINE_FAILURE", "TARGET_FAILURE", "CACHE_HIT_OR_MIXED_EXECUTION", "EVIDENCE_INCOMPLETE", "SANITIZER_REJECTED"}, rbe.CLASSES)
        self.assertEqual((rbe.cache.VERSION, rbe.cache.MANIFEST_SHA256, rbe.REMOTE_PLATFORM), (proved["manifest_version"], proved["manifest_sha256"], proved["remote_platform"]))
        class DictSubclass(dict): pass
        class StringSubclass(str): pass
        hostile = [{**proved, "secret": "/private/token"}, DictSubclass(proved), {**proved, "classification": StringSubclass("PROVED_RBE")}, {**proved, "schema_version": True}, {**proved, "git_clean": 1}, {**proved, "target_counts": {"build": 1, "test": 42}}]
        for value in hostile: self.assertEqual(rbe.record(), rbe.normalize(value))
        for mutate in (lambda value: value["rbe"].pop("output_count"), lambda value: value["rbe"].update(secret=1), lambda value: value["rbe"].update(output_count=-1), lambda value: value["rbe"]["spawns"].update(count=2), lambda value: value["rbe"]["spawns"].update(valid_digest_count=2)):
            value = json.loads(json.dumps(proved)); mutate(value); self.assertEqual(rbe.record(), rbe.normalize(value))
        value = json.loads(json.dumps(proved)); value["rbe"] = DictSubclass(value["rbe"]); self.assertEqual(rbe.record(), rbe.normalize(value)); self.assertNotIn("private", json.dumps(rbe.record()))

    def _run(self, change: str = "ready", clean: bool = True) -> tuple[dict[str, object], list[list[str]], list[Path]]:
        calls: list[list[str]] = []; roots: list[Path] = []
        def runner(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            calls.append(argv)
            if argv[-1] == "shutdown":
                output = Path(argv[2].split("=", 1)[1])
                if change == "shutdown_output_swap": output.rename(output.with_name("saved-output")); output.mkdir()
                if change == "shutdown_exception": raise OSError("/private")
                return subprocess.CompletedProcess(argv, 7 if change == "shutdown_nonzero" else 0)
            output = Path(argv[1].split("=", 1)[1]); phase, root = output.parent, output.parents[1]; roots.append(root)
            event = Path(next(item.split("=", 1)[1] for item in argv if item.startswith("--build_event_json_file="))); log = Path(next(item.split("=", 1)[1] for item in argv if item.startswith("--execution_log_json_file=")))
            event_change = change if change in {"duplicate_finished", "foreign_summary", "failed_summary", "cached", "remote", "command", "target", "persistent"} else "ready"
            event.write_bytes(bep(event_change)); log.write_bytes(execution("worker" if change == "worker" else "remote"))
            target = output / "execroot/x/bin/app/slug_cli_v2/slug"
            if change != "output_missing":
                target.parent.mkdir(parents=True); target.write_bytes(b"x"); target.chmod(0o600 if change == "output_nonexec" else 0o700)
                if change == "output_multiple": second = output / "execroot/y/bin/app/slug_cli_v2/slug"; second.parent.mkdir(parents=True); second.write_bytes(b"x"); second.chmod(0o700)
            if change == "bep_replace": event.rename(event.with_name("old-bep")); event.write_bytes(bep())
            elif change == "execution_replace": log.rename(log.with_name("old-execution")); log.write_bytes(execution())
            elif change == "bep_symlink": event.unlink(); event.symlink_to("/dev/null")
            elif change == "execution_mode": log.chmod(0o640)
            elif change == "execution_missing": log.unlink()
            elif change == "execution_bad": log.write_bytes(b"{")
            elif change == "root_swap": root.rename(root.with_name("saved-root")); root.mkdir()
            elif change == "phase_swap": phase.rename(phase.with_name("saved-rbe")); phase.mkdir()
            elif change == "output_swap": output.rename(output.with_name("saved-output")); output.mkdir()
            return subprocess.CompletedProcess(argv, 0)
        with mock.patch.object(rbe.cache, "load_manifest", return_value=(rbe.cache.BUILD_LABEL, TESTS)) as manifest, mock.patch.object(rbe.cache, "_preflight", return_value=HEAD), mock.patch.object(rbe.cache, "_clean", return_value=clean):
            result = rbe.run_gate(runner=runner)
        manifest.assert_called_once_with()
        return result, calls, roots

    def test_private_run_manifest_reuse_output_and_cleanup(self) -> None:
        result, calls, roots = self._run(); self.assertEqual("PROVED_RBE", result["classification"]); self.assertEqual(2, len(calls)); self.assertTrue(roots[0].name.startswith("slug-buildbuddy-full-rbe-")); self.assertFalse(roots[0].exists())
        self.assertEqual(["bazel", "--ignore_all_rc_files", f"--output_base={roots[0] / 'rbe/output'}", "shutdown"], calls[1])
        self.assertEqual((1, 43, 43), (result["rbe"]["output_count"], result["rbe"]["test_completion_count"], result["rbe"]["passed_test_count"]))

    def test_artifact_root_output_and_cleanup_failures(self) -> None:
        self.assertEqual("PROVED_RBE", self._run("execution_replace")[0]["classification"])
        for change, expected in (("bep_replace", "EVIDENCE_INCOMPLETE"), ("bep_symlink", "EVIDENCE_INCOMPLETE"), ("execution_mode", "EVIDENCE_INCOMPLETE"), ("execution_missing", "EVIDENCE_INCOMPLETE"), ("execution_bad", "EVIDENCE_INCOMPLETE"), ("worker", "CACHE_HIT_OR_MIXED_EXECUTION"), ("output_missing", "TARGET_FAILURE"), ("output_nonexec", "TARGET_FAILURE"), ("output_multiple", "TARGET_FAILURE"), ("duplicate_finished", "EVIDENCE_INCOMPLETE"), ("foreign_summary", "EVIDENCE_INCOMPLETE"), ("failed_summary", "TARGET_FAILURE"), ("cached", "CACHE_HIT_OR_MIXED_EXECUTION")):
            with self.subTest(change=change): self.assertEqual(expected, self._run(change)[0]["classification"])
        for change in ("root_swap", "phase_swap", "output_swap", "shutdown_output_swap", "shutdown_nonzero", "shutdown_exception"):
            result, _, roots = self._run(change); self.assertEqual(rbe.record(), result); self.assertTrue(all(not root.exists() for root in roots))
        actual = rbe.cache._remove_original
        def failed(*args: object) -> bool: actual(*args); return False
        with mock.patch.object(rbe.cache, "_remove_original", side_effect=failed): self.assertEqual(rbe.record(), self._run()[0])
        self.assertEqual(rbe.record(), self._run(clean=False)[0])

    def test_reserved_namespace_refusal_and_removal(self) -> None:
        parent = Path(tempfile.gettempdir()); parent_fd = os.open(parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        wrong = Path(tempfile.mkdtemp(prefix="slug-buildbuddy-rbe-")); allowed = Path(tempfile.mkdtemp(prefix="slug-buildbuddy-full-rbe-"))
        try:
            self.assertFalse(rbe._remove_reserved(wrong, parent_fd)); self.assertTrue(wrong.exists()); self.assertTrue(rbe._remove_reserved(allowed, parent_fd)); self.assertFalse(allowed.exists())
        finally:
            os.close(parent_fd)
            if wrong.exists(): wrong.rmdir()

    def test_cli_privacy_no_args_and_existing_gate_imports(self) -> None:
        phase, _ = self._phase(); proved = rbe.record("PROVED_RBE", phase, HEAD, True); out, err = StringIO(), StringIO()
        with mock.patch.object(cli.gate, "run_gate", return_value=proved), redirect_stdout(out), redirect_stderr(err): self.assertEqual(0, cli.main([]))
        self.assertEqual("", err.getvalue()); self.assertEqual(json.dumps(proved, sort_keys=True, separators=(",", ":")) + "\n", out.getvalue())
        out, err = StringIO(), StringIO()
        with mock.patch.object(cli.gate, "run_gate", side_effect=RuntimeError("/private/token")), redirect_stdout(out), redirect_stderr(err): self.assertEqual(1, cli.main([]))
        self.assertNotIn("private", out.getvalue()); self.assertEqual("", err.getvalue()); self.assertEqual(rbe.record(), json.loads(out.getvalue()))
        with mock.patch.object(cli.gate, "run_gate") as run, redirect_stdout(StringIO()), redirect_stderr(StringIO()): self.assertEqual(1, cli.main(["unexpected"])); run.assert_not_called()
        from tools.v2_oracle_lib import buildbuddy_build_cache, buildbuddy_cache, buildbuddy_build_rbe
        self.assertIs(rbe.cache, buildbuddy_cache); self.assertIs(buildbuddy_build_cache.parsed, buildbuddy_cache); self.assertIs(rbe.one, buildbuddy_build_rbe)


if __name__ == "__main__": unittest.main()
