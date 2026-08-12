# Current Slug V2 Packet

Packet: `WP-5-host-selected-extension-mapping-owner-implementation-r3`
Milestone: cross-stage M7 selected Bzlmod semantic owner
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: retain the complete pre-evaluation selected extension identity and
contextual repository-mapping projection.

## Active implementation contract

Implement exactly the independently accepted corrected owner in
`app/slug_bzlmod_v2/src/selected_repo_spec.rs`. This is the sole authorized
file. Cap formatted net growth at 520 production lines, 800 test lines, and
1,320 total relative to `2644f091`.

Compute `HostSelectedModuleRoutesKey` first. A route Need is invalid and a
completed route error is terminal before any root-file work. Then borrow
`RootModuleFilesKey` for the source-ordered root usage slice and take
nonroot usages only from the selected discovered route sources. Use only
resolved selected entries; unpruned graph entries do not own selected usage
membership.

Retain one compact private result containing:

- the accepted selected routes and complete root usage input for structural
  equality and invalidation;
- selected usage owners in route order, source-ordered extension IDs,
  collision-safe unique names, proxy imports, root override/inject intent,
  and complete contextual repository mappings;
- canonical IDs formed from each bzl label resolved through that selected
  owner's accepted dependency mapping, extension name, and optional isolation
  identity (root proxy name or nonroot selected module key plus proxy name);
- `must_exist` as structural override identity without claiming generated
  repository existence.

Use the corrected two-phase algorithm:

1. resolve every selected usage ID and first-encounter unique name, then build
   every selected module's full no-overrides mapping by adding all proxy
   imports to the accepted dependency-only mapping;
2. resolve every root override/inject target through the completed root
   no-overrides mapping with an empty override table, then build final full
   mappings by substituting those already resolved canonical targets for the
   overridden generated destinations.

Preserve source/route order and return typed projection errors for unresolved
bzl labels, isolation/export defects, unique-name exhaustion, no-overrides
mapping conflicts, invisible override targets, and final mapping conflicts.
Completed predecessor errors win over Need; Need unions remain invalid and
non-self-equal. Slug diagnostic wording and collision-safe internal identity
bytes are Slug-native. Selected membership, label resolution, source order,
extension grouping/isolation, imports, override target resolution, and final
mapping entries are exact for the admitted Bazel 9.2 pre-evaluation slice.

## Required proof

Add colocated pure and real-DICE tests discriminating:

- root and nonroot ordinary aggregation, distinct isolated identities, two MVO
  owner contexts, innate rules, aliases, and first-encounter name collisions;
- no-overrides imports from another extension as valid override targets;
- duplicate imports, missing targets, chains/cycles, final replacement, and
  distinct typed predecessor/projection errors;
- route-error-before-root-work and complete-error-over-Need ordering;
- graph/root usage/edit/remove/restore A/B/A, cold/warm reuse, Need invalidity,
  and semantic equality;
- default ignore-dev behavior now fixed by `2644f091`;
- structural absence of unpruned graph consumption, a second route/usage
  owner, extension evaluation, registry/materialization/file/network I/O,
  loading/consumer edges, public exports, locks, or raw filesystem access.

Run focused tests, full `slug_bzlmod_v2`, full `slug_loading_v2`,
formatting/diff/cap/scope checks, compact-representation and AI-cleanup audits,
and fresh independent implementation review.

A second file, public API, predecessor mutation, another graph/route/usage
owner, extension evaluation or generated-repository existence validation,
RepoSpec/I/O/materializer/loading/consumer edge, command/analysis/execution
work, JVM/Java, or any cap excess is `REPLAN`. One bounded defect is
`REVISE`; a second material correction is `REPLAN`.

## Accepted predecessor evidence

This section is historical and grants no separate file, action, cap, or
scheduling authority.

Commit `11be92b9` owns root extension usages. Commit `2644f091` corrects
pinned Bazel 9.2 root semantics: ignored-dev `use_repo` still reserves names,
root override/inject are globally suppressed when dev dependencies are
ignored, and finalization validates replacement visibility, inject/import
conflicts, and overriding/overridden chains. Its 340 owner tests, all
integrations, full loading suite, formatting/diff/cap/cleanup checks, real-DICE
failure/restoration row, and independent review pass.

The r2 mapping implementation was stopped before Rust. Pinned Bazel
`BazelDepGraphFunction.resolveRepoOverrides` proved that deps-only target
resolution was wrong. The independently accepted r3 correction above matches
the checked-in root extension fixture: an innate extension import may be the
target replacing another extension repository. Generated-name existence,
extension evaluation, generated RepoSpecs/internal mappings, lockfile/final
module products, materialization, loading, and consumers remain deferred.
