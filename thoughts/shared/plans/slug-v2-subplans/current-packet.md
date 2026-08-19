# Current Slug V2 Packet

Packet: `WP-6-7A-host-nonregistry-repo-file-observation-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling and Rust base: `12f68983`
Accepted repository-source design/correction: `9040e168` / `edc533ff`

## Formal frontier REPLAN evidence

`12f68983` accepts the repository-source observation owner at +297 production,
+30 colocated proof and +700 external proof lines, +1,027 semantic aggregate,
and 15,267/3,170/18,437 physical lines. Focused proof passes 3/3, full
`slug_bzlmod_v2` passes 439+193, loading passes 204, query passes 121, and the
inherited core baseline remains 245/246 only on the stale generic visibility
wording assertion. Formatting, diff-check, compact-retention cleanup and
independent final review pass.

The selected graph is still not an implementable observation owner.
`HostSelectedModuleGraphKey` joins legacy `HostDiscoveredModuleKey` values.
Its nonregistry branch reaches `HostNonregistryModuleClosureKey`, whose Host
include horizon is legacy-only and computes
`HostNonregistryPackagePreflightKey`. Preflight computes the carrierless
`HostNonregistryRepositoryIgnoreKey`; ignore computes the carrierless,
event-owning `HostNonregistryRepoFileKey` before the legacy `.bazelignore`
source. Observing closure, discovery or selected graph now would bypass,
duplicate or relocate that exact REPO event owner.

`HostNonregistryRepoFileKey` is the uniquely smallest complete next producer.
It owns only repository `REPO.bazel` source -> neutral root REPO semantics ->
pure evaluation and its one local REPO batch. Its source edge now has the
accepted observed sibling. Registry `RegistryFileKey`, registry preparation
and patch observations remain a separate later frontier.

## Exact docs authority

This design may write only:

1. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`, <=40 net lines.
2. `thoughts/shared/plans/slug-v2-subplans/current-packet.md`, <=220 net lines.
3. `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`,
   <=180 net lines.
4. `.codex/skills/slug-agent-orchestration/references/routing-log.md`, <=30 net
   lines.

Aggregate docs growth is <=470 net lines. Rust, Cargo, BUILD files, fixtures and
oracles are read-only during design.

## Frozen natural owner and future authority

Add a private structural
`HostNonregistryRepoFileObservationKey(HostNonregistryRepoFileKey)` and private
`ObservedHostNonregistryRepoFile`. The carrier contains exactly one local
`Arc<Result<HostRepoFileValue, HostRouteRepoFileError>>` plus one compact
`PathObservationEpoch`; it is `Dupe` and `Allocative`, with borrowed
result/epoch accessors. Do not export the key or carrier.

Use one Legacy/Observed driver in `repo_file.rs`. Legacy selects only
`RepositorySourceFileKey`; observed selects only the accepted
`RepositorySourceFileObservationKey`. Both then select the same neutral
`RootRepoFileSemanticsProjectionKey` only after a Present source and perform
the same pure REPO evaluation. Neither key computes its sibling. The legacy key
moves the driver's exact local semantic Result Arc unchanged.

Future Rust authority is exactly
`app/slug_bzlmod_v2/src/repo_file.rs`, baseline 2,679 physical lines:
<=180 production, <=320 tests, <=500 aggregate semantic and <=3,200 physical.
Every new or touched helper stays below 200 lines; the existing file is the sole
cohesive owner/proof exception.

## Order and terminal algebra

The exact order is repository source first; only Present continues to neutral
REPO semantics; evaluation is last. Preserve the current DICE-invariant
treatment and exact legacy terminal polarity.

Observed Need or typed outer from the source returns immediately, carrierless,
without semantics or parent batch. Accept and retain the complete source epoch
before inspecting its semantic Result. Source Absent and source semantic error
retain that epoch and store the existing empty local REPO batch. A semantics
projection failure retains the same epoch and empty batch. REPO parse/evaluation
error and success retain that epoch and store exactly the existing local event
batch, including an empty batch as a semantic Complete batch rather than no
batch. No epoch union or Need union is needed at this single-observed-child
owner.

Need is invalid/self-unequal. Complete typed outer is valid/equal by outer
value. Complete carrier is valid/equal by semantic Result plus epoch.

## Events, families and retention

Each Legacy/Observed sibling remains sole owner of its matching local REPO batch.
The repository-source child is eventless. Need, typed outer and cancellation
store no parent batch; warm reuse is silent. Preserve exact event text, prefix,
empty-batch behavior and child-before-parent order.

Legacy dependencies contain only legacy repository source plus the neutral
semantics projection when Present. Observed dependencies contain only observed
repository source plus the same neutral projection when Present. Activate no
repository-ignore, package-preflight, closure, discovery, selected graph,
registry, extension or public caller.

Retain only the local semantic Result Arc plus source epoch. Source child
carrier, source bytes, logical path, reporter, evaluator and event scratch are
dependency-owned or compute-local. Add no second carrier Arc, collection,
cache, store, interner, lock, task, direct Host read, revision, certificate or
new event state.

## Required discriminating proof

Prove:

- distinct identity/hash/Display, `Dupe`/`Allocative`, accessors and
  Complete/Need/outer validity/equality;
- real source Need, typed outer, Absent, Present and semantic error with exact
  source epoch or carrierlessness and semantics nonactivation;
- neutral policy failure plus REPO parse/evaluation success and error with exact
  prefixes and exact legacy semantic parity/Result Arc projection;
- exact epoch iteration and per-demand `Arc::ptr_eq`, held carrier lifetime,
  and source conflict/operation-mismatch outer propagation;
- exact observed and legacy dependency rows and reverse-family isolation;
- source-child event silence, exact parent empty/nonempty/error batches, warm
  suppression and no batch on Need/outer/cancel;
- genuine poll-drop cancellation and identical-request same-DICE recovery;
- local and immutable A -> B -> absent -> directory -> A restoration while
  retaining the original semantic Result and epoch Arcs; and
- zero ignore/preflight/closure/discovery/selected-graph/registry/extension/public
  activation.

Run focused REPO proof, full bzlmod, affected loading/query/core baselines, fmt,
diff-check, exact accounting and AI-cleanup/Buck2 retention review. Reuse
accepted evidence; no fixture or Bazel oracle is authorized.

## Compatibility

Exact: existing nonregistry REPO source order, UTF-8 policy, values/errors,
diagnostics, event text/batches and every legacy result. Slug-native: the private
sibling/carrier, compact source epoch and typed outer. Unsupported/deferred:
nonregistry ignore/preflight/closure/discovery/selected graph; registry
file/preparation/patch observations; generated extension repositories,
rules_rust actions, M8/M7B and exact identity bytes.

## STOP and sole successor

STOP on Rust during design, another file/owner, caller/export, event ownership or
text drift, retained scratch/state, direct Host read, upper/registry activation,
cap excess, M7A closure, M8/M7B/M9 or more than one successor. REPLAN rather than
change source-first ordering, invent a lower producer or merge the REPO event
owner upward.

After independent design ACCEPT schedule exactly one bounded
`WP-6-7A-host-nonregistry-repo-file-observation-implementation`. After its
independent ACCEPT schedule only the docs-only
`WP-6-7A-host-nonregistry-repository-ignore-observation-design`.
