# Current bounded work packet

Packet: `WP-2A-m1-root-single-observed-analysis-seam-implementation`

Implement only the accepted configured-analysis prerequisite frozen in
`e2cc4119`. Rust remains based on `31a8b1d3`; this packet does not restore or
activate the rejected neutral-root candidate.

## Authority

Write only:

- `app/slug_analysis_v2/src/dice.rs`;
- `app/slug_analysis_v2/src/lib.rs`; and
- `app/slug_analysis_v2/tests/root_analysis.rs`.

Do not edit core, loading, workspace or bzlmod Rust; Cargo/BUILD metadata;
fixtures, oracle data, generated files, plans or routing records.

## Required implementation

Add the doc-hidden structural sibling
`ConfiguredNodeAnalysisObservationKey(ConfiguredNodeAnalysisKey)`, its
doc-hidden constructor/accessors, the doc-hidden
`prepare_configured_node_analysis_observed` entry point and the named observed
preparation outcome alias. Export only the sibling, entry point and alias from
`lib.rs`. Neither configured-analysis key may compute the other.

Refactor preparation and analysis through one private
`ConfiguredAnalysisMode::{Legacy, Observed}` semantic driver. Preserve the
existing legacy API, key value, semantic Arc shape, errors, equality/validity,
event behavior and call graph. Use bounded mode-aware child helpers so every
live edge selects only the matching family:

- requested and root string-setting package loads;
- execution-platform and toolchain root-module anchors;
- initial and iterative toolchain package batches plus selected implementation;
- alias, generated-file, platform/constraint and declared-dependency recursion;
- configured and null child preparation/analysis; and
- null-source path resolution.

Observed mode must use `RootPackageLoadObservationKey`,
`RootModuleLoadingAnchorObservationKey`,
`ConfiguredNodeAnalysisObservationKey` and
`ResolvedPathObservationKey` exclusively. Legacy mode must continue using
only its existing siblings. Project each observed child's semantic value while
leaving its observation epoch dependency-owned; retain no epoch, loaded
package, event batch or other carrier in the observed analysis key/value.

The observed key value is
`LoadingPreparationOutcome<Result<Arc<Result<Arc<ConfiguredNodeResult>,
AnalysisError>>, ObservedPathFrontierError>>`. Need is invalid/unequal;
completed semantic success is valid/equal by the configured result; completed
semantic error remains invalid/unequal like legacy; completed typed outer error
is valid/equal by outer value. The shared driver must move the same semantic
Result Arc into either key projection.

For joined children, typed outer error wins Need and semantic error, and the
first outer in existing deterministic input/result order wins. Without outer,
preserve existing Need-over-semantic and first-semantic ordering. Sequential
stage order and current DICE-infrastructure-to-`AnalysisError` mapping remain
unchanged. Cancellation publishes no terminal or local event.

The matching package child remains the only MODULE/`.bzl`/BUILD event owner.
The selected configured-analysis sibling stores exactly one local analysis
event batch for a completed semantic terminal, including semantic error, and
none for Need or outer. Do not add another event owner or suppress events in
the child keys.

## Proof

Keep the existing private unit tests and add only focused terminal/forced-outer
coverage there. Extend `tests/root_analysis.rs` for:

- distinct legacy/observed key identity and success/semantic-error/Need/outer
  equality and validity;
- observed requested/default-setting preparation with zero legacy package,
  anchor, resolved-path or analysis activation;
- recursive configured/null, alias/generated/platform and toolchain closure
  without a family escape;
- exactly one cold MODULE/`.bzl`/BUILD and analysis event sequence, warm
  suppression and no Need/outer/cancel publication;
- default, explicit, edited, restored and A/B/A root settings;
- semantic parity with legacy results/errors and unchanged legacy lifecycle;
  and
- complete semantic Result-Arc reuse through the shared projection.

Run focused tests, full `cargo test -p slug_analysis_v2`, affected
`slug_loading_v2` and `slug_core_v2 --lib` suites serially,
`cargo fmt --all -- --check`, `git diff --check`, and the archive checker.
Reuse existing
Bazel 9.2 evidence; add no fixture/oracle. Finish with Buck2 retained-state and
AI code-cleanup scans plus independent implementation review.

## Compatibility and memory

Public source/rule/filegroup results, outputs, errors and events remain exact,
as do root string-setting/default transitions and configured action closure.
The observed analysis family and typed association are Slug-native. Broader
analyzed observation, multi-target/external/cquery cutover, repository or
materializer work, native-Windows raw bytes and exact Bazel identity bytes
remain deferred.

Retained state is only the existing semantic Result Arc in the analysis value.
Observed child Arc-backed epochs remain in their DICE values. Mode, package
projections, vectors/maps, joined outcomes and event scratch remain
compute-local. Add no retained collection/cache/interner/store, lock, task,
direct Host read or duplicate semantic driver.

## Caps and STOP

Against `31a8b1d3`, caps are 620 production plus 50 colocated test lines in
`dice.rs`, 8 production lines in `lib.rs`, 560 test lines in
`root_analysis.rs`, and 1,238 aggregate net lines. Physical caps are
2,880/65/1,015 and 3,960 combined from exact baselines 2,208/53/452.

STOP on any other file, cap excess, public behavior/API beyond the named
doc-hidden seam, a legacy edge in observed mode, observed edge in legacy mode,
one sibling computing the other, duplicate driver/event owner, value-carrying
key, retained epoch/carrier, new state/lock/task/Host read, repository work,
fixture/oracle change or nondiscriminating proof. `REPLAN` if exact family
isolation requires another owner or cannot preserve recursive analysis/event
semantics within these caps.

After implementation acceptance, return to docs-only scheduling for the
neutral singleton-root-`Single` implementation retry using frozen design
`3e90fc88` plus this prerequisite. Do not combine that core activation with
this packet or close M1.

This scheduling diff is measured from `e2cc4119`: allow at most 40 net lines
in canonical, 80 in Stage 2, 160 in this manifest and 280 aggregate.
`git diff --check` must pass.
