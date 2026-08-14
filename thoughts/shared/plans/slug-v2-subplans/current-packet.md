# Current Slug V2 Packet

Packet: `WP-2A-m1-root-module-frontier-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: add one callerless Bzlmod-private observed root-MODULE key whose
complete semantic terminals retain their exact finite Host observation
frontier, without changing the legacy key, public anchor, or loading callers.

## Accepted predecessor and frozen design

Commit `53833591` gives `HostRootModuleFileKey` a finite private active-ancestry
cycle terminal. Commit `b0d46420` resumes the frontier design with focused/full
validation and independent cleanup acceptance recorded.

Live source inspection and independent design review accept exactly one new
retained DICE key in `host_module.rs` plus one non-retained async preflight
helper in `host_include.rs`. No second key, carrier family, public export,
workspace representation, or Cargo edge is required.

## Frozen implementation contract

1. Add crate-private `ObservedHostRootModuleFile { result,
   observations }`, where `result` is the existing
   `Arc<Result<HostRootModuleFileValue, HostRootModuleFileError>>` and
   `observations` is the accepted `PathObservationEpoch`. Derive structural
   equality, cheap-clone signaling, and memory accounting; expose only
   crate-private borrowing accessors.
2. Add crate-private `HostRootModuleFileObservationKey`, keyed only by normalized
   workspace, with a distinct display identity. Its value is
   `SourcePreparationOutcome<Result<ObservedHostRootModuleFile,
   ObservedPathFrontierError>>`; equality is complete-only and Need is invalid,
   matching the legacy root key's request-policy dependency behavior.
3. Preserve command policy first. A policy error completes as the unchanged
   inner `CommandPolicy` error with an empty epoch. Compute root bytes only via
   `HostFileBytesObservationKey`. Root Host errors and validation errors retain
   that exact epoch; missing root remains bootstrap Need and publishes no parent
   carrier. Outer child/union errors remain completed outer errors.
4. In `host_include.rs`, add one crate-private async observed-preflight helper,
   not a DICE key or retained carrier. Preserve whole-horizon parsing,
   first-seen package dedupe, joined package computation, whole-batch Need union,
   and source-order semantic selection while consuming only
   `HostRootPackageLookupObservationKey`. Return the legacy horizon/error plus
   its command-local accumulated epoch or an outer frontier error.
5. Preserve the dynamic root driver and active-ancestry guard. Compute include
   files only through `HostFileBytesObservationKey`, using the existing unique
   logical-path join and source-order interpretation. Union each package or
   file epoch immediately before interpreting that occurrence. A completed
   semantic terminal retains only its decisive completed source-order prefix;
   later joined work may remain dependency-owned cache state but must not enter
   the certificate. When the first encountered occurrence is Need, return the
   existing whole-batch Need union and no carrier.
6. Retain the current ordering: root validation, horizon preflight, grouped file
   observation, source-order missing/file/validation errors, then active-path
   cycle classification before child extension or evaluation accumulation.
   Success and evaluation error seal only after the finite horizon empties and
   retain the complete frontier. Equal duplicate demands coalesce with the
   earliest source-order Arc; conflict or operation mismatch is a typed outer
   error, never a panic or legacy semantic error.
7. Reuse the existing root evaluation and event-finalization leaf. On the
   observed path, the observed key is the sole root event owner: a semantic
   Complete stores exactly one equivalent batch, empty for pre-evaluation
   errors; Need, outer error, and cancellation store none. Neither root key
   computes the other, and the preflight helper adds no event authority.
8. Retain only one semantic-result Arc and one Arc-backed epoch in the DICE
   value. Horizons, ancestry nodes, joined maps, source strings, inspections,
   package/file carriers, evaluator, event batch, transaction, and union scratch
   remain compute-local and release on completion, error, Need, or cancellation.
   Add no lock across a DICE await, direct/reconstructed Host read, second
   collection, cache, graph, store, interner, or historical snapshot.

Existing admitted serial acyclic MODULE/include parsing, validation,
diagnostics, source-order error/Need selection, repeated occurrence evaluation,
event order, and exact Host observation values remain exact. Dynamic frontier
aggregation, decisive-prefix identity, sealing, and certificate equality are
Slug-native. The cycle terminal remains Slug-native. The public loading anchor,
package source, BUILD/.bzl/glob, loading/core/public publication,
routed/materialized repositories, overlap/final validation, and exact Bazel
identity bytes remain unsupported/deferred.

## Proof and validation

Add colocated proof for:

- root present, Host error, validation error, missing/bootstrap Need, and empty
  policy-error frontiers;
- nested and repeated alias occurrences, exact package/file prefix order, and
  `Arc::ptr_eq` retention across duplicate observations;
- direct/indirect cycle frontiers and preservation of validation/error order;
- package and include-file semantic errors, child/union outer errors, grouped
  Need, and source-order exclusion of speculative later joined observations;
- complete-only equality/validity, warm reuse, recovery, and A/B/A restoration;
- equivalent root event batches, empty completed-error batches, no batch for
  Need/outer errors, and no legacy root/package/file key activation; and
- cancellation/source proof that only drop-safe command scratch crosses each
  await and the final carrier retains no evaluator, event batch, source bytes,
  transaction, horizon, or ancestry.

Run focused root-module and include-preflight tests, the full
`slug_bzlmod_v2` suite, direct `slug_loading_v2` and `slug_core_v2` checks,
`cargo fmt --all -- --check`, strict Clippy with inherited-baseline
disposition, the V2 archive checker, artifact scan, and `git diff --check`.
Do not run Cargo commands concurrently in one target directory.

Because `host_module.rs` already exceeds 2,000 lines, require independent
pre- and post-implementation cohesion/AI-cleanup review. Keep the dynamic
driver/evaluator/event owner in `host_module.rs` and label/package preflight in
`host_include.rs`; a further split requires a concrete separable responsibility
and `REPLAN`.

## Authority and caps

Write only:

- `app/slug_bzlmod_v2/src/host_module.rs`;
- `app/slug_bzlmod_v2/src/host_include.rs`; and
- at completion only,
  `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`,
  `slug-v2-subplans/current-packet.md`, and
  `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Read only this packet and owner section, the plan-authoring guide,
`docs/developers/dice.md`, the Buck2 utility-reuse skill and matching Stage 9
Arc/`Dupe`/`Allocative` row, Bzlmod
`src/{host_module,host_include,host_file,host_package,interim_module,module_eval,lib}.rs`,
workspace `src/{lib,path_observation,path_resolution}.rs`, loading
`src/bzl_module.rs`, root `Cargo.toml`, app
`slug_{workspace,bzlmod,loading}_v2/Cargo.toml`, and directly referenced focused
tests.

Caps are:

- `host_module.rs`: 280 production, 430 in-module tests, 710 total net, and
  3,904 physical lines from 3,194;
- `host_include.rs`: 190 production, 170 in-module tests, 360 total net, and
  1,148 physical lines from 788; and
- aggregate: 470 production, 600 tests, and 1,070 total net Rust lines.

Completion ledgers are capped at 200 net lines. No correction is authorized.

## STOP / REPLAN

STOP on every other Rust file; a second DICE key or retained certificate,
legacy key/caller/error/event behavior, public export/API/output, Cargo or
workspace representation, loading/core caller, oracle/fixture, new
graph/store/container/interner, direct/reconstructed/historical Host reads,
retained evaluator/event/transaction/source/horizon/ancestry, lockfile/registry,
package-source/BUILD/.bzl/glob, routed/materialized repository, watcher, JVM,
unrelated cleanup, or any cap/ceiling excess.

REPLAN if the observed preflight cannot preserve whole-horizon parse, joined
Need, and source-order terminal behavior; any completed terminal lacks its
decisive Host prefix; event parity requires computing the legacy key or storing
two root batches on one path; exact Arc union requires another retained
container; a third Rust file or second key is required; or any correction is
needed.

## Immediate successor

On acceptance, schedule only docs-only
`WP-2A-m1-root-module-anchor-frontier-carrier-design` to decide the public
Bzlmod-to-loading carrier boundary. Do not combine anchor activation, package
source, BUILD/.bzl/glob, loading/core publication, or another consumer.
