# Current Slug V2 Packet

Packet: `WP-5-host-nonregistry-package-preflight-cap-replan`
Milestone: cross-stage M7 prerequisite implementation
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: freeze route-independent package policy and BUILD-marker preflight for
one materialized nonregistry module.

## Accepted predecessor boundary

Commit `054844d4` accepts the route-independent nonregistry MODULE-closure
design. Its live-owner audit ends `REPLAN` before Rust because exact include
package preflight is still route-bound.

`RepositoryMaterializationKey` and `RepositorySourceFileKey` already own
route-independent local/immutable source bytes. However,
`ExternalRepositoryPackageLookupKey`, `HostRouteRepositoryIgnoreKey`,
`HostRouteRepoFileKey`, and `HostRepositoryPathKey` all require a
`RootRepositoryRoute` carrying apparent/canonical repository names.
`HostRootPackageBoundaryKey` is main-repository-only. Reusing either path
would fabricate routing for a transitive override; bypassing them would lose
REPO.bazel `ignore_directories()`, .bazelignore, deleted-package policy,
BUILD.bazel-before-BUILD selection, symlink/kind handling, and first-error
ordering.

A further identity constraint is explicit: root package policy stores
`--deleted_packages` as `PackageIdentifier` values, while a discovered
module's final canonical repository name is a post-MVS product. The
preselection owner cannot guess `name+` or a multiple-version suffix.

## Accepted design contract

Design the smallest crate-private package-preflight owner identified by
normalized workspace, exact `NonrootModuleKey`, and `PackagePath`. It must
compute root files first, require the exact `NonRegistry(RepoSpec)`, and use
only `RepositorySourceFileKey` for `REPO.bazel`, `.bazelignore`,
`<package>/BUILD.bazel`, and `<package>/BUILD`.

Freeze a route-independent REPO.bazel semantic value over the existing
`evaluate_repo_file` semantics and event contract, plus a route-independent
repository-ignore matcher reusing the existing parser. Preserve validation and
terminal order: invalid package, package-policy input, deleted-package
classification where structurally knowable, repository-ignore evaluation,
ignored package, BUILD.bazel, BUILD, then no marker. All source values retain
existing Missing/wrong-kind/symlink/Need/error semantics and local versus
immutable invalidation.

Resolve the deleted-package identity boundary explicitly. Either identify an
accepted preselection representation that Bazel 9.2 uses before final canonical
mapping, or admit only a structurally proven empty/unambiguous external-deleted
policy and fail closed on every ambiguous entry until selected-graph identity
exists. Do not compare by guessed module name or fabricate a canonical repo.

The output is only a typed package classification and selected marker identity;
it does not return BUILD bytes, evaluate BUILD/Bzl, prepare MODULE fragments,
or activate discovery/MVS/consumers. Freeze exact Rust allowlist, formatted
caps, focused local/immutable A/B/A and REPO/.bazelignore/marker/Need/error/
event/reuse proofs, downstream validation, and independent review gate. Return
`REPLAN` if the existing source owner cannot express REPO/.bazelignore
semantics without duplicate IO or if deleted-package policy requires
post-selection mapping.

## Compatibility

Exact: Bazel 9.2 package-policy and BUILD-marker preflight for explicitly
admitted nonregistry source and deleted-policy shapes. Slug-native: DICE type
and diagnostic names, Host observation framing, compact storage, and non-Bazel
identity bytes. Unsupported/deferred: ambiguous external deleted-package
mapping, MODULE closure/evaluation consumption, command overrides, recursive
selection, post-selection mappings, BUILD/Bzl evaluation, toolchains, Test,
execution/results/BEP/coverage, unadmitted RepoSpecs, Windows, JVM/Java, and
exact Bazel identity bytes.

## Scope, proof, and stops

Edit only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`;
  and
- `thoughts/shared/plans/slug-v2-subplans/08-ruleset-and-command-conformance.md`.

Cap formatted net documentation growth at 230 lines. Add no Rust, Cargo/BUILD
metadata, fixture, asset, dependency, public surface, command behavior, or
production representation. Validate diff/scope/cap, active archive layout,
packet consistency, live owner and pinned-source citations, and independent
latest-diff review.

Stop with `REPLAN` on duplicate source/IO ownership, route/apparent/fabricated
canonical identity, lost REPO/.bazelignore/marker semantics, premature BUILD
bytes/evaluation, graph consumer, lock-held compute, JVM/Java, fifth file, or
cap excess.
Independent review accepts the route-independent package-preflight design.
The concrete seam needs no new file or public API. It composes the existing
source owner and pure evaluators through crate-private DICE keys, while
preserving the current route-bound owners unchanged for their accepted
consumers.

## Active REPLAN contract

The uncommitted implementation preserves the accepted architecture and passes
the full `slug_bzlmod_v2` suite (290 unit tests plus all integration tests).
Independent review found no semantic identity or ordering defect, but counted
about 440 formatted production lines against the frozen 360-line cap. A first
bounded boilerplate reduction did not recover the required margin without
collapsing the distinct REPO, ignore, and package-preflight DICE owners. The
implementation packet therefore ends `REPLAN` on its explicit cap stop. Its
Rust diff remains unaccepted and must not be committed under this packet.

Freeze only the corrected implementation evidence contract. Preserve the exact
five-Rust-file allowlist, all semantic requirements, the 520-test-line cap, and
all stops from the superseded implementation proposal. Raise only formatted
net production growth to 460 lines and total growth to 980 lines. The
correction must justify the measured split at the `#[cfg(test)]` boundary,
retain the completed equality/validity and no-route/no-canonical ownership
proof, and add the missing real-key immutable A/B/A, event/reuse,
Directory/source-error, and terminal-order cases before implementation review.

This design-only correction may edit only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`;
  and
- `thoughts/shared/plans/slug-v2-subplans/08-ruleset-and-command-conformance.md`.
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`, for the
  required single REPLAN row only.

Cap this correction at 90 formatted net documentation lines. Add no further
Rust change while this correction is active. Validate scope, cap arithmetic,
packet consistency, active archive layout, the recorded focused/full test
results, and independent correction review. On ACCEPT, schedule only
`WP-5-host-nonregistry-package-preflight-implementation-r2` with the corrected
caps and the same five Rust files. Stop on any semantic widening, sixth Rust
file, public surface, route/canonical identity, direct IO, BUILD evaluation,
consumer activation, or a second material correction.
