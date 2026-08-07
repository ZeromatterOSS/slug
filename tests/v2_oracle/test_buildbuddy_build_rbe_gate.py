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

from tools.v2_oracle import buildbuddy_build_rbe_gate as cli
from tools.v2_oracle_lib import buildbuddy_build_rbe as rbe

DIGEST = "a" * 64
BEP = (b'{"id":{"targetCompleted":{"label":"//app/slug_cli_v2:slug"}},"completed":{"success":true}}\n'
       b'{"id":{"buildFinished":{}},"finished":{"exitCode":{"name":"SUCCESS","code":0}}}\n')


def execution(runner: str = "remote", **changes: object) -> bytes:
    event: dict[str, object] = {"remotable": True, "runner": runner, "action_digest": {"hash": DIGEST, "sizeBytes": 1}, "cache_hit": False, "status": "", "exit_code": 0}
    event.update(changes)
    return (json.dumps({"SpawnExec": event}, separators=(",", ":")) + "\n").encode()


class BuildBuddyBuildRbeGateTest(unittest.TestCase):
    def _run(self, change: str = "ready", code: int = 0, clean: bool = True) -> tuple[dict[str, object], list[list[str]], list[Path]]:
        calls, roots = [], []
        def runner(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            calls.append(argv)
            if argv[-1] == "shutdown":
                output = Path(argv[2].split("=", 1)[1])
                if change == "shutdown_output_swap": output.rename(output.with_name("saved-output")); output.mkdir()
                if change == "shutdown_exception": raise OSError("/private")
                return subprocess.CompletedProcess(argv, 7 if change == "shutdown_nonzero" else 0)
            output = Path(argv[1].split("=", 1)[1]); phase, root = output.parent, output.parents[1]; roots.append(root)
            self.assertEqual(0o700, stat.S_IMODE(root.stat().st_mode))
            bep = Path(next(value.split("=", 1)[1] for value in argv if value.startswith("--build_event_json_file=")))
            log = Path(next(value.split("=", 1)[1] for value in argv if value.startswith("--execution_log_json_file=")))
            for path in (bep, log, phase / "stdout", phase / "stderr"):
                self.assertEqual(0o600, stat.S_IMODE(path.stat().st_mode))
            bep.write_bytes(BEP); log.write_bytes(execution()); self._execution = log
            if change == "bep_remote": bep.write_bytes(BEP.replace(b'"SUCCESS","code":0', b'"REMOTE_ERROR","code":34'))
            elif change == "bep_command": bep.write_bytes(BEP.replace(b'"SUCCESS","code":0', b'"COMMAND_LINE_ERROR","code":2'))
            elif change == "bep_target": bep.write_bytes(BEP.replace(b'"SUCCESS","code":0', b'"BUILD_FAILURE","code":1'))
            if change != "output_missing":
                target = output / "execroot/x/bin/app/slug_cli_v2/slug"; target.parent.mkdir(parents=True); target.write_bytes(b"x"); target.chmod(0o600 if change == "output_nonexec" else 0o700)
                if change == "output_multiple":
                    second = output / "execroot/y/bin/app/slug_cli_v2/slug"; second.parent.mkdir(parents=True); second.write_bytes(b"x"); second.chmod(0o700)
            if change == "bep_replace": bep.rename(bep.with_name("old-bep")); bep.write_bytes(BEP)
            elif change == "execution_replace": log.rename(log.with_name("old-execution")); log.write_bytes(execution())
            elif change == "bep_symlink": bep.unlink(); bep.symlink_to("/dev/null")
            elif change == "execution_mode": log.chmod(0o640)
            elif change == "execution_missing": log.unlink()
            elif change == "execution_empty": log.write_bytes(b"")
            elif change == "execution_bad": log.write_bytes(b"{")
            elif change == "root_swap": root.rename(root.with_name("saved-root")); root.mkdir()
            elif change == "phase_swap": phase.rename(phase.with_name("saved-rbe")); phase.mkdir()
            elif change == "output_swap": output.rename(output.with_name("saved-output")); output.mkdir()
            return subprocess.CompletedProcess(argv, code)
        with mock.patch.object(rbe, "_preflight", return_value=None), mock.patch.object(rbe, "_clean", return_value=clean):
            result = rbe.run_gate(runner=runner)
        return result, calls, roots

    def test_exact_command_and_private_one_phase_run(self) -> None:
        argv = rbe.command("bazel", Path("/p/rbe/output"), Path("/p/rbe/bep.json"), Path("/p/rbe/execution.json"), "f" * 64)
        self.assertEqual(["bazel", "--output_base=/p/rbe/output", "build", "--config=buildbuddy-rbe", "--@rules_rust//rust/toolchain/channel=nightly", "--noremote_accept_cached", "--noremote_upload_local_results", "--remote_download_outputs=toplevel", "--remote_timeout=900", "--jobs=4", "--remote_instance_name=", "--bes_backend=", "--bes_results_url=", "--disk_cache=", "--build_event_publish_all_actions", "--action_env=SLUG_BUILDBUDDY_BUILD_RBE_NONCE=" + "f" * 64, "--build_event_json_file=/p/rbe/bep.json", "--execution_log_json_file=/p/rbe/execution.json", "//app/slug_cli_v2:slug"], argv)
        for forbidden in ("remote_executor", "remote_cache=", "remote_default_exec", "remote_local_fallback"):
            self.assertFalse(any(forbidden in value for value in argv))
        with self.assertRaises(rbe.GateError): rbe.command("bazel", Path("/p"), Path("/b"), Path("/e"), "secret")
        result, calls, roots = self._run()
        self.assertEqual("PROVED_BUILD_RBE", result["classification"]); self.assertEqual(2, len(calls)); self.assertFalse(roots[0].exists())
        self.assertEqual(argv[2:15], calls[0][2:15]); self.assertEqual(["bazel", "--ignore_all_rc_files", f"--output_base={roots[0] / 'rbe/output'}", "shutdown"], calls[1])

    def test_preflight_exact_files_platform_and_cleanliness(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); (root / ".bazelversion").write_bytes(b"9.2.0\n"); (root / ".bazelrc").write_bytes(b"rc")
            digest = __import__("hashlib").sha256(b"rc").hexdigest()
            with mock.patch.object(rbe, "REPO_ROOT", root), mock.patch.object(rbe, "BAZELRC_SHA256", digest), mock.patch.object(rbe, "_clean", return_value=True), mock.patch.object(rbe.platform, "system", return_value="Linux"), mock.patch.object(rbe.platform, "machine", return_value="x86_64"):
                rbe._preflight()
                for path, data in ((root / ".bazelversion", b"9.3.0\n"), (root / ".bazelrc", b"bad")):
                    original = path.read_bytes(); path.write_bytes(data)
                    with self.assertRaises(rbe.GateError): rbe._preflight()
                    path.write_bytes(original)
                with mock.patch.object(rbe, "_clean", return_value=False), self.assertRaises(rbe.GateError): rbe._preflight()
                with mock.patch.object(rbe.platform, "machine", return_value="aarch64"), self.assertRaises(rbe.GateError): rbe._preflight()
                for name in (".bazelversion", ".bazelrc"):
                    path, saved = root / name, root / (name + ".saved"); path.rename(saved); path.symlink_to(saved)
                    with self.subTest(symlink=name), self.assertRaises(rbe.GateError): rbe._preflight()
                    path.unlink(); saved.rename(path)
                original_read = rbe.os.read
                for name in (".bazelversion", ".bazelrc"):
                    path, saved, changed = root / name, root / (name + ".old"), []
                    def replace(fd: int, size: int) -> bytes:
                        data = original_read(fd, size)
                        if not changed and os.fstat(fd).st_ino == path.lstat().st_ino:
                            changed.append(True); payload = path.read_bytes(); path.rename(saved); path.write_bytes(payload)
                        return data
                    with self.subTest(replacement=name), mock.patch.object(rbe.os, "read", side_effect=replace), self.assertRaises(rbe.GateError): rbe._preflight()
                    path.unlink(); saved.rename(path)

    def test_all_spawnexec_runners_and_strict_fields(self) -> None:
        for runner, bucket in (("remote", "remote_execution"), ("remote cache hit", "remote_cache_hit"), ("local", "local"), ("worker", "worker"), ("linux-sandbox", "linux_sandbox"), ("mystery", "other")):
            with self.subTest(runner=runner):
                summary = rbe.spawn_summary(rbe.parsed.json_sequence(execution(runner)))
                self.assertEqual(1, summary["count"]); self.assertEqual(1, summary[bucket])
        missing = object()
        for field, accepted, rejected, key in (("remotable", True, (missing, None, "true", False, 1), "remotable_error_count"), ("cache_hit", False, (missing, None, "false", True, 0), "cache_hit_error_count"), ("status", "", (missing, None, "error", False, 0), "status_error_count"), ("exit_code", 0, (missing, None, "0", False, 1), "exit_error_count")):
            for value, expected in ((accepted, 0), *((item, 1) for item in rejected)):
                event = json.loads(execution())["SpawnExec"]
                if value is missing: event.pop(field)
                else: event[field] = value
                with self.subTest(field=field, value=value): self.assertEqual(expected, rbe.spawn_summary([{"SpawnExec": event}])[key])
        invalid = rbe.spawn_summary(rbe.parsed.json_sequence(execution(action_digest={"hash": "bad", "sizeBytes": 1})))
        self.assertEqual(0, invalid["valid_digest_count"])
        combined = execution() + execution("worker")
        self.assertEqual(2, rbe.spawn_summary(rbe.parsed.json_sequence(combined))["count"])

    def test_bep_output_and_all_fixed_classifications(self) -> None:
        self.assertEqual({"PROVED_BUILD_RBE", "CONFIG_DRIFT", "REMOTE_UNAVAILABLE", "COMMAND_LINE_FAILURE", "TARGET_FAILURE", "CACHE_HIT_OR_MIXED_EXECUTION", "EVIDENCE_INCOMPLETE", "SANITIZER_REJECTED"}, rbe.CLASSES)
        phase, outcome = rbe.phase_record(BEP, execution(), 0); phase["output_count"] = 1
        self.assertEqual("PROVED_BUILD_RBE", rbe.classify(phase, outcome))
        for outcome, expected in (("remote", "REMOTE_UNAVAILABLE"), ("command", "COMMAND_LINE_FAILURE"), ("target", "TARGET_FAILURE")):
            with self.subTest(expected=expected): self.assertEqual(expected, rbe.classify(phase, outcome))
        for key in ("process_success_count", "build_finished_success_count", "target_success_count", "output_count"):
            current = {**phase, key: 0}; self.assertEqual("TARGET_FAILURE", rbe.classify(current, "success"))
        current = {**phase, "spawns": {**phase["spawns"], "valid_digest_count": 0}}
        self.assertEqual("EVIDENCE_INCOMPLETE", rbe.classify(current, "success"))
        for key in ("persistent_action_cache_hit_count",):
            self.assertEqual("CACHE_HIT_OR_MIXED_EXECUTION", rbe.classify({**phase, key: 1}, "success"))
        for key in ("remotable_error_count", "cache_hit_error_count", "status_error_count", "exit_error_count", "remote_cache_hit", "local", "worker", "linux_sandbox", "other"):
            current = {**phase, "spawns": {**phase["spawns"], key: 1}}
            self.assertEqual("CACHE_HIT_OR_MIXED_EXECUTION", rbe.classify(current, "success"))
        remote = BEP.replace(b'"SUCCESS","code":0', b'"REMOTE_ERROR","code":34')
        self.assertEqual("remote", rbe.phase_record(remote, execution(), 0)[1])
        for change, expected in (("bep_remote", "REMOTE_UNAVAILABLE"), ("bep_command", "COMMAND_LINE_FAILURE"), ("bep_target", "TARGET_FAILURE")):
            with self.subTest(change=change): self.assertEqual(expected, self._run(change)[0]["classification"])

    def test_private_replacement_attacks_anchors_shutdown_and_cleanup(self) -> None:
        self.assertEqual("PROVED_BUILD_RBE", self._run("execution_replace")[0]["classification"])
        for change in ("bep_replace", "bep_symlink", "execution_mode", "execution_missing", "execution_empty", "execution_bad"):
            with self.subTest(change=change): self.assertEqual("EVIDENCE_INCOMPLETE", self._run(change)[0]["classification"])
        for change in ("output_missing", "output_nonexec", "output_multiple"):
            with self.subTest(change=change): self.assertEqual("TARGET_FAILURE", self._run(change)[0]["classification"])
        for change in ("root_swap", "phase_swap", "output_swap", "shutdown_output_swap", "shutdown_nonzero", "shutdown_exception"):
            with self.subTest(change=change):
                result, _, roots = self._run(change); self.assertEqual(rbe.record(), result); self.assertTrue(all(not root.exists() for root in roots))
        actual = rbe.cache._remove_original
        def failed(*args: object) -> bool: actual(*args); return False
        with mock.patch.object(rbe.cache, "_remove_original", side_effect=failed): self.assertEqual(rbe.record(), self._run()[0])
        self.assertEqual(rbe.record(), self._run(clean=False)[0])
        original, swapped = rbe.cache.os.read, []
        def replace_after_open(fd: int, size: int) -> bytes:
            value = original(fd, size)
            if not swapped and hasattr(self, "_execution") and os.fstat(fd).st_ino == self._execution.lstat().st_ino:
                swapped.append(True); self._execution.rename(self._execution.with_name("midread-old")); self._execution.write_bytes(execution())
            return value
        with mock.patch.object(rbe.cache.os, "read", side_effect=replace_after_open): self.assertEqual("EVIDENCE_INCOMPLETE", self._run()[0]["classification"])
        with mock.patch.object(rbe, "_preflight", side_effect=rbe.GateError("CONFIG_DRIFT")), mock.patch.object(rbe, "_clean", return_value=True):
            self.assertEqual("CONFIG_DRIFT", rbe.run_gate()["classification"])
        with mock.patch.object(rbe, "_clean", return_value=False): self.assertEqual("CONFIG_DRIFT", rbe.run_gate()["classification"])
        with mock.patch.object(rbe, "_preflight", return_value=None), mock.patch.object(rbe, "_clean", return_value=True), mock.patch.object(rbe.tempfile, "mkdtemp", side_effect=OSError): self.assertEqual("EVIDENCE_INCOMPLETE", rbe.run_gate()["classification"])
        with mock.patch.object(rbe, "_preflight", return_value=None), mock.patch.object(rbe, "_clean", return_value=True), mock.patch.object(rbe.cleanup, "_private_file", side_effect=OSError): self.assertEqual("EVIDENCE_INCOMPLETE", rbe.run_gate()["classification"])

    def test_closed_schema_cli_privacy_and_no_raw_leakage(self) -> None:
        phase, _ = rbe.phase_record(BEP, execution(), 0); phase["output_count"] = 1
        proved = rbe.record("PROVED_BUILD_RBE", phase)
        self.assertEqual(proved, rbe.normalize(proved)); self.assertEqual(set(rbe.record()), set(proved))
        class DictSubclass(dict): pass
        class StringSubclass(str): pass
        hostile = {**proved, "secret": "/private/token"}
        self.assertEqual(rbe.record(), rbe.normalize(hostile)); self.assertEqual(rbe.record(), rbe.normalize(DictSubclass(proved)))
        self.assertEqual(rbe.record(), rbe.normalize({**proved, "classification": StringSubclass("PROVED_BUILD_RBE")})); self.assertEqual(rbe.record(), rbe.normalize({**proved, "schema_version": True}))
        for mutation in (lambda value: value.update(remote_platform="other"), lambda value: value["rbe"].pop("output_count"), lambda value: value["rbe"].update(secret=1), lambda value: value["rbe"].update(output_count=-1), lambda value: value["rbe"].update(output_count=True), lambda value: value["rbe"]["spawns"].pop("local"), lambda value: value["rbe"]["spawns"].update(secret=1), lambda value: value["rbe"]["spawns"].update(count=2), lambda value: value["rbe"]["spawns"].update(valid_digest_count=2), lambda value: value["rbe"]["spawns"].update(status_error_count=2)):
            current = json.loads(json.dumps(proved)); mutation(current); self.assertEqual(rbe.record(), rbe.normalize(current))
        current = json.loads(json.dumps(proved)); current["rbe"] = DictSubclass(current["rbe"]); self.assertEqual(rbe.record(), rbe.normalize(current))
        self.assertNotIn("private", json.dumps(rbe.record()))
        out, err = StringIO(), StringIO()
        with mock.patch.object(cli, "run_gate", return_value=proved), redirect_stdout(out), redirect_stderr(err): self.assertEqual(0, cli.main([]))
        self.assertEqual("", err.getvalue()); self.assertEqual(json.dumps(proved, sort_keys=True, separators=(",", ":")) + "\n", out.getvalue())
        out, err = StringIO(), StringIO()
        with mock.patch.object(cli, "run_gate", side_effect=RuntimeError("/private/token")), redirect_stdout(out), redirect_stderr(err): self.assertEqual(1, cli.main([]))
        self.assertEqual("", err.getvalue()); self.assertNotIn("private", out.getvalue()); self.assertEqual(rbe.record(), json.loads(out.getvalue()))
        with mock.patch.object(cli, "run_gate", return_value=rbe.record("CONFIG_DRIFT")), redirect_stdout(StringIO()), redirect_stderr(StringIO()): self.assertEqual(1, cli.main([]))
        with redirect_stdout(StringIO()), redirect_stderr(StringIO()): self.assertEqual(1, cli.main(["unexpected"]))


if __name__ == "__main__": unittest.main()
