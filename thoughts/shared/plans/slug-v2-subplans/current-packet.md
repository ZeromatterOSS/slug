# Current Slug V2 Packet

Packet: `WP-6-7A-host-nonregistry-repo-file-observation-implementation-retry-2`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `aff21fdb`
Rust base: `12f68983`
Accepted semantic design: `3c598dd5`
Accepted first proof-cap correction: `6b75865f`
Accepted second proof-cap correction: `aff21fdb`

## Accepted second correction and retained candidate

Correction `aff21fdb` accepts the bounded proof-only REPLAN. Resume the exact
one-file Rust candidate from Rust base `12f68983`, semantic design
`3c598dd5` and first correction `6b75865f`.

Against `12f68983`, live `repo_file.rs` is +170 production and +628 proof,
+798 aggregate semantic, at 3,477 physical lines. Focused observed proof passes
3/3; scope is exactly the authorized file, formatting is applied and
diff-check is clean. Production ownership, source-first order, family
selection, event ownership and compact Result-Arc+epoch retention remain sound.

The first corrected <=550 proof, <=730 aggregate and <=3,450 physical envelope
cannot honestly contain the frozen parent matrix. Safe factoring can recover
only about 25--40 lines. The candidate already exceeds the proof ceiling by 78
lines, while exact identity, semantic variants, batches, dependency rows,
lifetime and legacy parity still need additional proof. Removing >=78 live
proof lines would delete discriminating Need/outer, cancellation, family, epoch
or lifecycle evidence.
No production semantic or owner change is required.

## Exact Rust authority and caps

Write only `app/slug_bzlmod_v2/src/repo_file.rs`. From the 2,679-line Rust
base, keep production <=180, proof <=720,
aggregate semantic growth to <=900 and final physical size to <=3,700. Helpers
remain below 200 lines. Every other file is read-only.

## Frozen correction authority

Preserve the accepted private key/carrier, matching Legacy/Observed source
family, source-first Present-only continuation, exact Result-Arc projection,
carrierless Need/typed outer, semantic-Complete local REPO batches and compact
one-Result-Arc-plus-epoch retention. Add no production owner, caller, export,
state, event, Host read, cache, store, interner, lock or task.

This second correction changes only the proof envelope:

- keep production at <=180 from the 2,679-line base;
- raise proof from <=550 to <=720 lines;
- raise aggregate semantic growth from <=730 to <=900; and
- raise final physical size from <=3,450 to <=3,700.

This adds at most 170 proof-semantic and 250 physical lines. The measured
candidate has 92 proof-semantic, 102 aggregate-semantic and 223 physical lines
of corrected headroom.
It may fund only compact proof completion/restructuring; production semantics,
owner, events, retention, family selection and upper activation stay frozen.

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

- distinct key equality/hash as well as Display; carrier accessors,
  Dupe/Allocative and Complete/Need/outer equality/validity;
- real source Need, typed outer, Absent, Present and semantic error with exact
  result variants, epoch/carrier polarity and later-child suppression;
- neutral policy failure and REPO parse/evaluation success/error with exact
  error classes/messages, diagnostic/print batch text/order, prefixes and
  legacy Result-Arc/value/event parity for success and every error class;
- exact epoch iteration and per-demand Arc identity, held lifetime, and source
  conflict/operation-mismatch outer propagation;
- exact observed/legacy direct-dependency rows on success, policy, parse and
  evaluation lanes, including the neutral semantics child only when reached,
  plus reverse-family isolation;
- tracker-observed source-child event silence, parent empty/nonempty/error
  batches, warm suppression and no batch on Need/outer/cancel;
- real poll-drop and identical-request same-DICE recovery;
- local and immutable A -> B -> absent -> directory -> A restoration. Retain
  duplicate handles to the first Result and epoch Arcs and prove those held
  handles stay readable and pointer-identical to their duplicates after churn.
  Prove restored carrier equality and exact restored per-demand Arc identity
  against the restored observed source child. Do not require pointer identity
  between independently reconstructed but equal first/restored epochs; and
- zero ignore/preflight/closure/discovery/selected-graph/registry/extension/public
  activation.

Run focused REPO proof, full bzlmod, affected loading/query/core baselines, fmt,
diff-check, exact cap accounting and AI-cleanup/Buck2 retention review. Reuse
accepted evidence; add no fixture or Bazel oracle.

Compact repeated epoch/row/source-fixture loops where useful, but do not remove
any already discriminating Need/outer, source prefix, conflict/mismatch,
cancellation, warm, family, lifecycle or upper-exclusion proof.

## Compatibility

Exact: existing nonregistry REPO source order, UTF-8 policy, result/error/
diagnostic/event behavior and every legacy result. Slug-native: private sibling,
Result-Arc+epoch carrier and typed outer. Unsupported/deferred: observed ignore,
preflight, closure, discovery/selected graph; registry file/preparation/patches;
extensions, rules_rust actions, M8/M7B and exact identity bytes.

## STOP and sole successor

STOP a second retry file/key/caller/export, production semantic/order/event/
memory/family drift, direct Host read, upper/registry activation, proof
deletion, cap excess, M7A closure, M8/M7B/M9 or a second successor. REPLAN
again if the full matrix cannot fit the corrected envelope.

Only after independent retry ACCEPT schedule the docs-only
`WP-6-7A-host-nonregistry-repository-ignore-observation-design`.
