from __future__ import annotations

import hashlib
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

from tools.v2_oracle import buildbuddy_cache_gate as cli
from tools.v2_oracle_lib import buildbuddy_cache as gate

DIGEST = "d" * 64
HEAD = "a" * 40
TESTS = tuple(f"//tests:t{number:02d}" for number in range(43))
LABELS = (gate.BUILD_LABEL,) + TESTS


def sequence(events: list[dict[str, object]]) -> bytes:
    return b"\n".join(json.dumps(event, separators=(",", ":")).encode() for event in events) + b"\n"


def bep(phase: str, change: str = "ready") -> bytes:
    events: list[dict[str, object]] = [
        {"id": {"targetCompleted": {"label": gate.BUILD_LABEL}}, "completed": {"success": True}},
        *({"id": {"targetCompleted": {"label": test}}, "completed": {"success": True}} for test in TESTS),
        *({"id": {"testSummary": {"label": test}}, "testSummary": {"overallStatus": "PASSED", "totalRunCount": 1, **({"totalNumCached": 1} if phase == "replay" else {})}} for test in TESTS),
        {"id": {"buildFinished": {}}, "finished": {"exitCode": {"name": "SUCCESS"}}},
    ]
    if change == "duplicate_finished": events.append(events[-1])
    elif change == "missing_finished": events.pop()
    elif change == "duplicate_completion": events.insert(1, events[0])
    elif change == "missing_completion": events.pop(1)
    elif change == "foreign_completion": events.insert(1, {"id": {"targetCompleted": {"label": "//foreign:target"}}, "completed": {"success": True}})
    elif change == "failed_completion": events[1] = {**events[1], "completed": {"success": False}}
    elif change == "duplicate_summary": events.insert(-1, events[-2])
    elif change == "missing_summary": events.pop(-2)
    elif change == "foreign_summary": events.insert(-1, {"id": {"testSummary": {"label": "//foreign:test"}}, "testSummary": {"overallStatus": "PASSED", "totalRunCount": 1, "totalNumCached": 0}})
    elif change == "failed_summary": events[-2] = {**events[-2], "testSummary": {"overallStatus": "FAILED", "totalRunCount": 1, "totalNumCached": int(phase == "replay")}}
    elif change == "bad_runs": events[-2] = {**events[-2], "testSummary": {"overallStatus": "PASSED", "totalRunCount": 2, "totalNumCached": int(phase == "replay")}}
    elif change == "bad_cached": events[-2] = {**events[-2], "testSummary": {"overallStatus": "PASSED", "totalRunCount": 1, "totalNumCached": int(phase != "replay")}}
    elif change == "impossible_cached": events[-2] = {**events[-2], "testSummary": {"overallStatus": "PASSED", "totalRunCount": 0, "totalNumCached": 1}}
    elif change == "missing_replay_cached": events[-2]["testSummary"].pop("totalNumCached")  # type: ignore[index,union-attr]
    elif change.startswith("cached_"): events[-2]["testSummary"]["totalNumCached"] = {"cached_null": None, "cached_bool": False, "cached_string": "0"}[change]  # type: ignore[index,union-attr]
    elif change.startswith("exit_"): events[-1]["finished"]["exitCode"]["code"] = {"exit_null": None, "exit_bool": False, "exit_string": "0"}[change]  # type: ignore[index,union-attr]
    elif change == "remote": events.insert(-1, {"id": {"aborted": {}}, "aborted": {"reason": "REMOTE_ENVIRONMENT_FAILURE", "description": "/private/secret"}})
    elif change == "command": events[-1] = {"id": {"buildFinished": {}}, "finished": {"exitCode": {"name": "COMMAND_LINE_ERROR", "code": 2}}}
    elif change == "target": events[-1] = {"id": {"buildFinished": {}}, "finished": {"exitCode": {"name": "BUILD_FAILURE", "code": 1}}}
    elif change == "persistent": events.insert(-1, {"id": {"buildMetrics": {}}, "buildMetrics": {"actionSummary": {"actionCacheStatistics": {"hits": 1}}}})
    return sequence(events)


def execution(phase: str, runner: str | None = None, **changes: object) -> bytes:
    event: dict[str, object] = {"cacheable": True, "remoteCacheable": True, "runner": runner or ("linux-sandbox" if phase == "prime" else "remote cache hit"), "action_digest": {"hash": DIGEST, "sizeBytes": 4}, "cache_hit": phase == "replay", "status": "", "exit_code": 0}
    event.update(changes)
    return sequence([{"SpawnExec": event}])


class FullCacheGateTest(unittest.TestCase):
    def test_manifest_exact_shape_and_fixed_path_anchors(self) -> None:
        data = (gate.VERSION + "\n" + "build\t" + gate.BUILD_LABEL + "\n" + "".join(f"test\t{test}\n" for test in TESTS)).encode()
        with mock.patch.object(gate, "MANIFEST_SHA256", hashlib.sha256(data).hexdigest()), mock.patch.object(gate, "_manifest_bytes", return_value=data):
            self.assertEqual((gate.BUILD_LABEL, TESTS), gate.load_manifest())
        for bad in (data[:-1], data.replace(b"test\t//tests:t00", b"other\t//tests:t00"), data.replace(b"//tests:t00", b"//tests:t99")):
            with mock.patch.object(gate, "MANIFEST_SHA256", hashlib.sha256(bad).hexdigest()), mock.patch.object(gate, "_manifest_bytes", return_value=bad), self.assertRaises(gate.GateError): gate.load_manifest()
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory) / "repo"; oracle = repo / "tests/v2_oracle"; oracle.mkdir(parents=True); target = oracle / "buildbuddy_cache_targets.txt"; target.write_bytes(data)
            digest = hashlib.sha256(data).hexdigest()
            with mock.patch.object(gate, "REPO_ROOT", repo), mock.patch.object(gate, "MANIFEST_SHA256", digest): self.assertEqual((gate.BUILD_LABEL, TESTS), gate.load_manifest())
            exact_copy = Path(directory) / "exact-copy.txt"; exact_copy.write_bytes(data)
            with mock.patch.object(gate, "MANIFEST_SHA256", digest), mock.patch.object(cli.gate, "run_gate") as run, redirect_stdout(StringIO()), redirect_stderr(StringIO()): self.assertEqual(1, cli.main([str(exact_copy)])); run.assert_not_called()
            tests = repo / "tests"; saved = repo / "real-tests"; tests.rename(saved); tests.symlink_to(saved, target_is_directory=True)
            with mock.patch.object(gate, "REPO_ROOT", repo), mock.patch.object(gate, "MANIFEST_SHA256", digest), self.assertRaises(gate.GateError): gate.load_manifest()

    def test_manifest_midread_name_and_directory_replacement(self) -> None:
        data = (gate.VERSION + "\n" + "build\t" + gate.BUILD_LABEL + "\n" + "".join(f"test\t{test}\n" for test in TESTS)).encode(); digest = hashlib.sha256(data).hexdigest()
        for change in ("name", "directory"):
            with self.subTest(change=change), tempfile.TemporaryDirectory() as directory:
                repo = Path(directory) / "repo"; oracle = repo / "tests/v2_oracle"; oracle.mkdir(parents=True); target = oracle / "buildbuddy_cache_targets.txt"; target.write_bytes(data)
                original, changed = gate.os.read, []
                def replace(fd: int, size: int) -> bytes:
                    payload = original(fd, size)
                    if not changed:
                        changed.append(True)
                        if change == "name": target.rename(target.with_name("saved-manifest")); target.write_bytes(data)
                        else: oracle.rename(oracle.with_name("saved-v2_oracle")); oracle.mkdir(); (oracle / target.name).write_bytes(data)
                    return payload
                with mock.patch.object(gate, "REPO_ROOT", repo), mock.patch.object(gate, "MANIFEST_SHA256", digest), mock.patch.object(gate.os, "read", side_effect=replace), self.assertRaises(gate.GateError): gate.load_manifest()

    def test_exact_full_vector_nonce_and_legacy_command(self) -> None:
        nonce = "f" * 64; output, event, log = Path("/private/output"), Path("/private/bep"), Path("/private/execution")
        expected = ["bazel", f"--output_base={output}", "test", "--config=buildbuddy-cache", "--noremote_accept_cached", "--@rules_rust//rust/toolchain/channel=nightly", "--remote_executor=", "--bes_backend=", "--bes_results_url=", "--disk_cache=", "--noremote_local_fallback", "--cache_test_results=yes", "--runs_per_test=1", "--test_sharding_strategy=disabled", f"--action_env=SLUG_BUILDBUDDY_CACHE_GATE_NONCE={nonce}", f"--test_env=SLUG_BUILDBUDDY_CACHE_GATE_NONCE={nonce}", f"--build_event_json_file={event}", f"--execution_log_json_file={log}", *LABELS]
        self.assertEqual(expected, gate.full_command("prime", "bazel", output, event, log, nonce, LABELS))
        replay = gate.full_command("replay", "bazel", output, event, log, nonce, LABELS)
        self.assertEqual("--remote_accept_cached", replay[4])
        for forbidden in ("--remote_cache=", "--spawn_strategy=", "--test_strategy=", "--remote_upload_local_results", "--noremote_upload_local_results", "--remote_cache_async", "--noremote_cache_async"):
            self.assertFalse(any(item.startswith(forbidden) for item in expected))
        for bad_phase, bad_nonce, bad_labels in (("other", nonce, LABELS), ("prime", "secret", LABELS), ("prime", nonce, LABELS[:-1]), ("prime", nonce, LABELS[:-2] + (LABELS[-1], LABELS[-2]))):
            with self.assertRaises(gate.GateError): gate.full_command(bad_phase, "bazel", output, event, log, bad_nonce, bad_labels)
        legacy = ["bazel", f"--output_base={output}", "test", "--config=buildbuddy-cache", "--@rules_rust//rust/toolchain/channel=nightly", "--remote_cache=grpcs://remote.buildbuddy.io", "--remote_instance_name=", "--remote_executor=", "--bes_backend=", "--bes_results_url=", "--disk_cache=", "--spawn_strategy=worker,sandboxed,local", "--test_strategy=local", "--cache_test_results=yes", "--runs_per_test=1", "--test_sharding_strategy=disabled", "--noremote_local_fallback", "--build_event_publish_all_actions", f"--build_event_json_file={event}", f"--execution_log_json_file={log}", f"--action_env=SLUG_BUILDBUDDY_CACHE_GATE_NONCE={nonce}", f"--test_env=SLUG_BUILDBUDDY_CACHE_GATE_NONCE={nonce}", "--noremote_accept_cached", "--remote_upload_local_results", "--noremote_cache_async", *LABELS]
        self.assertEqual(legacy, gate.command("prime", "bazel", output, event, log, nonce, LABELS))

    def test_shared_parser_api_and_spawn_strictness(self) -> None:
        self.assertEqual([{"a": 1}, {"b": 2}], list(gate.json_sequence(b'{"a":1}\n {"b":2}')))
        for data in (b"[]", b"{", b"\xff"):
            with self.assertRaises(gate.GateError): list(gate.json_sequence(data))
        self.assertEqual(1, gate._field({"a": 1}, "a")); self.assertEqual("x", gate._field({}, "a", default="x"))
        self.assertFalse(gate._boolean({}, "a")); self.assertTrue(gate._boolean({"a": True}, "a")); self.assertEqual(2, gate._count(2))
        self.assertEqual('{"hash":"' + DIGEST + '","sizeBytes":4}', gate._digest({"hash": DIGEST, "sizeBytes": "4"}))
        for call in (lambda: gate._boolean({"a": 1}, "a"), lambda: gate._count(True), lambda: gate._count(-1), lambda: gate._digest({"hash": "bad", "sizeBytes": 1}), lambda: gate._digest({"hash": DIGEST, "sizeBytes": True})):
            with self.assertRaises(gate.GateError): call()
        for runner, bucket in (("local", "local"), ("worker", "worker"), ("linux-sandbox", "linux_sandbox"), ("remote cache hit", "remote_cache_hit"), ("remote", "other")):
            summary = gate.spawn_summary(gate.json_sequence(execution("prime", runner=runner)), "prime")
            self.assertEqual(1, summary[bucket])
        for field, value, error in (("cache_hit", "false", "cache_error_count"), ("status", "bad", "status_error_count"), ("exit_code", 1, "exit_error_count")):
            self.assertEqual(1, gate.spawn_summary(gate.json_sequence(execution("prime", **{field: value})), "prime")[error])
        with self.assertRaises(gate.GateError): gate.spawn_summary(gate.json_sequence(execution("prime", action_digest={"hash": "bad", "sizeBytes": 1})), "prime")

    def test_singleton_bep_and_cache_predicates(self) -> None:
        prime = gate.phase_record(bep("prime"), execution("prime"), "prime", TESTS, 0); prime["output_count"] = 1
        replay = gate.phase_record(bep("replay"), execution("replay"), "replay", TESTS, 0); replay["output_count"] = 1
        self.assertEqual("PROVED_CACHE_ONLY", gate.classify(prime, replay))
        self.assertEqual((43, 43, 0), (prime["test_completion_count"], prime["passed_test_count"], prime["remotely_cached_test_count"]))
        self.assertEqual(43, replay["remotely_cached_test_count"])
        for change in ("duplicate_finished", "missing_finished", "duplicate_completion", "missing_completion", "foreign_completion", "duplicate_summary", "missing_summary", "foreign_summary", "impossible_cached", "cached_null", "cached_bool", "cached_string", "exit_null", "exit_bool", "exit_string"):
            with self.subTest(change=change), self.assertRaises(gate.GateError): gate.phase_record(bep("prime", change), execution("prime"), "prime", TESTS, 0)
        with self.assertRaises(gate.GateError): gate.phase_record(bep("replay", "missing_replay_cached"), execution("replay"), "replay", TESTS, 0)
        for change, expected in (("failed_completion", "TARGET_FAILURE"), ("failed_summary", "TARGET_FAILURE"), ("bad_runs", "TARGET_FAILURE"), ("bad_cached", "CACHE_MISS_OR_MIXED_REPLAY"), ("remote", "REMOTE_UNAVAILABLE"), ("command", "COMMAND_LINE_FAILURE"), ("target", "TARGET_FAILURE"), ("persistent", "CACHE_MISS_OR_MIXED_REPLAY")):
            current = gate.phase_record(bep("prime", change), execution("prime"), "prime", TESTS, 0); current["output_count"] = 1
            self.assertEqual(expected, gate.classify(current, replay), change)
        current = json.loads(json.dumps(prime)); current["eligible_spawns"]["digest_multiset_sha256"] = "e" * 64
        self.assertEqual("CACHE_MISS_OR_MIXED_REPLAY", gate.classify(current, replay))
        current = json.loads(json.dumps(replay)); current["eligible_spawns"].update(remote_cache_hit=0, local=1)
        self.assertEqual("CACHE_MISS_OR_MIXED_REPLAY", gate.classify(prime, current))

    def test_closed_schema_classes_counts_and_hostile_values(self) -> None:
        prime = gate.phase_record(bep("prime"), execution("prime"), "prime", TESTS, 0); prime["output_count"] = 1
        replay = gate.phase_record(bep("replay"), execution("replay"), "replay", TESTS, 0); replay["output_count"] = 1
        proved = gate.record("PROVED_CACHE_ONLY", prime, replay, HEAD, True)
        self.assertEqual(proved, gate.normalize(proved)); self.assertEqual(set(gate.record()), set(proved))
        self.assertEqual({"build": 1, "test": 43}, proved["target_counts"]); self.assertEqual(gate.CLASSES, gate.FAILURES | {"PROVED_CACHE_ONLY"})
        class DictSubclass(dict): pass
        class StringSubclass(str): pass
        hostile = [
            {**proved, "secret": "/private/token"}, DictSubclass(proved), {**proved, "classification": StringSubclass("PROVED_CACHE_ONLY")},
            {**proved, "schema_version": True}, {**proved, "git_clean": 1}, {**proved, "target_counts": {"build": 1, "test": 42}},
        ]
        for value in hostile: self.assertEqual(gate.record(), gate.normalize(value))
        for mutate in (lambda value: value["prime"].pop("output_count"), lambda value: value["prime"].update(secret=1), lambda value: value["prime"].update(output_count=-1), lambda value: value["prime"]["eligible_spawns"].update(count=2), lambda value: value["prime"]["eligible_spawns"].update(digest_multiset_sha256="bad")):
            value = json.loads(json.dumps(proved)); mutate(value); self.assertEqual(gate.record(), gate.normalize(value))
        value = json.loads(json.dumps(proved)); value["prime"] = DictSubclass(value["prime"]); self.assertEqual(gate.record(), gate.normalize(value))
        self.assertNotIn("private", json.dumps(gate.record()))

    def _run(self, change: str = "ready", post_clean: bool = True) -> tuple[dict[str, object], list[list[str]], list[Path]]:
        calls: list[list[str]] = []; roots: list[Path] = []
        def runner(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            calls.append(argv)
            if argv[-1] == "shutdown":
                output = Path(argv[2].split("=", 1)[1])
                if change == "shutdown_output_swap": output.rename(output.with_name("saved-output")); output.mkdir()
                if change == "shutdown_exception": raise OSError("/private")
                return subprocess.CompletedProcess(argv, 1 if change == "shutdown_nonzero" else 0)
            output = Path(argv[1].split("=", 1)[1]); phase_root, root = output.parent, output.parents[1]; roots.append(root)
            phase = "prime" if "--noremote_accept_cached" in argv else "replay"
            event_path = Path(next(item.split("=", 1)[1] for item in argv if item.startswith("--build_event_json_file=")))
            log_path = Path(next(item.split("=", 1)[1] for item in argv if item.startswith("--execution_log_json_file=")))
            event_change = change if phase == "prime" and change in {"duplicate_finished", "missing_finished", "duplicate_completion", "missing_completion", "foreign_completion", "duplicate_summary", "missing_summary", "foreign_summary", "failed_completion", "failed_summary", "bad_runs", "bad_cached", "impossible_cached", "remote", "command", "target", "persistent"} else "ready"
            event_path.write_bytes(bep(phase, event_change)); log_path.write_bytes(execution(phase))
            target = output / "execroot/x/bin/app/slug_cli_v2/slug"
            if change != "output_missing":
                target.parent.mkdir(parents=True); target.write_bytes(b"x"); target.chmod(0o600 if change == "output_nonexec" else 0o700)
                if change == "output_multiple":
                    second = output / "execroot/y/bin/app/slug_cli_v2/slug"; second.parent.mkdir(parents=True); second.write_bytes(b"x"); second.chmod(0o700)
            if phase == "prime":
                if change == "bep_replace": event_path.rename(event_path.with_name("old-bep")); event_path.write_bytes(bep(phase))
                elif change == "execution_replace": log_path.rename(log_path.with_name("old-execution")); log_path.write_bytes(execution(phase))
                elif change == "bep_symlink": event_path.unlink(); event_path.symlink_to("/dev/null")
                elif change == "execution_mode": log_path.chmod(0o640)
                elif change == "execution_missing": log_path.unlink()
                elif change == "execution_bad": log_path.write_bytes(b"{")
                elif change == "root_swap": root.rename(root.with_name("saved-root")); root.mkdir()
                elif change == "phase_swap": phase_root.rename(phase_root.with_name("saved-prime")); phase_root.mkdir()
                elif change == "output_swap": output.rename(output.with_name("saved-output")); output.mkdir()
            return subprocess.CompletedProcess(argv, 0)
        clean = mock.Mock(side_effect=[True] if post_clean else [False])
        with mock.patch.object(gate, "load_manifest", return_value=(gate.BUILD_LABEL, TESTS)), mock.patch.object(gate, "_preflight", return_value=HEAD), mock.patch.object(gate, "_clean", clean):
            result = gate.run_gate(runner=runner)
        return result, calls, roots

    def test_private_two_phase_run_output_and_cleanup(self) -> None:
        result, calls, roots = self._run()
        self.assertEqual("PROVED_CACHE_ONLY", result["classification"]); self.assertEqual(4, len(calls)); self.assertEqual(2, len(roots))
        self.assertEqual(["prime", "replay"], ["prime" if "--noremote_accept_cached" in call else "replay" for call in calls[:2]])
        self.assertEqual(roots[0], roots[1]); self.assertFalse(roots[0].exists())
        for phase in ("prime", "replay"):
            self.assertEqual(1, result[phase]["output_count"]); self.assertEqual(43, result[phase]["passed_test_count"])

    def test_artifact_output_anchor_and_evidence_failures(self) -> None:
        self.assertEqual("PROVED_CACHE_ONLY", self._run("execution_replace")[0]["classification"])
        for change, expected in (
            ("bep_replace", "EVIDENCE_INCOMPLETE"), ("bep_symlink", "EVIDENCE_INCOMPLETE"), ("execution_mode", "EVIDENCE_INCOMPLETE"), ("execution_missing", "EVIDENCE_INCOMPLETE"), ("execution_bad", "EVIDENCE_INCOMPLETE"),
            ("output_missing", "TARGET_FAILURE"), ("output_nonexec", "TARGET_FAILURE"), ("output_multiple", "TARGET_FAILURE"),
            ("duplicate_finished", "EVIDENCE_INCOMPLETE"), ("duplicate_completion", "EVIDENCE_INCOMPLETE"), ("foreign_completion", "EVIDENCE_INCOMPLETE"), ("foreign_summary", "EVIDENCE_INCOMPLETE"), ("failed_summary", "TARGET_FAILURE"), ("bad_cached", "CACHE_MISS_OR_MIXED_REPLAY"),
        ):
            with self.subTest(change=change): self.assertEqual(expected, self._run(change)[0]["classification"])
        for change in ("root_swap", "phase_swap", "output_swap", "shutdown_output_swap", "shutdown_nonzero", "shutdown_exception"):
            with self.subTest(change=change):
                result, _, roots = self._run(change); self.assertEqual(gate.record(), result); self.assertTrue(all(not root.exists() for root in roots))

    def test_replacement_during_read_cleanup_and_clean_suppression(self) -> None:
        original, changed = gate.os.read, []
        def replace(fd: int, size: int) -> bytes:
            data = original(fd, size)
            if not changed and size >= (1 << 20):
                changed.append(True)
                for root in Path(tempfile.gettempdir()).glob("slug-buildbuddy-cache-*/prime/bep.json"):
                    saved = root.with_name("midread-bep"); root.rename(saved); root.write_bytes(bep("prime"))
            return data
        with mock.patch.object(gate.os, "read", side_effect=replace): self.assertEqual("EVIDENCE_INCOMPLETE", self._run()[0]["classification"])
        actual = gate._remove_original
        def fail_remove(*args: object) -> bool: actual(*args); return False
        with mock.patch.object(gate, "_remove_original", side_effect=fail_remove): self.assertEqual(gate.record(), self._run()[0])
        self.assertEqual(gate.record(), self._run(post_clean=False)[0])
        with mock.patch.object(gate, "load_manifest", return_value=(gate.BUILD_LABEL, TESTS)), mock.patch.object(gate, "_preflight", side_effect=gate.GateError("CONFIG_DRIFT")):
            self.assertEqual("CONFIG_DRIFT", gate.run_gate()["classification"])

    def test_reserved_cache_namespace_refusal_and_removal(self) -> None:
        parent = Path(tempfile.gettempdir()); parent_fd = os.open(parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        wrong = Path(tempfile.mkdtemp(prefix="slug-buildbuddy-prime-")); allowed = Path(tempfile.mkdtemp(prefix="slug-buildbuddy-cache-"))
        try:
            self.assertFalse(gate._remove_reserved(wrong, parent_fd)); self.assertTrue(wrong.exists())
            self.assertTrue(gate._remove_reserved(allowed, parent_fd)); self.assertFalse(allowed.exists())
        finally:
            os.close(parent_fd)
            if wrong.exists(): wrong.rmdir()

    def test_cli_canonical_privacy_and_shared_one_label_imports(self) -> None:
        prime = gate.phase_record(bep("prime"), execution("prime"), "prime", TESTS, 0); prime["output_count"] = 1
        replay = gate.phase_record(bep("replay"), execution("replay"), "replay", TESTS, 0); replay["output_count"] = 1
        proved = gate.record("PROVED_CACHE_ONLY", prime, replay, HEAD, True)
        out, err = StringIO(), StringIO()
        with mock.patch.object(cli.gate, "run_gate", return_value=proved), redirect_stdout(out), redirect_stderr(err): self.assertEqual(0, cli.main([]))
        self.assertEqual("", err.getvalue()); self.assertEqual(json.dumps(proved, sort_keys=True, separators=(",", ":")) + "\n", out.getvalue())
        for failure in (RuntimeError("/private/token"), gate.record("CONFIG_DRIFT")):
            out, err = StringIO(), StringIO()
            patcher = mock.patch.object(cli.gate, "run_gate", side_effect=failure) if isinstance(failure, Exception) else mock.patch.object(cli.gate, "run_gate", return_value=failure)
            with patcher, redirect_stdout(out), redirect_stderr(err): self.assertEqual(1, cli.main([]))
            self.assertEqual("", err.getvalue()); self.assertNotIn("private", out.getvalue()); self.assertEqual(set(gate.record()), set(json.loads(out.getvalue())))
        exact_copy = Path("/private/exact-copy")
        with mock.patch.object(cli.gate, "run_gate") as run, redirect_stdout(StringIO()), redirect_stderr(StringIO()): self.assertEqual(1, cli.main([str(exact_copy)])); run.assert_not_called()
        from tools.v2_oracle_lib import buildbuddy_build_cache, buildbuddy_build_rbe
        self.assertIs(buildbuddy_build_cache.parsed, gate); self.assertIs(buildbuddy_build_rbe.parsed, gate)


if __name__ == "__main__": unittest.main()
