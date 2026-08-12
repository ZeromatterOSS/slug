# Current Slug V2 Packet

Packet: `WP-5-host-nonregistry-module-closure-design`
Milestone: cross-stage M7 prerequisite design
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: freeze a route-independent nonregistry MODULE/include closure owner.

## Accepted predecessor boundary

Commit `e7e4a772` accepts the embedded/registry discovered-module leaf.
Commit `1b2f1a0a` accepts the general nonregistry discovery audit, which ends
`REPLAN` before Rust.

The audit found that `RepositoryMaterializationKey` and
`RepositorySourceFileKey` already select root `RepoSpec` by workspace and
module name and can own root MODULE bytes for local and immutable sources.
The complete evaluator already accepts a supplied closure. The missing bridge
is include closure preparation: `DirectLocalModuleInspectionKey`,
`DirectLocalIncludePackageHorizonKey`, and
`DirectLocalModulePreparationKey` are keyed by a root apparent repository
name and call `RootRepositoryRouteKey`/route-bound package lookup. A
transitive root override has no required root apparent name, and archive/Git
sources must retain the selected immutable materialization rather than enter
the direct-local route. Widening that route or fabricating an apparent name
would be inexact.

## Active design contract

Design one crate-private nonregistry MODULE closure preparation key identified
by normalized workspace plus exact `NonrootModuleKey`. It must compute root
files/override classification first, require exactly one retained
`NonRegistry(RepoSpec)`, and use the existing sole
`RepositoryMaterializationKey` and `RepositorySourceFileKey` rather than
reading or materializing independently.

Freeze root MODULE inspection, breadth-first include preflight, package
boundary/deletion/ignore policy, exact fragment source observations, compile-
complete-before-execution ordering, repeated labels, typed missing/wrong-kind/
cycle capability/errors, logical source IDs/spans, and complete closure
equality. Determine the smallest reuse seam from the direct-local owners
without making apparent names or canonical repos semantic inputs. Local live
sources and immutable archive/Git generations must invalidate through their
existing materialization/source dependencies; operational roots not exposed by
semantic equality remain distinct from content/source identity as already
accepted.

This packet owns preparation only. Evaluation into
`HostDiscoveredModule`, command overrides, recursive discovery/MVS,
mappings/extensions/registrations, lockfile products, package/Bzl consumers,
configured analysis, and commands remain later. Freeze the exact Rust
allowlist, production/test/total caps, focused local and immutable A/B/A,
Need/error/order/reuse evidence, downstream validation, and independent review
gate. Return `REPLAN` if package-boundary reuse still requires a route or if
one closure cannot preserve both live-local and immutable identity.

## Compatibility

Exact: Bazel 9.2 nonregistry MODULE/include closure semantics for explicitly
admitted local and immutable RepoSpec shapes. Slug-native: DICE type names,
diagnostic wording outside accepted shapes, compact storage, Host observation
framing, and non-Bazel identity bytes. Unsupported/deferred: evaluation/
discovery consumption, command overrides, recursive discovery/MVS,
post-selection identities, package/BUILD/Bzl loading, toolchains, Test,
execution/results/BEP/coverage, unadmitted repository rules, Windows, JVM/Java,
and exact Bazel identity bytes.

## Scope, proof, and stops

Edit only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`;
  and
- `thoughts/shared/plans/slug-v2-subplans/08-ruleset-and-command-conformance.md`.

Cap formatted net documentation growth at 220 lines. Add no Rust, Cargo/BUILD
metadata, fixture, asset, dependency, public surface, command behavior, or
production representation. Validate diff/scope/cap, active archive layout,
packet consistency, live owner citations, and independent latest-diff review.

Stop with `REPLAN` on route/apparent/canonical-name dependency, duplicate
materialization/source/package ownership, lost RepoSpec/content/include
identity, untracked IO, lock-held compute, evaluation or graph consumer,
second graph, JVM/Java, fifth file, or cap excess.
