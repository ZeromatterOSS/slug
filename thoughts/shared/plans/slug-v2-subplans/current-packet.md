# Current Slug V2 Packet

Packet: `WP-2A-m1-repository-package-source-observation-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Design/scheduling base: `4c838b83`
Rust base: `1815c019`
Result: implement only the accepted observed repository-package BUILD-source
sibling; activate no caller.

## Authority and caps

Write only:

- `app/slug_bzlmod_v2/src/host_package.rs`
- `app/slug_bzlmod_v2/src/host_package_observation_tests.rs`
- `app/slug_bzlmod_v2/src/lib.rs`

Against `1815c019`: at most 260 production net lines and 5,050 physical
lines in `host_package.rs`; 360 test net lines and 750 physical lines in
`host_package_observation_tests.rs`; 6 production net lines and 405 physical
lines in `lib.rs`; 626 aggregate semantic lines and 6,205 combined physical
lines. Keep new/touched helpers below 200 lines; the existing host-package
owner is a cohesive large-file exception.

## Required implementation

Keep public `RepositoryPackageSourceKey` and its Value exact. Add one
structurally distinct, doc-hidden public
`RepositoryPackageSourceObservationKey` and one doc-hidden carrier containing
only the local source Result Arc plus one compact `PathObservationEpoch`.
Export only the key/carrier required by the later loading-crate consumer; add no
caller.

Use one Legacy/Observed source driver in exact support -> external package
lookup -> selected BUILD-source order. Legacy selects only legacy children;
observed selects only the three accepted observed counterparts. Neither sibling
computes the other. Project the exact local source Result Arc to legacy.

Observed begins with the support epoch. Union each Complete lookup/source epoch
left-first before semantic inspection via stable shared-Arc union. Equal
duplicates keep the earliest support/lookup Result Arc; conflict or operation
mismatch is a typed observed outer. Preserve these prefixes:

- support evaluation/error/Unsupported and LookupCompute: support only;
- invalid/deleted/ignored/no-build/lookup semantic and SourceCompute: support
  then lookup;
- source error/Absent/success: support then lookup then source.

Need or typed outer at any child returns immediately with no carrier. Existing
DICE compute failures retain semantic ModuleEvaluation, LookupCompute or
SourceCompute polarity. This sequential owner adds no Need union. Need is
invalid/self-unequal; Complete outer compares by outer value; Complete carrier
compares semantic Result+epoch.

The source parent remains eventless for every terminal and cancellation.
Evaluation, routed REPO/policy/path and source children remain sole matching-
family event owners. Do not activate recursive external `.bzl`,
`RepositoryPackageLoadKey`, query/build/publication or their batches.

Retain only the existing source semantic, including selected bytes Arc, in one
local Result Arc plus compact epoch. Child semantic Arcs, selected-path
temporaries, union state and outcome scratch remain compute-local. Add no
collection/store/cache/interner, lock/task, direct Host read,
revision/certificate or event owner.

## Compatibility and proof

Exact: support/lookup/source values and errors, BUILD.bazel-before-BUILD
selection/bytes, legacy Result-Arc behavior, and child event text/order.
Slug-native: sibling/carrier, typed outer and complete retry epoch. Deferred:
recursive external `.bzl`, package load/query/build publication, broader
identities and exact identity bytes.

Prove distinct identity/Display/export; exact legacy Arc/result and observed
semantic parity; exact support->lookup->source epoch order and every Result Arc;
stable duplicate first Arc, conflict and operation mismatch; every prefix and
Need/outer position with validity/equality/no carrier; DICE-compute semantic
polarity; all support/lookup/source terminals and BUILD precedence/fallback;
exact selected bytes Arc; eventless parent and child event order/warm
suppression; both family directions and zero external-Bzl/load/query/build
activation; real poll-drop cancellation/recovery; create/edit/delete/recreate,
BUILD.bazel<->BUILD and A/B/A; compact Allocative retention, Buck2/AI cleanup,
focused/full bzlmod/loading/query and established core baselines, fmt/check/
diff/accounting, Clippy/archive disposition and one independent latest-diff
review.

## STOP / REPLAN

STOP on every other file, Cargo/BUILD/fixture/oracle write, caller/public
behavior change, recursive external-Bzl/load/query/build activation, mixed
families, rebuilt/partial epoch, event drift, retained child/scratch state, new
I/O/store/lock/task, cap excess, multiple successors or M1 closure. `REPLAN`
if a lower mutable edge lacks a complete observed carrier, exact legacy
semantics need another owner/file, the hidden export is unusable, or the source
parent must own an event. After ACCEPT, return only to the docs-only upper-
source audit for recursive external `.bzl` ownership.
