# Current Slug V2 Packet

Packet: `WP-6-m2e-analysis-error-activation-sidecar-prerequisite`
Milestone: M4 configured query
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Cross-owner: `app/slug_core_v2/src/runtime/dice.rs`
Result: prerequisite correction after root delegating-node activation.

## Observable slice

Preserve the complete analysis error selected by build/cquery command roots
while `ConfiguredNodeAnalysisKey` remains success-only for DICE validity and
equality. Missing-executable and unsupported-native-root requests must publish
their existing semantic diagnostics and recover after edits instead of failing
activation-closure selection with an unavailable root node.

## Ownership and stops

Keep `ConfiguredNodeAnalysisKey` as the sole semantic analysis owner and keep
complete errors invalid/non-equal. Correct the command-effect/activation
sidecar ownership needed to select an error terminal; do not make errors valid,
add a peer analysis key/cache, or weaken success equality. Reuse the existing
activation and command-effect substrate.

Exclude configured-query traversal/output breadth, new delegating node kinds,
external repository topology, platform/toolchain retention, parser changes,
exact Bazel hash bytes, JVM/Java, CI, and compatibility behavior. Vendored
Buck2 `starlark-rust` remains the sole Starlark parser/evaluator substrate.

## Validation

First pin the clean `7eeb59db` failure in the focused core/server error cases.
Then prove missing-executable and unsupported-native diagnostics, edit recovery,
success warm reuse, and unchanged external-visibility/CLI-cycle baselines. Run
focused analysis/core/server/CLI tests serially and rebuild `slug_cli_v2` before
CLI validation. Stop if the correction requires error validity, a second
semantic owner, parser work, or query breadth.
