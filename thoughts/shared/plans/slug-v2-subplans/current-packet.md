# Current Slug V2 Packet

Packet: `WP-2A-m1-root-module-anchor-frontier-carrier-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: add one callerless doc-hidden app-internal observed anchor carrier/key
that projects the accepted private root-module frontier across the existing
Bzlmod-to-loading dependency direction without changing the live public anchor
or activating loading.

## Accepted predecessor and frozen decision

Commit `2640d1c0` accepts the callerless Bzlmod-private
`HostRootModuleFileObservationKey`: complete root MODULE success and semantic
errors retain their decisive exact Host prefix; Need, cancellation, and outer
frontier errors retain no parent carrier or event batch.

Docs-only design review rejects changing `RootModuleLoadingAnchorKey`. That
live key is already consumed by loading, computes the legacy root producer, and
has no outer `ObservedPathFrontierError` channel. Switching it would
prematurely activate the observed graph and would require panic, error
laundering, or a public Value/error change.

The accepted design is a separate sealed `#[doc(hidden)] pub` sibling in
Bzlmod, reexported only for later app-internal loading use. This completes the
one-way carrier ABI now; loading already depends on Bzlmod, so no reverse edge
or current loading edit is required.

## Frozen implementation contract

1. In `host_module.rs`, add `#[doc(hidden)] pub
   ObservedRootModuleLoadingAnchor` with private fields:
   `result: Result<RootModuleLoadingAnchor, RootModuleLoadingAnchorError>`
   and `observations: PathObservationEpoch`. Derive
   `Debug, Clone, PartialEq, Eq, Allocative, Dupe`; expose only doc-hidden
   borrowed `result()` and `observations()` accessors.
2. Add `#[doc(hidden)] pub RootModuleLoadingAnchorObservationKey`, keyed only
   by normalized workspace, with the normal structural key derives, public
   app-internal constructor, and display identity
   `observed-root-module-loading-anchor:{workspace}`.
3. Its Value is
   `SourcePreparationOutcome<Result<ObservedRootModuleLoadingAnchor,
   ObservedPathFrontierError>>`; equality is `complete_eq` and validity is
   `is_complete`. Need remains invalid/self-unequal; completed semantic and
   outer errors remain valid structural values.
4. Compute only `HostRootModuleFileObservationKey` once. Forward bootstrap or
   path Need unchanged. Forward a completed outer frontier error unchanged and
   construct no carrier. On completed semantic success/error, reuse the exact
   existing `HostRootModuleFileCarrier` Arc to construct the unchanged public
   anchor or anchor-error wrapper and carry the exact Arc-backed epoch.
5. Keep the anchor `result` inline. Do not add
   `Arc<Result<RootModuleLoadingAnchor, ...>>`: each public wrapper already
   owns the one existing semantic-result Arc. The final retained value owns
   only that Arc plus the accepted Arc-backed epoch.
6. Store no evaluation data. The observed root child remains the sole root
   event owner. Never compute `HostRootModuleFileKey` or
   `RootModuleLoadingAnchorKey`, re-evaluate MODULE files, reconstruct Host
   demands, or retain evaluator, event batch, source bytes, horizon, ancestry,
   transaction, loading result, or union scratch.
7. In `lib.rs`, reexport only the new carrier and key under
   `#[doc(hidden)]`. This is an app-internal Rust visibility boundary, not a
   user CLI, wire, output, diagnostic, or stability promise.
8. Leave existing `RootModuleLoadingAnchor{,Error,Key}`, registrations,
   Display/source, key identity, dependency, equality, events, tests, and all
   loading/core callers unchanged.

Existing admitted acyclic MODULE/include behavior, public anchor behavior,
diagnostics, events, and exact Host observations remain exact. The callerless
carrier representation, certificate association, and equality are
Slug-native. Loading consumption, package source, BUILD/`.bzl`/glob closure,
core final validation, overlapping publication, routed/materialized
repositories, and exact Bazel identity bytes remain unsupported/deferred.

## Proof and validation

Add colocated `host_module.rs` proof for:

- semantic success and semantic error parity with the legacy anchor mapping;
- exact semantic-result Arc and every epoch observation Arc retained by pointer;
- bootstrap and path Need forwarding with no carrier;
- forced outer frontier-error forwarding with no semantic carrier or event;
- observed root activation exactly once and zero legacy root/anchor activation;
- the observed child as the only root event owner;
- complete-only equality/validity, warm reuse, mutation, and A/B/A restoration;
- cancellation/source proof that only drop-safe locals cross the await; and
- unchanged public anchor identity, registrations, error, dependency, and event
  regressions.

Run the focused observed-anchor and existing public-anchor tests, full
`slug_bzlmod_v2`, direct `slug_loading_v2` and `slug_core_v2` checks,
`cargo fmt --all -- --check`, strict Clippy with inherited-baseline
disposition, the V2 archive checker, artifact scan, and `git diff --check`.
No Bazel oracle is needed because no admitted behavior is activated or changed.
Do not run Cargo commands concurrently in one target directory.

Because `host_module.rs` exceeds 2,000 lines, require independent pre/post
cohesion and ownership review. The projection belongs beside both root
producer and anchor families; a split requires a concrete independent
responsibility and `REPLAN`.

## Authority and caps

Write only:

- `app/slug_bzlmod_v2/src/host_module.rs`;
- `app/slug_bzlmod_v2/src/lib.rs`; and
- at completion only, canonical/current/Stage 2.

Read only this packet and owner section, the plan-authoring guide,
`docs/developers/dice.md`, the Buck2 utility-reuse skill and matching Stage 9
Arc/`Dupe`/`Allocative` row, Bzlmod `src/{host_module,lib}.rs`,
loading `src/bzl_module.rs`, workspace `src/{lib,path_observation}.rs`,
root `Cargo.toml`, app `slug_{workspace,bzlmod,loading}_v2/Cargo.toml`,
and directly referenced focused tests.

Caps from the live baselines are:

- `host_module.rs`: 100 production, 220 in-module tests, 320 total net, and
  4,205 physical lines from 3,885;
- `lib.rs`: 4 production, zero tests, 4 total net, and 383 physical lines
  from 379; and
- aggregate: 104 production, 220 tests, and 324 total net Rust lines.

Completion ledgers are capped at 180 net lines. No correction is authorized.

## STOP / REPLAN

STOP on every other Rust file; changing the existing public/legacy anchor key,
value, error, dependency, event, or caller; loading/core edits; user-facing
API/wire/output/diagnostic behavior; another key or carrier; another event
owner; outer-error laundering; direct/reconstructed or historical Host reads;
new graph/store/container/interner; retained evaluator/event/source/
transaction/horizon/ancestry/loading state; package source,
BUILD/`.bzl`/glob, routed/materialized repository, watcher, JVM, oracle, or
any cap/ceiling excess.

REPLAN if the doc-hidden types cannot cross Bzlmod-to-loading without widening
user behavior; exact semantic/epoch Arcs cannot be reused; outer error or Need
cannot remain distinct; event parity requires storing a second batch; the
existing public anchor must change; a third Rust file is required; or any
correction is needed.

## Immediate successor

On acceptance, schedule only docs-only
`WP-2A-m1-root-module-anchor-frontier-loading-consumer-design`. Do not combine
loading activation, package-source aggregation, BUILD/`.bzl`/glob, core
publication, or another frontier.
