# Current Slug V2 Work Packet

Packet: WP-7-133-m7a-builtin-python-package-source-r1
Status: ACCEPTED; M7A remains partial

## Outcome and compatibility

Admit the exact selected Bazel 9.2 runtime source/Loading closure for
`@@bazel_tools//tools/python` so the missing-BUILD stop is no longer a
catalog absence. Copy seven pinned upstream files verbatim: runtime
`tools/python/BUILD.tools` becomes catalog `tools/python/BUILD`; include its
one local load and six exports (`python_version.bzl`,
`python_bootstrap_template.txt`, `srcs_version.bzl`, `toolchain.bzl`,
`utils.bzl`, `private/py_test_alias.bzl`). Exact claims are pinned bytes,
nonexecutable modes, package/source-label topology and successful Loading
of the declared `python_version`, `PY2`/`PY3`, `bzl_srcs`, `toolchain_type`,
`py2_interpreter_path`, `py3_interpreter_path` and
`autodetecting_toolchain` targets, with names/kinds checked against pinned
`BUILD.tools`. This is a package Loading claim, not configured evaluation of
those targets. Slug's
collision-safe catalog route manifest identity remains Slug-native; this
packet does not claim Bazel's embedded archive checksum or full directory.
Wrapper scripts, the separate `tools/python/runfiles` package, Python
actions/toolchains, native Python rules, full authentic root build and M7A
remain unsupported/deferred. Existing WP831/832 exact FeatureFlagInfo
slices remain exact and are not widened by source ingress.

## Source, decision and semantic owner

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`:
`src/create_embedded_tools.py:67-103` replaces `/BUILD.tools` with `/BUILD`
and discards source-tree BUILD on collision;
`scripts/bootstrap/compile.sh:305-312` independently makes that runtime link.
`tools/BUILD:41-70`
includes `//tools/python:embedded_tools`. Runtime
`tools/python/BUILD.tools:13-36,69-108` loads `python_version.bzl`, exports
the six declared files and defines the selected package targets.
`tools/python/python_version.bzl:20-44` creates the version rule returning
`config_common.FeatureFlagInfo(value = "PY3")`. The source-tree
`tools/python/BUILD` is Bazel's own packaging/test BUILD, not runtime
`@bazel_tools` content. Its `embedded_tools` glob also carries additional
files, intentionally outside this selected Loading closure. No new Stage 1
fixture is needed: exact upstream byte/hash comparisons and the existing
WP832 configured FeatureFlagInfo evidence are the pinned-source regression.

Stage 5's immutable `BuiltinBazelToolsSnapshot::Bazel9_2` CATALOG and
`BuiltinBazelToolsSourceFileKey` own each byte, mode, exact SHA and sorted
directory listing. Existing `HostBuiltinBazelToolsRepositoryMapping` and
external package/Bzl keys consume this source route. The CATALOG-derived
manifest SHA participates in Slug-native route identity; updating its
expected test vectors is intentional, not a Bazel checksum claim. Static
`include_bytes!` storage is process-lifetime; DICE-retained source values
use existing Arc/Dupe/Allocative ownership and unchanged complete equality.
No new DICE key, lock, interner, cache, mutable registry, filesystem bypass,
host observation, command scratch borrowing or action representation is
introduced. The current built-in catalog remains the cohesive source owner;
do not run or revive the legacy `scripts/sync_bazel_tools.sh` path.

## Evidence and validation

Before implementation, an isolated
`slug query '@bazel_tools//tools/python:python_version' --output=label_kind`
from the
preserved authentic workspace must show the missing BUILD stop; it is a
diagnostic, not an oracle or constructor proof. Port each selected file from
the pinned Git object through `apply_patch`; compare exact bytes to
`git show` afterward and assert catalog SHA/mode for every file. Assert
`tools/python/BUILD` differs from source-tree `tools/python/BUILD` and equals
`BUILD.tools`; exclude `BUILD.tools` and out-of-slice runfiles from Slug's
catalog. Prove sorted directory listing, selected package marker and
catalog-derived route manifest identity on one DICE and across independent
transactions. A focused Loading proof must assert all six `exports_files`
source labels, especially `tools/python:private/py_test_alias.bzl`, stay in
the parent package, and that neither `tools/python/private` nor the deferred
`tools/python/runfiles` has a package marker. Extend the existing catalog file-set/compile-data proof,
including a Slug-only `BUILD.bazel` sidecar. Preserve representative JDK,
C++ and test catalog controls; no hardcoded marker-format test may replace
the current catalog/writer proof.

Resolve pinned nightly with `rustup which`; serialize Cargo. Compile
selected Bzlmod/Loading test executables using Cargo JSON `--no-run`,
preflight named nonignored selectors and run focused/protected tests.
Direct-check Bzlmod and Loading dependents, rebuild `slug_cli_v2` before
authentic Slug replay, and check/clean `slugd` before and after. Run isolated
`slug query '@bazel_tools//tools/python:python_version' --output=label_kind`
and assert the exact target kind; the focused Loading test owns the complete
eight-target inventory because external `:all` Query patterns are deferred.
Then run the authentic rules_python `//:probe`; a
parallel first error does not prove the WP832 constructor path executed.
Where feasible, build a root-visible authentic rules_python
`construct_config_settings` target to probe the exact
`config_settings.bzl:172` call independently. Record the next observed stop without claiming
full Python/root bootstrap. Run pinned rustfmt, `git diff --check`, plan
and archive checks. Obtain independent DESIGN and FINAL review of this
catalog/public source boundary.

## Scope and stops

Base clean main `e054b08f5`. Allowlist: this manifest and canonical
Current; compact Stage 5/9 owner notes after acceptance; seven verbatim
files under `app/slug_bzlmod_v2/builtin/bazel_tools/tools/python/` plus
Slug-only `tools/python/BUILD.bazel`; root `.gitattributes` only for the
pinned runtime BUILD's upstream blank line at EOF;
`app/slug_bzlmod_v2/BUILD.bazel`,
`src/builtin_repository.rs`, `tests/builtin_bazel_tools.rs`; only affected
catalog/manifest/package expectation tests in
`app/slug_bzlmod_v2/src/{host_module,host_package,source_preparation}.rs`
and `app/slug_loading_v2/src/external_subtree_package_set_tests.rs` and
`app/slug_loading_v2/src/canonical_repository_load_route_tests.rs` if their existing assertions
require the new package; a focused Loading test in the latter owner.
Estimated seven upstream files and roughly 100-220 Rust/BUILD proof and
registration lines are a review trigger, not a cap. The `builtin_repository`
module is large but remains the cohesive existing catalog owner; do not
split or add a second registry to evade that boundary. `REPLAN` if package
Loading requires unadmitted Python semantics, a different archive file
mapping, new semantic owner/DICE key, more than the declared selected
source closure, or an unsupported source label. Do not invent or edit
upstream `@bazel_tools` bytes.

## Immediate predecessor

WP832 accepted at `e054b08f5` the selected configured Starlark
`FeatureFlagInfo` slice. Its parallel authentic `//:probe` replay first
reported missing `@@bazel_tools//tools/python` BUILD content, but did not
prove the constructor path executed. M7A remains partial.

## Acceptance (2026-09-24 17:41 UTC)

Independent DESIGN and FINAL reviews ACCEPT the selected source/Loading
slice. All seven local runtime files compare byte-for-byte to pinned Bazel
9.2 Git objects, preserve nonexecutable modes and match catalog SHA-256;
`BUILD.tools` is runtime `BUILD`, not Bazel's source-tree BUILD. A
single-path Git whitespace attribute preserves its upstream blank EOF line
without changing bytes or weakening checks on other files. An
ignored-RC/no-fetch Bazel 9.2 `query @bazel_tools//tools/python:all`
reported the eight selected rule names/kinds; direct private-export Query
reported a source file in the parent package. Six exact Bzlmod integration,
four Bzlmod library and four Loading library focused/protected tests passed
after Cargo JSON `--no-run` builds and exact nonignored preflight. Those
tests cover catalog bytes/modes, sorted listing, Slug-native manifest hash,
package marker/subtree boundaries, all eight rule targets and six parent
exports. Direct Bzlmod/Loading checks, rebuilt CLI, pinned rustfmt,
diff hygiene, plan/archive checks and post-smoke `slugd` absence pass.

Before ingress, an isolated Slug exact-target Query failed with the missing
`@@bazel_tools//tools/python` BUILD error; after rebuilding, the same Query
returned `_python_version_flag rule`. Slug's external `:all` Query pattern
remains deferred; the Loading test proves the complete selected inventory.
The normalized-PATH authentic `//:probe` replay first reported missing
`module_version()` at `@@platforms//:BUILD:21`. Parallel loading does not
prove the WP832 rules_python constructor path executed. A scratch root
`construct_config_settings` Build ended at `analysis_not_implemented`, so it
is inconclusive and not counted as constructor evidence. This packet took
roughly 25 minutes wall time, with source research and review overlapping
implementation; the authentic replay took about 35 seconds. Complete
Python/root bootstrap and M7A remain open.
