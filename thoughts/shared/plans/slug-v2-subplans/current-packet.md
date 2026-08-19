# Current Slug V2 Packet

Packet: `WP-6-7A-effective-module-override-observation-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling and Rust base: `a3efa1b7`

## Completion and frontier-audit record

Accepted `a3efa1b7` completes the root MODULE-files observation owner. Final
accounting against `335cfa45` is +76 production/+119 tests in
`host_module.rs`, +303 production/+298 tests in `module_eval.rs`, +796
aggregate semantic lines and 10,778 physical lines. Focused 2/2 and full
`slug_bzlmod_v2 --lib` 428/428 pass; accepted loading 138/138 and query 53/53
remain green. Core remains 245/246 only at the recorded inherited stale
visibility wording. Formatting, diff hygiene, archive-baseline disposition,
Buck2 retention, AI cleanup and independent terminal review pass.

The selected-graph frontier audit finds one uniquely smaller prerequisite.
`HostSelectedModuleGraphKey` begins with `RootModuleFilesKey`, then repeatedly
computes `HostEffectiveModuleOverrideKey` before any discovered-module
horizon. The same effective-override key is also the first shared dependency of
`HostDiscoveredModuleKey`, `ModuleSourcePreparationKey`, nonregistry
preflight and selected repository-definition projection. It currently computes
carrierless `RootModuleFilesKey` and command policy. Therefore an observed
selected graph cannot preserve the accepted root epoch without either this
sibling or duplicated override semantics.

Do not freeze selected graph or discovered-module siblings yet.
`HostDiscoveredModuleKey` still reaches carrierless registry preparation and
nonregistry closure branches; those are the next frontier audit after this
small shared child. This is a formal smaller-prerequisite result, not permission
to activate another caller.

## Design write authority and measured future scope

This design packet may edit only:

1. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`: <=40 net;
2. this manifest: <=180 net;
3. `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`:
   <=160 net;
4. `.codex/skills/slug-agent-orchestration/references/routing-log.md`: <=30 net.

Aggregate docs growth is <=410 net lines. Rust, tests, Cargo/BUILD, fixtures and
oracles are read-only during design.

After independent design ACCEPT, the exact future Rust authority is only
`app/slug_bzlmod_v2/src/module_eval.rs`, from the 6,052-line
`a3efa1b7` baseline: <=160 production and <=240 test semantic lines, <=400
aggregate semantic lines and <=6,500 physical lines. The file is a cohesive
large-owner exception; every new/touched helper is <200 lines. No export,
caller or second file is authorized.

## Frozen owner and carrier

Add a crate-private structural `HostEffectiveModuleOverrideObservationKey`
with the same workspace+module-name identity as the legacy key and a distinct
observed Display. Add
`ObservedHostEffectiveModuleOverride { result, observations }` with a
crate-private constructor and borrowed accessors, where
`result` is exactly one local
`Arc<Result<HostEffectiveModuleOverride, HostEffectiveModuleOverrideError>>`
and `observations` is one compact `PathObservationEpoch`. Require `Dupe`
and `Allocative` plus crate-private borrowed accessors. Retain no child Result
Arc or other collection.

Use one Legacy/Observed effective-override driver. Legacy selects only
`RootModuleFilesKey`; observed selects only
`RootModuleFilesObservationKey`. Both then compute the same
`RootModuleCommandPolicyKey` and run one pure projection for root-name
rejection, command override precedence, root override and None. Neither sibling
computes the other. The legacy wrapper must move the exact local Result Arc
unchanged.

## Frozen order and terminal algebra

Order is root files first, then command policy, then pure projection. Observed
root Complete installs its epoch before semantic inspection and forwards that
epoch unchanged; there is no second observed child and no epoch union.

- root DICE compute failure is semantic `RootModuleFiles` with empty prefix;
- root semantic failure retains the full root-files prefix;
- root Need or typed outer returns immediately with no carrier and activates no
  command policy;
- command-policy DICE failure is semantic `CommandPolicy` with the root prefix;
- forbidden root-name command override, command override, root override and
  None all retain the root prefix.

Preserve the existing error classes/messages and exact override values,
including normalized command-local path identity. Need is invalid/self-unequal;
Complete typed outer is valid/equal by outer value; Complete carrier is
valid/equal by semantic Result plus epoch. There is no Need union, joined batch,
new error class or semantic Debug projection.

Both siblings are eventless. Root-files/root-MODULE children keep sole ownership
of their existing batches. Need/outer/cancellation publishes none and warm reuse
stays silent.

## Retention, proof, compatibility and STOP

Retain only the effective-override semantic Result Arc plus compact epoch.
Root-files carrier, command-policy value, normalized-path temporary and driver
scratch remain dependency-owned or compute-local. Add no map, collection,
cache, store, interner, lock, task, direct Host read, revision, certificate or
event owner.

Required proof:

- distinct key identity/hash/Display and complete-only validity/equality;
- exact legacy Result/value/error and projection-Arc parity;
- observed exact root epoch equality and per-demand `Arc::ptr_eq`, with no
  added demands;
- root Need/typed outer/semantic and command-policy compute-error prefixes,
  carrierlessness and later-child suppression;
- root-name rejection, command override, root override and None parity;
- both family directions and zero selected-graph/discovered/preparation/
  repository-definition activation;
- parent eventlessness, child event parity, warm suppression and real
  poll-drop/successor recovery;
- root override and command override create/edit/remove/A-B-A plus held Result
  and epoch Arc lifetime;
- final cap, Allocative/retention and cleanup scan.

Exact: effective override values/errors/order, normalized command path and
legacy Result behavior. Slug-native: observed sibling/carrier/epoch/typed outer
association. Unsupported/deferred: selected graph, discovered registry and
nonregistry modules, extension evaluation/instantiation, generated repository
mapping/package loading, external rules_rust analysis/actions, M8/M7B and exact
identity bytes.

STOP on Rust during design; during implementation stop on every other file,
caller/export, selected graph/discovered/source-preparation/repository
activation, family/order/error/event/retention drift, direct Host read, cap
excess or M7A/M8/M7B/M9 closure. REPLAN rather than duplicate root-files
projection, retain the child carrier or invent another owner.

After independent design ACCEPT, schedule exactly one bounded
`WP-6-7A-effective-module-override-observation-implementation`. After its
independent implementation ACCEPT, return only to the docs-only
`WP-6-7A-selected-module-graph-observation-frontier-design`.
