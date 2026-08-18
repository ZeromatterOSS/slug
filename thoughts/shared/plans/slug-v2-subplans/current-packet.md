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

## Design objective

Audit `slug_analysis_v2/src/dice.rs` around
`prepare_configured_node_analysis`, `ConfiguredNodeAnalysisKey`,
`compute_configured_child`, root string-setting default lookup and every
recursive configured-analysis/package edge. Freeze the smallest DICE-owned
observed continuation that lets a neutral root rule proceed without activating
legacy `RootPackageLoadKey` after observed classification.

Decide whether the natural owner is:

1. a structurally distinct observed configured-analysis key plus observed
   preparation entry point sharing one mode-aware semantic driver with the
   legacy key; or
2. one strictly smaller prerequisite that can preserve the already-loaded
   observed package/result/event authority through root preparation and the
   recursive analysis closure.

Do not design a key carrying a `LoadedPackage` value or event batch as
identity, a side store, direct Host read, caller-managed cache, or a parent that
computes both analysis/package families. The result must keep DICE dependencies
structural and leave child observation epochs dependency-owned; the neutral
build terminal still retains no partial rule-analysis epoch.

Freeze:

- legacy and observed key identities, complete-only equality/validity and
  exact family selection;
- root requested-package preparation, required string-setting validation,
  explicit/default configuration and recursive child/toolchain/platform
  behavior;
- Need, semantic error, typed outer error and cancellation precedence;
- exactly one package/`.bzl`/analysis event authority and unchanged cold
  order/warm suppression;
- no duplicate retained package, event batch, epoch, collection, cache,
  interner, lock or task;
- how the neutral root calls the new seam without an existing build-root child
  or legacy package activation;
- future Rust allowlist, test-module ownership, production/test/aggregate and
  physical caps measured from `31a8b1d3`; and
- focused activation/event/configuration/lifecycle proof plus broad
  core/loading validation, formatting, archive, retention, cleanup and
  independent review.

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

- neutral root rule activation with observed anchor/package/analysis only and
  zero legacy package/analysis sibling activation;
- exactly one MODULE/`.bzl`/BUILD event sequence, one analysis event, warm
  suppression and no failed-attempt publication;
- default, explicit, edited and restored root string-setting configurations;
- recursive configured dependencies/action closure without a legacy-family
  escape;
- rule semantic/Need/error/outer/cancellation parity and no terminal carrier;
- unchanged exported-source exact carrier/revision lifecycle and filegroup
  loaded-only behavior; and
- PackageAll, multi-target, external and cquery family isolation.

Reuse accepted Bazel 9.2 and Slug evidence; add no fixture or oracle.

## STOP / REPLAN

STOP on implementation, any unlisted file, public API/behavior drift, a second
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
