# Current Slug V2 Packet

Packet: `WP-6-7A-root-module-files-observation-proof-cap-correction-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `dca3c5af`
Rust base: `335cfa45`
Accepted semantic design: `335cfa45`

## Formal REPLAN evidence and exact authority

The retained two-file Rust candidate is cohesive and compiles: full
`slug_bzlmod_v2 --lib` passes 426/426, formatting and diff-check pass, and the
dirty scope is exactly `host_module.rs` plus `module_eval.rs`. Measured against
`335cfa45`, `host_module.rs` is +4 production at 4,535 physical lines and
`module_eval.rs` is 315/30, +285 production at 5,736 physical lines. The latter
exceeds the frozen +180 production cap by 105 lines. The new observed-lockfile
helper is about 66 lines and the shared Legacy/Observed driver about 128 lines;
every touched helper is already below 200. Removing 105 lines would require
macro compression, duplicate ownership or abandoning the accepted shared
driver, so the cap STOP is real rather than a cleanup opportunity.

One proof correction is also required. The candidate currently formats
`HostRootModuleFileError` and lockfile `HostFileError` with Debug text when
projecting the observed aggregate to its `CompactString` semantic result. That
does not discriminate the accepted legacy `RootModuleEvaluationKey` and
`VisibleLockfileKey` error surfaces. The retry must use explicit semantic
projections: preserve exact legacy messages for equivalent command-policy,
validation, evaluation, visible-lockfile mode/read/parse terminals; preserve
the accepted Slug-native Need/typed-outer and Host-only source-kind/path error
classes without Debug-derived public text. Real legacy/observed comparisons
must prove the overlapping root and visible-lockfile errors.

During this design packet write only:

1. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`: <=40 net lines;
2. `thoughts/shared/plans/slug-v2-subplans/current-packet.md`: <=180 net lines;
3. `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`:
   <=140 net lines;
4. `.codex/skills/slug-agent-orchestration/references/routing-log.md`: <=30 net
   lines.

Aggregate docs growth is <=390 net lines. Retain the two Rust files exactly as
the non-writable candidate. Every other file is read-only.

## Frozen retry correction

Keep the owner, structural key/carrier, extension-usage transfer, mode-first
observed lockfile, matching-family driver, root-then-lockfile left-first epoch
algebra, carrierless Need/outer, child-only events, compact Result-Arc+epoch
retention and every selected-graph/extension/analysis STOP from `335cfa45`.
This correction changes no semantic owner, key identity, event owner, public
API, retained collection or caller.

For the retry, authority remains exactly:

1. `app/slug_bzlmod_v2/src/host_module.rs`: <=80 production, <=120 test
   semantic lines and <=4,740 physical lines from 4,531;
2. `app/slug_bzlmod_v2/src/module_eval.rs`: <=340 production, <=300 test
   semantic lines and <=6,100 physical lines from 5,451.

Aggregate semantic growth is <=840 lines and combined physical size is
<=10,840. These measured increases leave 55 production and 364 physical lines
over the current `module_eval.rs` candidate for the explicit projections and
discriminating proof. They do not authorize another helper, owner, file or
retained value. Every new/touched helper stays below 200 lines.

The proof must retain the original identity/Display/validity/equality,
extension-usage, exact epoch Arc/order, Off/no-file, Need/outer/suppression,
duplicate/conflict/mismatch, family/event/cancellation and lifecycle matrix.
Add real comparison of equivalent legacy and observed root validation/
evaluation failures and visible-lockfile mode/read/parse failures. Assert that
the semantic Result contains no Debug-derived error formatting. Host-only
source-kind/path failures remain explicitly Slug-native and structurally
stable. Re-measure production versus test sections rather than charging all
raw lines to one class.

Exact: legacy RootModuleFiles values/errors/order/events and equivalent
observed semantic projections. Slug-native: sibling/carrier/epoch/typed outer,
Need, and Host-only source-kind/path errors. Deferred: selected graph/registry,
extension evaluation/instantiation, generated route/package, external
rules_rust analysis/actions, M8/M7B and identity bytes.

STOP Rust, Cargo/BUILD, fixtures/oracles, exports/callers, cap excess, Debug
error projection, family/order/event/memory drift, another owner/file, direct
Host read, selected graph/extension/package/analysis activation or milestone
closure. If the explicit projections or proof do not fit the corrected caps,
REPLAN again rather than compressing or weakening parity.

After independent design ACCEPT, schedule exactly one successor:
`WP-6-7A-root-module-files-observation-completion-implementation-retry` over
the retained candidate. After implementation ACCEPT, schedule only the
docs-only `WP-6-7A-selected-module-graph-observation-frontier-design`.
