# Current Slug V2 Packet

Packet: `WP-6-7A-host-nonregistry-repo-file-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Rust base: `12f68983`
Accepted design: `3c598dd5`

## Exact Rust authority and caps

Write only `app/slug_bzlmod_v2/src/repo_file.rs`, baseline 2,679 physical
lines. Production growth is <=180, test growth is <=320, aggregate semantic
growth is <=500 and final physical size is <=3,200. Every new or touched helper
stays below 200 lines. Every other Rust, Cargo, BUILD, fixture, oracle and docs
file is read-only during implementation.

## Frozen owner and driver

Add private structural
`HostNonregistryRepoFileObservationKey(HostNonregistryRepoFileKey)` and private
`ObservedHostNonregistryRepoFile`. Its carrier contains exactly one local
`Arc<Result<HostRepoFileValue, HostRouteRepoFileError>>` plus one compact
`PathObservationEpoch`; derive `Dupe` and `Allocative` and expose only
borrowed result/epoch accessors. Add no export or caller.

Use one Legacy/Observed driver. Legacy selects only
`RepositorySourceFileKey`; observed selects only
`RepositorySourceFileObservationKey`. Only a Present source continues to the
same neutral `RootRepoFileSemanticsProjectionKey` and pure REPO evaluation.
Neither sibling computes the other. Move the driver's exact local semantic
Result Arc into the legacy value.

## Exact order and terminal algebra

Preserve repository source first -> neutral semantics only for Present -> pure
evaluation. Preserve current DICE-invariant behavior and all legacy terminal
classes.

Observed source Need or typed outer returns immediately, carrierless, before
semantics and without parent batch. Accept the complete source epoch before
semantic inspection. Source Absent and source semantic error retain that epoch
and store the existing empty local batch. Neutral policy failure retains the
same epoch and empty batch. REPO parse/evaluation error and success retain the
same epoch and exact existing local batch, including semantic Complete with an
empty batch. This single observed child has no epoch union and no Need union.

Need is invalid/self-unequal. Complete typed outer is valid/equal by outer
value. Complete carrier is valid/equal by semantic Result plus epoch.

## Events, families and retention

Each Legacy/Observed sibling is sole owner of its matching local REPO batch.
Repository source remains eventless. Need/outer/cancel stores none; warm reuse is
silent. Preserve exact event text, prefix, empty-batch behavior and
child-before-parent order.

Legacy direct dependencies are legacy source plus neutral semantics when
Present. Observed direct dependencies are observed source plus the same neutral
semantics when Present. Activate no repository-ignore, package-preflight,
closure, discovery, selected graph, registry, extension or public caller.

Retain only the local semantic Result Arc plus source epoch. Source carrier and
bytes, logical path, reporter/evaluator and event scratch are dependency-owned
or compute-local. Add no second carrier Arc, collection, cache, store, interner,
lock, task, direct Host read, revision, certificate or new event state.

## Required proof

Discriminate:

- key/carrier identity, hash, Display, accessors, Dupe/Allocative and
  Complete/Need/outer equality/validity;
- real source Need, typed outer, Absent, Present and semantic error with exact
  epoch/carrier polarity and later-child suppression;
- neutral policy failure and REPO parse/evaluation success/error with exact
  prefixes and legacy Result-Arc/value/event parity;
- exact epoch iteration and per-demand Arc identity, held lifetime, and source
  conflict/operation-mismatch outer propagation;
- exact observed/legacy dependency rows and reverse-family isolation;
- source child silence, parent empty/nonempty/error batches, warm suppression
  and no batch on Need/outer/cancel;
- real poll-drop and identical-request same-DICE recovery;
- local and immutable A -> B -> absent -> directory -> A restoration with held
  semantic Result and epoch Arcs; and
- zero ignore/preflight/closure/discovery/selected-graph/registry/extension/public
  activation.

Run focused REPO proof, full bzlmod, affected loading/query/core baselines, fmt,
diff-check, exact cap accounting and AI-cleanup/Buck2 retention review. Reuse
accepted evidence; add no fixture or Bazel oracle.

## Compatibility

Exact: existing nonregistry REPO source order, UTF-8 policy, result/error/
diagnostic/event behavior and every legacy result. Slug-native: private sibling,
Result-Arc+epoch carrier and typed outer. Unsupported/deferred: observed ignore,
preflight, closure, discovery/selected graph; registry file/preparation/patches;
extensions, rules_rust actions, M8/M7B and exact identity bytes.

## STOP and sole successor

STOP on a second file/key/caller/export, source-order or event drift, retained
scratch/state, direct Host read, upper/registry activation, proof weakness, cap
excess, M7A closure, M8/M7B/M9 or a second successor. REPLAN rather than move the
REPO batch upward or invent another producer.

After independent implementation ACCEPT schedule only the docs-only
`WP-6-7A-host-nonregistry-repository-ignore-observation-design`.
