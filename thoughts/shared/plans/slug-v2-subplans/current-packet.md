# Current Slug V2 Packet

Packet: `WP-6-7A-repository-source-file-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Rust base: `ae8aa35e`
Accepted design: `9040e168`

## Exact Rust authority and caps

Write only:

1. `app/slug_bzlmod_v2/src/source_preparation.rs`, from 14,940 physical lines:
   <=300 production and <=30 colocated proof semantic lines, <=15,320 physical.
2. `app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs`, from 2,470
   physical lines: <=500 test semantic lines, <=3,020 physical.

Aggregate semantic growth is <=830 lines and combined physical size is
<=18,340. `source_preparation.rs` is a cohesive large-owner exception. Every
new or touched helper must stay below 200 lines. Every other file is read-only.

## Frozen implementation authority

Add crate-private structural
`RepositorySourceFileObservationKey(RepositorySourceFileKey)` and distinctly
named `ObservedRepositorySourceFileValue`. Preserve the existing private
`ObservedRepositorySourceFile` enum as the compute-local resolved semantic
projection. The new carrier contains exactly one local
`Arc<Result<RepositorySourceFileValue, RepositorySourceFileError>>` plus one
compact cumulative `PathObservationEpoch`. The key and carrier implement
`Allocative`; the carrier implements `Dupe`. Expose only crate-private
construction and borrowed result/epoch accessors. Do not export through
`lib.rs` or add a production caller.

Use one Legacy/Observed repository-source driver. Validate the relative path
before any child. Legacy selects only `RepositoryMaterializationKey` then
`ResolvedPathKey`. Observed selects only
`RepositoryMaterializationObservationKey` then
`ResolvedPathObservationKey`. Both use the same neutral `PathObservationKey`
for FileBytes. Neither source sibling computes the other.

Reuse/refactor the existing resolved-source and stable shared-epoch helpers.
Preserve exact relative-path, materialization, namespace, symlink, path-kind and
FileBytes semantics. Project the legacy key's exact existing unwrapped
value/error shape and preserve the nested bytes Arc; only the observed sibling
retains the new local Result Arc.

## Order and terminal algebra

The exact order is relative-path validation -> materialization -> resolved path
-> FileBytes when the resolved node is readable.

Invalid relative path and materialization DICE compute failure are semantic
Complete with an empty epoch. Materialization Need or typed outer returns
immediately with no carrier and suppresses resolution. Accept a completed
observed materialization epoch before semantic inspection; its semantic error
retains that prefix.

After materialization success, construct the existing Host or Materialization
namespace and requested path. Invalid materialized path and resolution DICE
compute failure retain the materialization prefix. Resolution Need or typed
outer returns immediately with no carrier. Union the materialization prefix
left-first with the complete resolved-path epoch before semantic inspection.
Resolution semantic error, Absent and WrongKind retain that merged prefix and
suppress FileBytes.

FileBytes Need returns immediately with no carrier. FileBytes DICE compute
failure retains the materialization+resolution prefix. On Complete, append the
exact shared FileBytes demand/result to the existing prefix before semantic
inspection. Present success, inconsistent Missing and observation Error retain
the full epoch.

Equal duplicate demands preserve the earliest exact Arc. Conflicting values or
operation mismatch return the existing typed `ObservedPathFrontierError`.
There is no joined batch or Need union. Need is invalid/self-unequal; Complete
typed outer is valid/equal by outer value; Complete carrier is valid/equal by
semantic Result plus epoch.

## Events, families and retention

Source parent, resolved-path owners and FileBytes owner remain eventless on
success, semantic error, Need, outer and cancellation. Accepted materialization,
request and root-MODULE descendants remain their sole event owners. Warm reuse
does not replay a child batch.

Legacy direct dependencies contain only legacy materialization/resolution plus
neutral FileBytes. Observed direct dependencies contain only observed
materialization/resolution plus neutral FileBytes. No package preflight,
REPO-file, repository-ignore, module preparation, closure, discovery,
selected-graph, registry or public caller is activated.

The observed source carrier retains only its local semantic Result Arc (which
may own the existing bytes Arc) plus the compact cumulative epoch.
Materialization/resolution carriers, requested/resolved paths, namespace,
source and union scratch remain dependency-owned or compute-local. Add no other
carrier Arc, collection, cache, store, interner, lock, task, direct Host read,
revision, certificate or event state.

## Required proof

Discriminate:

- distinct key identity/hash/Display, accessors, `Dupe`/`Allocative`, equality
  and validity for Need, typed outer and carrier;
- invalid relative path and materialization compute/Need/outer/semantic
  terminals with exact empty/full prefixes and later suppression;
- invalid materialized path and resolution compute/Need/outer/semantic,
  Absent/WrongKind terminals with exact prior/merged prefixes;
- FileBytes compute/Need/Present/Missing/Error terminals, exact prior/full
  prefixes and carrierlessness;
- materialization -> resolution -> FileBytes epoch membership/order and every
  per-demand `Arc::ptr_eq`;
- stable duplicate first Arc, conflicting value and operation mismatch;
- local and immutable source namespaces, Present/Absent/WrongKind and exact
  bytes Arc;
- exact legacy value/error/order parity and nested bytes Arc preservation;
- exact legacy and observed dependency-family rows with neutral FileBytes;
- child-owned events, parent silence, warm suppression and no batch on
  Need/outer/cancel;
- real poll-drop/no-publication/same-DICE recovery;
- local and immutable edit/delete/directory/recreate plus A-B-A restoration
  with held semantic Result, bytes and epoch Arcs; and
- zero upper preparation/closure/discovery/selected-graph/registry/public
  activation.

Reuse accepted materialization/path/source proof and
`docs/developers/dice.md`. No fixture or Bazel oracle is authorized. Run
focused owner proof, full bzlmod, affected loading/query/core baselines, fmt,
diff-check, exact cap accounting and AI-cleanup/Buck2 retention review.

## Compatibility

Exact: relative-path validation, materialization/source order, Host versus
Materialization namespace, symlink resolution, Absent/WrongKind/FileBytes
semantics, source values/errors, nested bytes Arc and all legacy behavior.
Slug-native: the structural sibling, local Result Arc, compact epoch and typed
outer. Unsupported/deferred: observed package-preflight/REPO/ignore/module
preparation and nonregistry closure, registry file/source preparation and
patches, discovered modules, selected graph, extension evaluation/
instantiation, generated repository loading, rules_rust analysis/actions,
M8/M7B and exact Bazel identity bytes.

## STOP and sole successor

STOP on a third file; any caller or lib export; upper preparation/closure/
discovery/selected-graph or registry activation; direct Host read; semantic,
error, order, event or family drift; retained scratch/state; cap excess; proof
deletion; M7A closure; M8/M7B/M9; or a second successor. REPLAN rather than
reconstruct route identity, split an unused path owner, weaken exact Arc
validation or compress required proof beyond discrimination.

After independent implementation ACCEPT, schedule only the docs-only
`WP-6-7A-selected-module-graph-observation-frontier-design`.
