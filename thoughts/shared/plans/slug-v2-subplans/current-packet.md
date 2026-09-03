# Current Slug V2 Packet

Packet: WP-4-5-7A-builtin-external-bzl-load-routing-audit

Milestone: M7A bootstrap-critical loading/repository execution closure. Audit
the first generic public-label external Bzl load from exact built-in
`@bazel_tools` content into a Bzlmod-selected repository.

Status: ready for one bounded docs-only audit. No Rust, catalog, fixture or
Cargo edit is authorized by this packet.

## Accepted predecessor

`WP-5-7A-bazel-tools-lib-cc-configure-catalog-implementation-r1` returns
`ACCEPT`. It adds exact pinned Bazel 9.2
`tools/cpp/lib_cc_configure.bzl` bytes plus one lexically ordered catalog row
and existing-table proof. Rust growth is 6 production and 11 proof lines, 17
total, plus the fixed 784-byte/18-line asset. The frozen
`da7e4ae162120582a7a703b5657286dffe61fdf37cc489a4fc7625608517370c`
asset hash, non-executable source mode, sorted direct listing and complete
`c313fad68f4e475d744dc6de7b658515b33c634905222e934a9d09129371f56f`
manifest digest pass.

Focused built-in repository tests and the loading direct-dependent suite pass,
and `slug_cli_v2` rebuilds. The bounded authentic rules_rust 0.73 replay clears
`UnsupportedCatalog(lib_cc_configure.bzl)`, evaluates the exact facade and then
stops while resolving its sole load:

`repository-qualified external load is deferred:
@rules_cc//cc/toolchains:toolchain_config_utils.bzl`

The facade imports only `escape_string` as `_escape_string` and re-exports it.
The next boundary is therefore generic load routing, not more built-in catalog
content or C++ configuration behavior.

## Audit question

Determine the smallest bounded Rust-native composition that gives a built-in
`@@bazel_tools` Bzl module Bazel 9.2-compatible resolution of a public apparent
repository label such as
`@rules_cc//cc/toolchains:toolchain_config_utils.bzl`, then loads that exact
selected external source through Slug's existing recursive Bzl graph.

The audit must establish:

1. the exact Bazel 9.2 repository-mapping semantics for loads issued by the
   built-in repository, including the apparent `rules_cc` name and selected
   canonical `rules_cc+` destination used by this replay;
2. which existing Stage 5 mapping/selection owner and Stage 4 canonical route,
   source observation, recursive manifest and frozen-module owners can be
   composed without duplicating identity;
3. whether the current unqualified same-repository label parser can be
   generalized safely or needs a distinct typed public-label resolution step;
4. all DICE/locking and invalidation obligations at the composition seam; and
5. the first independent replay boundary after the load succeeds, or `REPLAN`
   if a bounded generic route is not available.

The authentic apple_support/rules_cc consumer is a discriminator only. It may
not select a hard-coded mapping, source path, function body or special-case
branch.

## Required evidence

- Trace pinned Bazel 9.2 source for built-in repository mappings and Starlark
  `load()` label resolution; reuse accepted isolated oracle evidence where it
  already discriminates apparent versus canonical names.
- Trace `HostBuiltinBazelToolsRepositoryMapping*`, canonical repository route,
  external Bzl source/load keys and `BzlLoadManifest` ownership in the live
  checkout. Read `docs/developers/dice.md` before proposing any key or lock
  change.
- Prove the selected route retains exact canonical repository and mapping
  identity, complete recursive source observations, deterministic error
  ordering and warm/A-B-A invalidation. Unmodeled mapping or source state must
  fail closed.
- Distinguish exact label/mapping/loading behavior from Slug-native DICE and
  observation representation. Keep rules_cc contents and subsequent Starlark
  semantics outside this packet unless they are already accepted unchanged.
- Replay only as needed to confirm the next boundary; do not turn the audit
  into an implementation or add a new oracle without a demonstrated evidence
  gap.

## Audit allowlist and bounds

Documentation changes are limited to the canonical plan, Stages 4 and 5, and
this manifest. The audit is capped at 100 canonical-plan lines, 320 Stage 4
lines, 320 Stage 5 lines and 420 manifest lines. Prefer a smaller record and an
implementation packet with a narrow Rust/test allowlist and gross-line caps.

Do not edit Rust, Cargo metadata, tests, fixtures, built-in catalog content or
external repository bytes during this audit. Do not add a rules_cc,
apple_support, C++ or toolchain special case. Do not copy selected repository
content into `@bazel_tools`, add a second loader, weaken mapping identity, read
the filesystem directly or hold a lock across DICE computation.

## Terminal result

Return `ACCEPT` only with a bounded implementation packet naming the exact
owner composition, compatibility classifications, Rust/test allowlist, proof
matrix, replay gate and hard size caps. The packet must also record exact Bazel
source/test anchors or a skip reason, request/overlap and retained-memory
classification (including `none`), fixture/provenance outcome, and the
complexity-trigger/cohesion review. Return `REPLAN` if built-in mapping
semantics are not owned, canonical route selection is ambiguous, recursive
loading would duplicate or weaken source/invalidation identity, a safe change
requires an unbounded loader rewrite, or the first successful public-label
load crosses another unaudited semantic owner.
