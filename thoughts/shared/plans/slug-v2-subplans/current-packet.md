# Current Slug V2 Packet

Packet: `WP-2A-m1-repository-package-load-observation-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Rust base: `93f43264`
Accepted design: `a342a2c2`
Result: implement and validate only the callerless observed repository-package
load sibling.

## Authority and caps

Write exactly:

- `app/slug_loading_v2/src/bzl_module.rs`: +360 production, <=6,955 physical;
- `app/slug_loading_v2/src/lib.rs`: +4 export lines, <=86 physical;
- `app/slug_loading_v2/src/host_package_load_tests.rs`: +560 tests,
  <=3,438 physical.

Aggregate semantic growth is <=924 and combined physical size <=10,479 against
`93f43264`. Keep touched helpers below 200 lines; the two existing large files
are accepted cohesive owner/proof exceptions.

## Required implementation

Add doc-hidden public structural `RepositoryPackageLoadObservationKey` and
`ObservedRepositoryPackageLoad` with read-only accessors and export only those
two names through `lib.rs`. Its exact value is
`SourcePreparationOutcome<Result<ObservedRepositoryPackageLoad,
ObservedPathFrontierError>>`; the carrier retains exactly one local package
Result Arc plus one compact epoch. Activate no caller.

Use one Legacy/Observed driver. Legacy selects only
`RepositoryPackageSourceKey` and `ExternalBzlModuleEvalKey`; observed selects
only their observed siblings. Preserve source -> UTF-8/parse -> complete label
prevalidation -> direct children in AST order -> synchronous package attempt ->
postvalidation. Move the exact driver Result Arc to legacy.

Observed starts with source observations and merges every Complete child epoch
left-first before semantic inspection. Equal duplicates keep the first Arc;
conflict/operation mismatch is typed outer. Source DICE failure is semantic
`SourceCompute` with empty prefix. Source semantic/encoding/parse/load-label
keeps source; child DICE failure keeps prior; child semantic keeps merged;
glob/evaluation/postvalidation/success keeps full reached epoch. Need/typed
outer is immediate, carrierless, activates no later child, stores no parent
batch and is not unioned.

Each sibling stores only its matching semantic-Complete local BUILD batch,
including empty/error prefixes. Source remains eventless and recursive child
batches precede the package. Need/outer/cancel stores none. Need is invalid and
self-unequal; Complete outer equality is by outer value and carrier equality by
semantic Result+epoch.

Retain only the existing LoadedPackage semantic graph in the local Result Arc
plus compact epoch. Keep BUILD bytes/string/AST, resolved-load/loaded-module
vectors, attempt/evaluator/prepared-map, event and union scratch compute-local.
Add no child/source or other additional carrier Arc, collection/cache/interner/
lock/task, Host read, revision, certificate or event owner.

## Proof and terminal

Prove identity/hash/Display/accessors; exact legacy Arc/value/error/event parity;
source and first/middle/last child Need/outer/semantic exact prefixes and later
suppression; duplicate first Arc/conflict/mismatch; all BUILD parse/evaluation/
postvalidation terminals; exact child-before-parent empty/error/success batches
and warm suppression; both family directions/concurrent roots and zero upper
activation; poll-drop recovery; BUILD and `.bzl` A-B-delete-recreate-A; exact
bytes/manifest lifetime; retention/Allocative, cleanup and accounting.

Exact compatibility is BUILD/package semantics, load order, local events and
legacy behavior. Sibling/carrier/outer/epoch/export seam is Slug-native. Query/
core cutover, external glob support, exported-source certificate aggregation,
public publication and exact identity bytes remain deferred.

Run focused package-load proof, full loading, direct bzlmod/query dependents,
fmt, diff-check and caps serially; require retention/cleanup and independent
review. STOP on any other file/caller, upper activation, semantic/family/event/
epoch drift, retained scratch, helper/cap excess or M1 closure. REPLAN if the
complete carrier needs another owner/state. After ACCEPT commit and return only
to a docs-only upper-owner audit.
