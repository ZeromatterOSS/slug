# Current Slug V2 Packet

Packet: `WP-6-7A-repository-source-file-observation-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling/Rust base: `ae8aa35e`
Accepted materialization design: `b2fd01e7`

## Exact docs authority

This design packet is docs-only. Write only:

1. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`: <=40 net lines.
2. `thoughts/shared/plans/slug-v2-subplans/current-packet.md`: <=220 net lines.
3. `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`: <=180 net lines.
4. `.codex/skills/slug-agent-orchestration/references/routing-log.md`: <=30 net lines.

Aggregate docs delta is <=470 net lines. Rust, tests, Cargo/BUILD metadata,
fixtures, oracles and every other plan are read-only during design.

## Accepted predecessor and frontier decision

Implementation `ae8aa35e` completes the private repository-materialization
sibling from Rust base `cc847c98` and design `b2fd01e7`. Final one-file
accounting is +168 production/+393 tests/+561 aggregate at 14,940 physical
lines. Focused 3/3, full bzlmod 436/436, loading 138/138 and full query
121/121 pass. Core remains 245/246 only on the inherited stale visibility
wording assertion. Formatting, diff, cleanup/retention and independent review
pass.

Do not freeze `HostSelectedModuleGraphKey` yet. Its nonregistry discovery path
uses `HostNonregistryModuleClosureKey`, while module-source preparation,
package preflight, REPO handling and repository-ignore handling all reuse the
carrierless `RepositorySourceFileKey`. That key validates the requested
repository-relative path, computes the accepted materialization owner, resolves
the materialized source path and reads FileBytes. Its current helper returns
semantics but no cumulative path epoch.

`HostRepositorySourceFileObservationKey` is route-based and cannot substitute
without reconstructing route/request identity. A detached observed repository
path key has no independent consumer because the source owner naturally owns
both resolution and FileBytes. Registry preparation separately crosses
`RegistryFileKey` plus patch-path resolution and remains a later frontier.
The uniquely smallest next reusable owner is therefore a crate-private observed
sibling of `RepositorySourceFileKey`.

## Frozen key, carrier and driver

Add crate-private structural
`RepositorySourceFileObservationKey(RepositorySourceFileKey)` and a distinctly
named `ObservedRepositorySourceFileValue` carrier. Keep the existing private
`ObservedRepositorySourceFile` enum as the compute-local resolved semantic
projection. The new carrier contains exactly:

- one local
  `Arc<Result<RepositorySourceFileValue, RepositorySourceFileError>>`; and
- one compact cumulative `PathObservationEpoch`.

The key and carrier implement `Allocative`; the carrier implements `Dupe`.
Expose only crate-private construction and borrowed result/epoch accessors for
later matching-family owners. Do not export either through `lib.rs` and add no
caller in this packet.

Use one Legacy/Observed repository-source driver. Validate the relative path
before any child. Legacy selects only `RepositoryMaterializationKey` and
`ResolvedPathKey`. Observed selects only
`RepositoryMaterializationObservationKey` and
`ResolvedPathObservationKey`. Both use the same neutral `PathObservationKey`
for the final FileBytes demand. Neither source sibling computes the other.
Project the legacy key's exact existing value/error shape while preserving the
exact nested bytes Arc; only the observed sibling retains the new local Result
Arc.

Refactor/reuse the existing resolved-source and stable shared-epoch append
helpers. Do not duplicate path resolution, symlink, node-kind or FileBytes
semantics.

## Exact order and terminal algebra

The order is relative-path validation -> materialization -> resolved path ->
FileBytes when the resolved node is a readable file.

Invalid relative path and materialization DICE compute failure are semantic
Complete with an empty epoch. Materialization Need or typed outer is immediate
and carrierless. Accept a completed observed materialization epoch before
semantic inspection; materialization semantic error retains that prefix and
suppresses resolution.

After materialization success, construct the Host or Materialization namespace
and requested path. Invalid materialized path and resolution DICE compute
failure retain only the materialization prefix. Resolution Need or typed outer
is immediate and carrierless. Union the materialization prefix left-first with
the complete resolved-path epoch before semantic inspection. Resolution
semantic error, Absent and WrongKind retain the merged prefix and activate no
FileBytes child.

FileBytes Need is immediate and carrierless. FileBytes DICE compute failure
retains the materialization+resolution prefix. For a Complete FileBytes result,
append its exact shared demand/result left-first before semantic inspection.
Present success, inconsistent Missing and observation Error retain the full
epoch. Equal duplicate demands preserve the earliest exact Arc; conflicting
values or operation mismatch return the existing typed
`ObservedPathFrontierError`. There is no joined batch or Need union.

Need is invalid/self-unequal. Complete typed outer is valid/equal by outer
value. Complete carrier is valid/equal by semantic Result plus epoch.

## Events, families and retention

The source parent, resolved-path owners and FileBytes owner remain eventless for
success, semantic error, Need, outer and cancellation. Accepted materialization,
request and root-MODULE descendants remain their sole existing event owners.
Warm reuse must not replay a child batch.

Legacy direct dependencies contain only legacy materialization/resolution plus
neutral FileBytes. Observed direct dependencies contain only observed
materialization/resolution plus neutral FileBytes. No package-preflight,
REPO-file, repository-ignore, module preparation, closure, discovery,
selected-graph, registry or public caller is activated.

The observed source carrier retains only its one local semantic Result Arc
(which may own the existing bytes Arc) plus the compact cumulative epoch.
Materialization/resolution carriers, requested/resolved paths, namespace,
source scratch and union scratch stay dependency-owned or compute-local. Add no
other carrier Arc, collection, cache, store, interner, lock, task, direct Host
read, revision, certificate or event state.

## Future Rust authority and caps

After independent design ACCEPT, future Rust authority is exactly:

1. `app/slug_bzlmod_v2/src/source_preparation.rs`, from 14,940 physical lines:
   <=300 production and <=30 colocated proof semantic lines, <=15,320 physical.
2. `app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs`, from 2,470
   physical lines: <=500 test semantic lines, <=3,020 physical.

Aggregate semantic growth is <=830 lines and combined physical size is
<=18,340. `source_preparation.rs` is a cohesive large-owner exception. Keep
every new or touched helper below 200 lines. A third Rust file, public export,
caller or cap excess is REPLAN.

## Required proof

Discriminate:

- distinct key identity/hash/Display, accessors, `Dupe`/`Allocative`, equality
  and validity for Need, typed outer and carrier;
- invalid relative path and every materialization, resolution and FileBytes
  compute/Need/outer-or-union/semantic position with exact prefixes and later
  suppression;
- materialization -> resolution -> FileBytes epoch membership/order and
  per-demand `Arc::ptr_eq`, stable duplicate first Arc, conflict and operation
  mismatch;
- local and immutable Present, Absent, WrongKind, FileBytes Missing/Error and
  success with exact bytes Arc;
- exact legacy value/error/order parity and exact nested bytes Arc;
- exact legacy and observed dependency-family rows with neutral FileBytes;
- child-owned events, parent silence, warm suppression, real poll-drop/no
  publication/same-DICE recovery;
- local and immutable edit/delete/directory/recreate plus A-B-A restoration
  with held semantic Result, bytes and epoch Arcs; and
- zero upper preparation/closure/discovery/selected-graph/registry/public
  activation.

Reuse accepted materialization/path/source proof and
`docs/developers/dice.md`. No new fixture or Bazel oracle is authorized.

## Compatibility

Exact: relative-path validation, materialization/source order, namespace and
symlink resolution, Absent/WrongKind/FileBytes semantics, source values/errors,
nested bytes Arc and legacy behavior. Slug-native: the structural sibling,
local Result Arc, compact epoch and typed outer. Unsupported/deferred:
observed package-preflight/REPO/ignore/module preparation and nonregistry
closure, registry file/source preparation and patches, discovered modules,
selected graph, extension evaluation/instantiation, generated repository
loading, rules_rust analysis/actions, M8/M7B and exact Bazel identity bytes.

## STOP and sole successor

STOP on Rust during design; a third future file; any caller or lib export;
upper preparation/closure/discovery/selected-graph or registry activation;
direct Host read; semantic, error, order, event or family drift; retained
scratch/state; cap excess; proof deletion; M7A closure; M8/M7B/M9; or a second
successor. REPLAN rather than reconstruct route identity, split an unused path
owner or weaken exact Arc validation.

After independent design ACCEPT, schedule exactly one
`WP-6-7A-repository-source-file-observation-implementation`. After independent
implementation ACCEPT, return only to the docs-only
`WP-6-7A-selected-module-graph-observation-frontier-design`.
