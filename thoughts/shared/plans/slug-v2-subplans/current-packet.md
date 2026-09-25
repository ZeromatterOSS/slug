# Current Slug V2 Work Packet

Packet: WP-7-145-m7a-bazel-tools-cc-whitelist-r1
Status: ACCEPTED; M7A remains partial

## Outcome and compatibility

Advance the authentic Rust-toolchain `cquery //:probe --output=label`
past its post-WP844 missing built-in package
`@@bazel_tools//tools/build_defs/cc/whitelists/parse_headers_and_layering_check`,
then report the first new stop. Port only that Bazel 9.2 runtime package's
single upstream `BUILD` file verbatim into Slug's immutable built-in
`@bazel_tools` catalog. Do not invent whitelist contents, copy Bazel test
mocks, widen to sibling packages or claim configured C++ toolchain closure.

The 241 source bytes, non-executable mode, SHA-256, package/group/filegroup
declarations, empty `packages=[]` and target label are **exact** to pinned
Bazel 9.2. Slug's catalog route/manifest digest, DICE key and generated
repository display/path identity remain **Slug-native**. Other unported
`@bazel_tools` packages, rules_cc `cc_toolchain` providers/configuration,
legacy suite selection, exact Bazel configuration/output identity and M7A
closure are **unsupported/deferred**. WP837–844 accepted slices remain
unchanged.

## Pinned source and current owner

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
tracks exactly one file in
`tools/build_defs/cc/whitelists/parse_headers_and_layering_check/`:
`BUILD`, Git mode 100644, 241 bytes, SHA-256
`cce7a8a7321bdcd332d89d01581a3023bc7d73765e8e303419b91f890ebda583`.
It declares `licenses(["notice"])`, an empty `package_group` named
`disabling_parse_headers_and_layering_check_allowed` with `packages=[]`,
and a packaging `filegroup(name="srcs", srcs=["BUILD"],
visibility=["//tools/build_defs:__pkg__"])`; it has no load or transitive
file dependency. Pinned Bazel `tools/build_defs/BUILD:24-25` includes its
`:srcs` in `embedded_tools`, and `tools/BUILD:43-50` embeds that into
`@bazel_tools`. The empty package list is semantically decisive: the
permissive `packages=["//..."]` in Bazel's `MockCcSupport` test is not
runtime content and must not be ported. Pinned rules_cc 0.2.17
`cc/private/rules_impl/cc_toolchain.bzl:335-338` supplies the exact
implicit attr label, with `semantics.get_repo() == "bazel_tools"`.

Slug's `app/slug_bzlmod_v2/src/builtin_repository.rs` has a sorted
compile-time `CATALOG` but no child under `tools/build_defs/cc/whitelists`.
The existing source-file and directory-listing keys derive bytes/kind
from that catalog, not a Host read. Its ordered path/content/mode manifest
hash is the structural route owner and must change when this file is
added; no new DICE key, lock or filesystem observation is admitted.
Existing Loading `package_group` and configured PackageSpecificationInfo
owners remain unchanged. A Slug-only sibling `BUILD.bazel` sidecar is
needed solely to declare Bazel compile-data inputs for Slug's own build;
it must not enter the runtime catalog or change upstream bytes.

## Implementation and evidence

Copy the upstream `BUILD` verbatim and verify byte count, SHA-256 and
non-executable mode. Add one sorted catalog row with that pinned digest,
the local `slug_builtin_catalog_files()` sidecar, and its compile-data
dependency. Update exact FILES/sidecar assertions, child-directory and
selected-file tests, and the current snapshot's architecture-conditional
manifest digest literals using the catalog's existing hash algorithm.
Do not modify the hash algorithm, representation, package-group loader,
or other catalog entries. The global Slug-native manifest identity changes
because the catalog legitimately grew; source file equality remains exact.

Add a small offline fixture with no external module, querying the exact
`@bazel_tools` package-group label and `:srcs` target; use type/kind and
dependency discriminators to prove both upstream targets load without an
invented edge. The exact catalog-byte test, not a query heuristic, proves
the group's `packages=[]` content. Generate and
independently verify Bazel 9.2 expected results in fresh ignored-RC roots,
then replay through a rebuilt Slug CLI. Focused Bzlmod tests must prove
catalog SHA/mode, selected BUILD bytes and parent/child listings; existing
package-group tests protect empty membership unless a demonstrated gap
requires a new Loading test. The authentic cquery must clear the missing
package error and reach a source-attributed first new stop; a resource
failure or identical missing package is diagnostic only. Record host
resources and avoid a blind retry.

Resolve pinned Rust toolchain; compile Bzlmod with JSON `--no-run`, exact
test preflight and focused tests, then rebuild `slug_cli_v2` before Slug
fixture/authentic replay. Serialize Cargo and check/clean stale `slugd`
before and after daemon-sensitive smokes. Run format, diff, plan-status,
archive and fixture hygiene. Independent DESIGN review must accept the
upstream-verbatim and manifest-identity boundary before production edits;
independent FINAL review checks the diff/evidence. Update compact Stage
5/9 owner notes only after acceptance.

Allowlist: this packet and canonical Current;
`app/slug_bzlmod_v2/builtin/bazel_tools/tools/build_defs/cc/whitelists/parse_headers_and_layering_check/{BUILD,BUILD.bazel}`;
`app/slug_bzlmod_v2/BUILD.bazel`, `src/builtin_repository.rs`,
`tests/builtin_bazel_tools.rs`, and `src/host_module.rs` only for fixed
manifest identity tests; one small new fixture under
`tests/v2_oracle/fixtures/builtin-cc-whitelist-package/`; compact Stage
5/9 notes after acceptance. No rules_cc patch, Loading/Analysis production,
Core, remote cache/action or extra `@bazel_tools` content.

`REPLAN` if the authentic runtime package differs from the pinned upstream
file, if a verbatim file cannot be represented by the catalog, if Loading
does not own empty package-group semantics, if the new package requires
unlisted upstream files before the next stop, or if catalog addition needs
a new DICE or Host owner. Any later `cc_toolchain` failure is a separate
packet, not license to add invented whitelist permissions.

## Acceptance receipt

Independent DESIGN review ACCEPT preceded the production edit. The only
upstream runtime file is 241 bytes, Git mode 100644, SHA-256
`cce7a8a7321bdcd332d89d01581a3023bc7d73765e8e303419b91f890ebda583`;
the local catalog file matches all three, and the local `BUILD.bazel`
sidecar is excluded from the runtime file list. The catalog remains sorted;
its existing manifest algorithm yields 89 Linux x86_64 entries and SHA-256
`331063679b7adc7084be104a455c13df476cdda2aa13acb07edf2dc7e12d9e7c`,
or 87 other-platform entries and SHA-256
`f4e5f390e55c75e9a91fb3dadf950927c8b0b56ab7ea7da99d4c5eed6d9d5151`.

`cargo fmt --all`, Bzlmod library/integration JSON `--no-run`, exact
library-test name preflight, eight catalog unit tests, one host route test
and all seven `builtin_bazel_tools` integration tests passed. `cargo build
-p slug_cli_v2` passed after the production catalog change. The new
four-row fixture passed fresh ignored-RC Bazel 9.2 generation at
`/tmp/slug-wp845-bazel-gen-mIBtC8`, independent fresh-root verification at
`/tmp/slug-wp845-bazel-verify-aXzX2j`, and rebuilt Slug replay at
`/tmp/slug-wp845-slug-S8eOEl`. An initial Slug harness invocation used
a relative binary path and failed before launching Slug; replay with the
absolute rebuilt binary succeeded. No BuildBuddy credential file was read.

Authentic Rust-toolchain alias workspace
`/tmp/slug-wp797-authentic-2YQmSt` ran `cquery //:probe --output=label` on
the rebuilt one-shot CLI. It cleared the old missing-package diagnostic
and exited 2 with the new `native filegroup nondefault licenses is
unsupported` error after roughly two minutes. This originates in Slug's
configured filegroup guard (`slug_analysis_v2/src/dice/filegroup.rs:92-110`),
not in the catalog lookup; Loading projects package licenses to native
target attributes at `slug_loading_v2/src/package.rs:2725-2732`. The
runtime whitelist `BUILD:1` has
`licenses(["notice"])`; pinned rules_cc 0.2.17's generated
`cc/private/toolchain/BUILD.tpl:21-23` also has the same declaration and
several filegroups at lines 44-68. The error does not name its target, so
this receipt does not attribute it to a specific filegroup or claim
configured C++ closure. Host had about 28 GiB available RAM and 30 GiB
free `/tmp` space before replay. No stale `slugd` was present before or
after smokes. Plan-status, archive-status, fixture discovery and
`git diff --check` passed. Independent FINAL review ACCEPT verified the
verbatim upstream file, exact catalog/manifest ownership, four-row oracle,
and the new source-backed Analysis boundary. The reviewer specifically
confirmed that no target label is available for the `licenses` failure;
configured C++ closure is not claimed. Compact Stage 5/9 owner notes were
updated after acceptance.

## Immediate predecessor

WP844 at `0cdf1b9b5` accepted Bazel 9.2's `.bzl`-injected
`native.cc_toolchain_suite` as a name-only empty filegroup, with two
focused Loading tests, fresh four-row Bazel/Slug evidence and independent
FINAL review. Authentic cquery cleared the generated suite call and first
stopped at this missing upstream `@bazel_tools` package. The full WP844
receipt remains in that commit's `current-packet.md`; M7A remains partial.
