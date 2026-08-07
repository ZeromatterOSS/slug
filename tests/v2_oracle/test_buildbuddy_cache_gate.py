from __future__ import annotations

import hashlib
import json
import os
import stat
import subprocess
import unittest
from contextlib import redirect_stderr, redirect_stdout
from io import StringIO
from pathlib import Path
from unittest import mock

from tools.v2_oracle_lib import buildbuddy_cache as gate
from tools.v2_oracle import buildbuddy_cache_gate as cli


ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "tests/v2_oracle/buildbuddy_cache_targets.txt"
DIGEST = "d" * 64
OTHER_DIGEST = "e" * 64
VERSION_OUTPUT = b"Bazelisk version: v1.29.0\nBuild label: 9.2.0\nBuild target: bazel\n"
ABSENT = object()


def sequence(values: list[dict[str, object]]) -> bytes:
    return b"\n".join(json.dumps(value, indent=2).encode() for value in values)


class BuildBuddyCacheGateTest(unittest.TestCase):
    def test_manifest_is_exact_and_includes_runtime(self) -> None:
        build, tests = gate.load_manifest(MANIFEST)
        self.assertEqual("//app/slug_cli_v2:slug", build)
        self.assertEqual(43, len(tests))
        self.assertIn("//app/slug_core_v2:runtime_test", tests)
        self.assertNotIn("//app/slug_cli_v2:cli_fixture_test", tests)

    def test_manifest_drift_is_rejected(self) -> None:
        path = self._temp("bad.txt", b"x\n")
        with self.assertRaisesRegex(gate.GateError, "") as error:
            gate.load_manifest(path)
        self.assertEqual("CONFIG_DRIFT", error.exception.classification)

    def test_json_sequence_accepts_pretty_sequence_and_rejects_nonobject(self) -> None:
        self.assertEqual([{"one": 1}, {"two": 2}], list(gate.json_sequence(sequence([{"one": 1}, {"two": 2}]))))
        with self.assertRaises(gate.GateError):
            list(gate.json_sequence(b"[]"))

    def test_command_hardens_cache_only_and_shares_nonce(self) -> None:
        argv = gate.command("prime", "bazel", Path("/private/output"), Path("/private/bep"), Path("/private/log"), "secret", ("//a:b",))
        self.assertIn("--config=buildbuddy-cache", argv)
        self.assertIn("--remote_cache=grpcs://remote.buildbuddy.io", argv)
        self.assertIn("--remote_instance_name=", argv)
        self.assertIn("--remote_executor=", argv)
        self.assertIn("--bes_backend=", argv)
        self.assertIn("--bes_results_url=", argv)
        self.assertIn("--noremote_accept_cached", argv)
        self.assertNotIn("--remote_executor=grpcs://", " ".join(argv))
        self.assertEqual("//a:b", argv[-1])
        self.assertEqual("--output_base=/private/output", argv[1])
        self.assertEqual("test", argv[2])

    def test_classify_accepts_only_full_cache_replay(self) -> None:
        prime, replay = self._records(2)
        self.assertEqual("PROVED_CACHE_ONLY", gate.classify(prime, replay, ("a", "b")))
        replay["eligible_spawns"]["local"] = 1
        self.assertEqual("CACHE_MISS_OR_MIXED_REPLAY", gate.classify(prime, replay, ("a", "b")))

    def test_classify_remote_and_target_failures(self) -> None:
        prime, replay = self._records(1)
        prime["build_finished"] = {"name": "REMOTE_ERROR", "code": 1}
        self.assertEqual("REMOTE_UNAVAILABLE", gate.classify(prime, replay, ("a",)))
        prime, replay = self._records(1)
        prime["passed_test_count"] = 0
        self.assertEqual("TARGET_FAILURE", gate.classify(prime, replay, ("a",)))
        command = gate.phase_record(self._command_bep({"command": {"code": "COMMAND_NOT_FOUND"}}), b"", "replay", (), 2)
        self.assertEqual("REMOTE_UNAVAILABLE", gate.classify(self._records(0)[0] | {"build_finished": {"name": "REMOTE_ERROR", "code": 1}}, command, ()))

    def test_command_failure_diagnostics_are_allowlisted_and_precede_target_failure(self) -> None:
        semantic = {
            "command": {"OPTIONS_PARSE_FAILURE": "COMMAND_OPTIONS_PARSE", "STARLARK_OPTIONS_PARSE_FAILURE": "COMMAND_STARLARK_OPTIONS_PARSE", "ARGUMENTS_NOT_RECOGNIZED": "COMMAND_ARGUMENTS_NOT_RECOGNIZED", "INVOCATION_POLICY_PARSE_FAILURE": "COMMAND_INVOCATION_POLICY", "INVOCATION_POLICY_INVALID": "COMMAND_INVOCATION_POLICY"},
            "remoteOptions": {"REMOTE_DEFAULT_EXEC_PROPERTIES_LOGIC_ERROR": "REMOTE_OPTIONS_CONFIGURATION", "DOWNLOADER_WITHOUT_GRPC_CACHE": "REMOTE_OPTIONS_CONFIGURATION", "EXECUTION_WITH_INVALID_CACHE": "REMOTE_OPTIONS_CONFIGURATION"},
            "remoteExecution": {"CREDENTIALS_INIT_FAILURE": "REMOTE_EXECUTION_CONFIGURATION", "CACHE_INIT_FAILURE": "REMOTE_EXECUTION_CONFIGURATION", "RPC_LOG_FAILURE": "REMOTE_EXECUTION_CONFIGURATION", "EXEC_CHANNEL_INIT_FAILURE": "REMOTE_EXECUTION_CONFIGURATION", "CACHE_CHANNEL_INIT_FAILURE": "REMOTE_EXECUTION_CONFIGURATION", "DOWNLOADER_CHANNEL_INIT_FAILURE": "REMOTE_EXECUTION_CONFIGURATION", "REMOTE_DOWNLOAD_OUTPUTS_MINIMAL_WITHOUT_INMEMORY_DOTD": "REMOTE_EXECUTION_CONFIGURATION", "REMOTE_DOWNLOAD_OUTPUTS_MINIMAL_WITHOUT_INMEMORY_JDEPS": "REMOTE_EXECUTION_CONFIGURATION"},
            "executionOptions": {"INVALID_STRATEGY": "EXECUTION_OPTIONS_CONFIGURATION", "RESTRICTION_UNMATCHED_TO_ACTION_CONTEXT": "EXECUTION_OPTIONS_CONFIGURATION", "REMOTE_FALLBACK_STRATEGY_NOT_ABSTRACT_SPAWN": "EXECUTION_OPTIONS_CONFIGURATION", "STRATEGY_NOT_FOUND": "EXECUTION_OPTIONS_CONFIGURATION", "DYNAMIC_STRATEGY_NOT_SANDBOXED": "EXECUTION_OPTIONS_CONFIGURATION", "MULTIPLE_EXECUTION_LOG_FORMATS": "EXECUTION_OPTIONS_CONFIGURATION"},
            "execution": {"EXECUTION_LOG_INITIALIZATION_FAILURE": "EXECUTION_LOG_CONFIGURATION"},
            "buildConfiguration": {"PLATFORM_MAPPING_EVALUATION_FAILURE": "BUILD_CONFIGURATION", "INVALID_CONFIGURATION": "BUILD_CONFIGURATION", "INVALID_BUILD_OPTIONS": "BUILD_CONFIGURATION", "MULTI_CPU_PREREQ_UNMET": "BUILD_CONFIGURATION", "HEURISTIC_INSTRUMENTATION_FILTER_INVALID": "BUILD_CONFIGURATION", "CYCLE": "BUILD_CONFIGURATION", "CONFLICTING_CONFIGURATIONS": "BUILD_CONFIGURATION", "INVALID_OUTPUT_DIRECTORY_MNEMONIC": "BUILD_CONFIGURATION", "CONFIGURATION_DISCARDED_ANALYSIS_CACHE": "BUILD_CONFIGURATION", "INVALID_PROJECT": "BUILD_CONFIGURATION"},
        }
        semantic_pairs = {(category, code): value for category, codes in semantic.items() for code, value in codes.items()}
        self.assertEqual(semantic, gate.COMMAND_FAILURE_CLASSES)
        self.assertEqual(33, len(semantic_pairs))
        self.assertEqual(64, len(gate.B92_FAILURE_DETAIL_CATEGORY_KEYS))
        self.assertEqual(131, len(gate.B92_EXIT2_SOURCE_PAIRS))
        self.assertEqual("cbc5777ca02212ba3a5d20847c469eb221bd29b3c217162e6be39c5f5bf86d57", hashlib.sha256(gate.B92_EXIT2_CANONICAL_BYTES).hexdigest())
        self.assertTrue(gate.B92_EXIT2_CANONICAL_BYTES.startswith(b"slug-bazel-9.2-failure-detail-exit2-v1\n"))
        opaque: list[str] = []
        for ordinal, pair in enumerate(gate.B92_EXIT2_SOURCE_PAIRS, 1):
            category, code = pair
            failure_class = semantic_pairs.get(pair, f"B92_EXIT2_CLASS_{ordinal:03d}")
            self.assertEqual(failure_class, gate.B92_EXIT2_CLASSES[pair])
            record = gate.phase_record(self._command_bep({category: {"code": code}}), b"", "prime", (), 2)
            self.assertEqual(failure_class, record["command_failure_class"])
            self.assertEqual("COMMAND_LINE_FAILURE", gate.classify(record, self._records(0)[1], ()))
            self.assertNotIn(f'"{category}":', json.dumps(record))
            self.assertNotIn(f'"{code}"', json.dumps(record))
            if pair not in semantic_pairs:
                opaque.append(failure_class)
        self.assertEqual(98, len(opaque))
        self.assertEqual(98, len(set(opaque)))
        self.assertEqual("B92_EXIT2_CLASS_034", gate.B92_EXIT2_CLASSES[("command", "COMMAND_NOT_FOUND")])
        self.assertEqual("B92_EXIT2_CLASS_040", gate.B92_EXIT2_CLASSES[("command", "NOT_IN_WORKSPACE")])
        self.assertEqual("B92_EXIT2_CLASS_041", gate.B92_EXIT2_CLASSES[("command", "IN_OUTPUT_DIRECTORY")])

    def test_command_failure_structural_results_and_private_suppression(self) -> None:
        cases = (
            (ABSENT, "MISSING_FAILURE_DETAIL"), (None, "MISSING_FAILURE_DETAIL"),
            (1, "MALFORMED_FAILURE_DETAIL"), ([], "MALFORMED_FAILURE_DETAIL"), ({}, "MALFORMED_FAILURE_DETAIL"),
            ({"message": "header=x-buildbuddy-api-key=secret nonce=private/path"}, "MALFORMED_FAILURE_DETAIL"),
            ({"message": "header=x-buildbuddy-api-key=secret nonce=private/path", "command": {"code": "OPTIONS_PARSE_FAILURE"}}, "COMMAND_OPTIONS_PARSE"),
            ({"message": 1, "command": {"code": "OPTIONS_PARSE_FAILURE"}}, "MALFORMED_FAILURE_DETAIL"),
            ({"command": "bad"}, "MALFORMED_FAILURE_DETAIL"), ({"command": {}}, "MALFORMED_FAILURE_DETAIL"),
            ({"command": {"code": 1}}, "MALFORMED_FAILURE_DETAIL"), ({"command": {"code": "OPTIONS_PARSE_FAILURE", "path": "private/path"}}, "MALFORMED_FAILURE_DETAIL"),
            ({"command": {"code": "OPTIONS_PARSE_FAILURE"}, "remoteOptions": {"code": "EXECUTION_WITH_INVALID_CACHE"}}, "MALFORMED_FAILURE_DETAIL"),
            ({"x-buildbuddy-api-key=secret": {"code": "nonce=private/path"}}, "UNSUPPORTED_GENERAL_FAILURE_DETAIL"),
            ({"command": {"code": "HEADER_NONCE_PRIVATE_PATH_SECRET"}}, "UNRECOGNIZED_B9_2_EXIT2_DETAIL"),
        )
        for detail, expected in cases:
            record = gate.phase_record(self._command_bep(detail), b"", "prime", (), 2)
            self.assertEqual(expected, record["command_failure_class"])
            self.assertEqual("COMMAND_LINE_FAILURE", gate.classify(record, self._records(0)[1], ()))
            for private in ("secret", "header=", "x-buildbuddy-api-key", "nonce=", "private/path", "HEADER_NONCE"):
                self.assertNotIn(private, json.dumps(record))
        non_command = sequence([{"id": {"buildFinished": {}}, "finished": {"exitCode": {"name": "ARBITRARY_ENUM", "code": 2}, "failureDetail": {"message": "stderr credential=secret", "command": {"code": "OPTIONS_PARSE_FAILURE"}}}}])
        record = gate.phase_record(non_command, b"", "prime", (), 2)
        self.assertEqual("NONE", record["command_failure_class"])
        self.assertNotIn("ARBITRARY_ENUM", json.dumps(record))

    def test_digest_mismatch_is_cache_failure_but_zero_is_incomplete(self) -> None:
        prime, replay = self._records(1)
        replay["eligible_spawns"]["digest_multiset_sha256"] = "different"
        self.assertEqual("CACHE_MISS_OR_MIXED_REPLAY", gate.classify(prime, replay, ("a",)))
        replay["eligible_spawns"]["count"] = 0
        self.assertEqual("EVIDENCE_INCOMPLETE", gate.classify(prime, replay, ("a",)))
        prime, replay = self._records(1)
        replay["persistent_action_cache_hit_count"] = 1
        self.assertEqual("CACHE_MISS_OR_MIXED_REPLAY", gate.classify(prime, replay, ("a",)))

    def test_metrics_hits_and_unsuccessful_completion_are_strict(self) -> None:
        tests = ("//a:t",)
        bep = self._bep(tests, "prime") + sequence([{"id": {"buildMetrics": {}}, "buildMetrics": {"actionSummary": {"actionCacheStatistics": {"hits": 2}}}}])
        execution = sequence([{"runner": "local", "cacheable": True, "remoteCacheable": True, "digest": {"hash": DIGEST, "sizeBytes": 1}, "cacheHit": False, "status": "", "exitCode": 0}])
        record = gate.phase_record(bep, execution, "prime", tests, 0)
        self.assertEqual(2, record["persistent_action_cache_hit_count"])
        bad = self._bep(tests, "prime").replace(b'"success": true', b'"success": false')
        self.assertEqual(0, gate.phase_record(bad, execution, "prime", tests, 0)["build_success_count"])
        missing_test_completion = sequence([{"id": {"targetCompleted": {"label": "//app/slug_cli_v2:slug"}}, "completed": {"success": True}}, {"id": {"testSummary": {"label": "//a:t"}}, "testSummary": {"overallStatus": "PASSED", "totalRunCount": 1, "totalNumCached": 0}}, {"id": {"buildFinished": {}}, "finished": {"exitCode": {"name": "SUCCESS", "code": 0}}}])
        self.assertEqual(0, gate.phase_record(missing_test_completion, execution, "prime", tests, 0)["passed_test_count"])

    def test_each_test_must_run_and_cache_exactly_once(self) -> None:
        tests = ("//a:first", "//a:second")
        local_execution = sequence([{"runner": "local", "cacheable": True, "remoteCacheable": True, "digest": {"hash": DIGEST, "sizeBytes": 1}, "cacheHit": False, "status": "", "exitCode": 0}])
        uneven = self._bep(tests, "prime").replace(b'"totalRunCount": 1', b'"totalRunCount": 0', 1).replace(b'"totalRunCount": 1', b'"totalRunCount": 2', 1)
        record = gate.phase_record(uneven, local_execution, "prime", tests, 0)
        self.assertEqual(0, record["test_run_count"])
        self.assertEqual(0, record["passed_test_count"])
        impossible_cache = self._bep(tests, "replay").replace(b'"totalNumCached": 1', b'"totalNumCached": 0', 1).replace(b'"totalNumCached": 1', b'"totalNumCached": 2', 1)
        remote_execution = sequence([{"runner": "remote cache hit", "cacheable": True, "remoteCacheable": True, "digest": {"hash": DIGEST, "sizeBytes": 1}, "cacheHit": True, "status": "", "exitCode": 0}])
        with self.assertRaises(gate.GateError):
            gate.phase_record(impossible_cache, remote_execution, "replay", tests, 0)

    def test_phase_record_rejects_unknown_prime_runner(self) -> None:
        tests = ("//a:t",)
        bep = self._bep(tests, "prime")
        execution = sequence([{"runner": "mystery", "cacheable": True, "remoteCacheable": True, "digest": {"hash": DIGEST, "sizeBytes": 1}, "cacheHit": False, "status": "", "exitCode": 0}])
        record = gate.phase_record(bep, execution, "prime", tests, 0)
        self.assertEqual(1, record["eligible_spawns"]["unknown"])
        replay = self._records(1)[1]
        replay["eligible_spawns"]["digest_multiset_sha256"] = record["eligible_spawns"]["digest_multiset_sha256"]
        self.assertEqual("CACHE_MISS_OR_MIXED_REPLAY", gate.classify(record, replay, tests))

    def test_spawn_summary_ignores_ineligible_local_and_rejects_nonboolean_hit(self) -> None:
        entries = [{"runner": "local", "cacheable": False, "remoteCacheable": False}, {"runner": "remote cache hit", "cacheable": True, "remoteCacheable": True, "digest": {"hash": DIGEST, "sizeBytes": 1}, "cacheHit": True, "status": "", "exitCode": 0}]
        summary = gate.spawn_summary(entries, "replay")
        self.assertEqual(0, summary["local"])
        self.assertEqual(1, summary["remote_cache_hit"])
        entries[1]["cacheHit"] = "true"
        self.assertEqual(1, gate.spawn_summary(entries, "replay")["cache_hit_failures"])

    def test_spawn_and_bep_scalar_types_fail_closed(self) -> None:
        malformed = [{"runner": "local", "cacheable": "true", "remoteCacheable": True}]
        with self.assertRaises(gate.GateError):
            gate.spawn_summary(malformed, "prime")
        tests = ("//a:t",)
        execution = sequence([{"runner": "local", "cacheable": True, "remoteCacheable": True, "digest": {"hash": DIGEST, "sizeBytes": 1}, "cacheHit": False, "status": "", "exitCode": False}])
        with self.assertRaises(gate.GateError):
            gate.phase_record(self._bep(tests, "prime"), execution, "prime", tests, 0)
        malformed_bep = self._bep(tests, "prime").replace(b'"totalRunCount": 1', b'"totalRunCount": true')
        with self.assertRaises(gate.GateError):
            gate.phase_record(malformed_bep, sequence([]), "prime", tests, 0)

    def test_phase_record_rejects_missing_terminal_data(self) -> None:
        with self.assertRaises(gate.GateError):
            gate.phase_record(b"", b"", "prime", ("//a:t",), 1)

    def test_digest_validation_is_strict_and_canonical(self) -> None:
        self.assertEqual(json.dumps({"hash": DIGEST, "sizeBytes": 2}, sort_keys=True, separators=(",", ":")), gate._digest({"hash": DIGEST, "sizeBytes": "2", "ignored": "x"}))
        for digest in ({"hash": "A" * 64, "sizeBytes": 1}, {"hash": DIGEST, "sizeBytes": "02"}, {"hash": DIGEST, "sizeBytes": -1}):
            with self.assertRaises(gate.GateError):
                gate._digest(digest)

    def test_run_gate_is_mocked_and_cleans_private_paths(self) -> None:
        calls: list[list[str]] = []
        roots: list[Path] = []

        def runner(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]:
            calls.append(argv)
            self.assertEqual(ROOT, _.get("cwd"))
            if argv[-1] == "version":
                return subprocess.CompletedProcess(argv, 0, VERSION_OUTPUT)
            if len(argv) > 2 and argv[2] == "test":
                phase = "prime" if "--noremote_accept_cached" in argv else "replay"
                bep = Path(next(value.split("=", 1)[1] for value in argv if value.startswith("--build_event_json_file=")))
                log = Path(next(value.split("=", 1)[1] for value in argv if value.startswith("--execution_log_json_file=")))
                roots.append(Path(argv[1].split("=", 1)[1]).parents[1])
                labels = tuple(value for value in argv if value.startswith("//"))
                tests = labels[1:]
                self.assertEqual(0o600, stat.S_IMODE(os.fstat(_.get("stdout").fileno()).st_mode))
                self.assertEqual(0o600, stat.S_IMODE(os.fstat(_.get("stderr").fileno()).st_mode))
                _.get("stderr").write(b"header=x-buildbuddy-api-key=secret nonce=private/path")
                bep.write_bytes(self._bep(tests, phase))
                log.write_bytes(sequence([{"runner": "local" if phase == "prime" else "remote cache hit", "cacheable": True, "remoteCacheable": True, "digest": {"hash": DIGEST, "sizeBytes": 4}, "cacheHit": phase == "replay", "status": "", "exitCode": 0}]))
            return subprocess.CompletedProcess(argv, 0, b"raw token", b"raw token")

        original_read_bytes = Path.read_bytes
        def checked_read_bytes(path: Path) -> bytes:
            self.assertNotEqual("stderr", path.name)
            return original_read_bytes(path)

        with mock.patch.object(Path, "read_bytes", checked_read_bytes), mock.patch.object(gate, "_git", side_effect=("a" * 40, "")):
            result = gate.run_gate(MANIFEST, runner=runner)
        self.assertEqual("PROVED_CACHE_ONLY", result["classification"])
        self.assertNotIn("raw token", json.dumps(result))
        self.assertNotIn("x-buildbuddy-api-key", json.dumps(result))
        self.assertEqual(["bazel", "--ignore_all_rc_files", "version"], calls[0])
        tests = [argv for argv in calls if len(argv) > 2 and argv[2] == "test"]
        self.assertEqual(2, len(tests))
        nonces = [{item for item in argv if "CACHE_GATE_NONCE=" in item} for argv in tests]
        self.assertEqual(nonces[0], nonces[1])
        self.assertEqual(2, len([argv for argv in calls if argv[-1] == "shutdown"]))
        self.assertEqual({"schema_version", "classification", "mode", "bazel_version", "host_platform", "git_head", "git_clean", "manifest_sha256", "bazelrc_sha256", "target_counts", "prime", "replay"}, set(result))
        self.assertEqual("buildbuddy-cache-only", result["mode"])
        self.assertEqual("linux-x86_64", result["host_platform"])
        self.assertNotEqual(next(item for item in tests[0] if item.startswith("--output_base=")), next(item for item in tests[1] if item.startswith("--output_base=")))
        self.assertIn("--noremote_accept_cached", tests[0])
        self.assertIn("--remote_accept_cached", tests[1])
        self.assertTrue(roots and all(not root.exists() for root in roots))

    def test_remote_abort_payload_is_remote_unavailable(self) -> None:
        tests = ("//a:t",)
        bep = self._bep(tests, "prime") + sequence([{"id": {"targetCompleted": {"label": "//a:other"}}, "aborted": {"reason": "REMOTE_ENVIRONMENT_FAILURE", "description": "raw secret"}}])
        execution = sequence([{"runner": "local", "cacheable": True, "remoteCacheable": True, "digest": {"hash": DIGEST, "sizeBytes": 1}, "cacheHit": False, "status": "", "exitCode": 0}])
        prime = gate.phase_record(bep, execution, "prime", tests, 0)
        replay = self._records(1)[1]
        self.assertEqual("REMOTE_UNAVAILABLE", gate.classify(prime, replay, tests))
        self.assertNotIn("raw secret", json.dumps(prime))

    def test_preflight_rejects_wrong_version_before_gate(self) -> None:
        def runner(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]:
            return subprocess.CompletedProcess(argv, 0, b"Build label: 9.1.0\n")

        with self.assertRaises(gate.GateError) as error:
            gate.run_gate(MANIFEST, runner=runner)
        self.assertEqual("CONFIG_DRIFT", error.exception.classification)

        for output in (b"", VERSION_OUTPUT + b"Build label: 9.2.0\n"):
            with self.assertRaises(gate.GateError):
                gate._preflight("bazel", lambda argv, **_: subprocess.CompletedProcess(argv, 0, output))

    def test_preflight_rejects_platform_and_git_drift(self) -> None:
        runner = lambda argv, **_: subprocess.CompletedProcess(argv, 0, VERSION_OUTPUT)
        with mock.patch.object(gate.platform, "system", return_value="Darwin"), self.assertRaises(gate.GateError):
            gate._preflight("bazel", runner)
        with mock.patch.object(gate, "_git", side_effect=("not-a-sha", "")), self.assertRaises(gate.GateError):
            gate._preflight("bazel", runner)

    def test_all_prime_runners_and_near_miss(self) -> None:
        for name in ("local", "worker", "linux-sandbox"):
            self.assertEqual(1, gate.spawn_summary([{"runner": name, "cacheable": True, "remoteCacheable": True, "digest": {"hash": DIGEST, "sizeBytes": 1}, "cacheHit": False, "status": "", "exitCode": 0}], "prime")["local"])
        self.assertEqual(1, gate.spawn_summary([{"runner": "local-ish", "cacheable": True, "remoteCacheable": True, "digest": {"hash": OTHER_DIGEST, "sizeBytes": 1}, "cacheHit": False, "status": "", "exitCode": 0}], "prime")["unknown"])

    def test_cli_hides_exception_and_keeps_stderr_empty(self) -> None:
        stdout, stderr = StringIO(), StringIO()
        with mock.patch.object(cli, "run_gate", side_effect=RuntimeError("token=not-allowed")), redirect_stdout(stdout), redirect_stderr(stderr):
            self.assertEqual(1, cli.main([]))
        self.assertEqual("", stderr.getvalue())
        self.assertEqual({"classification": "SANITIZER_REJECTED", "schema_version": 1}, json.loads(stdout.getvalue()))

    def test_cleanup_failure_is_fail_closed(self) -> None:
        roots: list[Path] = []
        def runner(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]:
            if argv[-1] == "shutdown":
                raise OSError("no raw detail")
            if argv[-1] == "version":
                return subprocess.CompletedProcess(argv, 0, VERSION_OUTPUT)
            if len(argv) > 2 and argv[2] == "test":
                roots.append(Path(argv[1].split("=", 1)[1]).parents[1])
            return subprocess.CompletedProcess(argv, 1)

        with mock.patch.object(gate, "_git", side_effect=("a" * 40, "")), self.assertRaises(gate.GateError) as error:
            gate.run_gate(MANIFEST, runner=runner)
        self.assertEqual("SANITIZER_REJECTED", error.exception.classification)
        self.assertTrue(roots and not roots[0].exists())

    def test_nonzero_shutdown_is_fail_closed_and_still_removes_root(self) -> None:
        roots: list[Path] = []
        def runner(argv: list[str], **_: object) -> subprocess.CompletedProcess[bytes]:
            if argv[-1] == "version":
                return subprocess.CompletedProcess(argv, 0, VERSION_OUTPUT)
            if len(argv) > 2 and argv[2] == "test":
                roots.append(Path(argv[1].split("=", 1)[1]).parents[1])
                return subprocess.CompletedProcess(argv, 1)
            return subprocess.CompletedProcess(argv, 9)

        with mock.patch.object(gate, "_git", side_effect=("a" * 40, "")), self.assertRaises(gate.GateError) as error:
            gate.run_gate(MANIFEST, runner=runner)
        self.assertEqual("SANITIZER_REJECTED", error.exception.classification)
        self.assertTrue(roots and not roots[0].exists())

    def _records(self, count: int) -> tuple[dict[str, object], dict[str, object]]:
        def record(remote: bool) -> dict[str, object]:
            return {"process_exit_code": 0, "build_finished": {"name": "SUCCESS", "code": 0}, "command_failure_class": "NONE", "build_success_count": 1, "passed_test_count": count, "test_run_count": count, "remotely_cached_test_count": count if remote else 0, "persistent_action_cache_hit_count": 0, "eligible_spawns": {"count": 1, "digest_multiset_sha256": "same", "cache_hit_failures": 0, "status_failures": 0, "exit_failures": 0, "local": 0 if remote else 1, "remote_cache_hit": 1 if remote else 0, "disk_cache_hit": 0, "remote_execution": 0, "unknown": 0}}
        return record(False), record(True)

    @staticmethod
    def _bep(tests: tuple[str, ...], phase: str) -> bytes:
        events: list[dict[str, object]] = [{"id": {"targetCompleted": {"label": "//app/slug_cli_v2:slug"}}, "completed": {"success": True}}]
        events.extend({"id": {"targetCompleted": {"label": test}}, "completed": {"success": True}} for test in tests)
        events.extend({"id": {"testSummary": {"label": test}}, "testSummary": {"overallStatus": "PASSED", "totalRunCount": 1, "totalNumCached": 1 if phase == "replay" else 0}} for test in tests)
        events.append({"id": {"buildFinished": {}}, "finished": {"exitCode": {"name": "SUCCESS", "code": 0}}})
        return sequence(events)

    @staticmethod
    def _command_bep(detail: object = ABSENT) -> bytes:
        finished: dict[str, object] = {"exitCode": {"name": "COMMAND_LINE_ERROR", "code": 2}}
        if detail is not ABSENT:
            finished["failureDetail"] = detail
        return sequence([{"id": {"buildFinished": {}}, "finished": finished}])

    def _temp(self, name: str, data: bytes) -> Path:
        path = ROOT / "target" / name
        path.parent.mkdir(exist_ok=True)
        path.write_bytes(data)
        self.addCleanup(path.unlink)
        return path


if __name__ == "__main__":
    unittest.main()
