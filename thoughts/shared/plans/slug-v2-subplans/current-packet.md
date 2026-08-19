# Current Slug V2 Packet

Packet: `WP-6-7A-root-module-files-observation-completion-implementation-retry`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Rust base: `335cfa45`
Accepted semantic design: `335cfa45`
Accepted cap/proof correction: `47746115`

## Exact authority and caps

Write only:

1. `app/slug_bzlmod_v2/src/host_module.rs`: <=80 production, <=120 test
   semantic lines and <=4,740 physical lines from 4,531;
2. `app/slug_bzlmod_v2/src/module_eval.rs`: <=340 production, <=300 test
   semantic lines and <=6,100 physical lines from 5,451.

Aggregate semantic growth is <=840 lines and combined physical size is
<=10,840. Both files are cohesive large-owner exceptions; every new/touched
helper is <200 lines. Every other file is read-only.

## Frozen implementation contract

Add a private structural `RootModuleFilesObservationKey` and
`ObservedRootModuleFiles { result, observations }`, where `result` is exactly
one local `Arc<Result<RootModuleFiles, CompactString>>` and `observations` is
one compact `PathObservationEpoch`. Require `Dupe` and `Allocative` with only
crate-private borrowed accessors; add no export or caller.

Move the existing evaluated `Arc<[RootExtensionUsage]>` into the private
`HostRootModuleFileValue` instead of dropping it in
`evaluate_root_module_terminal`. Create no second usage collection. One
mode-aware RootModuleFiles driver must preserve exact legacy
`RootModuleEvaluationKey` -> `VisibleLockfileKey` selection and move its local
Result Arc unchanged into `RootModuleFilesKey`. Observed mode selects only
`HostRootModuleFileObservationKey` and the private observed lockfile projection;
neither sibling computes the other.

Observed lockfile handling preserves legacy order: compute
`RootModuleLockfileModeKey` first; Off returns `VisibleLockfileRead::Ignored`
without a file key; otherwise compute `HostFileBytesObservationKey` for
`MODULE.bazel.lock` and use the same
`parse_visible_lockfile_bytes_for_mode`. Do not use `WorkspaceRawFileKey`,
`HostVisibleLockfileKey` or a direct Host API in observed mode.

Merge the Complete root epoch first and a Complete lockfile epoch left-first
before semantic inspection. Equal duplicate demands retain the first exact Arc;
conflict/operation mismatch is typed `ObservedPathFrontierError`. Root compute
failure is semantic with empty prefix; root semantic retains root prefix;
lockfile-mode compute failure retains root; Off retains root only; file/parse/
success retains root+lockfile. Need/typed outer is immediate and carrierless,
with no later activation. There is no Need union or joined batch.

Use explicit semantic-error projectors rather than Debug formatting. Equivalent
command-policy, root/include validation/evaluation and visible-lockfile
mode/read/parse terminals reproduce the accepted legacy `CompactString`
messages. Slug-native Need/typed outer and Host-only path/source-kind terminals
remain structurally distinct and use stable explicit messages. Do not expose a
Rust enum Debug representation as a semantic error.

Need is invalid/self-unequal; Complete outer is valid/equal by outer; Complete
carrier is valid/equal by semantic Result+epoch. Parent siblings are eventless.
Matching root-module children remain sole owners of their exact local MODULE
batches; Need/outer/cancel publishes none and warm reuse suppresses replay.

Retain only the local RootModuleFiles Result Arc plus epoch. Child carriers,
source/AST/evaluator/include horizon/ancestry/parser/event/union scratch remain
dependency-owned or compute-local. Existing semantic collections may live only
inside the required Result Arc. Add no collection, cache, store, interner, lock,
task, Host read, revision or certificate.

## Proof, compatibility and STOP

Prove identity/Display/validity/equality; exact legacy Result/error/value and
projection-Arc parity; real `use_extension`/`use_repo("rust_toolchains")`
retention; exact root->lockfile epoch order and per-demand ptr identity; Off
no-file activation; missing/present/bad lockfile and mode error prefixes; root
and lockfile Need/outer/semantic suppression; duplicate/conflict/mismatch; both
family directions and concurrent isolation; child-only MODULE events/warm
suppression; real cancellation/recovery; and MODULE/include/lockfile
create-edit-delete-recreate plus A/B/A and held-Arc lifetime.

Add real legacy/observed comparisons for equivalent root validation/evaluation
and visible-lockfile mode/read/parse failures. Assert exact legacy messages and
stable Host-only typed messages, with no Debug-derived semantic string.

Exact: root MODULE/include/extension-usage/visible-lockfile values, equivalent
errors, legacy Result and child events. Slug-native: sibling/carrier/epoch/typed
outer, Need and Host-only path/source-kind errors. Deferred: selected graph/
registry, extension evaluation/instantiation, generated route/package,
external rules_rust analysis/actions, M8/M7B and identity bytes.

Run focused owner tests, full bzlmod, affected accepted loading/query/core
baselines, fmt, diff-check, cap accounting and AI-cleanup/Buck2 retention review.

STOP on any other file, export/caller, family/order/error/event/retention drift,
Debug-derived semantic error, direct Host read, selected graph/registry/
extension/package/analysis activation, Cargo/BUILD/fixtures/oracles, cap excess
or M7A/M8/M7B/M9 closure. REPLAN rather than weaken Arc validation, compress
the shared owner or synthesize extension usages.

After independent ACCEPT, schedule exactly one docs-only successor:
`WP-6-7A-selected-module-graph-observation-frontier-design`.
