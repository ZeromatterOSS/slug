# Current Slug V2 Packet

Packet: `WP-2A-m1-multi-build-observed-publication-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling and accepted design base: `a2d440cb`
Accepted Rust base: `3f1d4dd4`
Result: implement the accepted bounded observed-publication sibling for
already-admitted root-only multi-target native builds.

## Exact Rust authority and caps

Write exactly:

1. `app/slug_core_v2/src/runtime/dice.rs`: <=380 production plus <=40
   colocated-test net; <=11,700 physical lines.
2. `app/slug_core_v2/src/runtime/tests/build_command_tests.rs`: <=500 test
   net; <=3,900 physical lines.

Aggregate semantic <=920 and combined physical <=15,600 from the accepted
11,264/3,389 baselines. Every other Rust file, Cargo, BUILD, fixture, oracle,
generated artifact and caller/public surface is forbidden.

## Accepted owner decision

The audit accepts the existing `BuildCommandRootKey` aggregation as the
uniquely smallest complete next natural owner. It already owns the structural
ordered target slice, rejects recursive and nonroot multi requests, computes
the target batch in request order, reduces compatible Needs before the first
semantic error, builds the action closure, and hands one semantic Result Arc
to native retry, selection, event reconciliation and atomic publication.

The one-shot `evaluate_workspace_targets{,_with_bzlmod_inputs}` adapters are
not a smaller owner. They create a fresh runtime, eagerly inject a Host-observed
workspace snapshot and directly project `WorkspaceBuildEvaluation` outside
`AcceptedCommand`. Their public/lifecycle migration remains separate and
must not be used to bypass the native aggregate owner.

No lower prerequisite is missing. Accepted observed anchor, root-package and
path children expose the local branch epochs, while observed configured
analysis intentionally exposes only its semantic value and remains represented
by the exact selected dependency closure. `SourceCertificate::from_epoch`
already accepts a nonempty multi-demand epoch. The missing state belongs at the
aggregate root and native validation policy: an observed branch carrier, one
terminal aggregate certificate and one private subset association. Do not
create another DICE producer, branch map, selected snapshot or revision owner.

## Frozen identity and activation

Extend only the existing private `BuildCommandRootObservationKey` admission:

- retain the accepted singleton root PackageAll and external Single identities;
- additionally admit exactly `targets.len() > 1` requests already validated by
  `BuildCommandRootKey`, which therefore contain only root-repository Single
  and PackageAll patterns;
- keep empty requests and root singleton Single on their current legacy/neutral
  routes;
- preserve exact request rejection for recursive, mixed root/external and
  multi-external patterns; and
- keep every direct `BuildCommandRootKey` caller legacy-only.

The observed multi root initializes `RequestRevisionKey` before target-kind
classification because any admitted Single may be an exported source. This is
a private Slug-native dependency change only; public results and no-source
commands remain exact. Repository selection stays `StrictPathOnly`: this
packet admits no external repository sidecar. Add one private terminal
epoch-validation policy, default `Exact`, with
`SelectedDependencySuperset` used only by observed root multi requests. Its
local terminal epoch must be an exact pointer-identical subset of the
closure-selected path epoch. Every other observed root retains full epoch
equality.

## Frozen driver, batch and certificate algebra

Use one bounded mode-aware aggregate driver so legacy projection remains exact
and observed multi selection cannot drift.

1. Compute the matching-family root-module anchor first. Observed mode installs
   its exact epoch before semantic inspection.
2. Compute every target branch with `compute_join` in request order. Legacy
   mode selects only legacy package/analysis/path children. Observed mode
   selects only observed root-package and configured-analysis siblings plus
   the existing exact root FileBytes demand.
3. Each observed Complete branch returns its semantic target/error, its compact
   locally owned anchor/package/source epoch and any source certificate. Need
   and typed outer carry no
   branch carrier and activate no work beyond that branch's existing order.
4. Inspect the full input-ordered batch. Merge every Complete branch epoch
   left-first before semantic inspection. Equal duplicate demands retain the
   earliest exact Arc; value conflict or operation mismatch is a typed outer.
   Preserve the first typed outer, then any incompatible-Need failure, then the
   deterministic union of all compatible Needs, then the first semantic error
   in request order, otherwise ordered success. Do not invent a new public
   BuildCommandError if existing Need kinds cannot union: STOP/REPLAN.
5. DICE/infrastructure failures keep exact legacy post-join behavior. Observed
   failures project to the existing semantic Infrastructure error with the
   reached prefix; they do not panic or become a new outer class.
6. Only after successful target aggregation, compute the configured action
   closure through the matching family and preserve its existing BFS/layer
   order and outer > Need > semantic behavior. Analysis children keep their
   own path/event authority; the command selected snapshot remains their
   acceptance owner.

Build one aggregate `SourceCertificate` from every source-bearing Complete
branch certificate in request order, using stable shared-Arc construction.
Equal duplicates preserve the first certificate Arc; conflict/mismatch fails
closed before terminal selection. No-source terminals retain none. The
certificate must be an exact pointer-identical subset of the terminal epoch.
Retain it beside, not inside or by reconstructing, the one semantic Result Arc;
the public projection still consumes only that Result Arc.

The Complete observed terminal retains exactly one semantic Result Arc, one
local anchor-plus-Complete-branch `PathObservationEpoch`, and at most one
aggregate certificate epoch sharing the same Arcs. Complete carrier equality is
semantic Result+epoch+certificate; Complete typed outer is valid/equal by
outer value; Need is invalid/self-unequal. Branch outcomes, epoch snapshots,
Needs, source lists, action-frontier collections and union scratch are
compute-local.

## Events, acceptance and lifetime

The aggregate root remains eventless. Root-package and analysis children remain
the sole local batch owners and public branch/event order stays exact.
Certificate-bearing multi terminals use the already accepted
`SourceCertifiedCurrentClosure` policy, and implementation must prove every
reachable semantic-Complete event owner stores `Some(batch)`, including
empty; multi terminals without a certificate remain `Strict`. All singleton, query,
cquery, legacy/direct and one-shot policies remain unchanged.

Terminal-first epoch association, selected-demand membership, revision
reobservation, repository materializer acceptance and path/repository/event
snapshot replacement remain the existing native acceptance owner's job. For
the private multi association, forbid repository requests/validations and
require every terminal demand to exist in the selected epoch with exact
demand/value/Arc identity; reject terminal-only demands. Additional selected
demands from configured-analysis and action-closure dependencies remain exact
closure-owned entries copied directly from the command epoch and need not be
duplicated or retained in the terminal. Terminal-first association may install
or prefer local Arcs before selection, but it must not add membership: a local
demand absent from the selected dependency set still fails.
All other observed roots keep exact length/demand/value/Arc equality.
Need/outer/cancel, union conflict, validation, revision, selection, materializer
or publication failure leaves every prior snapshot unchanged and emits no
provisional events.

Retain no branch carrier Arc, child Result Arc, outcome/target map, selected
snapshot duplicate, cache, interner, store, new lock/task or direct Host read.
Keep the existing semantic target/action closure in the Result Arc and the two
compact Arc-backed epochs only. Require `Allocative` and `Dupe`, a Buck2
retention scan, AI cleanup, and touched shared helpers below 200 lines.

## Compatibility and proof

Exact: public multi-build target values/errors/order, configured semantics,
repositories and events; accepted singleton build/query/cquery behavior; every
legacy/direct API and one-shot adapter.

Slug-native: the private multi observed identity, typed outer, local
branch/terminal epoch, selected-dependency-superset association, aggregate
certificate and revision/event association.

Unsupported/deferred: mixed or multi external build, recursive build patterns,
one-shot migration, broader actions/external globs and exact Bazel identity
bytes.

Implementation proof must discriminate:

- identity/Display/equality/validity and routing for empty, singleton, root
  multi, mixed/external and direct legacy requests;
- anchor plus first/middle/last PackageAll, rule, filegroup and exported-source
  branches; Need/outer/semantic positions, later branch completion, full Need
  union, first semantic and exact target order;
- exact epoch demand order/membership and per-demand `Arc::ptr_eq`, equal
  duplicate first Arc, conflict and operation mismatch;
- revision-before-source activation, no-source behavior, two-source aggregate
  certificate exact subset, duplicate sources, source error certificate and
  terminal-first selected Arc survival;
- mixed source+rule and recursive action-closure paths with selected remainder
  change/reuse, exact terminal-subset Arcs, terminal-only demand rejection and
  strict repository-sidecar rejection;
- source+rule+filegroup and repeated-package builds, configured action-closure
  BFS, exact child-before-child public event order, semantic-error batches,
  warm suppression, source edit suppression and BUILD/analysis change replay;
- observed-to-zero-legacy and legacy-to-zero-observed family isolation,
  concurrent roots, and zero external/query/one-shot activation;
- real poll-drop cancellation/recovery, forced revision retry and later Need,
  selection/materializer failure rollback, create/edit/delete/directory/
  recreate and A/B/A for multiple sources and BUILD files; and
- held semantic/certificate/epoch lifetime, exact cap accounting, full focused
  and broad validation, retention/cleanup and independent implementation
  review.

## Implementation terminal

The existing two files are cohesive owner/proof exceptions; do not add a third
Rust file. Run focused multi-build, build-command and native acceptance tests,
then full core/loading/analysis/query coverage, formatting and diff checks.
Record inherited baselines without weakening them. Finish with Buck2 retention,
AI cleanup and an independent implementation review.

STOP/REPLAN on another owner/file, widened external/recursive admission,
legacy/public/one-shot drift, incomplete epoch or certificate, incompatible
Need coercion, changed child event authority, retained branch state, cap excess,
partial validation or M1 closure.

End with exactly one independently reviewed decision: ACCEPT and return only
to one docs-only M1 next-owner audit, or REPLAN. Do not close M1.
