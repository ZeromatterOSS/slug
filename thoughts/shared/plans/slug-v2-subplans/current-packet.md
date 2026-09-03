# Current Slug V2 Packet

Packet: WP-5-7A-bazel-tools-lib-cc-configure-catalog-implementation-r1

Milestone: M7A bootstrap-critical loading/repository execution closure. Admit
the one-file exact built-in Bazel 9.2 catalog slice selected by the accepted
`lib_cc_configure.bzl` audit, then replay to the next honest boundary.

Status: ready for one bounded implementation. The source bytes, catalog owner,
classification, allowlist, proof and replay gate below are frozen.

## Accepted predecessor and audit result

`WP-4-5-7A-repository-context-which-implementation-r1` is accepted at
240/498/738 production/proof/total gross Rust additions. Its bounded
authenticated replay clears rules_shell's Unix executable lookups and stops at
the built-in catalog's typed `UnsupportedCatalog` result for
`tools/cpp/lib_cc_configure.bzl` while apple_support configures generated local
C++ repositories.

`WP-5-7A-bazel-tools-lib-cc-configure-catalog-audit` returns `ACCEPT`. The
complete smallest built-in closure is exactly one file:

| Catalog path | Pinned source | SHA-256 | Bytes / lines | Catalog mode |
|---|---|---|---|---|
| `tools/cpp/lib_cc_configure.bzl` | Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a`, same path | `da7e4ae162120582a7a703b5657286dffe61fdf37cc489a4fc7625608517370c` | 784 / 18 | `0644`, `executable: false` |

The file has one direct load,
`@rules_cc//cc/toolchains:toolchain_config_utils.bzl`, importing
`escape_string` as `_escape_string`; its sole public binding is
`escape_string = _escape_string`. This is an external selected rules_cc source,
not another `@bazel_tools` catalog member. The facade has no recursive built-in
load, glob, repository call, native rule, generated input or configured value.

Installed Bazel 9.2 has byte-identical content and the same SHA-256. Its install
base reports mode `0755`, the same extraction artifact observed for the already
accepted `tools/cpp/cc_configure.bzl`: pinned source and Slug use `0644` while
installed bytes are identical and extraction reports `0755`. Preserve the
established source/archive catalog convention; do not copy install-base mode.
Installed `tools/cpp/BUILD` is byte-identical to pinned `tools/cpp/BUILD.tools`
(`0e3fdd3293d39a64f3fbda755d8a3c0bbd3b6ffd5fb0717ab594bc1f1e29a535`,
4,102 bytes/128 lines, source `0644`, extracted `0755`), not the source-tree
`BUILD`, but it is outside this
source-only closure: existing loading already crossed the package and failed
directly on the missing facade. Do not add either BUILD file.

## Compatibility classification

- Exact: all 784 facade bytes, its path, non-executable catalog polarity,
  immediate `tools/cpp` membership, direct load label/import alias and exported
  `escape_string` binding.
- Slug-native: the existing versioned `BuiltinBazelToolsSnapshot`,
  domain-separated manifest digest, DICE source/listing values and their
  equality/integrity representation. Adding the member changes that existing
  manifest identity; with the frozen ordering its expected digest is
  `c313fad68f4e475d744dc6de7b658515b33c634905222e934a9d09129371f56f`.
- Unsupported/deferred: new or changed rules_cc loading/evaluation behavior,
  invocation semantics of the re-exported function, local C++ repository
  configuration, native/configured/toolchain behavior, install-base
  materialization and mode, broader `@bazel_tools` content, and any later replay
  failure. Existing recursive external rules_cc loading is reused and must pass
  unchanged. The consumer does not authorize an apple_support, rules_cc, C++ or
  toolchain special case.

## Required implementation

1. Copy the pinned source file verbatim to
   `app/slug_bzlmod_v2/builtin/bazel_tools/tools/cpp/lib_cc_configure.bzl`.
   Preserve 784 bytes, trailing newline and non-executable source mode.
2. Insert one lexically ordered `CatalogEntry` between `cc_configure.bzl` and
   `windows_cc_configure.bzl`, using `include_bytes!`, the frozen SHA-256 and
   `executable: false`.
3. Update the existing `tools/cpp` direct-listing expectation with
   `lib_cc_configure.bzl`, add its exact path/hash/mode row to the existing
   public catalog table, and update both existing complete-manifest digest
   expectations. Reuse the table-driven exact-source, generic byte/mode
   manifest and integrity tests; do not duplicate them.
4. Rebuild and replay. Record the first independent failure after the facade;
   it becomes a docs-first packet, never an opportunistic widening here.

Do not add a DICE key, manifest version, catalog copy, filesystem/install-base
fallback, runtime source choice, materialization path, package marker, rules_cc
source, evaluator builtin, repository method or consumer dispatch.

## Implementation allowlist and caps

Only these files may change:

- `app/slug_bzlmod_v2/builtin/bazel_tools/tools/cpp/lib_cc_configure.bzl`
- `app/slug_bzlmod_v2/src/builtin_repository.rs`
- `app/slug_bzlmod_v2/tests/builtin_bazel_tools.rs`

The catalog asset is fixed at exactly 784 bytes and 18 lines. Rust changes are
capped at 12 production, 30 proof and 42 total gross added lines; total gross
additions across all three files are capped at 60. Reformatting, comment churn
or unrelated cleanup does not create headroom. If proof or implementation
exceeds any cap, return `REPLAN` before widening the allowlist.

## Required proof and validation

- Compare the checked-in asset byte-for-byte with pinned
  `/tools/cpp/lib_cc_configure.bzl`; verify the frozen SHA-256, 784-byte size,
  18 lines and non-executable mode.
- Prove source lookup returns the exact hash/bytes and `executable == false`;
  `listing_rows("tools/cpp")` remains sorted, unique and direct with the new
  member; missing/file/wrong-kind and catalog integrity tests remain green.
- Prove the complete built-in manifest changes to the frozen digest and that
  existing byte/mode discrimination remains green.
- Run formatting and serial focused `slug_bzlmod_v2` built-in repository tests,
  followed by the loading direct-dependent suite. Use the explicit nightly
  Cargo executable from the active environment and one shared target directory;
  do not run Cargo commands in parallel.
- Rebuild `slug_cli_v2`, clean stale `slugd` processes before and after, and run
  the authentic rules_rust 0.73 replay with the already admitted bounded PATH.
  The replay must clear `UnsupportedCatalog(lib_cc_configure.bzl)` and report
  its next first failure without implementing it.
- Run diff/check-whitespace, archive-baseline and generated/artifact hygiene
  gates. Verify only the three allowlisted files changed and no JVM/helper,
  installed-tree copy, credential or temporary oracle artifact entered Git.

## Terminal stops

Return `ACCEPT` only if exact byte/hash/mode, sorted listing, manifest identity,
focused tests, loading gates, CLI rebuild and authentic replay all pass within
the caps. Return `REPLAN` on pinned/checked-in byte mismatch, source-mode drift,
an unexpected additional built-in load/package dependency, a required new
semantic owner, allowlist/cap pressure, or inability to clear the typed catalog
failure. Do not fix the replay's next independent boundary in this packet.
