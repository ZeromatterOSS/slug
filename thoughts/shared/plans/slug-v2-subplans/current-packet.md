# Current Slug V2 Packet

Packet: `WP-2A-m1-root-single-neutral-owner-implementation-retry`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Core Rust base: `31a8b1d3`
Frozen design: `3e90fc88`
Accepted prerequisite: `69d37ddb`
Result: publicly publish the exact observed root exported-source carrier through
one neutral singleton-root-`Single` owner while preserving exact rule/filegroup
behavior and every broader legacy family.

## Authority and caps

Write only:

- `app/slug_core_v2/src/runtime/dice.rs`; and
- `app/slug_core_v2/src/runtime/tests/build_command_tests.rs` (new).

Against `31a8b1d3`, exclude only the frozen line-identical test-body relocation
from category growth, never from physical accounting. Caps remain 360
production, 450 test and 810 aggregate net semantic lines. Final physical caps
are 12,275 for `dice.rs`, 2,800 for `build_command_tests.rs` and 15,075
combined.

## Required implementation

Before semantic edits, retain the shared build/cquery fixture block through
`resolved_identity` in the inline parent `tests` module. Move only the
contiguous test tail beginning at
`multi_target_exported_sources_do_not_enter_revision_bridge` through the final
build-branch collection test into `runtime/tests/build_command_tests.rs`.
Replace it with nested
`mod build_command_tests { include!("tests/build_command_tests.rs"); }`; the
new file begins `use super::*;`. Change no relocated test body or production
visibility.

Add private `SingletonRootSingleBuildCommandKey(BuildCommandRootKey)`, admitted
only for a structurally validated singleton root-repository
`TargetPattern::Single`. Its DICE value is complete-only
`SourcePreparationOutcome<Result<SingletonRootSingleBuildCommandTerminal,
ObservedPathFrontierError>>`; equality uses `complete_eq` and validity
requires Complete. Need and observed outer error publish no terminal.

The terminal owns one exact semantic
`Arc<Result<BuildCommandEvaluation, BuildCommandError>>` plus
`Option<PathObservationEpoch>`. Store `Some(epoch)` exactly when that same
semantic result owns a `SourceCertificate`: exported-source success or
completed `RootSource` error. Build the epoch as the stable left-first union
of the observed anchor epoch, observed root-package epoch and certificate's
exact FileBytes demand/Result Arc. Every other terminal stores `None`; never
expose a partial rule-analysis carrier.

Implement `NativeCommandRoot` directly for the neutral key. Preserve the
current sole-root-Single request-revision initialization and analysis-error
root relaxation; expose source certificate/observations only through the
terminal invariant; preserve Need; map observed outer error to typed session
failure. After acceptance, consume the terminal, move its exact semantic Arc
and existing event buffer, and drop the optional epoch.

Extract one post-package `compute_loaded_build_branch` helper shared by legacy
and neutral paths, plus only the minimum shared final action-closure/result
helper needed to avoid a duplicate driver. The neutral driver alone computes:

1. observed root loading anchor;
2. observed root package;
3. target lookup/kind once;
4. existing revision then FileBytes for exported source;
5. `prepare_configured_node_analysis_observed` then
   `ConfiguredNodeAnalysisObservationKey` for rules, projecting only its exact
   semantic Result Arc and leaving every child epoch dependency-owned; or the
   existing loaded-only filegroup continuation; and
6. exact terminal construction.

It computes neither existing build-root sibling, no legacy anchor/package/
configured-analysis key and no second package family. Public constructor
selection is existing observed PackageAll, then neutral singleton root Single,
then unchanged legacy root. Multi-target, external, recursive and cquery
identities remain unchanged.

The neutral root stores no event batch. Observed anchor/package and observed
configured-analysis children remain the only semantic event owners; generic
selected-closure acceptance remains the only command publisher. Rule/filegroup
child epochs are dependency-owned; retain no duplicate terminal epoch,
classification, event batch, collection, cache, interner, lock or task. Hold
no lock across DICE and perform no direct Host read.

## Compatibility and proof

Existing public exported-source, rule and filegroup result/output/error/event
bytes remain exact. The internal neutral family cutover, carrier association,
shared-Arc validation, revision retry and fail-closed outer handling are
Slug-native. Broader analyzed observation, multi-target, external/repository/
materializer, cquery, native-Windows raw bytes and exact Bazel identity bytes
remain unsupported/deferred. Reuse accepted Bazel/source evidence; add no
fixture or oracle.

Require discriminating tests for:

- neutral identity, complete equality/validity and zero legacy package,
  anchor, configured-analysis or resolved-path activation;
- exported success and RootSource error carrier/certificate/selected exact Arc
  identity, including pointer-distinct/missing/extra/value mismatch abort;
- anchor/package/target/rule/filegroup terminals retaining no partial carrier;
- Need, semantic error, observed outer error and cancellation precedence;
- exactly one cold MODULE/`.bzl`/BUILD then analysis event sequence, warm
  suppression and no failed-attempt publication;
- validation before revision finalization/commit, concurrent source retry and
  unchanged/changed/missing/error/delete/recreate/A-B-A lifecycle;
- exact public rule analysis, root setting/default transition, and filegroup
  result/event behavior; and
- accepted PackageAll plus multi/external/cquery nonactivation.

Run focused neutral/public/native-demand tests; the complete core library and
integration suites; the complete loading and analysis suites; formatting,
direct check, `git diff --check` and `scripts/v2_archive_status.sh`. Record
only demonstrated inherited broad/Clippy/archive stops. Run Buck2 retention and
AI cleanup scans and an independent implementation review.

## STOP / REPLAN

STOP on any other file; changed relocated test body; public API/behavior drift;
existing build-root child or legacy/second package or configured-analysis
family; duplicate/partial carrier; new event owner, retained store/collection/
cache/interner/lock/task/Host read; repository/materializer work; Cargo/BUILD/
fixture/oracle/generated writes; or cap excess. `REPLAN` on a required third
Rust file, partial selected epoch, duplicate driver/event authority, unsafe
revision ordering, unbounded split or any inability to preserve exact public
rule/filegroup behavior.

## Immediate predecessors

`3e90fc88` accepts the neutral-owner design after `23f9c8d1` recorded the
constructor-kind REPLAN. The first implementation attempt then stopped because
configured rules re-entered the legacy package family. `69d37ddb` accepts the
callerless observed configured-analysis preparation/key family through root
settings, recursive/null/delegating nodes, platforms and toolchains. This retry
must consume that exact seam; it may not reconstruct or bypass analysis
preparation in core.
