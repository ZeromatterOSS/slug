# Current Slug V2 Packet

Packet: `WP-2A-m1-repository-package-source-observation-implementation-retry`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling/design base: `9ff3422d`
Rust base: `1815c019`
Result: complete and validate the retained repository-package source sibling
under the accepted proof-cap correction.

## Authority and caps

Write exactly:

- `app/slug_bzlmod_v2/src/host_package.rs`: at most 260 production net lines
  and 5,050 physical lines;
- `app/slug_bzlmod_v2/src/host_package_observation_tests.rs`: at most 480 test
  net lines and 880 physical lines;
- `app/slug_bzlmod_v2/src/lib.rs`: at most 6 production net lines and 405
  physical lines.

Aggregate semantic growth is at most 746 lines and combined physical size at
most 6,335 lines, all measured against `1815c019`. The retained candidate is
authoritative; do not recreate or broaden it.

## Required implementation

Preserve the doc-hidden public structural source sibling/carrier and the one
matching-family support -> lookup -> selected-source driver. Legacy selects
only legacy children and moves the exact local source Result Arc. Observed
selects only accepted observed children, unions every Complete epoch left-first
before semantic inspection, preserves the first equal Arc, and retains exactly
one local source Result Arc plus the compact complete epoch.

Prefixes remain support for support semantic and LookupCompute,
support+lookup for lookup semantic and SourceCompute, and full
support+lookup+source for source semantic, Absent and success. Need or typed
outer at any child is immediate and carrierless; DICE compute failures retain
their existing semantic class and prior prefix. The sequential owner has no
Need union. Complete outer equality is by outer value, Complete carriers by
semantic Result+epoch, and Need is invalid/self-unequal.

Keep the parent eventless. Evaluation, route-REPO/policy/path and source
children remain the sole matching-family event owners. Recursive external
`.bzl`, package load, query and build keys remain dormant. Support/lookup/
source child Arcs, path selection, unions and outcomes stay compute-local; add
no retained collection, store, cache, interner, lock, task, Host read, revision,
certificate, event owner or caller.

The bounded correction may add only a small pure reducer used by the live
driver. Finish discriminating support/lookup/source Need and typed outer with
no carrier and later-child nonactivation; exact error variant and exact epoch
prefix for support Unsupported/evaluation, LookupCompute, InvalidPackageName,
canonical Deleted, ignore-retained Deleted, NoBuild, lookup semantic,
SourceCompute, source semantic, SelectedSourceAbsent and success; conflict and
operation mismatch through the production union/reducer seam; exact ROOT ->
route-REPO -> evaluation child batch text/order including empty batches, parent
none and warm suppression; complete reverse-family exclusion and zero
external-Bzl/load/query/build activation. Retain cancellation recovery,
BUILD.bazel -> BUILD -> restore/A-B-A, exact bytes/epoch Arcs, legacy
projection pointer identity, validity/equality, Allocative/retention and cleanup
proof.

## Compatibility and terminal

Exact: source/support/lookup values and errors, BUILD marker precedence and
bytes, legacy Result-Arc behavior, and child events. Slug-native: structural
sibling/carrier, typed outer and complete epoch association. Deferred:
recursive external `.bzl`, package load/query/build publication and exact
identity bytes.

Run focused and full bzlmod tests, direct loading/query dependents, fmt,
diff-check, cap accounting, Buck2-style retained-state scan, AI cleanup and
independent implementation review. STOP on another file, key/state/event/
semantic change, caller or upper activation, mixed families, proof weakening,
cap excess, multiple successors or M1 closure. If the corrected caps cannot
hold, REPLAN. After ACCEPT, commit the implementation and return only to the
docs-only upper-source owner audit.
