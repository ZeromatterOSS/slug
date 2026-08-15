# Current Slug V2 Packet

Packet: `WP-2A-m1-root-package-source-frontier-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: add one callerless doc-hidden Bzlmod root-package source frontier
carrier/key over the accepted package-lookup and Host-file frontiers, without
activating loading or changing the legacy source API/behavior.

## Accepted predecessor and frozen design

The loading-consumer audit recorded in `c457a6d3` proves that direct
`RootPackageLoadKey` activation would publish a partial certificate: package
source, recursive `.bzl`, and glob inputs remain outside the accepted anchor
frontier. The finite source producer is the uniquely required first
prerequisite.

The frozen design adds a separate observed `RootPackageSource` sibling in
Bzlmod. One mode-aware private driver replaces the legacy source orchestration
and serves both legacy and observed wrappers. Small mode-selecting helpers
compute exactly one legacy or observed package-lookup/file child; neither DICE
key computes the other. The legacy wrapper extracts the same one semantic
Result Arc and discards a guaranteed-empty transient epoch. The observed
wrapper moves that exact Arc and the composed epoch into its carrier.

## Frozen implementation contract

1. In `host_package.rs` add `#[doc(hidden)] pub
   ObservedRootPackageSource` with private
   `result: Arc<Result<RootPackageSource, RootPackageSourceError>>` and
   `observations: PathObservationEpoch`. Derive
   `Debug, Clone, PartialEq, Eq, Allocative, Dupe` and expose only doc-hidden
   borrowed `result()` and `observations()` accessors.
2. Add `#[doc(hidden)] pub RootPackageSourceObservationKey` with the same
   workspace/private-request structural identity as `RootPackageSourceKey`,
   doc-hidden public `for_build`/`for_bzl` constructors, normal key derives,
   and the legacy display identity prefixed by `observed-`.
3. Its Value is
   `SourcePreparationOutcome<Result<ObservedRootPackageSource,
   ObservedPathFrontierError>>`; equality is `complete_eq` and validity is
   `is_complete`. Need remains invalid/self-unequal. Completed semantic and
   outer errors remain structural valid values.
4. Replace the legacy key's orchestration with one private
   `compute_root_package_source` driver parameterized by
   `RootPackageSourceMode::{Legacy, Observed}`. It returns an ephemeral
   projection containing exactly one terminal semantic Result Arc plus an epoch
   that is guaranteed empty in legacy mode. Do not duplicate the full driver,
   create a generic certificate framework, or compute one source key from the
   other.
5. Preserve candidate order exactly. BUILD inspects only the declared package.
   `.bzl` walks the existing deepest-to-declared candidates. In observed mode,
   union every completed observed lookup epoch before interpreting its semantic
   result. Nondeclared NoBuild/Deleted/Invalid continues; lookup error,
   intervening Package, or a declared terminal stops with exactly the decisive
   prefix. Exclude later speculative child state.
6. Preserve path/source order. After selection, a platform-path error completes
   semantically with the lookup prefix. Compute exactly one mode-selected
   Host-file child and, in observed mode, union its completed epoch before
   interpreting Host error, Missing, or Present. Reuse the exact Present bytes
   Arc in `RootPackageSource`.
7. Lookup/file Need returns the identical `SourcePreparationNeeds` and no
   parent carrier. Child or union mismatch/conflict returns a completed outer
   `ObservedPathFrontierError` with no semantic carrier. Cancellation drops
   only local candidates, paths, epoch and Arc scratch. The legacy outer-error
   branch is an explicit unreachable invariant, never a public error conversion.
8. The retained observed value owns only the single semantic Result Arc and the
   existing Arc-backed epoch. Retain no lookup/file carrier, policy, candidate
   vector, path scratch, evaluator, source text/AST, event batch, transaction,
   or second collection. Preserve `Dupe` cheap-clone signaling and
   `Allocative` accounting.
9. Store no evaluation data. The observed path activates exactly its observed
   lookup and file dependencies, never legacy lookup/file/source keys. Leave
   `RootPackageSourceKey`'s public Value, errors, constructors, Display,
   equality, validity, callers, and admitted behavior unchanged.
10. In `lib.rs` reexport only the carrier/key under `#[doc(hidden)]` for the
    later natural Bzlmod-to-loading dependency. This is app-internal Rust
    visibility, not a user API/wire/output/stability promise.

Existing serial source selection, bytes, diagnostics, Need/error order and exact
Host observations remain exact. The callerless certificate
association/aggregation/equality is Slug-native. Recursive `.bzl`, glob and
loading aggregation, core final validation, public overlap, repository/
materializer work, and exact Bazel identity bytes remain unsupported/deferred.

## Proof and validation

Add colocated proof for:

- legacy success/error/Need parity and unchanged key identity;
- BUILD one-lookup/one-file order;
- `.bzl` deepest-to-declared negative candidates and package-boundary stop;
- lookup semantic classes and platform-path error retaining the exact prefix;
- Host error, Missing and Present retaining the full prefix;
- exact semantic, bytes and every epoch result Arc by pointer;
- forced lookup/file union mismatch/conflict as completed outer error with no
  carrier;
- lookup/file Need with no carrier/event and complete-only equality/validity;
- observed activation once with zero legacy lookup/file/source activation;
- no evaluation data on semantic, outer or Need paths;
- warm reuse, mutation and A/B/A restoration; and
- cancellation source proof plus cfg(windows) platform-path coverage or a
  recorded target-availability stop.

Run focused new source-frontier and unchanged source-projection tests, full
`slug_bzlmod_v2`, direct `slug_loading_v2` and `slug_core_v2` checks,
`cargo fmt --all -- --check`, strict Clippy with inherited-baseline
disposition, `scripts/v2_archive_status.sh`, artifact scan and
`git diff --check`. No Bazel oracle is required because no admitted behavior
changes. Do not run Cargo commands concurrently in one target directory.

Because `host_package.rs` already exceeds 2,000 lines, require independent
pre/post cohesion and nine-category cleanup review. Keep the sibling beside
the private request/error/lookup/source fixtures; splitting requires a concrete
independent responsibility and `REPLAN`.

## Authority and caps

Write only:

- `app/slug_bzlmod_v2/src/host_package.rs`;
- `app/slug_bzlmod_v2/src/lib.rs`; and
- at completion only, canonical/current/Stage 2.

Caps from the live 3,995/383-line baselines are:

- `host_package.rs`: 240 production, 420 in-module tests, 660 total net and
  4,655 physical lines;
- `lib.rs`: 4 production, zero tests, 4 total net and 387 physical lines; and
- aggregate: 244 production, 420 tests and 664 total net Rust lines.

Completion ledgers are capped at 180 net lines. No correction is authorized.

## STOP / REPLAN

STOP on every other file; changing the legacy source API/value/error/caller or
accepted lookup/Host-file/anchor owners; loading/core edits; a second key,
carrier, collection or event owner; key-to-key compute; duplicated full source
driver; partial prefixes; outer-error laundering; direct/reconstructed or
historical Host reads; retained child/policy/candidate/path/evaluator/event/
source-text/AST/transaction state; recursive `.bzl` evaluation, glob/directory,
repository/materializer, watcher, JVM, oracle, public behavior, or any cap/
ceiling excess.

REPLAN if the mode driver cannot replace rather than duplicate the legacy
orchestration; exact semantic/observation Arcs cannot be reused; a complete
semantic error cannot retain its decisive prefix; outer error/Need cannot stay
distinct; legacy behavior or a third Rust file must change; the large file has
a concrete split boundary; or any correction is needed.

## Immediate successor

On acceptance schedule only docs-only `WP-2A-m1-host-bzl-module-frontier-design`.
Do not combine recursive evaluation, glob aggregation, loading activation,
core publication, or another consumer.
