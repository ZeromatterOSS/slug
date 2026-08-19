# Current Slug V2 Packet

Packet: `WP-2A-m1-external-singleton-observed-build-implementation-retry`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `ce110d9a`
Accepted Rust base: `a4dd40d6`
Accepted semantic design: `1a217e2a`
Accepted proof-authority correction: `ce110d9a`
Result: publish exactly one nonroot exported-source build through the existing
observed build root and correct its stale loading static proof.

## Exact Rust authority and caps

Write exactly:

1. `app/slug_core_v2/src/runtime/dice.rs`: <=260 production net and <=11,220
   physical;
2. `app/slug_core_v2/src/runtime/tests/build_command_tests.rs`: <=360 test net
   and <=3,350 physical; and
3. `app/slug_loading_v2/src/host_package_load_tests.rs`: zero net and <=3,439
   physical, with only the exact assertion replacement frozen below.

Aggregate semantic <=620 and combined physical <=18,009 against `a4dd40d6`.
No relocation or other loading byte. Cargo manifests, BUILD, fixtures, oracles,
generated evidence, exports, other callers and planning docs are forbidden.
The large files remain cohesive owner exceptions; every new or materially
touched helper stays below 200 lines.

## Frozen owner, identity and driver

Keep the same structural `BuildCommandRootObservationKey(BuildCommandRootKey)`.
Its constructor admits the existing singleton root PackageAll plus every
syntactic nonroot Single. External wrong-kind rule/filegroup requests enter
observed and fail only after package classification. Every root Single,
multi-target and other identity preserves the neutral/legacy route. Keep the
sole public observed -> neutral -> legacy constructor order and add no key or
public seam.

Keep PackageAll observed behavior unchanged. One private mode-aware external
semantic driver selects only matching legacy or observed route/package/source
children; neither mode activates the other. Preserve target classification,
diagnostics and semantic projection on that shared path.

Observed order is root loading anchor -> repository route -> repository package
load -> exact ExportedFile classification -> RequestRevisionKey -> selected
repository source. Compute revision only after classification and before
source; missing/wrong-kind targets activate neither revision nor source.

Validate and union every completed observed child epoch left-first before
semantic inspection. Equal duplicates keep the first exact Result Arc;
conflict/operation mismatch is typed `ObservedPathFrontierError`. Need or typed
outer is immediate and carrierless with no later activation. This sequential
owner performs no Need union.

Freeze prefixes: anchor compute failure empty, anchor semantic anchor; route
compute anchor, route semantic anchor+route; package compute anchor+route,
package semantic anchor+route+package; target/revision failure
anchor+route+package; source compute anchor+route+package; source Present,
Absent, semantic, directory WrongKind and success full
anchor+route+package+source. Success is one loaded-only target with empty action
closure. Preserve the distinct legacy infrastructure channel and its existing
post-`compute_join` ordering; observed compute errors remain semantic with the
decisive prefix.

## Certificate, publication and events

Every terminal after a completed source child retains a `SourceCertificate`
made from that child's entire observed epoch. Present, Absent, accepted
directory WrongKind and source semantic error retain it; earlier terminals do
not. It is an exact demand/value/Arc subset of the full root epoch. Exact error
formatting, codes, values and equality remain unchanged.

Only admitted external Single initializes request revision, exposes the full
epoch/certificate and selects `ClosureRepositories`. PackageAll remains
strict-empty. `selected_snapshot` remains the sole repository sidecar owner;
full path demand/value/Arc comparison remains unconditional and no repository
collection enters the terminal. Finalization reobserves the complete
certificate through the active materializer. Equal demands preserve exact
Arcs; changed demands alone publish one revision and retry. Need, outer,
cancel and every acceptance failure preserve prior path/repository/event state.

The observed root owns no event batch. Matching anchor/module/package/source
children remain sole owners. Preserve exact cold child order/text, semantic-
error batches, warm suppression, and external -> root PackageAll no replay.

## Exact loading-proof correction

In `app/slug_loading_v2/src/host_package_load_tests.rs`, replace exactly:

```rust
assert!(!core.contains("RepositoryPackageLoadObservationKey"));
```

with:

```rust
assert!(core.contains("RepositoryPackageLoadObservationKey"));
```

Keep the adjacent positive query assertion and every other file byte
unchanged. The core integration proof remains the authority for external-only
observed activation, both family directions, later-child suppression and zero
query/multi-build/one-shot activation.

## Retention, compatibility, proof and STOP

Retain exactly one local build Result Arc plus one compact full epoch. The
semantic Result may retain one compact source certificate epoch sharing exact
child Arcs. Child carriers/outcomes, selected path and union/event scratch stay
compute-local or dependency-owned. Add no map, Vec, cache, store, interner,
lock, task, direct Host read, revision duplicate, event owner or historical
snapshot.

Exact: public external source values/errors/classification, BUILD/module event
text/order, root PackageAll, root Single, multi-target and all legacy/direct
APIs. Slug-native: observed admission, carrier/epoch/typed-outer association,
external-only repository selection and private certificate attachment.
Unsupported/deferred: multi-build, one-shot, broader actions, external globs
and exact Bazel identity bytes.

Prove identity/routing, exact shared-driver legacy parity and both family
directions; every prefix/Need/outer/compute/semantic terminal and later-child
suppression; exact epoch membership/Arc order, certificate subset, duplicate/
conflict/mismatch; revision-before-source; repository request/validation and
strict PackageAll polarity; exact lifecycle/retained bytes, cold/warm/error/no-
replay events, pointer abort, cancellation/recovery and zero broader activation.

Require the focused public test, 33/33 build-command group, full loading
138/138, full core with only the recorded stale visibility-wording baseline,
formatting/diff, exact accounting, Buck2 retention, AI cleanup and independent
final review.

STOP on any other file/loading byte, new key/state/event owner, lower/public
change, partial validation, strict-root relaxation, behavior/family/order
drift, retained scratch, direct Host read, cap excess, broader activation or M1
closure. REPLAN on any new blocker. After ACCEPT return only to one docs-only
remaining M1 owner audit.
