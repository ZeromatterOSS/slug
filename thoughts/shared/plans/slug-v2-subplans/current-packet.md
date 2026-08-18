# Current Slug V2 Packet

Packet: `WP-2A-m1-root-single-neutral-owner-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `0226d60a`
Rust base: `31a8b1d3`
Result: freeze one neutral private singleton-root-`Single` owner that classifies
target kind once and preserves exact public exported-source/rule/filegroup
behavior without computing both legacy and observed root families.

## Authority

This packet is documentation-only. Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`; and
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Only at terminal closure, also write the one audit-REPLAN rollup row in:

- `.codex/skills/slug-agent-orchestration/references/routing-log.md`.

Against `0226d60a`, caps are 40 net canonical lines, 200 current-manifest
lines, 180 Stage 2 lines, 20 routing-log lines and 440 aggregate documentation
lines.

## REPLAN basis and evidence

The predecessor audit proved that its strict constraints are jointly
unsatisfiable. `BuildCommandRootObservationKey::new` must choose a DICE family
from request syntax, but `PackageTargetKind::ExportedFile` is learned only
after root-package loading and target lookup. A legacy package preclassifier
followed by the observed root activates both package families for exported
sources, duplicates child-event authority and adds selected path dependencies
whose exact Result Arcs are absent from the carrier. An observed preclassifier
does the symmetric wrong-family activation for rules/filegroups. An untracked
probe loses DICE dependency/publication ownership or needs a forbidden side
certificate.

M1 nevertheless requires one daemon-owned observation/source-certificate/
publication spine. The smallest coherent design subject is therefore one
neutral private owner for constructor-admitted singleton root-repository
`TargetPattern::Single`, not a parent that computes both existing root keys.
The internal cutover may be Slug-native only after this packet freezes its
event, carrier, memory and public-compatibility contract.

Reuse exact public source behavior and revision evidence in `42f4a64b` and
`f0849151`, the observed publication seam in `31a8b1d3`, pinned Bazel 9.2
`TargetDefinitionContext`/`InputFile`, `ExportsFilesTest`,
`BuildViewTest.testTopLevelInputFile`, `FileFunction`/`FileStateValue`, and
Buck2 DICE dependency/equality/transaction/cancellation rules. No new fixture
or oracle is justified unless the design finds an observable gap.

## Required design decision

Freeze the exact private key and terminal algebra for one singleton-root-
`Single` owner. It must:

- be selected structurally by the sole public build constructor only after the
  existing request validation admits exactly one root-repository `Single`;
- compute one observed loading anchor and one observed root package, then do
  target lookup/kind classification once;
- continue directly to exported-source revision/FileBytes publication or the
  existing rule/filegroup semantic continuation without computing either
  existing build-root sibling or a second package family;
- leave singleton root `PackageAll` on its accepted observed owner and leave
  multi-target, external, recursive and cquery identities unchanged;
- preserve anchor -> package -> lookup/kind -> revision -> FileBytes order,
  first semantic/outer/Need precedence, cancellation and child/final events;
- expose a complete carrier for validation only when the terminal owns every
  selected path demand and exact Result Arc; no partial carrier is admissible;
- preserve the exact FileBytes Arc in the source certificate and through every
  retry/preflight/selection union, with complete validation before revision
  finalization or snapshot commit; and
- project the accepted terminal to the existing public result/event shape
  without retaining classification scratch or a second semantic value.

Decide whether the terminal is an enum with an optional observed carrier or a
single carrier algebra, and prove that rule analysis cannot accidentally
publish a partial epoch. Freeze one child/final event authority for cold, warm,
Need, semantic error, typed outer error, revision retry and cancellation.
Name exact equality/validity behavior and demonstrate that no lock spans DICE.

The retained exported-source state is at most one semantic Result Arc, one
compact exact epoch and the existing source certificate. Rule/filegroup
terminals retain only their existing semantic public value/events; observed
child epochs remain dependency-owned. Driver vectors, kind classification,
unions, comparison and retry state are compute- or command-local. Add no cache,
interner, collection, lock, task, Host read, graph, event store or certificate.

The 14,148-line `runtime/dice.rs` owner is already beyond the complexity
trigger and has three physical lines of prior packet headroom. Choose a bounded
cohesive production/test split before authorizing growth: either a focused
command-owner module with the minimum private seams, or a test sibling that
materially lowers `dice.rs` while keeping private-owner tests discriminating.
Do not authorize a nominal split that leaves the large owner growing. Freeze
the future Rust allowlist, production/test/aggregate net and per-file physical
caps, focused and direct-dependent validation, Buck2 retention scan, AI cleanup
and independent implementation review.

## Compatibility and lifetimes

Existing public exported-source, rule and filegroup result/output/error/event
bytes remain exact. The internal singleton-Single family cutover, carrier
association, exact shared-Arc validation, revision retry and fail-closed outer
handling are Slug-native. Broader analyzed observation, multi-target,
external/repository/materializer, cquery, native-Windows raw bytes and exact
Bazel identity bytes remain unsupported/deferred.

## STOP / REPLAN

STOP on Rust, Cargo, BUILD, fixture, oracle or generated-artifact writes;
implementation claims; public API/behavior drift; a parent that computes both
root families; a second package family; partial carrier validation; duplicate
event/carrier retention; direct or reconstructed Host reads; repository/
materializer work; a nominal complexity split; routing-log omission at
terminal closure; or docs cap excess. `REPLAN` if one neutral owner cannot
preserve exact exported-source revision publication and exact public rule/
filegroup behavior without a side store, duplicate driver, partial epoch or
unbounded split.

## Immediate predecessor

`0226d60a` scheduled the exported-source owner audit from accepted Rust
`31a8b1d3`. The audit's independent review returned `REPLAN`: target kind is
unavailable at constructor family selection, and no neutral preclassifier can
preserve strict family, event and exact-Arc isolation. The required routing-log
row is intentionally carried into this immediate design's terminal closure
because the audit's three-file allowlist forbade that fourth write.
