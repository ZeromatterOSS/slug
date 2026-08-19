# Current Slug V2 Packet

Packet: `WP-6-7A-effective-module-override-observation-proof-cap-correction-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `c2a18a94`
Rust base: `a3efa1b7`
Accepted semantic design: `c2d1f893`
Retained Rust candidate: `app/slug_bzlmod_v2/src/module_eval.rs` at the scheduling-base worktree; non-writable during this design.

## Exact docs authority and measured stop

Write only:
1. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md` (<=40 net);
2. this manifest (<=180 net);
3. `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md` (<=140 net);
4. `.codex/skills/slug-agent-orchestration/references/routing-log.md` (<=30 net).
Aggregate docs net <=390. Do not edit Rust, Cargo/BUILD metadata, fixtures, oracles or any other file.

The retained one-file candidate proves the accepted owner and compiles, but the frozen cap/proof matrix cannot both be satisfied. Against `a3efa1b7`, `module_eval.rs` is currently +170 production/+236 tests/+406 aggregate and 6,458 physical lines. The original +160 production/+240 tests/+400 aggregate cap leaves no room for the smallest production-used root-outcome reducer and only four test lines for the missing dependency-row, cancellation, event and lifecycle discriminators. Forcing the proof under the old cap would either fabricate outcomes outside the live reducer or overcompress the owner.

## Frozen correction

Do not change the accepted owner, value, order, errors, events or retention. The retry remains exactly `app/slug_bzlmod_v2/src/module_eval.rs`, but correct its caps to <=200 production, <=320 tests, <=520 aggregate semantic lines and <=6,700 physical lines from the 6,052-line `a3efa1b7` baseline. This leaves 30 production, 84 test, 114 aggregate and 242 physical lines over the current candidate. It may fund only the frozen proof and small pure seams used by the live driver. Every touched helper remains <200 lines.

Preserve the crate-private structural observed key and crate-private carrier constructor/borrowed accessors. Preserve one Legacy/Observed driver: legacy selects only `RootModuleFilesKey`; observed selects only `RootModuleFilesObservationKey`; both then compute the same `RootModuleCommandPolicyKey` and one pure root-name/command/root/None projection. The legacy wrapper moves the driver's exact Result Arc. Observed forwards the exact root epoch unchanged before semantic inspection. Need and typed outer remain carrierless; root compute failure is empty-prefix, root semantic and every later terminal retain the root prefix. The parent remains eventless and retains only one local Result Arc plus compact epoch.

The retry may retain one production-used pure root-outcome reducer and one pure legacy projection seam. They may only expose the existing terminal algebra for discriminating tests; add no key, hook, injected state, semantic branch, event or retained value.

## Required correction proof

Replace or compact the existing proof so it discriminates:
- the crate-private carrier constructor/accessors, distinct identity/hash/Display and complete-only equality;
- real parent dependency rows: observed root Need has only the observed root-files direct edge and no parent command-policy/later edge, even though the root-MODULE child legitimately reads command policy for ignore-dev;
- the production-used reducer at root Need, typed outer and semantic positions, with exact prefixes, validity/equality and no carrier;
- the command-policy compute-error projector with the exact root prefix;
- `Arc::ptr_eq` for the Result Arc moved through the live legacy projection and every held root-epoch demand Arc;
- root-name rejection, None, root override and command override parity, with root and command create/change/remove/A-B-A restoration;
- cold child-owned event parity, parent eventlessness, warm suppression, a genuinely polled-and-dropped parent compute, no publication, and successful same-DICE recovery;
- both family directions and zero selected-graph/discovered/preparation/repository-definition activation.

Retain exact values/errors/order/normalized command paths and legacy behavior. The sibling/carrier/epoch/typed outer association remains Slug-native. Selected graph, discovery, preparation, registry/nonregistry closure, extensions, generated repositories, external rules_rust actions, M8/M7B and identity bytes remain deferred.

## STOP and successor

STOP on Rust during this design, another file/key/caller/export, semantic or event change, retained child carrier/collection, direct Host read, cache/store/interner/lock/task, selected-graph or later-owner activation, cap excess or milestone closure. REPLAN again if the correction cannot fit.

After independent design ACCEPT, schedule exactly `WP-6-7A-effective-module-override-observation-implementation-retry` with the one-file corrected authority. After that implementation ACCEPT, schedule only `WP-6-7A-selected-module-graph-observation-frontier-design`.
