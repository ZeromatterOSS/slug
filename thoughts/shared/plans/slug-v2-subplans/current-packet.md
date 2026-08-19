# Current Slug V2 Packet

Packet: `WP-6-7A-host-nonregistry-repository-ignore-observation-implementation-retry`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `4c9f344b`
Rust base: `b08b7f2e`
Accepted semantic design: `9c0a5473`
Accepted proof-cap correction: `4c9f344b`

## Objective and exact authority

Complete the accepted private repository-ignore observation owner without
changing its production semantics, event ownership, matching families or
retained representation. The pre-existing candidate is writable only for
compact proof restructuring and additions required below.

Write exactly `app/slug_bzlmod_v2/src/repository_ignore.rs` from its 3,297-line
`b08b7f2e` baseline. Caps are <=180 production, <=720 proof, <=900 aggregate
semantic and <=4,250 physical lines. Touched helpers remain below 200 lines.
Every other Rust/Cargo/BUILD/fixture/oracle/caller/public file is read-only.

## Frozen production contract

Preserve the private
`HostNonregistryRepositoryIgnoreObservationKey` and
`ObservedHostNonregistryRepositoryIgnore`, one local matcher Result Arc plus
one compact `PathObservationEpoch`, `Dupe`/`Allocative`, borrowed
accessors and no export/caller.

Preserve one Legacy/Observed driver and exact repo -> source -> parser order.
Legacy selects only legacy REPO/source siblings; observed selects only their
accepted observed siblings. Both modes retain the same neutral Windows
long-path parser dependency when reached. Neither computes the other family.

Repo semantic retains repo-only. Merge the accepted repo prefix left-first
with a Complete source epoch before source semantics; source terminals retain
repo+source. Merge that accumulated prefix left-first with a Complete parser
epoch before parser semantics; parser terminals retain full. Equal duplicates
keep the earliest exact Arc; conflict/operation mismatch is typed outer. Need
or typed outer at any position is carrierless and suppresses later work. There
is no Need union.

The ignore parent stays eventless; matching REPO child remains sole local batch
owner and source/parser remain eventless. Legacy projection moves the exact
local Result Arc. Retain no child carrier/source bytes/parser scratch or second
collection/state.

## Required retry correction

Keep every existing discriminator and add or restructure proof to cover:

- real source-position Need and typed outer with no parser/later activation;
- repo and source semantic variants with exact error class/message and exact
  repo-only versus repo+source prefixes;
- parser error/success full-prefix polarity and carrierless outer behavior;
- exact epoch iteration order and per-demand `Arc::ptr_eq`, earliest duplicate
  Arc, parent-union conflict and operation mismatch;
- exact parent/source/parser batch silence, matching child REPO batch
  text/order, cold-to-warm suppression and cancellation recovery;
- exact observed and legacy dependency rows, reverse isolation, shared neutral
  Windows parser dependencies and zero preflight/closure/discovery/
  selected-graph/registry/extension/public activation;
- independent local and immutable REPO-file A -> B -> absent -> directory -> A
  changes as well as independent `.bazelignore` lifecycle, matcher restoration,
  held Result/epoch readability and restored child-to-parent Arc identity; and
- Complete carrier/outer and Need equality/validity plus exact legacy semantic
  and event parity.

Run focused ignore proof, full bzlmod, affected loading/query/core baselines,
fmt, diff-check, exact accounting and AI-cleanup/Buck2 retention review. Add no
fixture or Bazel oracle because parser grammar/platform semantics, values,
errors and events remain exact and unchanged.

## Compatibility and STOP

Exact: current nonregistry REPO -> `.bazelignore` -> parser ordering,
grammar/platform behavior, matcher values/errors and every legacy child event.
Slug-native: private sibling, Result-Arc+epoch carrier and typed outer.
Unsupported/deferred: package preflight/closure/discovery/selected graph;
registry preparation/patches; extension repositories, M8/M7B and identity
bytes.

STOP a second Rust file/key/caller/export, production semantic/order/event/
memory/family change, direct Host read, upper/registry activation, proof
deletion, cap excess or milestone closure. REPLAN if the matrix cannot fit.

After independent implementation ACCEPT schedule only the docs-only
`WP-6-7A-host-nonregistry-package-preflight-observation-design`.
