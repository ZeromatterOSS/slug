# Current bounded work packet

Packet: `WP-2A-m1-root-single-observed-analysis-seam-design`

This packet is documentation-only. It records the failed implementation
premise in frozen design `3e90fc88` and designs the uniquely smallest
configured-analysis seam needed before neutral singleton-root-`Single`
implementation may resume. Rust remains at accepted base `31a8b1d3`; the
rejected implementation candidate is not accepted.

## Why REPLAN fired

The two-file neutral-owner implementation reached the intended observed
anchor, observed root-package, target-kind classification and exported-source
carrier. Focused source success, exact-Arc validation, pointer-distinct abort,
revision edit/delete/recreate and cancellation checks passed. The complete core
library exposed a disallowed rule-analysis path: after the neutral owner loaded
`RootPackageLoadObservationKey`, both
`slug_analysis_v2::prepare_configured_node_analysis` and
`ConfiguredNodeAnalysisKey::compute_inner` independently computed
`RootPackageLoadKey`.

That second legacy package family replayed MODULE/`.bzl`/BUILD events and
violated the frozen one-family/event-authority contract. Constructing
`ConfiguredNodeAnalysisKey` directly does not solve it because the key itself
loads the legacy package. Bypassing preparation would also lose exact root
string-setting validation/default transitions. A DICE-only correction is
therefore impossible within the accepted two-file allowlist and caps. The
implementation diff was discarded; no Rust or relocated test file remains.

## Authority

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest;
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`;
- at terminal close only, update the existing 2026-08-18 neutral-owner row in
  `.codex/skills/slug-agent-orchestration/references/routing-log.md`.

Do not edit Rust, Cargo/BUILD metadata, fixtures, oracle data, generated files
or any other plan. Do not restore the rejected candidate from `/tmp`.

## Frozen ownership decision

The natural owner is the configured-analysis family in
`slug_analysis_v2/src/dice.rs`. Add the public-but-hidden structural sibling
`ConfiguredNodeAnalysisObservationKey(ConfiguredNodeAnalysisKey)` and the
public-but-hidden `prepare_configured_node_analysis_observed` entry point. One
private `ConfiguredAnalysisMode::{Legacy, Observed}` driver owns preparation
and analysis semantics for both siblings; neither key computes the other.
Keep the existing legacy API, key value, error text, equality and validity
unchanged.

The observed preparation outcome is a named alias for
`LoadingPreparationOutcome<Result<Result<ConfiguredNodeAnalysisObservationKey,
AnalysisError>, ObservedPathFrontierError>>`. The observed key value is
`LoadingPreparationOutcome<Result<Arc<Result<Arc<ConfiguredNodeResult>,
AnalysisError>>, ObservedPathFrontierError>>`. Need remains outside both Result
layers; the inner error is the existing semantic analysis error and the outer
error is only the typed observation-frontier failure. The shared driver moves
the same semantic Result Arc into either projection.

Mode-aware child helpers select exactly one matching family for every live
edge:

- requested-package and root string-setting package loads use only
  `RootPackageLoadKey` or only `RootPackageLoadObservationKey`;
- execution-platform and toolchain anchor/package closure uses only the
  matching root-module anchor and package family, including iterative native
  reference packages and selected toolchain analysis;
- alias, generated-file, platform/constraint, declared dependency and null
  source recursion prepares and computes only the matching configured-analysis
  sibling; and
- null source resolution uses `ResolvedPathKey` only for legacy and
  `ResolvedPathObservationKey` only for observed analysis.

Observed package, anchor and resolved-path epochs are deliberately discarded
from the analysis terminal after their semantic values are projected. They
remain owned by the already-cached DICE children. No `LoadedPackage`, event
batch or epoch enters key identity or the observed analysis value. Existing
vectors/maps remain compute-local.

The neutral singleton-root owner calls observed preparation after its initial
observed package classification. Preparation requests the same structural
`RootPackageLoadObservationKey`, so DICE reuses that child and event batch; its
target lookup is semantic validation, not a second routing classification or
event owner. The neutral owner then computes only the observed analysis key.
It does not pass a loaded package across crates, compute an existing build-root
child, retain a rule-analysis epoch or validate a partial rule carrier.

## Terminal, event and ordering contract

Observed Need is invalid and unequal. `Complete(Ok(semantic_success))` is valid
and equal by the existing configured result. `Complete(Ok(semantic_error))`
remains invalid and unequal exactly like the legacy analysis key.
`Complete(Err(outer))` is valid/equal by typed outer value, matching the
observed loading frontier while remaining fail-closed at the neutral caller.

Preserve the existing Need-over-semantic-error rule for joined preparation and
analysis children. Any typed outer error beats Need and semantic error because
it is an integrity failure; among outers, retain the first child in the
existing deterministic input/result order. Sequential stages keep their live
order. DICE infrastructure failures retain their current semantic
`AnalysisError` mapping. Cancellation publishes no terminal or local event.

The matching package key remains the sole MODULE/`.bzl`/BUILD event owner.
The selected configured-analysis sibling stores exactly one local analysis
event batch on a completed semantic terminal, including a semantic error, and
stores none for Need or outer failure. Re-requesting the same observed package
or anchor key is DICE reuse, not a second event authority. Cold child-before-
parent order and warm suppression remain exact.

## Future implementation boundary

Against Rust base `31a8b1d3`, write only:

- `app/slug_analysis_v2/src/dice.rs`;
- `app/slug_analysis_v2/src/lib.rs`; and
- `app/slug_analysis_v2/tests/root_analysis.rs`.

Keep the existing small private unit-test module in `dice.rs`; extend it only
for terminal algebra/forced-outer discrimination. Put activation, events,
configuration, recursion and lifecycle coverage in the existing 452-line
`root_analysis.rs` integration owner, which already owns in-memory path epochs
and DICE event tracking. No test relocation or new file is authorized.

Semantic caps are 620 production plus 50 colocated test lines in `dice.rs`, 8
production lines in `lib.rs`, 560 test lines in `root_analysis.rs`, and 1,238
aggregate net lines. Physical caps are 2,880, 65 and 1,015 lines respectively,
with 3,960 combined, from exact baselines 2,208/53/452. No Cargo or BUILD edit
is required.

## Compatibility

Existing public exported-source, Starlark-rule and filegroup results, outputs,
errors and event bytes remain exact. Root string-setting/default-transition
semantics and configured action closure remain exact for the admitted slice.
The internal neutral/observed analysis-family cutover, carrier association and
shared-Arc mechanics are Slug-native. Broader analyzed observation,
multi-target, external/repository/materializer, cquery migration, native-Windows
raw bytes and exact Bazel identity bytes remain unsupported/deferred.

## Proof required by the design

The future implementation must discriminate:

- observed preparation and analysis with only observed anchor, package,
  resolved-path and configured-analysis keys, and zero legacy siblings;
- neutral root rule activation through the same observed package key followed
  by the observed preparation/key seam, with no second event batch;
- exactly one MODULE/`.bzl`/BUILD event sequence, one analysis event, warm
  suppression and no failed-attempt publication;
- default, explicit, edited and restored root string-setting configurations;
- recursive configured dependencies, null sources, aliases, generated files,
  platforms and toolchains without a family escape;
- success/semantic-error/Need/outer equality and validity, mixed outer-over-
  Need-over-semantic ordering, cancellation and no terminal carrier;
- unchanged exported-source exact carrier/revision lifecycle and filegroup
  loaded-only behavior; and
- PackageAll, multi-target, external and cquery family isolation.

Run focused observed-analysis tests, the complete `slug_analysis_v2` suite,
the affected core/loading suites, formatting, diff-check and the archive
checker. Reuse accepted Bazel 9.2 and Slug evidence; add no fixture or oracle.
Finish with Buck2-retention and AI-cleanup scans plus independent review.

## STOP / REPLAN

STOP on implementation, any unlisted file, public API/behavior drift beyond
the named doc-hidden sibling/entry point, a second
package/event family, duplicate driver/event owner, value-carrying key, partial
carrier, new store/cache/interner/lock/task/direct Host read, repository work or
docs cap excess. `REPLAN` if the complete recursive configured-analysis
closure requires an unbounded duplicate, cannot preserve exact root
configuration/event behavior, or has no single natural owner.

## Documentation caps and successor

Against scheduling base `a87a3c8d`, allow at most 40 net lines in canonical,
180 in Stage 2, 160 in this manifest and 30 in the routing row, 410 aggregate.
`git diff --check` must pass.

After independent design acceptance, schedule exactly one bounded
observed-analysis prerequisite implementation. Neutral-root implementation
resumes only after that prerequisite is accepted; do not combine both Rust
packets, activate cquery, or close M1.
