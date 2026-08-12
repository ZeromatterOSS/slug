# Current Slug V2 Packet

Packet: `WP-5-host-selected-extension-mapping-owner-design-r2`
Milestone: cross-stage M7 prerequisite design
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: audit and freeze the first additive selected-extension mapping owner,
or return `REPLAN` at the first missing post-selection semantic leaf.

## Active design contract

Perform a read-only ownership and pinned Bazel 9.2 audit now that commit
`11be92b9` retains complete root extension usages. Determine whether one
callerless private Host owner can compose:

- `HostSelectedModuleRoutesKey`, including each selected module's exact
  canonical identity and contextual dependency mapping;
- root usages from `RootModuleFilesKey`; and
- nonroot usages already retained inside each discovered selected module.

The audit must pin the exact ordering and identity for resolving every usage's
bzl label through its owning module context, grouping nonisolated usages,
forming isolated identities, constructing Bazel collision-safe unique
extension names, projecting local/exported imports, and applying root
`override_repo`/`inject_repo` semantics. It must distinguish what can be
known before extension evaluation from generated-repository names and
existence that require evaluation.

Freeze structural equality, complete-error-over-Need ordering, DICE validity,
compact retained representation, and A/B/A/cold-warm proof. Classify every
surface as exact, Slug-native, or unsupported/deferred. End by freezing one
explicit implementation successor with at most three Rust files, caps,
proofs, and terminal stops, or `REPLAN` into the first smaller prerequisite.

This packet may edit only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest; and
- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`.

Cap formatted net growth at 320 manifest lines, 320 owner-plan lines, 45
canonical lines, and 685 total. Read-only inspection may cover pinned Bazel
9.2 source, the accepted selected graph/registry-spec/route owners, root and
nonroot MODULE evaluators, and checked-in extension/mapping oracle evidence.
Obtain fresh independent reserved-architecture review.

No Rust, Cargo/BUILD, fixture mutation, public API, legacy graph/catalog,
second selected graph/route, extension evaluation, repository rule execution,
generated RepoSpec/repository materialization, lockfile/final-module
publication, loading, consumer, command, analysis, execution, or JVM/Java work
is authorized. Return `REPLAN` if exact ownership needs an absent
usage/identity/ordering/override/generated-name leaf, extension execution or
I/O, another graph, a public seam, more than three future Rust files, or cannot
preserve complete-error-over-Need semantics. Return `REVISE` on one bounded
design correction; a second material correction is `REPLAN`. No production
representation may begin before independent `ACCEPT` and explicit
implementation activation.

## Accepted predecessor evidence

This evidence is historical and grants no file, action, cap, or scheduling
authority.

Commit `11be92b9` accepts the private root extension-usage owner at 386
production, 327 tests, and 713 total formatted net lines, within the frozen
520/750/1,270 caps. One shared evaluator state now serves root and nonroot
MODULE evaluation while preserving their distinct dev and override semantics.
Root/include evaluation retains Arc-backed ordered usages, normalized bzl
labels, proxy bindings and logical locations, import bijections, isolation
exports, ordered tags, root override/inject `must_exist`, and synthetic
repo-rule usages only on the private root evaluation result and
`RootModuleFiles`.

`EvaluatedRootModule`, `RootModuleGraph`, the selected graph/routes, public
exports, extension execution, and consumers remain unchanged. Six focused
root tests include real-DICE A/B/A and warm reuse; all 339 owner unit tests and
every integration suite pass; the complete direct-loading suite passes. The
pinned eight-file Bazel 9.2 root fixture and six protected extension/mapping
fixtures pass. AI cleanup and independent review accept the compact one-file
implementation. Archive content checks pass; the local clone still lacks the
known V1 archive tag/branch baseline.

The earlier selected-extension audit correctly stopped because root usages
were absent. That prerequisite is now closed, but this packet does not assume
that generated-repository mappings are derivable before extension evaluation.
