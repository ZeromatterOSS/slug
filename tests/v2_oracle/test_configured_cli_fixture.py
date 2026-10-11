from __future__ import annotations

import hashlib
import io
import json
import os
import shutil
import subprocess
import tarfile
from dataclasses import replace
from pathlib import Path

import pytest
from tools.v2_oracle import configured_cli_fixture as fixture_tool

from tools.v2_oracle.configured_cli_fixture import (
    DEFAULT_FIXTURE_ROOT,
    DRIVER,
    FixtureError,
    _driver_limits,
    _parser,
    _proof_limits,
    assemble,
    verify_fixture,
)


def _copy_fixture(tmp_path: Path) -> Path:
    fixture = tmp_path / "fixture"
    shutil.copytree(DEFAULT_FIXTURE_ROOT, fixture)
    return fixture


@pytest.mark.parametrize("deadline", [1, 12, 30])
def test_proof_deadline_boundaries_and_forwarding(deadline: int) -> None:
    args = _parser().parse_args(
        ["prove", "--harness", "/tmp/harness", "--deadline-seconds", str(deadline)]
    )
    assert args.deadline_seconds == deadline
    assert _proof_limits(deadline) == {
        "deadline_seconds": deadline,
        "cpu_seconds": deadline + 3,
        "shell_timeout_seconds": deadline + 3,
        "python_timeout_seconds": deadline + 5,
    }
    stdout = f"Diagnostic evidence: /tmp/logs\n{json.dumps({'limits': _proof_limits(deadline)})}\n"
    assert _driver_limits(stdout) == _proof_limits(deadline)
    assert 'str(harness), str(deadline)]' in (DRIVER.parent / "configured_cli_fixture.py").read_text()


def test_proof_deadline_default_and_rejections() -> None:
    assert _parser().parse_args(["prove", "--harness", "/tmp/harness"]).deadline_seconds == 12
    for invalid in ("0", "31", "not-an-integer"):
        with pytest.raises(SystemExit, match="2"):
            _parser().parse_args(
                ["prove", "--harness", "/tmp/harness", "--deadline-seconds", invalid]
            )


@pytest.mark.parametrize("deadline", [1, 12, 30])
def test_portable_driver_receives_checked_deadline_limits(deadline: int) -> None:
    result = subprocess.run(
        ["/bin/bash", str(DRIVER), "--portable-limits", str(deadline)],
        check=True,
        cwd=DRIVER.parents[2],
        capture_output=True,
        text=True,
    )
    assert json.loads(result.stdout.splitlines()[-1])["limits"] == _proof_limits(deadline)
    source = DRIVER.read_text()
    assert 'probe_guard=(timeout --kill-after=2 "$probe_shell_timeout")' in source
    assert '"--cpu=$process_cpu_limit"' in source
    assert "'portable-proof', $portable_deadline" in source


def test_authentic_fixture_verifies_and_assembles_offline(tmp_path: Path) -> None:
    verified = verify_fixture()
    assert len(verified.objects) == 35
    assert len(verified.registry_entries) == 177
    assert verified.total_bytes == 32_440_662
    assert (
        verified.inventory_sha256
        == "6895d4ffee1075659d9db5709bf4a1418b0cc772ac7d95cbeaf472c14c669538"
    )

    destination = tmp_path / "assembled"
    receipt = assemble(destination)
    assert receipt["object_count"] == 35
    assert receipt["registry_metadata_count"] == 177
    assert receipt["source_bytes"] == 32_440_662
    assert receipt["inventory_sha256"] == verified.inventory_sha256
    assert "local_path_override" not in (destination / "workspace/MODULE.bazel").read_text()
    assert (destination / "workspace/BUILD.bazel").read_text().endswith(
        'root_rule(name = "root", deps = [":writer_a", ":writer_b"])\n'
    )
    registry = json.loads((destination / "registry/bazel_registry.json").read_text())
    assert registry == {"mirrors": [(destination / "mirror").as_uri()]}
    metadata = list((destination / "registry/modules").glob("*/*/MODULE.bazel"))
    assert len(metadata) == 156
    assert (
        destination
        / "mirror/github.com/bazelbuild/platforms/releases/download/1.0.0/platforms-1.0.0.tar.gz"
    ).is_file()
    assert (
        destination
        / "mirror/github.com/protocolbuffers/protobuf/releases/download/v33.4/protobuf-33.4.bazel.tar.gz"
    ).is_file()
    assert (
        destination
        / "mirror/github.com/bazel-contrib/bazel_features/releases/download/v1.42.1/bazel_features-v1.42.1.tar.gz"
    ).is_file()
    assert (
        destination
        / "mirror/github.com/bazelbuild/bazel-skylib/releases/download/1.8.2/bazel-skylib-1.8.2.tar.gz"
    ).is_file()
    assert (
        destination
        / "mirror/github.com/bazelbuild/rules_java/releases/download/9.1.0/rules_java-9.1.0.tar.gz"
    ).is_file()
    assert (
        destination
        / "mirror/github.com/bazel-contrib/rules_python/releases/download/1.7.0/rules_python-1.7.0.tar.gz"
    ).is_file()
    assert (
        destination
        / "registry/modules/rules_python/1.7.0/patches/module_dot_bazel_version.patch"
    ).is_file()
    assert json.loads((destination / "inventory.json").read_text()) == receipt
    for item in verified.objects:
        if item.kind == "archive":
            assert item.source not in verified.bytes_by_source
            assert verified.archives_by_source[item.source].item == item
            with (destination / item.destination).open("rb") as stream:
                assert hashlib.file_digest(stream, "sha256").hexdigest() == item.sha256
            assert (destination / item.destination).stat().st_size == item.size
        else:
            assert item.source in verified.bytes_by_source
    with pytest.raises(FixtureError, match="already exists"):
        assemble(destination)


def test_authentic_fixture_rejects_missing_and_corrupt_objects(tmp_path: Path) -> None:
    missing = _copy_fixture(tmp_path / "missing")
    (
        missing
        / "mirror/github.com/bazelbuild/platforms/releases/download/1.0.0/platforms-1.0.0.tar.gz"
    ).unlink()
    with pytest.raises(FixtureError, match="missing fixture object"):
        verify_fixture(missing)

    platform = "mirror/github.com/bazelbuild/platforms/releases/download/1.0.0/platforms-1.0.0.tar.gz"
    swift = "mirror/github.com/bazelbuild/rules_swift/releases/download/3.1.2/rules_swift.3.1.2.tar.gz"
    shutil.copy2(DEFAULT_FIXTURE_ROOT / platform, missing / platform)
    (missing / swift).unlink()
    with pytest.raises(FixtureError, match="missing fixture object.*rules_swift"):
        verify_fixture(missing)

    corrupt = _copy_fixture(tmp_path / "corrupt")
    build = corrupt / "workspace/BUILD.bazel"
    build.write_bytes(build.read_bytes() + b"# corrupt\n")
    with pytest.raises(FixtureError, match="size mismatch"):
        verify_fixture(corrupt)
    shutil.copy2(DEFAULT_FIXTURE_ROOT / "workspace/BUILD.bazel", build)
    with (corrupt / swift).open("r+b") as stream:
        first = stream.read(1)
        stream.seek(0)
        stream.write(bytes([first[0] ^ 1]))
    with pytest.raises(FixtureError, match="hash mismatch.*rules_swift"):
        verify_fixture(corrupt)


def test_authentic_fixture_rejects_patch_that_no_longer_produces_metadata(
    tmp_path: Path,
) -> None:
    fixture = _copy_fixture(tmp_path)
    patch = (
        fixture
        / "registry/modules/rules_shell/0.6.1/patches/module_dot_bazel_version.patch"
    )
    mutated = patch.read_text().replace('+    version = "0.6.1",', '+    version = "0.6.2",')
    patch.write_text(mutated)
    digest = hashlib.sha256(patch.read_bytes()).hexdigest()
    manifest = fixture / "fixture.toml"
    manifest.write_text(
        manifest.read_text().replace(
            "5f0700eaa9a33770aae4ae8b06bec8e433f518eb50711378c8cd3a5d7854ff2d",
            digest,
        )
    )
    with pytest.raises(FixtureError, match="patched archive member"):
        verify_fixture(fixture)


def _tiny_fixture(tmp_path: Path) -> tuple[Path, dict]:
    root = tmp_path / "fixture"
    (root / "archives").mkdir(parents=True)
    original, expected = b'version = "0"\n', b'version = "1"\n'
    metadata = "modules/example/1/MODULE.bazel"
    def bundle(name: str, data: bytes, mode: str) -> bytes:
        output = io.BytesIO()
        with tarfile.open(fileobj=output, mode=mode) as archive:
            info = tarfile.TarInfo(name)
            info.size, info.mode = len(data), 0o644
            archive.addfile(info, io.BytesIO(data))
        return output.getvalue()
    payloads = {
        "archives/source.tar.gz": (bundle("MODULE.bazel", original, "w:gz"), "archive"),
        "module.patch": (b'--- a/MODULE.bazel\n+++ b/MODULE.bazel\n@@ -1 +1 @@\n-version = "0"\n+version = "1"\n', "patch"),
        "registry.tar": (bundle(metadata, expected, "w:"), "registry_bundle"),
        "registry.json": (json.dumps([dict(demand_index=1, path=metadata,
            sha256=hashlib.sha256(expected).hexdigest(), size=len(expected),
            source_url=f"https://bcr.bazel.build/{metadata}")]).encode(), "registry_inventory"),
    }
    manifest = ['[authentic]', 'inventory_version = 1',
                f'total_source_bytes = {sum(len(data) for data, _ in payloads.values())}']
    for source, (data, kind) in payloads.items():
        (root / source).write_bytes(data)
        manifest += ['[[objects]]', f'source = "{source}"', f'destination = "{source}"',
                     f'sha256 = "{hashlib.sha256(data).hexdigest()}"', f'size = {len(data)}',
                     'source_url = "https://example.invalid/source"', f'kind = "{kind}"']
    manifest += ['[registry_bundle]', 'archive = "registry.tar"',
                 'inventory = "registry.json"', 'entry_count = 1', '[[patch_checks]]',
                 'archive = "archives/source.tar.gz"', 'member = "MODULE.bazel"',
                 'patch = "module.patch"', f'expected = "{metadata}"',
                 f'preimage_sha256 = "{hashlib.sha256(original).hexdigest()}"']
    (root / "fixture.toml").write_text("\n".join(manifest) + "\n")
    return root, fixture_tool._load_manifest(root)[0]


def test_streaming_exact_limits_without_large_payloads(tmp_path: Path) -> None:
    root, _ = _tiny_fixture(tmp_path)
    manifest = root / "fixture.toml"
    original = manifest.read_text()
    for kind, limit in (("archive", 32 << 20), ("patch", 16 << 20)):
        changed = original.replace('kind = "archive"', f'kind = "{kind}"', 1)
        size = fixture_tool._load_manifest(root)[1][0].size
        for excess in (0, 1):
            manifest.write_text(changed.replace(f"size = {size}\n", f"size = {limit + excess}\n", 1))
            if excess:
                with pytest.raises(FixtureError, match="object limit"):
                    fixture_tool._load_manifest(root)
            else:
                assert fixture_tool._load_manifest(root)[1][0].size == limit
        manifest.write_text(original)
    def item(size: int, kind: str) -> fixture_tool.Object:
        return fixture_tool.Object("x", "x", "0" * 64, size, "url", kind)
    assert fixture_tool._source_totals((item(32 << 20, "archive"),)) == 32 << 20
    assert fixture_tool._source_totals((item(16 << 20, "patch"), item(16 << 20, "archive"))) == 32 << 20
    with pytest.raises(FixtureError, match="total byte"):
        fixture_tool._source_totals((item(32 << 20, "archive"), item(1, "patch")))
    with pytest.raises(FixtureError, match="retained small"):
        fixture_tool._source_totals((item(16 << 20, "patch"), item(1, "patch")))


@pytest.mark.parametrize("fault", ["short", "excess", "corrupt", "io"])
def test_streaming_full_eof_short_reads_and_io_failure(fault: str) -> None:
    data = b"a" * (fixture_tool.ARCHIVE_CHUNK_BYTES + 3)
    item = fixture_tool.Object("archive", "archive", hashlib.sha256(data).hexdigest(), len(data), "url", "archive")
    payload = data[:-1] if fault == "short" else data + b"x" if fault == "excess" else b"b" + data[1:] if fault == "corrupt" else data
    class ShortReads(io.BytesIO):
        def read(self, size: int = -1) -> bytes:
            assert 0 < size <= 64 << 10
            if fault == "io":
                raise OSError("synthetic read failure")
            return super().read(min(size, 7))
    with pytest.raises((FixtureError, OSError), match="EOF|exceeds|hash mismatch|synthetic"):
        fixture_tool._stream_archive(ShortReads(payload), item)
    output = io.BytesIO()
    fixture_tool._stream_archive(io.BytesIO(data), item, output)
    assert output.getvalue() == data


@pytest.mark.parametrize("fault", ["replace", "corrupt", "grow", "truncate", "symlink", "directory", "ancestor", "root"])
@pytest.mark.parametrize("consumer", ["assembly", "patch"])
def test_streamed_consumers_reject_replaced_or_mutated_sources(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, fault: str, consumer: str
) -> None:
    root, raw = _tiny_fixture(tmp_path)
    verified = verify_fixture(root)
    archive = root / "archives/source.tar.gz"
    data = archive.read_bytes()
    if fault == "ancestor":
        archive.parent.rename(root / "old-archives")
        archive.parent.mkdir()
        archive.write_bytes(data)
    elif fault == "root":
        root.rename(tmp_path / "old-root")
        shutil.copytree(tmp_path / "old-root", root)
    elif fault in {"symlink", "directory", "replace"}:
        archive.unlink()
        if fault == "symlink":
            archive.symlink_to(root / "registry.tar")
        elif fault == "directory":
            archive.mkdir()
        else:
            archive.write_bytes(data)
    else:
        archive.write_bytes(data + b"x" if fault == "grow" else data[:-1] if fault == "truncate" else b"x" + data[1:])
    destination = tmp_path / "assembled"
    with pytest.raises(FixtureError, match="identity|regular|size mismatch"):
        if consumer == "patch":
            fixture_tool._verify_patch(raw, verified)
        else:
            monkeypatch.setattr(fixture_tool, "verify_fixture", lambda _: verified)
            assemble(destination, root)
    assert not destination.exists()
    assert not list(tmp_path.glob(".assembled.tmp-*"))


@pytest.mark.parametrize("consumer", ["assembly", "patch"])
def test_streamed_consumers_reject_mutation_during_held_read_and_close(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, consumer: str
) -> None:
    root, raw = _tiny_fixture(tmp_path)
    verified = verify_fixture(root)
    archive = root / "archives/source.tar.gz"
    stream_archive = fixture_tool._stream_archive
    descriptors = []
    def mutate(stream, item, output=None):
        descriptors.append(stream.fileno())
        stream_archive(stream, item, output)
        if len(descriptors) == (2 if consumer == "patch" else 1):
            archive.write_bytes(b"x" + archive.read_bytes()[1:])
    monkeypatch.setattr(fixture_tool, "_stream_archive", mutate)
    with pytest.raises(FixtureError, match="identity|hash mismatch"):
        if consumer == "patch":
            fixture_tool._verify_patch(raw, verified)
        else:
            monkeypatch.setattr(fixture_tool, "verify_fixture", lambda _: verified)
            assemble(tmp_path / "assembled", root)
    for descriptor in descriptors:
        with pytest.raises(OSError):
            os.fstat(descriptor)
    assert not (tmp_path / "assembled").exists()
    assert not list(tmp_path.glob(".assembled.tmp-*"))


def test_patch_requires_listed_archive_and_preserves_member_limit(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    root, raw = _tiny_fixture(tmp_path)
    verified = verify_fixture(root)
    raw["patch_checks"][0]["archive"] = "unlisted.tar.gz"
    with pytest.raises(FixtureError, match="unlisted archive"):
        fixture_tool._verify_patch(raw, verified)
    raw["patch_checks"][0]["archive"] = "archives/source.tar.gz"
    monkeypatch.setattr(fixture_tool, "MAX_OBJECT_BYTES", 14)
    fixture_tool._verify_patch(raw, verified)
    monkeypatch.setattr(fixture_tool, "MAX_OBJECT_BYTES", 13)
    with pytest.raises(FixtureError, match="bounded regular"):
        fixture_tool._verify_patch(raw, verified)
    monkeypatch.setattr(fixture_tool, "MAX_OBJECT_BYTES", 14)
    archive_path = root / "archives/source.tar.gz"
    original = b'version = "0"\n'
    for names in [("./MODULE.bazel",), ("MODULE.bazel", "./MODULE.bazel"),
                  ("MODULE.bazel", "MODULE.bazel"), ("elsewhere/MODULE.bazel",)]:
        with tarfile.open(archive_path, "w:gz") as archive:
            for name in names:
                info = tarfile.TarInfo(name)
                info.size = len(original)
                archive.addfile(info, io.BytesIO(original))
        # Only this tiny synthetic archive is resident; authentic archives stay streamed.
        data = archive_path.read_bytes()
        item = replace(
            verified.archives_by_source["archives/source.tar.gz"].item,
            size=len(data), sha256=hashlib.sha256(data).hexdigest(),
        )
        with fixture_tool._held_archive(root, item) as (_, snapshot):
            verified.archives_by_source[item.source] = snapshot
        if len(names) == 1 and names[0] == "./MODULE.bazel":
            fixture_tool._verify_patch(raw, verified)
        else:
            with pytest.raises(FixtureError, match="ambiguous|missing"):
                fixture_tool._verify_patch(raw, verified)
