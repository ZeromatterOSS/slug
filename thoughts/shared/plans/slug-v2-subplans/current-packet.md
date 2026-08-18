# Current Slug V2 Packet

Packet: `WP-2A-m1-external-package-source-load-observation-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `7bc9e1da`
Rust base: `e4555dca`
Result: freeze the private observed routed Host path/source substrate as the
uniquely smaller prerequisite before external package-load and loading-query
publication; do not implement it.

## Authority and caps

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest;
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`; and
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`.

Cap net growth at 40 canonical, 220 manifest, 180 Stage 2, 30 routing and 470
aggregate lines against `7bc9e1da`.

## Frozen future Rust boundary

The accepted design schedules one implementation that may write only:

- `app/slug_bzlmod_v2/src/source_preparation.rs`;
- new `app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs`; and
- `app/slug_bzlmod_v2/src/lib.rs`.

Against Rust base `e4555dca`, future semantic net growth is capped at 340
production plus 10 test-glue lines in `source_preparation.rs`, 520 test lines in
the new test file, 6 production lines in `lib.rs`, and 876 aggregate lines.
Current physical bases are 12,247 and 395 lines; final physical size is capped
at 12,600 for `source_preparation.rs`, 540 for the new test file, 405 for
`lib.rs`, and 13,545 combined.

`source_preparation.rs` is already a 12,247-line cohesive materialization,
routed-path and byte-source owner. Do not create a false production seam. Put
all new focused tests in the new file and add only this nested test-tail glue
inside the existing `tests` module:

```rust
mod observation_tests {
    use super::*;
    include!("source_preparation_observation_tests.rs");
}
```

The include resolves beside `source_preparation.rs` and reuses existing private
test fixtures without moving or changing existing test bodies.

## Frozen owner and value algebra

Add structurally distinct `HostRepositoryPathObservationKey` and
`HostRepositorySourceFileObservationKey` newtypes around the corresponding
legacy key identities. Keep the path sibling crate-private. Export only the
source observation key and carrier from `slug_bzlmod_v2` under `#[doc(hidden)]`
for the later loading consumer. Give each legacy/observed pair one private
mode-aware driver: legacy computes only the legacy child family, observed only
the observed child family, and neither sibling computes the other.

The observed path and source values are
`SourcePreparationOutcome<Result<Carrier, ObservedPathFrontierError>>`.
Each Complete carrier retains exactly one semantic
`Arc<Result<..., RepositorySourceFileError>>` plus one Arc-backed
`PathObservationEpoch`, is `Allocative`/cheaply cloneable, and exposes borrowed
doc-hidden accessors where cross-crate composition requires them. Need returns
immediately with no carrier; an observed child outer remains typed outer;
semantic path/source errors remain inside the carrier Result and are DICE-valid
and equal exactly when Complete.

The path driver preserves validation and materialization semantics. Invalid
relative paths, route/materialization errors and materialization compute errors
complete semantically with an empty epoch because no path was selected.
Materialization Need remains Need. Legacy resolution uses only `ResolvedPathKey`.
Observed resolution uses only `ResolvedPathObservationKey`, forwards its exact
epoch on semantic success or error, and projects only its typed outer outward.

The source driver computes its matching path sibling first. A semantic path
error completes with that exact prefix epoch. Missing or wrong-kind paths do
the same. A present regular/special file preserves the existing direct
materialization-provenance dependency, then the observed branch computes
`PathObservationKey(FileBytes)` for the selected real path. Append that exact
shared Result Arc to the path epoch with `PathObservationEpoch::from_shared`;
path observations are left-first, an equal duplicate preserves the path
carrier's first Arc, and mismatch/conflict is typed outer. FileBytes Need
returns Need without a partial carrier. Bytes success, observation error, or
the selected-file disappearance race completes semantically with the complete
path-plus-FileBytes epoch. Legacy projection and error text remain unchanged.

## Events, memory and compatibility

Neither path nor source sibling stores an event batch. Existing materialization
and future REPO/ignore/BUILD/`.bzl` parents remain their own event authorities;
cancellation, Need and typed outer publish nothing. Retain no request graph,
route list, queue, map, store, cache, interner, lock, task or direct Host read.
All union scratch is compute-local. The compact carrier uses the accepted
Arc-backed `PathObservationEpoch`; do not replace it with another collection or
rebuild any selected Result Arc.

Routed path resolution, source bytes, missing/wrong-kind and race diagnostics,
local/immutable materialization namespaces, symlink behavior, errors and exit
classification remain exact. Structural observed keys, typed outer and carrier
association are Slug-native. REPO/ignore/BUILD/package/module observed parents,
recursive external `.bzl` loading, `RootQueryCommandKey`, multi-build, one-shot
evaluation, broader repository kinds and exact identity bytes remain deferred.
This reuses accepted Bazel 9.2 `FileFunction` path/symlink behavior and existing
direct-local source lifecycle evidence; add no fixture or oracle.

## Discriminating proof and validation

Add focused tests for:

- structural observed identity, Complete-only equality/validity, and exact
  legacy/observed semantic parity;
- invalid relative path and route/materialization semantic errors with empty
  epochs, plus local and immutable materialization namespaces;
- missing, wrong-kind, regular/special, symlink-retarget, FileBytes observation
  error and selected-file disappearance race outcomes;
- exact demand/value/`Arc::ptr_eq` membership for every resolution Lstat,
  ReadLink and selected FileBytes Result, including a duplicate whose left path
  Arc wins;
- injected path/FileBytes Need, typed outer, union mismatch/conflict, and
  semantic-prefix polarity with no partial carrier;
- cold/warm, edit/delete/recreate and A/B/A restoration, route/materialization
  identity changes, reverse legacy/observed family nonactivation, and zero local
  path/source event batches; and
- a real polled-pending cancellation with no publication followed by recovery,
  plus compact post-return retention and a callerless observed source compute
  proving the future loading consumer can receive the full carrier.

Run serially:

1. the new focused observation tests, including cancellation alone and in the
   default-parallel focused batch;
2. the full `slug_bzlmod_v2` library and integration suites;
3. full `slug_loading_v2` and `slug_query_v2` suites;
4. the established `slug_core_v2` library/integration checks, recording only
   the two inherited baselines if unchanged;
5. `cargo fmt --all -- --check`, `cargo check -p slug_bzlmod_v2`,
   `git diff --check e4555dca`, exact semantic/physical accounting, Buck2
   retention scan and AI cleanup categories 1-9.

Require independent design review. After ACCEPT, commit this docs-only design
and schedule exactly one bounded Host repository source-observation
implementation from Rust base `e4555dca` plus the accepted design. After its
implementation review and commit, schedule exactly one docs-only external
package source/load frontier design; that design must return directly to
loading-query publication after its bounded implementation. Do not close M1.

## STOP / REPLAN

STOP on Rust, Cargo, BUILD, fixture, oracle or generated-file writes; public or
upper-loader activation; a third key family; a production split; existing test
body movement; computing legacy and observed children together; direct
`ResolvedPathKey` or direct FileBytes observation in any upper observed parent
after these shared source drivers exist; Result-Arc reconstruction; partial carrier on
Need/outer; duplicate event ownership; retained scratch or a new
store/cache/interner/lock/task/Host read; semantic drift; nondiscriminating
proof; missing future allowlist/caps; multiple successors; docs cap excess; or
M1 closure. `REPLAN` if the exact resolution/FileBytes
Arcs cannot survive one same-owner carrier, source preparation requires another
file/owner, event authority moves, or the frozen include/caps cannot build.

## Immediate predecessor

`7bc9e1da` scheduled this audit after accepted route implementation
`e4555dca`. The audit finds that every external package BUILD and recursive
`.bzl` upper path converges on `HostRepositoryPathKey` and
`HostRepositorySourceFileKey`, whose legacy resolved-path and FileBytes
computes discard the only complete epoch. It therefore selects this uniquely
smaller prerequisite before package loading or query publication.
