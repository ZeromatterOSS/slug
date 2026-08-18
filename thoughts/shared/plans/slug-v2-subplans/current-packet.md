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

## Frozen design

Add private `SingletonRootSingleBuildCommandKey(BuildCommandRootKey)`, whose
constructor accepts only a structurally validated singleton root-repository
`TargetPattern::Single`. Its DICE value is a complete-only
`SourcePreparationOutcome<Result<SingletonRootSingleBuildCommandTerminal,
ObservedPathFrontierError>>`. The terminal owns exactly the semantic
`Arc<Result<BuildCommandEvaluation, BuildCommandError>>` plus an optional
`PathObservationEpoch`; equality uses `complete_eq` and validity requires
Complete. Need and observed outer error publish no terminal.

The owner must:

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
- store `Some(epoch)` exactly when the semantic result owns a
  `SourceCertificate`: exported-source success or completed `RootSource`
  error. That epoch is the stable left-first union of anchor, package and the
  certificate's exact FileBytes demand/Result Arc. Every other terminal stores
  `None`; no partial carrier is admissible;
- preserve the exact FileBytes Arc in the source certificate and through every
  retry/preflight/selection union, with complete validation before revision
  finalization or snapshot commit; and
- project the accepted terminal to the existing public result/event shape
  without retaining classification scratch or a second semantic value.

Implement `NativeCommandRoot` directly for the neutral key. It always
initializes the existing request revision, matching the current syntactically
sole root-Single path; it exposes the source certificate and observations only
through the terminal invariant above, preserves the current analysis-error
root relaxation, maps observed outer error to typed session computation
failure, and preserves Need. The accepted terminal projection consumes the
terminal, moves its exact semantic Arc and existing event buffer, and drops the
optional carrier.

Refactor the current root branch after package loading into one
`compute_loaded_build_branch` helper used by both legacy multi-target and the
neutral owner, and one result/action-closure finalizer if needed. The neutral
driver itself owns observed anchor -> observed package -> target lookup/kind;
it never computes `BuildCommandRootKey`, `BuildCommandRootObservationKey`, a
legacy anchor/package key or a second package load. Public selection order is
accepted PackageAll observed owner, then neutral singleton root-Single owner,
then unchanged legacy root. No other caller changes.

The neutral root stores no event batch. Existing observed anchor/package and
configured-analysis children remain the only semantic event owners, and the
generic selected-closure buffer remains the only command publication owner.
Cold order stays anchor before package before analysis; Need/cancellation
publish nothing; warm children do not replay. No lock spans a DICE compute.

The retained exported-source state is at most one semantic Result Arc, one
compact exact epoch and the existing source certificate. Rule/filegroup
terminals retain only their existing semantic public value/events; observed
child epochs remain dependency-owned. Driver vectors, kind classification,
unions, comparison and retry state are compute- or command-local. Add no cache,
interner, collection, lock, task, Host read, graph, event store or certificate.

Before semantic edits, retain the shared build/cquery fixture block in the
existing parent `tests` module through `resolved_identity`, then move the test
tail beginning at `multi_target_exported_sources_do_not_enter_revision_bridge`
through the final build-branch collection test into
`runtime/tests/build_command_tests.rs`. At that location declare
`mod build_command_tests { include!("tests/build_command_tests.rs"); }`; the new
file begins `use super::*;`. `include!` resolves relative to `runtime/dice.rs`,
and the nested child can access every retained private parent fixture without
exporting it. The roughly 2,267-line test-body relocation changes no test body
or production visibility and materially lowers `runtime/dice.rs`.

Future Rust writes are exactly:

- `app/slug_core_v2/src/runtime/dice.rs`; and
- `app/slug_core_v2/src/runtime/tests/build_command_tests.rs` (new).

Against `31a8b1d3`, excluding the line-identical relocation from category
growth but not from physical accounting, caps are 360 production, 450 new/test
and 810 aggregate net semantic lines; final physical caps are 12,275 for
`dice.rs`, 2,800 for `build_command_tests.rs` and 15,075 combined. Any required
third Rust file or cap excess is `REPLAN`.

## Required implementation proof

Require neutral identity/equality/validity and exact activation matrices;
exported success plus RootSource error exact carrier/certificate Arc identity;
anchor/package/target/rule/filegroup terminals with no partial retained carrier;
Need/semantic/outer/cancellation precedence; cold event order and warm
suppression; complete carrier-versus-selected validation before revision
finalization; pointer-distinct/missing/extra/value mismatch abort; unchanged,
changed, missing, error, delete/recreate and A-B-A source lifecycle; concurrent
revision retry; rule analysis and filegroup public result/event parity; and
PackageAll/multi/external/cquery family isolation. Re-run focused core tests,
the full core library/integration and loading suites, formatting, direct check,
diff check and archive status; record inherited broad/Clippy stops precisely.
Run Buck2 retention and AI cleanup scans plus independent implementation review.

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
