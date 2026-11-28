import re
import unittest

from tools.v2_oracle import rust_aquery_compare as compare
from tools.v2_oracle_lib.compare import load_expected
from tools.v2_oracle_lib.fixture import load_fixture

_TARGET = b"bazel-out/slugcfg-v2-" + b"a" * 64 + b"/bin/"
_EXEC = b"bazel-out/slugcfg-v2-" + b"b" * 64 + b"-exec/bin/"


def _reference_rows() -> tuple[bytes, ...]:
    fixture = load_fixture(compare.FIXTURE_ROOT)
    expected = load_expected(fixture.expected_oracle)
    rows = {row["name"]: row["stdout"].encode("utf-8") for row in expected["commands"]}
    return tuple(rows[name] for name in compare.CASES)


def _with_slug_domains(bazel: bytes) -> bytes:
    """A synthetic control from checked-in Bazel rows with only display domains changed."""
    return re.sub(rb"^  ActionKey: [0-9a-f]{64}\n", b"", bazel, flags=re.MULTILINE).replace(
        b"bazel-out/k8-fastbuild/bin/", _TARGET,
    ).replace(b"bazel-out/k8-opt-exec/bin/", _EXEC).replace(
        b"  Configuration: k8-fastbuild", b"  Configuration: slugcfg-v2:" + b"a" * 64,
    ).replace(b"  Execution platform: @platforms//host:host", b"  Execution platform: @@platforms//host:host").replace(
        b"# Configuration: 72a5446837369cd26452deae28ca8b1789d1ab4a7c21b1e160e1632e5f6b4b66",
        b"# Configuration: slugcfg-v2:" + b"a" * 64,
    )


class RustAqueryCompareTest(unittest.TestCase):
    def setUp(self) -> None:
        self.bazel = _reference_rows()
        self.slug = tuple(map(_with_slug_domains, self.bazel))

    def test_uses_the_four_pinned_invocations_and_paired_edition_edits(self) -> None:
        cases = compare.load_cases()
        self.assertEqual(tuple(command.name for command in cases), compare.CASES)
        self.assertEqual(tuple(len(command.mutations) for command in cases), (0, 0, 1, 1))
        self.assertEqual(cases[2].mutations[0].replace, 'edition = "2021"')
        self.assertEqual(cases[3].mutations[0].replace, 'edition = "2024"')
        compare.compare_sequences(self.bazel, self.slug)

    def test_rejects_input_identity_and_order_and_keeps_the_full_argv(self) -> None:
        baseline = self.slug[0]
        probe = _TARGET + b"pkg/probe"
        mapping = probe + b".repo_mapping"
        for wrong, field in (
            (baseline.replace(b"libsupport-483290713.rlib", b"libsupport-907578073.rlib"), "Rustc inputs"),
            (baseline.replace(probe + b", " + mapping, mapping + b", " + probe), "RunfilesTree inputs"),
            (re.sub(rb"^    '--codegen=link-arg=-fuse-ld=gold' .+\n", b"", baseline, flags=re.MULTILINE), "Rustc argv"),
            (baseline.replace(b"'--codegen=debuginfo=0'", b"'--codegen=strip=none'", 1).replace(
                b"'--codegen=strip=none' \\\n    '--codegen=strip=none'", b"'--codegen=strip=none' \\\n    '--codegen=debuginfo=0'",
            ), "Rustc argv"),
            (baseline.replace(b"CARGO_CFG_TARGET_OS=linux", b"CARGO_CFG_TARGET_OS=wrong"), "Rustc environment"),
            (baseline.replace(b"supports-path-mapping", b"wrong"), "Rustc execution_info"),
            (baseline.replace(_TARGET + b"pkg/probe]", _EXEC + b"pkg/probe]"), "Rustc outputs"),
        ):
            with self.subTest(field=field):
                self.assertNotEqual(wrong, baseline)
                with self.assertRaisesRegex(ValueError, field):
                    compare.compare_rows(self.bazel[0], wrong, edition="2024")

    def test_rejects_wrong_scope_action_order_platform_and_slug_keys(self) -> None:
        baseline = self.slug[0]
        rustc, symlink, runfiles, _ = baseline.split(b"\n\n")
        for wrong, field in (
            (b"\n\n".join([rustc, runfiles, symlink, b""]), "action order"),
            (baseline.replace(b"Creating runfiles tree", b"Creating unexpected tree"), "progress"),
            (baseline.replace(b"@@platforms//host:host", b"@@other//host:host"), "platform"),
            (baseline.replace(_TARGET + b"pkg/probe]", b"bazel-out/foreign/bin/pkg/probe]"), "configured path"),
            (baseline.replace(b"  Inputs:", b"  ActionKey: " + b"0" * 64 + b"\n  Inputs:", 1), "Slug ActionKey projection is deferred"),
        ):
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, field):
                compare.compare_rows(self.bazel[0], wrong, edition="2024")

    def test_enforces_bazel_key_shape_and_same_tool_restoration(self) -> None:
        malformed = self.bazel[0].replace(b"  ActionKey: ", b"  ActionKey: z", 1)
        with self.assertRaisesRegex(ValueError, "64-hex Bazel ActionKey"):
            compare.compare_rows(malformed, self.slug[0], edition="2024")
        changed_key = re.sub(rb"  ActionKey: [0-9a-f]{64}", b"  ActionKey: " + b"f" * 64, self.bazel[3], count=1)
        with self.assertRaisesRegex(ValueError, "restoration output must equal cold"):
            compare.compare_sequences((*self.bazel[:3], changed_key), self.slug)
        cold_key = re.search(rb"  ActionKey: [0-9a-f]{64}", self.bazel[0]).group()
        stale_key = re.sub(rb"  ActionKey: [0-9a-f]{64}", cold_key, self.bazel[2], count=1)
        with self.assertRaisesRegex(ValueError, "edition edit must change only the Rustc ActionKey"):
            compare.compare_sequences((*self.bazel[:2], stale_key, self.bazel[3]), self.slug)
        with self.assertRaisesRegex(ValueError, "pinned edition 2021"):
            compare.compare_sequences(self.bazel, (self.slug[0],) * 4)
        with self.assertRaisesRegex(ValueError, "all four ordered captures"):
            compare.compare_sequences(self.bazel, self.slug[:3])

    def test_rejects_shared_drift_beyond_the_edition_atom(self) -> None:
        bazel = self.bazel[2].replace(b"CARGO_PKG_VERSION=0.0.0", b"CARGO_PKG_VERSION=0.0.1")
        slug = self.slug[2].replace(b"CARGO_PKG_VERSION=0.0.0", b"CARGO_PKG_VERSION=0.0.1")
        compare.compare_rows(bazel, slug, edition="2021")
        with self.assertRaisesRegex(ValueError, r"Rustc environment\[.*Bazel cold.*Bazel edited"):
            compare.compare_sequences(
                (*self.bazel[:2], bazel, self.bazel[3]), (*self.slug[:2], slug, self.slug[3]),
            )

    def test_rejects_a_changed_bazel_configuration_checksum(self) -> None:
        edited = re.sub(rb"# Configuration: [0-9a-f]{64}", b"# Configuration: " + b"f" * 64, self.bazel[2])
        with self.assertRaisesRegex(ValueError, "changed the Rustc configuration checksum"):
            compare.compare_sequences((*self.bazel[:2], edited, self.bazel[3]), self.slug)
