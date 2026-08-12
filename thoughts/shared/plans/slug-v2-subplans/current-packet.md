# Current Slug V2 Packet

Packet: `WP-5-host-nonregistry-discovered-module-owner-design`
Milestone: cross-stage M7 prerequisite design
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: freeze the missing Host nonregistry discovered-module owner before
recursive discovery and MVS.

## Accepted predecessor boundary

Commit `e7e4a772` accepts the callerless `HostDiscoveredModuleKey` for the
unoverridden embedded `bazel_tools@<empty>` module and versioned registry
modules. The key computes root files before source category, bypasses the
embedded key for explicit overrides, retains the complete evaluated module and
typed built-in or selected-registry ordered attempt/hash provenance, and keeps
Need transient. Focused real-DICE lifecycle tests prove override bypass with
zero built-in activations, registry A/B/A semantic restoration, cold evaluated
capture and warm reuse. The full bzlmod crate, downstream loading/core checks,
formatting, caps, and independent review passed. It adds no consumer.

The selected-graph design still cannot proceed. Root `NonRegistry(RepoSpec)`
overrides may target dependencies that are not directly apparent from the root,
and cover local, archive, Git, and other repository rules. The existing
`DirectLocalModuleEvaluationKey` is keyed by a root apparent name, calls
`RootRepositoryRouteKey`, accepts only direct `local_path_override`, and
returns a support projection to loading. It is not a general discovery owner.
`ModuleSourcePreparationKey` can prepare nonregistry root bytes through
materialization, but its legacy path does not compose the complete include
closure/provenance needed by the accepted evaluator. Command
`--override_module` normalization remains separately absent.

## Active design contract

Audit pinned Bazel 9.2 nonregistry override rewriting, repository rule
materialization, MODULE/include preparation, expected-key validation, and
discovery provenance. Audit Slug's root override map, repository
materialization/source keys, direct-local include/preparation/evaluation
owners, and the admitted Host discovered-module leaf.

Freeze the smallest sole nonregistry module-value owner, or return `REPLAN`
at its first missing prerequisite. The design must determine whether one key
can be identified by normalized workspace, exact `NonrootModuleKey`, and
the structurally retained root `RepoSpec`, without routing through a root
apparent repository name. It must preserve complete RepoSpec/category identity,
materialization generation/provenance, exact root and include bytes, logical
source identities/spans, complete evaluator result, typed Need/errors, and
DICE invalidation.

Classify separately direct local, fixed archive, Git, and unsupported
repository-rule shapes. Reuse the accepted preparation/evaluator owners where
truthful; do not duplicate Host reads, materialization, include traversal, or
evaluation, and do not widen `RootRepositoryRouteKey`. State whether command
overrides must be normalized before this owner or remain a later overlay.
Freeze an explicit implementation allowlist, production/test/total caps,
serial A/B/A/error/Need evidence, downstream validation, and independent review
gate.

## Compatibility

Exact: Bazel 9.2 root nonregistry override category and complete MODULE
semantics for any individually admitted repository-rule shapes. Slug-native:
DICE type/diagnostic names, compact storage, Host observation framing, and
non-Bazel identity bytes. Unsupported/deferred: unadmitted repository rules,
command overrides unless structurally added by a later packet, recursive
discovery/MVS, mappings/extensions/registrations, RepoSpec/yanked/hash
aggregation, lockfile writing, package/BUILD/Bzl loading, configured
toolchains, Test, command consumers, execution/results/BEP/coverage, Windows,
JVM/Java, and exact Bazel identity bytes.

## Scope, proof, and stops

Edit only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`;
  and
- `thoughts/shared/plans/slug-v2-subplans/08-ruleset-and-command-conformance.md`.

Cap formatted net documentation growth at 240 lines. Add no Rust, Cargo/BUILD
metadata, asset, fixture/oracle record, generated file, dependency, public
surface, command behavior, or production representation.

Validation is `git diff --check`, exact-scope/net-line checks, active-layout
archive validation, packet-name consistency, pinned-source/live-owner
citations, and independent latest-diff review. Stop with `REPLAN` on missing
complete include/provenance identity, duplicated materialization or evaluator
ownership, apparent-name routing for transitive modules, fabricated RepoSpec or
canonical mapping, second graph, untracked IO, lock-held DICE compute, public
consumer, JVM/Java, fifth file, or cap excess.
