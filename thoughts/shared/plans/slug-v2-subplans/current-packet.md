# Current Slug V2 Packet

Packet: `WP-2A-m1-repository-package-load-observation-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling/Rust base: `f1d01834` / `93f43264`
Result: freeze only the observed repository-package-load sibling and its
callerless cross-crate seam before implementation.

## Authority and owner

Write only canonical/current/Stage/routing docs within 40/180/180/30 and 430
aggregate net lines. Rust, Cargo/BUILD, fixtures, oracles, generated artifacts,
caller/public activation and M1 closure are stopped.

`RepositoryPackageLoadKey` is the uniquely smallest complete natural owner. It
alone owns selected package source, BUILD decode/parse and full load-label
prevalidation, sequential AST-order external children, synchronous package
attempt/postvalidation, one local semantic Result Arc and the sole local BUILD
batch. `evaluate_host_package_attempt` is compute-local and has no DICE/event
identity; extracting it would split source/module lifetime and event authority.
Accepted observed package-source and external-Bzl carriers cover every lower
mutable edge. No smaller prerequisite or `REPLAN` is required.

## Frozen future implementation

Future Rust after independent ACCEPT is exactly:

- `app/slug_loading_v2/src/bzl_module.rs`: +360 production, <=6,955 physical;
- `app/slug_loading_v2/src/lib.rs`: +4 export lines, <=86 physical;
- `app/slug_loading_v2/src/host_package_load_tests.rs`: +560 tests,
  <=3,438 physical.

Aggregate semantic growth is <=924 and combined physical size <=10,479 against
`93f43264`. Keep each touched helper below 200 lines; the existing production
and proof files are cohesive owner exceptions.

Add structural `RepositoryPackageLoadObservationKey(RepositoryPackageLoadKey)`
and `ObservedRepositoryPackageLoad`, doc-hidden public only for later query/core
consumers, with read-only result/epoch accessors. The carrier is exactly one
local `Arc<Result<LoadedPackage, RepositoryPackageLoadError>>` plus one compact
`PathObservationEpoch`. Export the key/carrier through `lib.rs`, but activate no
caller.
Its exact value is `SourcePreparationOutcome<Result<
ObservedRepositoryPackageLoad, ObservedPathFrontierError>>`.

Use one Legacy/Observed package driver. Legacy selects only
`RepositoryPackageSourceKey` and `ExternalBzlModuleEvalKey`; observed selects
only their observed siblings. Neither computes the other. Preserve source ->
UTF-8/parse -> complete label prevalidation -> direct children in AST order ->
one synchronous attempt -> post-load validation. Project the exact driver
Result Arc to legacy.

Observed begins with the source epoch. Merge every Complete child epoch
left-first before semantic inspection; equal duplicates keep the first exact
Arc and conflict/operation mismatch is typed outer. Source DICE failure is
semantic `SourceCompute` with empty prefix. Source semantic, encoding, parse and
load-label terminals keep source. Child DICE failure keeps prior; child semantic
keeps merged. Glob-unsupported, evaluation, postvalidation and success keep the
full reached epoch. Need/typed outer is immediate and carrierless, activates no
later child, stores no parent batch and is never unioned.

Each sibling stores exactly its matching local semantic-Complete BUILD batch,
including empty and error-prefix batches. Source remains eventless; recursive
external children remain their own batch owners and precede the package.
Need/outer/cancellation stores none. Need is invalid/self-unequal; Complete
outer equality is by outer value and carrier equality by semantic Result+epoch.

Retain only the existing `LoadedPackage` semantic graph in the local Result Arc
plus compact epoch. Its targets, direct/reachable identities and frozen-module
lifetime closure remain exact payload. BUILD bytes/string/AST, resolved-load
and loaded-module vectors, attempt/evaluator/prepared-map, event and union
scratch remain compute-local. Add no child/source carrier Arc, collection,
cache/interner/lock/task, Host read, revision or certificate.

## Proof, compatibility and terminal

Prove distinct identity/hash/Display and hidden accessors; exact legacy
Result-Arc/value/error/event parity; source and first/middle/last child Need,
outer and semantic positions with exact prefixes and later suppression;
duplicate first Arc/conflict/mismatch; encoding/parse/load-label/evaluation/
glob/postvalidation/success; exact child-before-parent batch text/order,
empty/error prefixes and warm suppression; both family directions/concurrent
roots with zero query/core/public activation; poll-drop recovery; BUILD and
recursive `.bzl` edit/delete/recreate/A-B-A; exact bytes/manifest lifetime;
Allocative retention, cleanup and exact accounting.

Exact: BUILD/package values/errors/load order/local events and legacy behavior.
Slug-native: sibling/carrier/typed outer/epoch and doc-hidden seam. Deferred:
query/core cutover, external glob support, exported-source FileBytes/certificate
aggregation, public publication and exact identity bytes.

STOP on another file/caller, upper activation, family/event/terminal drift,
partial epoch, reconstructed Result Arc, retained scratch, helper/cap excess or
M1 closure. `REPLAN` if the complete package carrier needs another owner or
state. After independent design ACCEPT, schedule exactly one bounded
implementation; afterward return only to a docs-only upper-owner audit.
