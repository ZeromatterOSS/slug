from __future__ import annotations

import hashlib
import json
import shutil
from pathlib import Path

import pytest

from tools.v2_oracle.configured_cli_fixture import (
    DEFAULT_FIXTURE_ROOT,
    FixtureError,
    assemble,
    verify_fixture,
)


def _copy_fixture(tmp_path: Path) -> Path:
    fixture = tmp_path / "fixture"
    shutil.copytree(DEFAULT_FIXTURE_ROOT, fixture)
    return fixture


def test_authentic_fixture_verifies_and_assembles_offline(tmp_path: Path) -> None:
    verified = verify_fixture()
    assert len(verified.objects) == 26
    assert len(verified.registry_entries) == 177
    assert verified.total_bytes == 7_878_817
    assert (
        verified.inventory_sha256
        == "5800c9ed0df22c05229ddd908812304efa13e31609c5dd7377fd06d43d823818"
    )

    destination = tmp_path / "assembled"
    receipt = assemble(destination)
    assert receipt["object_count"] == 26
    assert receipt["registry_metadata_count"] == 177
    assert receipt["source_bytes"] == 7_878_817
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
    assert json.loads((destination / "inventory.json").read_text()) == receipt
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

    corrupt = _copy_fixture(tmp_path / "corrupt")
    build = corrupt / "workspace/BUILD.bazel"
    build.write_bytes(build.read_bytes() + b"# corrupt\n")
    with pytest.raises(FixtureError, match="size mismatch"):
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
