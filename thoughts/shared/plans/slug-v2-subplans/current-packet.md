# Current Slug V2 Packet

Packet: `WP-6-7A-host-root-apparent-repository-source-observation-observation-owner-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Audit base: pending docs commit / `1b573d5c`

## Goal and authority

Design only the smallest private observed owner for the existing callerless
root apparent repository source-observation key. Freeze its exact staged child
order, terminals, epoch law, event/retention/lifecycle contract and bounded
proof without activating any caller or public command path. Do not edit Rust,
tests, APIs, fixtures, Cargo/BUILD or exports in this packet.

Documentation authority is only the canonical plan, this manifest, Stage 6
and routing log, capped at <=40/<=180/<=220/<=30 additions respectively and
<=470 aggregate additions. Every other file is read-only.

## Learned frontier and decision

Accepted `1b573d5c` exposes the observed source-path key/carrier/opaque outer to
its source-observation sibling. Live
`root_apparent_repository_source_observation.rs` has one legacy private key and
exactly two staged children:

1. `HostRootApparentRepositorySourcePathInputKey` always runs first at line
   234. Need is immediate; DICE failure becomes SourcePathCompute; completed
   semantic failure becomes SourcePath; an inconsistent successful view becomes
   InvalidSourcePath.
2. Main completes successfully with no second child. Only a consistent Input
   computes `HostRepositorySourceObservationKey` at line 293. Its Need is
   immediate; DICE failure becomes ObservationCompute; completed semantic
   failure becomes Observation; wrong-polarity/inconsistent success becomes
   InvalidObservation; otherwise the certificate succeeds.

The result retains the exact source-path Result Arc plus zero-or-one exact
repository-source-observation Result Arc. Existing equality/validity are
Complete-only. The parent owns no event batch.

The now-nameable observed source-path child supplies its Result Arc and
transaction-local epoch. The second child is already the public semantic Host
observation: its Value is
`SourcePreparationOutcome<Arc<HostRepositorySourceObservationResult>>`, with no
separate epoch carrier. Therefore the observed parent must forward the
source-path prefix unchanged through every second-stage terminal/success and
must not invent an epoch, merge, observation accessor or Bzlmod prerequisite.

`HostRootApparentRepositorySourceObservationKey` has zero production callers.
Public command analysis independently uses Bzlmod `RootRepositoryRouteKey` and
`RootRepositoryRouteObservationKey` at `runtime/dice.rs:4476-4494`; root
bootstrap remains dormant. That parallel branch does not consume this terminal
certificate. No visibility or lower evidence prerequisite remains, and the
callerless private owner is uniquely smaller than public activation.

Choose exactly
`WP-6-7A-host-root-apparent-repository-source-observation-observation-owner-design`.

## Required owner design

Freeze one private observation key nominally wrapping the legacy key, with the
same three-argument Option constructor, root rejection, full requested
`PathBuf` identity and exact `observed-{legacy Display}`. Add one private local
carrier retaining only the source-observation Result Arc plus
`PathObservationEpoch`, with borrowed accessors and Complete-only equality/
validity. Add one private typed outer containing only the promoted lower
source-path opaque outer; it is carrierless and exposes no lower variant.

One shared `Legacy`/`Observed` driver must preserve the exact order above.
Legacy requests the legacy source-path child with empty epoch. Observed requests
only the observed source-path child; Need and its opaque outer are immediate,
while DICE compute failure remains the legacy SourcePathCompute semantic Result
with empty epoch. A completed child supplies its exact Result Arc and epoch.

Run existing path view/disposition logic once. SourcePath,
InvalidSourcePath and Main success retain/forward the path epoch. Only Input
requests the unchanged `HostRepositorySourceObservationKey`. Its Need is
carrierless. ObservationCompute, Observation, InvalidObservation and success
retain the exact predecessor/observation Arcs and forward the path prefix
unchanged. Add no observed variant of that second child, no union/merge/
mismatch/rebuild/fallback, no direct Host read and no caller.

The parent stays eventless. Exact dependency rows are legacy -> legacy path ->
conditional Host observation and observed -> observed path -> conditional same
Host observation. Main/path failures suppress the second child. Lower print or
path-observation batches remain lower-owned and every warm parent row is
batchless.

The carrier retains only the local Result Arc plus compact epoch; its Result
already owns the required child Arcs. Observed child carrier, views,
disposition/input/path clones, tracker/event and construction scratch die before
publication. Add no cache/store/interner/task/lock or command borrow. DICE owns
serialization; poll-drop publishes no partial carrier and same-DICE recovery is
lawful.

## Required proof and bounds

Require exactly three new tests:

- `observed_root_apparent_repository_source_observation_identity_staging_and_terminal_algebra`;
- `observed_root_apparent_repository_source_observation_real_order_events_and_parity`;
- `observed_root_apparent_repository_source_observation_lifecycle_cancellation_and_nonactivation`.

They must prove key/hash/Display/root rejection/accessors/equality/validity;
lower opaque outer, Need and every semantic terminal prefix; exact Main,
Builtin and Request success/error legacy semantic parity; first-child and
conditional second-child dependency order/suppression; exact retained Arcs;
unchanged path epoch/no merge; complete lower event vectors and warm silence;
held path/observation Result and parent carrier across mapping/policy/path/
source-content A-B-A, semantic-neutral path-epoch invalidation, each recovered
path epoch associated only with its same-transaction parent/global epoch,
poll-drop recovery and zero legacy/public/bootstrap activation. Fabricate no
opaque outer or malformed epoch and inject no private Host state.

Prospective one-file authority is exactly
`app/slug_core_v2/src/runtime/root_apparent_repository_source_observation.rs`,
baseline 932 physical lines, `#[cfg(test)]` at 340, SHA-256
`57f95be85cceb9a02d04ecff400b9c526837daecf2b54aa111afe3366650396a`.
Caps are <=300 production additions, <=720 proof additions, <=1,020 aggregate
additions and <=2,000 physical lines; at most six production/eight proof
helpers, exactly three new tests, shared driver below 180 and every changed
helper/test below 200. Preserve the accepted sibling smoke and legacy tests.
Add no `rustfmt::skip` or formatter/cap/test waiver. The file remains the
cohesive owner of its value/error/view, staged reducer, trackers and fixtures;
no hot-path or retained-representation change applies.

Prospective validation is the three exact tests, protected legacy source-
observation and observed source-path/source-input suites, full core, separate
runtime integration, direct commands check, formatting, exact one-file
allowlist/SHA/accounting/physical/helper/test/driver/source-shape/no-skip and
diff hygiene, serially. Full core may reproduce only the byte-identical accepted
library query diagnostic baseline. Runtime may reproduce only the accepted
`c8d2d0b5`-identical `PathObservationEpochKey`/configured-analysis-Needs
failure while its other 12 tests pass. These are baseline accounting, not
waivers; any changed/additional failure is a STOP. Reuse accepted lower and
Bazel source-observation evidence; add no oracle.

## Compatibility and stops

Existing source-observation values/views/errors, child order, semantic result
identity, equality/invalidation and lower event behavior remain **exact** Bazel
9 compatibility. The private observed key/carrier/typed outer and Result-Arc+
transaction-local epoch association are **Slug-native**. Carrier visibility,
any caller/public-command/bootstrap activation and exact Bazel configuration/
output/ActionKey bytes remain **unsupported/deferred**.

STOP Rust/tests in this audit/design packet, second file/key/owner/adapter,
visibility/export/caller/public-route change, observed second-child variant,
epoch merge/mismatch/rebuild, semantic/order/error/event/equality/retention/
lifecycle drift, retained scratch/task/lock, private/malformed injection,
fixture/oracle, cap/helper/test/format waiver, changed/additional validation
failure, milestone closure, M8/M7B or exact identity work. REPLAN before
widening or baseline drift.

## Terminal

If the one-file staged owner remains feasible, ACCEPT schedules exactly
`WP-6-7A-host-root-apparent-repository-source-observation-observation-implementation`,
then returns to a docs-only terminal carrier/publication frontier audit. M7
remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Accepted `1b573d5c` changes only the two visibility files by +56/-10, leaves the
source-observation owner callerless, and closes the sole lower visibility stop.
