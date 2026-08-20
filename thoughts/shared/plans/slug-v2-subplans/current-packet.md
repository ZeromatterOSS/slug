# Current Slug V2 Packet

Packet: `WP-6-7A-registry-policy-observation-proof-cap-correction-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `1cd4e65b`
Rust base: `a4623d6b`
Accepted design: `8d00d44a`

## Objective and exact authority

Formally correct the measured registry-policy proof authority before retrying
the same implementation. Write only the canonical plan, this manifest, Stage 6
and the routing log, at <=40/<=220/<=180/<=30 net lines and <=470 aggregate.
Retain the dirty `app/slug_bzlmod_v2/src/registry_dice.rs` candidate exactly as
non-writable evidence during this design packet; every Rust/test/oracle file is
STOP.

The live candidate measures +166 production/+609 proof/+775 aggregate at 2,188
physical lines versus `a4623d6b`. Focused proof passes 3/3; full bzlmod 457 plus
integrations, loading 138 plus integrations and query 53+56+1+11 pass. The
production owner, algebra and retention are sound; only the frozen proof budget
and two mechanically impossible discriminator wordings require correction.
Core retains the inherited 245/246 stale external-visibility wording baseline.

## Frozen implementation contract

Freeze the accepted private key/carrier and shared Legacy/Observed driver
unchanged. The carrier retains exactly one local policy Result Arc plus the
compact root-files epoch, with `Dupe`/`Allocative`, borrowed accessors and no
export/caller. URLs -> lockfile mode -> matching root-files remains exact;
URL/mode and root compute failures keep empty semantic prefixes, observed root
Need/typed outer stays carrierless, and Complete root semantics forward the
child epoch unchanged before inspection. There is no parent union/conflict
class. Parent is eventless; child events remain child-owned. Retain no child
carrier Arc, collection/cache/interner/store/lock/task, Host read, revision,
certificate or event state.

Correct only the retry ceilings to <=200 production, <=680 proof, <=880
aggregate semantic and <=2,300 physical from the unchanged 1,413-line baseline;
helpers remain below 200 lines. This leaves 71 proof, 105 aggregate and 112
physical lines over the measured candidate without funding production semantic,
event, family, memory, caller or owner changes.

## Corrected proof contract

Preserve identity/hash/Display/accessors/equality/validity, the production-used
legacy projector's exact Result `Arc::ptr_eq`, observed semantic parity, exact
epoch iteration and per-demand Arcs, real root Need and exact parent dependency
row, reverse family isolation, upper/HostRegistry/extension/public exclusion,
poll-drop recovery and held-handle lifetime proof.

Prove URL, mode, MODULE and visible-lockfile invalidation independently as
A->B->A on one DICE graph. Compare exact ordered observed and legacy child
`EventBatch` values in phase-separated cold child computations, then prove the
parent's exact child row and event silence; warm reuse remains silent. This
phase separation is required because a direct parent activation reports the
already-computed child as Reused and exposes no nested child batch.

Real missing injected URL/mode computes must prove their exact semantic errors,
empty epochs and zero root-files-family or later activation. Do not require an
activation row for a failed `InjectedKey`: DICE fails that compute before
calling the activation tracker. Prove typed outer
through the production-used root terminal projector plus the accepted lower
root-owner outer discriminator; a naturally computed root outer cannot be
injected because `PathObservationEpoch` rejects duplicate/conflicting/mismatched
entries at construction. Add no test hook or invalid epoch escape hatch. Root
Need still requires the real parent row and later suppression.

Run focused proof, full bzlmod and affected loading/query/core baselines, fmt,
diff-check, exact accounting and cleanup/retention review.

Exact: current URL/mode/root order, policy values/errors, legacy Result Arc and
child events. Slug-native: private sibling, Result-Arc+epoch and typed outer.
Deferred: registry I/O/generation, patches, preparation, discovery/selected
graph, extensions, M8/M7B and identity bytes.

STOP Rust during design, a second implementation file/key/caller/export,
registry-file/preparation/discovery/selected activation, test hooks, invalid
epochs, semantic/event/family/memory drift, cap excess or milestone closure.
After independent correction ACCEPT schedule exactly
`WP-6-7A-registry-policy-observation-implementation-retry`; after retry ACCEPT
return only to the docs-only selected-module-graph frontier audit. REPLAN again
if the corrected proof cannot fit without weakening discrimination.
