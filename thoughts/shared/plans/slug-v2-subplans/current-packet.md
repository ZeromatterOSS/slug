# Current Slug V2 Packet

Packet: WP-4-5-7A-builtin-external-bzl-load-routing-implementation-r1

Milestone: M7A bootstrap-critical loading/repository execution closure. Admit
the bounded exact shape of one nonempty apparent-repository child load
(`@name//...`) from a parsed built-in `@bazel_tools` Bzl module through the
already-owned canonical mapping and recursive source graph.

Status: ready for one bounded implementation. The owner composition,
compatibility classification, two-file allowlist, proof and stops are frozen.

## Accepted predecessor and audit result

`WP-5-7A-bazel-tools-lib-cc-configure-catalog-implementation-r1` is accepted at
6 production and 11 proof gross Rust additions plus the exact 784-byte asset.
The bounded replay evaluates the facade and stops at its sole load:

`repository-qualified external load is deferred:
@rules_cc//cc/toolchains:toolchain_config_utils.bzl`

`WP-4-5-7A-builtin-external-bzl-load-routing-audit` returns `ACCEPT`. Pinned
Bazel 9.2 `BzlLoadFunction.java:780-831` establishes source compilation before
repository-mapping lookup, followed by source-ordered load resolution.
Lines 881-897 retain the mapping and load DAG in the module context; lines
935-954 use the importer's full `RepositoryMappingFunction` result for ordinary
BUILD/Bzlmod loads; and lines 1071-1125 parse every load with that mapping.
`RepositoryMappingFunction.java:75-158` obtains a module repository's complete
Bazel dependency mapping. `BzlLoadFunctionTest.testLoadBzlFileFromBzlmod`
proves apparent-to-canonical child resolution. The bootstrap-only self mapping
at `BzlLoadFunction.java:939-945` is not this ordinary Bzlmod load family.

Slug already owns the corresponding graph. Exact built-in
`MODULE.bazel:37` declares `rules_cc` 0.2.17. The existing
`HostBuiltinBazelToolsRepositoryMapping*` derives the complete selected mapping
and owns `rules_cc -> rules_cc+`. `HostCanonicalRepositoryLoadRoute*` projects
that mapping plus immutable built-in source identity. Canonical external Bzl
resolution, source observation, cycle handling, recursive manifest and frozen
module lifetime are already accepted. Only the older root-shaped built-in
entry fails to import that route before resolving its public child load.

## Compatibility classification

- Exact: for a parsed built-in module with exactly one load spelled with one
  `@`, a nonempty apparent repository name and `//`, source/UTF-8/parse errors
  precede mapping and label errors; the importer's complete mapping resolves
  apparent `rules_cc` to canonical `rules_cc+`; the child label, observable
  Bzlmod load-context selection, re-exported `escape_string` identity and
  successful recursive source order match Bazel 9.2.
- Slug-native: existing DICE keys, legacy/observed path frontiers, canonical
  source inputs, manifest equality/fingerprint, cycle detector and retained
  frozen-module closure represent those semantics. Observation conflicts and
  unmodeled mapping/source state fail closed.
- Unsupported/deferred: canonical `@@repo//...`, explicit-main `@//...`, and
  built-in modules with two or more loads when any is repository-qualified,
  including Bazel's all-labels-before-any-child failure ordering; Bzlmod-
  bootstrap-only mapping behavior; arbitrary new root-route semantics;
  rules_cc contents or evaluator features not already admitted; later C++
  configuration logic; apple_support/C++ or toolchain branches; and the next
  replay failure. Empty and every nonadmitted built-in load shape retain their
  accepted root-shaped path or typed unsupported boundary unchanged.

## Required implementation

In `compute_external_bzl_module`, keep source lookup, UTF-8 decoding and parse
on the original built-in root source route. After collecting raw loads and
before label resolution, activate promotion only when the route is
`Root(route) if route.is_builtin_bazel_tools()`, the load count is exactly one,
and the raw string has exactly the apparent form `@name//...`: one leading `@`
and a nonempty repository segment. Empty, relative, canonical `@@...`,
explicit-main `@//...` and other nonadmitted shapes continue through their
existing root route and typed boundary. Multi-load modules containing a
repository-qualified load remain fail-closed/deferred; do not partially
resolve or activate a child.

For the admitted single-load shape, compute the existing canonical
`bazel_tools` load-route key: legacy uses
`HostCanonicalRepositoryLoadRouteKey`; observed uses its observation sibling
and merges that complete frontier after the catalog-source frontier.

Construct an invocation-local effective
`HostRepositorySourceRoute::Canonical` from the returned
`HostCanonicalRepositorySourceInput`, preserving the original
`RepositoryBzlLabel` and `BzlModuleContext`. Use that effective route for the
unchanged canonical label resolver, child route/source recursion, manifest,
evaluation and freeze. Reuse or narrowly generalize the existing canonical
route-input helper and existing `ExternalBzlModuleError::Route`; add no public
error family.

Do not change `HostRootRepositoryLoadRouteKey`, put mapping state into
`RootRepositoryRoute`, generalize mapping-free `resolve_external_load_label`,
add a DICE key/loader/cache, or hard-code `rules_cc`. Compute the canonical
route before evaluator creation; no evaluator, module mutation, lock or frozen
borrow may cross the new await.

## Ownership and lifecycle

- Requests/overlap: same external-module keys remain DICE-deduplicated;
  concurrent keys reuse the canonical-route dependency, while transactions
  retain their own observation epochs. No shared mutable or process-global
  state is added.
- Observed order: merge catalog source, canonical built-in mapping/route, then
  selected child routes and sources in load order. `Need`, cancellation and
  merge conflicts publish no parent result or event batch; mapping and source
  A/B/A must restore semantic equality and warm reuse.
- Retained memory: no new key field, retained type or cache. The effective
  route is invocation scratch. Successful modules retain mapping and recursive
  children only through the existing `BzlLoadManifest` and frozen closure.
- Cycles: retain existing external child cycle guards. Promotion must not
  introduce a DICE wait cycle or a second identity for recursive children.
- Fixtures/provenance: add no fixture. Reuse exact checked-in built-in bytes,
  inline synthetic registry/materialization proof, and the authenticated
  rules_rust/rules_cc replay. Inspection of materialized rules_cc is
  corroboration only, not a source or fixture.
- Complexity/cohesion: one driver seam and its direct proof are one cohesive
  slice. No Buck2 hot-path utility, compact representation or Stage 9 identity
  review is triggered. Return `REPLAN` before adding another production owner.

## Implementation allowlist and caps

Only these files may change:

- `app/slug_loading_v2/src/bzl_module.rs`
- `app/slug_loading_v2/src/canonical_repository_load_route_tests.rs`

Gross Rust additions are capped at 180 production, 360 proof and 540 total.
Formatting or unrelated cleanup does not create headroom. No Cargo, fixture,
catalog, Stage 5 or external-source edit is permitted.

## Required proof and validation

- Prove exact `rules_cc -> rules_cc+` importer mapping, the facade's direct
  `@@rules_cc+//cc/toolchains:toolchain_config_utils.bzl` child, recursive
  same-repository child identity, and the re-exported `escape_string`.
- Prove the exact structural gate: empty and nonadmitted built-in modules do
  not request a mapping or change child-key identity; exactly one nonempty
  apparent `@name//...` load promotes; canonical `@@...`, explicit-main
  `@//...` and multi-load public-label modules keep their typed unsupported
  boundary and activate no child. Broader all-label prevalidation is not
  claimed.
- Prove legacy/observed parity and complete frontier order for the admitted
  shape; source/encoding/parse errors precede mapping, mapping and label errors
  precede the sole child source, missing mapping and child fail closed without
  speculative activation, and no `bazel_tools` materialization request occurs.
- Prove `Need`, cancellation/recovery, cycle handling, warm reuse, mapping and
  source A/B/A restoration, equality/validity, and retained size bounds.
- Use the pinned Bazel source/test anchors above; no new oracle or fixture is
  justified because the exact ordering/mapping rule and live consumer are
  already discriminating.
- Run rustfmt/diff checks, serial focused built-in/canonical external-Bzl tests,
  full `slug_loading_v2`, a direct `slug_query_v2` dependent, and rebuild
  `slug_cli_v2`. Clean `slugd` before and after the bounded authentic replay.
- The replay must clear the repository-qualified-load error, freeze the
  rules_cc utility and built-in re-export, then record the next typed boundary
  without implementing it. Run archive and artifact hygiene gates.

## Terminal stops

Return `ACCEPT` only if the bounded single-load ordering, complete importer
mapping, unchanged nonadmitted paths, canonical child identity, recursive
manifest/frontier, lifecycle proofs, tests, rebuild and replay pass within both
files and all caps. Return `REPLAN` if the change requires multi-load
prevalidation, Stage 5 ownership, a shared root-load-route change, another
crate/file, a new DICE key or retained representation, direct filesystem
access, copied rules_cc content, an evaluator held across DICE, or any
consumer/ruleset/toolchain special case.
