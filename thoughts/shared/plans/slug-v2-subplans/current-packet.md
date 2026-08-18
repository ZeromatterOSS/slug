# Current Slug V2 Packet

Packet: `WP-2A-m1-root-package-all-build-publication-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Accepted implementation: `95002997`
Accepted design: `857ef363`
Result: publish the observed singleton root-package-all build carrier through
the existing native acceptance and public return boundary.

## Authority

Write only:

- `app/slug_core_v2/src/runtime/dice.rs`; and
- `app/slug_core_v2/src/runtime/events.rs`.

Against `95002997`, caps are 150 production plus 300 test net lines and 14,151
physical lines for `dice.rs`; 16 production plus 30 test net lines and 1,640
physical lines for `events.rs`; and 496 aggregate net Rust lines.

## Required implementation

Implement `NativeCommandRoot` for the private observed build key and add a
private default optional terminal-observations hook overridden only by that
sibling. Need remains Need; completed observed outer error becomes typed native
session failure; semantic errors remain in the carrier Result Arc. Add no
request revision, source certificate or root relaxation.

Construct the selected path epoch with `PathObservationEpoch::from_shared`.
Immediately after successful `prepare_accept` and before revision finalization
or snapshot commit, validate the complete carrier and selected epochs in
canonical demand order: equal length and demand keys, semantic Result equality
and exact `Arc::ptr_eq`. Require empty selected repository request and
validation sets. Every mismatch fails closed without panic, snapshot or event.

Add a private infallible consuming `AcceptedCommand::map_terminal` that moves
the exact semantic Arc, preserves events and drops the carrier epoch. Select
observed only after the existing public constructor admits structurally exact
singleton root-repository `PackageAll`; every other identity remains on the
identical legacy key/driver.

Retain only the public semantic Result Arc/events and existing accepted compact
Arc-backed path epoch. Carrier, comparison and attempt scratch stay local. Add
no Host read, lock, task, cache, interner, collection, graph or event owner.

## Required proof

Cover success and semantic-prefix exact-Arc equality; pointer-distinct equal
values and missing/extra/value mismatches; Need and observed outer abort;
cancellation and selection/injection/materializer failure without publication;
unchanged event order/replay; singleton observed versus non-singleton legacy
activation; empty repository selection; warm/edit/delete/recreate/A-B-A; and
post-return retained state. Run focused and broader core/loading tests, direct
checks, formatting, diff/cap accounting, Buck2-utility retention scan, AI code
cleanup and independent implementation review. Reuse accepted evidence; no
fixture or oracle write is authorized.

## Compatibility boundary

Existing singleton package/output/event behavior remains exact. Carrier versus
selected-snapshot association, exact shared-Arc validation and fail-closed
observed outer errors are Slug-native. Analyzed/exported/multi-target/external/
cquery publication, repository/materializer breadth, native-Windows raw-byte
ordering and exact Bazel identity bytes remain unsupported/deferred.

## STOP / REPLAN

STOP on any other file; Cargo/fixture/oracle write; broader caller, repository
or materializer activation; partial epoch validation; changed public terminal,
output, event, retry, selection, publication or restoration behavior; direct or
reconstructed Host read; another retained owner; public API; or cap excess.

`REPLAN` if exact selected Arcs cannot survive preparation, validation cannot
precede irreversible acceptance, projection needs a public/fallible API,
repository selection is nonempty or non-admitted identities cannot remain
legacy.

## Immediate successor

On independent acceptance commit this packet, record validation and exact cap
accounting, then return to one docs-only next-owner audit. Do not combine another
build/cquery/repository frontier or milestone close.
