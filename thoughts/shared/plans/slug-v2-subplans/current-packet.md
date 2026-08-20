# Current Slug V2 Packet

Packet: `WP-6-7A-registry-file-observation-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `de76a83e`
Rust base: `d0ebd79d`

## Objective and exact authority

Freeze the uniquely smallest shared registry-file observation owner. Write only
the canonical plan, this manifest, Stage 6 and routing at
<=40/<=220/<=180/<=30 net lines and <=470 aggregate. Rust/tests/fixtures/oracles
and public/caller files are read-only during design.

Future implementation authority is exactly
`app/slug_bzlmod_v2/src/registry_dice.rs`, baseline 2,213 physical lines and
first `#[cfg(test)]` boundary 905. Caps are <=320 production, <=760 proof,
<=1,080 aggregate semantic and <=3,400 physical; helpers remain below 200 lines.

## Frozen owner and algebra

Add private crate-visible `RegistryFileObservationKey(RegistryFileKey)` and
`ObservedRegistryFile`, with borrowed accessors, `Dupe`/`Allocative`, no export
or caller. Retain exactly one local
`Arc<Result<RegistryFileValue, RegistryFileError>>` plus one compact
`PathObservationEpoch`.

Use one Legacy/Observed driver. Preserve scheme dispatch first. Invalid local
URL and unsupported scheme are semantic Complete with empty prefix. Legacy
local remains policy -> legacy root files -> local IO/generation. Observed local
selects only observed policy -> observed root files -> unchanged local
IO/generation. Remote legacy selects legacy policy; remote observed selects only
observed policy before the unchanged plan/IO/generation path. Neither sibling
computes the other, and legacy projection moves the exact local Result Arc.

For observed local, accept the Complete policy epoch before policy semantics,
then merge policy prefix left-first with the Complete root epoch before root
semantics. Equal duplicates preserve the policy Arc; conflict or operation
mismatch is typed outer. Policy/root Need or typed outer is immediate and
carrierless. Policy compute failure has empty prefix; policy semantic keeps the
policy prefix and suppresses root/IO; root compute keeps policy-only; root
semantic keeps merged policy+root and suppresses IO. Local IO/generation
success/error retains the merged prefix. Remote policy compute is empty;
policy semantic and every plan/IO/checksum/generation terminal retain the policy
prefix. This sequential owner performs no Need union.

Need is invalid/self-unequal; Complete outer equality is outer-by-value and
Complete carrier equality is semantic Result+epoch. Registry-file siblings are
eventless. Reached root descendants retain sole MODULE/lockfile batch ownership;
IO/generation owns no event. Need/outer/cancel publishes no parent state and warm
reuse is silent.

Retain no policy/root carrier Arc, IO handle/scratch, attempt collection, cache,
interner, store, lock, task, direct Host read, revision, certificate or event
state. Existing RegistryIO/request-generation lifetime stays dependency-owned.

## Required proof and compatibility

Prove key/hash/Display/accessors/equality/validity; exact legacy Result Arc and
semantic parity; invalid URL/unsupported scheme; policy/root compute, Need,
outer and semantic positions with exact prefixes/later suppression; duplicate
first Arc/conflict/mismatch; every local/remote plan, Found/NotFound/read/
transport/checksum/generation terminal; exact epoch order/per-demand ptrs.

Prove exact local/remote dependency rows, both family directions, phase-separated
exact child batches with parent silence/warm, poll-drop same-DICE recovery, and
scripted local/remote Found/NotFound/error/generation plus independent policy
URL/mode/MODULE/lockfile A-B-A with held Result/epoch handles. Assert zero
preparation/discovery/selected/HostRegistry/extension/public activation.

Exact: current registry-file values/errors, scheme/plan/IO order, legacy Result
Arc and child events. Slug-native: private carrier, typed outer and IO-generation
boundary associated with the accepted path epoch. Deferred: root patches,
module-source preparation, discovery evaluation/events, selected graph,
selected repo specs, extensions, M8/M7B and identity bytes.

STOP Rust during design, a second future file/key/caller/export, IO semantic
redesign, patch/preparation/discovery/selected activation, event/family drift,
extra retained state, cap excess or milestone closure. After independent design
ACCEPT schedule exactly
`WP-6-7A-registry-file-observation-implementation`; after implementation ACCEPT
return to the docs-only selected-module-graph frontier. REPLAN if this owner
cannot preserve exact IO behavior within the frozen scope.
