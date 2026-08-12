# Current Slug V2 Packet

Packet: `WP-5-host-selected-module-graph-owner-design-r4`
Milestone: cross-stage M7 prerequisite design
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: freeze the sole future Host discovery-to-MVS selected-module graph
owner, or return `REPLAN` at the first missing prerequisite.

## Accepted prerequisite

Commit `c997f7e7` accepts one crate-private exact Bazel 9.2 module-version
domain. `BazelModuleVersion` owns normalized equality/hash and parsed
identifier ordering; root/nonroot evaluators and lockfile v28 share it, and
`HostDiscoveredModuleKey::try_new` rejects or normalizes before a DICE key can
exist. The effective override/source seam in `dbeb1fb9` already overlays
root and command declarations, retains provenance, projects command paths once
into the accepted local-path `RepoSpec`, and controls every source classifier.

## Active design contract

Audit and freeze one crate-private Host selected-graph DICE owner that follows
pinned Bazel 9.2 discovery then selection without activating loading or any
consumer. The audit must:

- identify the exact key/value/error shapes and the one construction seam over
  `RootModuleFilesKey`, `RootModuleCommandPolicyKey`,
  `HostEffectiveModuleOverrideKey`, `HostDiscoveredModuleKey`, and
  `BazelModuleVersion`;
- preserve root-first override application, the protected `bazel_tools`
  default sentinel and explicit override bypass, ordinary and nodep dependency
  traversal, dev-dependency policy, source-order error/Need precedence, and
  complete-only DICE validity;
- own discovery recursion, minimum-version selection, single-version rewrites,
  multiple-version override ceilings, compatibility-level conflicts, and graph
  rewriting exactly once, with structural equality over every selected module
  key, rewritten edge, complete evaluated module, and retained source
  provenance;
- decide the bounded representation for ordered root/direct dependencies and
  order-insensitive selected identities without inventing canonical repository
  names, mappings, extension names, selected RepoSpecs, lockfile products, or
  consumer-facing public types;
- classify exact, Slug-native, and unsupported/deferred surfaces, and freeze a
  future implementation allowlist, production/test caps, proof matrix, and
  terminal stops; and
- return `REPLAN` rather than guessing if discovery requires a missing policy,
  yanked-version owner, registry metadata/source capability, compatibility
  level, multiple-version normalization, graph recursion/cycle contract, or
  post-selection identity that no accepted leaf retains.

## Scope

This design packet may edit only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest; and
- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`.

The root may inspect Bazel 9.2 source and live Rust owners read-only. Cap net
manifest growth at 420 lines, owner-plan growth at 360 lines, canonical growth
at 45 lines, and 825 total. No Rust, Cargo/BUILD, wire/schema, fixture/oracle,
public API, legacy `ResolvedGraph`, source/materialization change, mapping,
loading, configured analysis, command, execution, or consumer edit is
authorized. Obtain a fresh independent reserved-architecture review.

Return `REPLAN` on a missing prerequisite or any required fourth file. Return
`REVISE` on one bounded design-contract correction. A second material
correction is `REPLAN`. No production implementation may begin before
independent `ACCEPT` and an explicitly activated implementation successor.

## Compatibility

Exact: admitted Bazel 9.2 discovery/selection behavior only if every semantic
input and retained identity is already owned and the audit freezes a bounded
implementation.

Slug-native: Rust/DICE type names, compact collection choices, lifecycle
instrumentation, and diagnostic framing outside named exact messages.

Unsupported/deferred: canonical/full repository mappings, extension unique
names/execution, lockfile production, package/Bzl loading, configured
analysis/toolchains/actions, Test, execution/results/BEP/coverage, native
Windows command-path semantics, JVM/Java, and exact Bazel identity bytes.

## Accepted implementation evidence

`c997f7e7` is independently accepted within its exact six-file boundary.
Focused grammar/equality/hash/order tests, retained root/nonroot normalization,
checked Host construction, real-DICE build-suffix and semantic A/B/A/reuse,
100 lockfile-v28 regressions, all 303 owner tests plus integrations/docs, and
the full loading suite pass. The two known core baseline failures remain the
unrelated deferred external-visibility and uninjected legacy path-epoch rows.
