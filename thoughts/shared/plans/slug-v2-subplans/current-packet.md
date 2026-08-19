# Current Slug V2 Packet

Packet: `WP-6-7A-effective-module-override-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Rust base: `a3efa1b7`
Accepted design: `c2d1f893`

## Exact authority and caps

Write only `app/slug_bzlmod_v2/src/module_eval.rs`, from the 6,052-line
`a3efa1b7` baseline: <=160 production and <=240 test semantic lines, <=400
aggregate semantic lines and <=6,500 physical lines. The file is a cohesive
large-owner exception; every new/touched helper is <200 lines. Every other file
is read-only.

## Frozen owner and carrier

Add crate-private structural
`HostEffectiveModuleOverrideObservationKey` with the same
workspace+module-name identity as the legacy key and a distinct observed
Display. Add
`ObservedHostEffectiveModuleOverride { result, observations }` with a
crate-private constructor and borrowed accessors. `result` is exactly one
local
`Arc<Result<HostEffectiveModuleOverride, HostEffectiveModuleOverrideError>>`;
`observations` is one compact `PathObservationEpoch`. Require `Dupe` and
`Allocative`. Add no export or caller.

Use one Legacy/Observed effective-override driver. Legacy selects only
`RootModuleFilesKey`; observed selects only
`RootModuleFilesObservationKey`. Both then compute the same
`RootModuleCommandPolicyKey` and run one pure projection for root-name
rejection, command override precedence, root override and None. Neither sibling
computes the other. The legacy wrapper moves the exact local Result Arc
unchanged.

## Frozen order and terminal algebra

Order is root files first, then command policy, then pure projection. Observed
root Complete installs its epoch before semantic inspection and forwards that
epoch unchanged. There is no second observed child or epoch union.

- root DICE compute failure is semantic `RootModuleFiles` with empty prefix;
- root semantic failure retains the full root-files prefix;
- root Need or typed outer returns immediately with no carrier and activates no
  command policy;
- command-policy DICE failure is semantic `CommandPolicy` with the root prefix;
- forbidden root-name command override, command override, root override and
  None all retain the root prefix.

Preserve existing error classes/messages and exact override values, including
normalized command-local path identity. Need is invalid/self-unequal; Complete
typed outer is valid/equal by outer value; Complete carrier is valid/equal by
semantic Result plus epoch. There is no Need union, joined batch, new error
class or semantic Debug projection.

Both siblings are eventless. Root-files/root-MODULE children keep sole ownership
of existing batches. Need/outer/cancellation publishes none and warm reuse stays
silent.

## Retention, proof, compatibility and STOP

Retain only the effective-override semantic Result Arc plus compact epoch.
Root-files carrier, command-policy value, normalized-path temporary and driver
scratch remain dependency-owned or compute-local. Add no map, collection,
cache, store, interner, lock, task, direct Host read, revision, certificate or
event owner.

Prove:

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

Run focused owner tests, full bzlmod, affected accepted loading/query/core
baselines, fmt, diff-check, cap accounting and AI-cleanup/Buck2 retention
review.

STOP on every other file, caller/export, selected graph/discovered/
source-preparation/repository-definition activation, family/order/error/event/
retention drift, direct Host read, cap excess or M7A/M8/M7B/M9 closure. REPLAN
rather than duplicate root-files projection, retain the child carrier or invent
another owner.

After independent ACCEPT, schedule exactly one docs-only successor:
`WP-6-7A-selected-module-graph-observation-frontier-design`.
