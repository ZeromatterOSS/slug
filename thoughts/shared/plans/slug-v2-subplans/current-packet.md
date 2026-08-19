# Current Slug V2 Packet

Packet: `WP-2A-m1-external-singleton-observed-build-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `1a217e2a`
Accepted Rust base: `a4dd40d6`
Accepted design: `1a217e2a`
Result: publish exactly one nonroot exported-source build through the existing
observed build root.

## Exact Rust authority and caps

Write exactly:

1. `app/slug_core_v2/src/runtime/dice.rs`: <=260 production net and <=11,220
   physical; and
2. `app/slug_core_v2/src/runtime/tests/build_command_tests.rs`: <=360 test net
   and <=3,350 physical.

Aggregate semantic <=620 and combined physical <=14,570 against
`a4dd40d6`. No relocation or third Rust file. Cargo manifests, BUILD,
fixtures, oracles, generated evidence, exports, other callers and planning docs
are forbidden during implementation. The large files remain cohesive owner
exceptions; every new or materially touched helper stays below 200 lines.

## Frozen owner, identity and driver

Keep the same structural `BuildCommandRootObservationKey(BuildCommandRootKey)`.
Its constructor admits the existing singleton root PackageAll plus every
syntactic nonroot Single. External wrong-kind rule/filegroup requests enter
observed and fail only after package classification. Every root Single
(including root rules/filegroups), multi-target and other identity preserves
the current neutral/legacy route. Keep the sole public observed -> neutral ->
legacy constructor order and add no key or public seam.

Keep the existing PackageAll observed driver unchanged. Refactor the existing
external branch into one private mode-aware semantic driver. Legacy generic
build selects only legacy route/package/source children; observed external
build selects only their observed siblings. Neither mode activates the other.
Target classification, diagnostics and semantic projection remain one shared
path.

Observed order is:

1. root loading anchor;
2. repository route;
3. repository package load;
4. target lookup and exact ExportedFile classification;
5. existing RequestRevisionKey dependency; then
6. selected repository source.

The anchor remains root-owned; route/package/source are branch-owned. Compute
request revision only after exported-source classification and before source.
Missing/wrong-kind targets never activate revision or source.

Validate and union every completed observed child epoch left-first before
semantic inspection. Equal duplicates keep the first exact Result Arc.
Conflict/operation mismatch is typed `ObservedPathFrontierError`. Need or
typed outer is immediate and carrierless with no later activation; this
sequential owner performs no Need union.

Freeze prefixes: anchor compute failure empty, anchor semantic anchor; route
compute anchor, route semantic anchor+route; package compute anchor+route,
package semantic anchor+route+package; target/revision failure
anchor+route+package; source compute anchor+route+package; source
Present/Absent/semantic/directory WrongKind and success full
anchor+route+package+source. Success is exactly one loaded-only exported-source
target, no analysis and empty action closure.

## Certificate, publication and events

Every terminal after a completed source child retains a `SourceCertificate`
made from the child's entire observed epoch. Present, Absent, accepted directory
WrongKind and source semantic error retain it; earlier terminals retain none.
The certificate is an exact demand/value/Arc subset of the full observed root
epoch. Private error storage may change only to expose that certificate; exact
formatting, terminal codes, values and equality stay unchanged.

For admitted external Single only, the observed root exposes its full epoch and
semantic certificate through `NativeCommandRoot`, initializes request
revision, and selects `ClosureRepositories`. PackageAll remains strict-empty.
`selected_snapshot` remains the sole repository request/result/validation
constructor; full path demand/value/Arc comparison remains unconditional and no
repository collection enters the terminal.

Finalization reobserves the complete certificate through the active
materializer. Equal demands preserve exact Arcs; changed demands alone publish
a revision and retry. Need, outer, cancel, association/selection/materializer/
revision failure and abort preserve prior selected path/repository/event state
and publish no provisional output.

The observed root owns no event batch. Matching anchor/module/package/source
children remain sole owners. Preserve exact cold child order/text,
semantic-error batches and warm suppression. A root observed build after an
external observed build must not replay the shared root anchor event; add no
event special case.

## Retention and compatibility

Retain exactly one local
`Arc<Result<BuildCommandEvaluation, BuildCommandError>>` plus one compact full
epoch. The semantic Result may retain one compact source certificate epoch
sharing exact child Arcs. Child carriers, route/package/source outcomes,
selected path and union/event scratch remain compute-local or dependency-owned.
Accepted selected repository/path/event epochs remain unchanged.

Add no map, Vec, cache, store, interner, lock, task, direct Host read, revision
duplicate, event owner, fallback or historical filesystem snapshot.

Exact: public external source values/errors/classification, BUILD/module event
text/order, root PackageAll, root Single, multi-target and every legacy/direct
API.

Slug-native: observed admission, epoch/carrier/typed outer association,
external-only closure repository selection and private certificate attachment.

Unsupported/deferred: multi-build aggregation, one-shot cutover, broader build
analysis/actions, external globs and exact Bazel identity bytes.

## Required proof, validation and STOP

Prove:

- identity/Display/equality/validity and exact PackageAll/neutral/legacy routing;
- exact shared-driver legacy parity and both-direction family isolation;
- anchor->route->package->source epoch order/membership and per-demand
  `Arc::ptr_eq`, duplicate first Arc, conflict and operation mismatch;
- every Need/outer/compute/semantic prefix and later-child nonactivation;
- Present, Absent, directory WrongKind, source error, missing target and
  wrong-kind parity, revision ordering and exact certificate presence;
- local-override external acceptance with nonempty selected repository requests
  and validations, exact selected path Arcs, plus strict-empty PackageAll;
- Host+Materialization symlink/create/edit/delete/directory/recreate A/B/A,
  exact bytes/manifest/result lifetimes and revision retry;
- exact cold/error event order, warm suppression, external->root no replay and
  no parent batch;
- concurrent observed/legacy roots, zero multi-build/one-shot/query activation,
  pointer-distinct abort, injected failures, real poll-drop cancellation, no
  publication and same-DICE recovery; and
- exact accounting, fmt/diff, focused build/revision/server, full core baseline,
  archive, Buck2 retention, AI cleanup and independent final review.

STOP on any other file, new key/state/event owner, lower-carrier or public API
change, partial validation, strict-root selection relaxation, behavior/family/
order drift, retained scratch, direct Host read, cap excess, multi-build/
one-shot activation or M1 closure. REPLAN if repository sidecars require
terminal retention, the exact source epoch cannot be certified, or this
two-file owner cannot preserve exact behavior. After ACCEPT return to one
docs-only remaining M1 owner audit.
