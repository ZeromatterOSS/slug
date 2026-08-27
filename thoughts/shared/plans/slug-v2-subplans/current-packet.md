# Current Slug V2 Packet

Packet: `WP-4-5-7A-repository-source-observation-owner-convergence`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `fdd13400d`.

Result: implement one shared Root/Canonical repository source-observation
owner and its observed sibling without changing the existing zero-copy result,
legacy root source-file keys, loading/core consumers, or package policy.

## Accepted design

Commit `85593f300` accepts apparent-free canonical source/listing wrappers and
the loading-owned canonical load route. Commit `9d55b7157` splits later package
adaptation into Bzlmod policy convergence followed by loading adaptation.
Stage A preflight in `495edfe4f` found that a DICE key cannot vary its fixed
value between root `HostRepositorySourceFileValue` and canonical
`HostRepositorySourceObservation`.

Commit `fdd13400d` and independent review accept the bounded correction:
`HostRepositorySourceObservation::{Builtin, Request}` is already the required
zero-copy result. Generalize only that observation owner over Root/Canonical
input and add its observed sibling. Leave `HostRepositorySourceFileKey`, its
observed key, and all loading/core exhaustive consumers unchanged.

This is generic BCR Starlark loading architecture. Bazel 9 owns all rule
definitions and control flow including `cc_internal`; `cc_common` is only a
demanding consumer of the generic host-builtin ABI. Zabel is peer guidance for
ownership and compact retained representation, while Bazel 9.2 remains
behavioral authority.

## Exact allowlist and caps

Allow exactly:

- new `app/slug_bzlmod_v2/src/source_preparation/repository_source_observation.rs`;
- `app/slug_bzlmod_v2/src/source_preparation.rs`;
- `app/slug_bzlmod_v2/src/source_preparation/canonical_repository_source.rs`;
- `app/slug_bzlmod_v2/src/lib.rs`;
- `app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs`; and
- `app/slug_loading_v2/src/canonical_repository_load_route_tests.rs`.

Cap production additions at 800, proof additions at 1,000 and aggregate
additions at 1,800 lines. Keep the new owner below 700 lines, every function at
120 lines, and additions to the already oversized `source_preparation.rs` at
80 mechanical lines. Deletions count separately and buy no unrelated scope.

Add no dependency, fixture, oracle, core, query, builtin-repository or package-
policy edit. Do not edit DICE ownership or locking outside these files.

## Required owner and API

Add one compact structural input enum:

- `Root(HostRepositorySourceInput)` preserves the existing apparent-bearing
  route exactly; and
- `Canonical(HostCanonicalRepositorySourceInput)` preserves the accepted
  apparent-free canonical route and materialization disposition.

Move the cohesive observation owner into the new module. Preserve
`HostRepositorySourceObservationKey::new(root, path)` as the exact root
constructor and add a canonical constructor accepting only the complete
canonical source input and repository-relative path. Its fixed DICE value stays
`HostRepositorySourceObservation` with the existing variants and accessors.

Add an observed sibling returning one retained result `Arc` plus
`PathObservationEpoch`. It uses one shared legacy/observed driver:

- built-in input requests only the existing built-in catalog source child and
  returns an empty path epoch;
- materialized input requests the existing materialization-result owner;
- observed materialized input then requests path resolution and file bytes in
  their accepted order and merges their epoch;
- outer infrastructure error precedes Need, which precedes semantic terminal;
  no complete value is published on Need or cancellation.

The doc-hidden observation error must retain enough Root/Canonical input for
diagnostics without fabricating an apparent name. Replace its root-only
`input()` exposure with an enumerated view or distinct root/canonical accessors;
there is no live caller to preserve.

Make only the two canonical source-file wrappers delegate to this shared owner.
Keep all four canonical source/listing legacy/observed wrappers and their public
behavior through this packet. Corrected Stage A later migrates policy callers
and `canonical_repository_load_route_tests.rs`, then deletes the four wrappers.

## DICE, identity and retained representation

Follow `docs/developers/dice.md`. Keys retain complete semantic source inputs,
not command scratch. Equality and hashing include workspace, canonical or root
route, disposition, selected specification, final mapping and generated plan.
Do not hash result values or source bytes. Complete-only value equality and
validity remain unchanged, and no lock may span a DICE compute.

Preserve the exact built-in catalog value and its `Arc` payload, logical path,
SHA-256 and executable bit. Preserve the existing request value and its shared
`Arc` bytes/logical path. Add no byte buffer, mapping/specification copy,
interner, global cache, side table, manual eviction or new dependency. Derive
`Allocative` and retain explicit size guards for the input, key and observed
carrier.

## Required proof

Add discriminating proof for:

- root/canonical parity across admitted built-in, local, immutable-registry and
  generated successes and errors;
- a selected transitive registry repository absent from the root mapping;
- pointer, logical-path, SHA and executable preservation without payload copy;
- exact dependency logs: Builtin versus Request, materialization/path owners,
  and absence of legacy/canonical wrapper recursion;
- legacy behavior with no epoch; observed built-in empty epoch; observed
  materialized resolution-before-file epoch;
- outer-error, Need and terminal ordering; cancellation; complete-only
  equality and validity;
- independent workspace, canonical name, disposition, selected specification,
  final mapping and generated-plan A/B/A key equality/hash restoration;
- retained-size bounds and `Allocative` coverage;
- unchanged old root constructors and direct root observation behavior; and
- parity of all four temporary canonical source/listing wrappers.

Tests may use existing test-only constructors and dependency instrumentation,
but may not invent a root alias for canonical selected-registry success.

## Compatibility

- **Exact:** existing root observation results/errors/dependency order and all
  accepted canonical wrapper behavior; canonical source selection follows
  Bazel 9.2 semantics.
- **Slug-native:** Root/Canonical enum layout, key names, structural hashes,
  observed carrier and retained-memory accounting.
- **Unsupported/deferred:** Bzlmod package-policy convergence, external package
  loading and `.bzl` adaptation, target-pattern expansion, registration,
  configured semantics, rules, actions and exact output identity.

## Validation

Run focused observation-owner and canonical load-route tests first. Then run
the full `slug_bzlmod_v2` and `slug_loading_v2` suites, named core/query/loading
dependents, and a locked `cargo build -p slug_cli_v2` serially. Run formatting,
`git diff --check`, allowlist/cap/function-size/duplicate-owner/no-lock guards
and `scripts/v2_archive_status.sh`; only the three accepted thoughts-path rows
may remain in the archive-checker baseline. Clean stale `slugd` before and
after daemon-sensitive tests. Require independent DICE/public-boundary/
retained-representation terminal review.

## Stops and successor

STOP and `REPLAN` for hashing or copying source bytes; changing the shared
success variants; changing old root key values, constructors or consumers;
deleting any canonical wrapper; adding duplicate source/materialization/path
ownership; fabricating an apparent alias; direct IO; changing package policy;
missing epoch order; lock across compute; dependency, allowlist or cap
expansion; or activating loading/package/registration/configured/rule/action
behavior.

On acceptance, select corrected
`WP-4-5-7A-canonical-source-policy-convergence-implementation`, adding
`canonical_repository_load_route_tests.rs` to its proof allowlist so it can
migrate callers and delete all four temporary wrappers. Stage B loading/package
adaptation follows only after corrected Stage A; the shared registration
expander follows only after Stage B.
