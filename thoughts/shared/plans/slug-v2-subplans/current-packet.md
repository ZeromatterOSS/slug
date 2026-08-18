# Current Slug V2 Packet

Packet: `WP-2A-m1-repository-package-source-observation-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `efdfc6ed`
Rust base: `1815c019`
Result: design only the observed repository-package BUILD-source sibling;
authorize no Rust or caller activation.

## Authority and caps

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`

Against `efdfc6ed`: at most 40 canonical net lines, 180 manifest net lines,
180 Stage 2 net lines and 400 aggregate net lines. Future Rust, only after
independent design ACCEPT, is exactly:

- `app/slug_bzlmod_v2/src/host_package.rs`: at most 260 production net lines
  and 5,050 physical lines;
- `app/slug_bzlmod_v2/src/host_package_observation_tests.rs`: at most 360 test
  net lines and 750 physical lines;
- `app/slug_bzlmod_v2/src/lib.rs`: at most 6 production net lines and 405
  physical lines.

Future aggregate semantic growth is at most 626 lines and combined physical
size at most 6,205 lines. Keep new/touched helpers below 200 lines; the existing
host-package owner is a cohesive large-file exception.

## Required design

Keep public `RepositoryPackageSourceKey` and its Value exact. Freeze one
structurally distinct, doc-hidden public
`RepositoryPackageSourceObservationKey` and one doc-hidden carrier containing
only the local source Result Arc plus one compact `PathObservationEpoch`.
Export only the key/carrier needed by the later loading-crate consumer; add no
caller now.

Use one Legacy/Observed source driver. Its exact sequential order is:

1. direct-local module support;
2. external repository package lookup;
3. the selected BUILD source, only after lookup selects a package marker.

Legacy selects only the existing legacy support, lookup and source children.
Observed selects only the accepted observed counterparts. Neither sibling
computes the other. Project the exact local source Result Arc to legacy.

Observed starts with the support epoch. Union each Complete lookup/source epoch
left-first into the accumulated prefix before semantic inspection, using stable
shared-Arc union. Equal duplicates keep the earliest support/lookup Result Arc;
conflict or operation mismatch is a typed observed outer. Freeze these prefixes:

- support evaluation/error/Unsupported: support only;
- LookupCompute: support only;
- invalid/deleted/ignored/no-build/lookup semantic: support then lookup;
- SourceCompute: support then lookup;
- source error/Absent/success: support then lookup then source.

Need or typed outer at any child returns immediately with no carrier. Existing
DICE compute failures retain their semantic ModuleEvaluation, LookupCompute or
SourceCompute class and the prefixes above. This owner is sequential and adds
no joined Need union. Need is invalid/self-unequal; Complete typed outer is
valid/equal by outer value; Complete carrier is valid/equal by semantic
Result+epoch.

The source parent remains eventless on success, semantic error, Need, outer and
cancellation. Evaluation, routed REPO/policy/path and source children remain
the sole matching-family event owners. Do not activate recursive external
`.bzl` evaluation or its batch, `RepositoryPackageLoadKey` or its BUILD
batch, query, build or publication.

Retain only the existing source semantic value, including its selected bytes
Arc, in one local Result Arc plus the compact epoch. Support/lookup/source child
semantic Arcs, selected-path temporaries, union state and outcome scratch remain
compute-local. Add no collection/store/cache/interner, lock/task, direct Host
read, revision/certificate or event owner.

## Compatibility and proof

Exact: support/lookup/source values and errors, BUILD.bazel-before-BUILD
selection and bytes, legacy Result-Arc behavior, and existing child event
text/order. Slug-native: sibling/carrier, typed outer and complete retry epoch.
Deferred: recursive external `.bzl`, package load/query/build publication,
broader identities and exact identity bytes.

Prove distinct identity/Display and hidden export shape; exact legacy Arc/result
parity and observed semantic parity; exact support->lookup->source membership/
order and every Result Arc; stable duplicate first Arc, conflict and operation
mismatch; every prefix and Need/outer position with validity/equality/no
carrier; DICE-compute semantic polarity; all support/lookup/source terminal
classes and BUILD marker precedence/fallback; exact selected bytes Arc;
eventless parent and exact child ROOT/REPO/evaluation order with warm
suppression; both family directions and zero external-Bzl/load/query/build
activation; real poll-drop cancellation/recovery; create/edit/delete/recreate,
BUILD.bazel<->BUILD and A/B/A; compact Allocative retention, Buck2/AI cleanup,
focused/full bzlmod/loading/query and established core baselines, fmt/check/
diff/accounting, Clippy/archive disposition and independent latest-diff review.

## STOP / REPLAN

STOP on Rust now, every other future file, Cargo/BUILD/fixture/oracle write, a
caller or public behavior change, recursive external-Bzl/load/query/build
activation, mixed families, rebuilt/partial epoch, event ownership drift,
retained child/scratch state, new I/O/store/lock/task, cap excess, multiple
successors or M1 closure. `REPLAN` if a lower mutable edge lacks a complete
observed carrier, exact legacy semantics require another owner/file, the later
loading consumer cannot use the bounded hidden export, or the source parent must
own an event. After independent design ACCEPT, schedule exactly one bounded
implementation; after implementation ACCEPT, return to the docs-only upper-
source audit for recursive external `.bzl` ownership.
