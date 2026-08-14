# Current Slug V2 Packet

Packet: `WP-2A-m1-root-module-anchor-frontier-carrier-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: freeze the smallest non-user-facing carrier boundary that associates the
accepted private root-module observation frontier with the existing
Bzlmod-to-loading anchor, without implementing it or activating loading.

## Accepted predecessor

Commit `2640d1c0` adds one callerless Bzlmod-private
`HostRootModuleFileObservationKey` and one ephemeral observed include
preflight helper. Every complete root MODULE success or semantic error retains
its decisive exact Host observation prefix; Need, cancellation, and outer
frontier errors retain no parent carrier or event batch. The legacy root and
public anchor keys remain unchanged.

The accepted implementation changes exactly `host_module.rs` and
`host_include.rs`: 420 production and 559 test lines, 979 total net.
Focused root/frontier proof passes 6/6, the complete owner modules pass 20/20
and 8/8, and all 582 Bzlmod unit/integration tests pass. Direct loading/core
checks, formatting, diff/artifact hygiene, ownership review, and the
nine-category cleanup review pass. Strict Clippy and archive checks stop only
on their recorded inherited baselines.

## Design questions

1. Map the exact live boundary: private
   `HostRootModuleFileObservationKey` and carrier, public
   `RootModuleLoadingAnchor{,Error,Key}`, crate reexports, and
   `RootPackageLoadKey`'s anchor-first consumption and error mapping.
2. Choose one natural app-internal carrier design. Decide whether the existing
   anchor key may consume the observed producer and retain a doc-hidden
   certificate while preserving its public semantic value, or whether a
   separate sealed anchor sibling is required. Do not expose Host observation
   details as user API or create a reverse loading-to-Bzlmod dependency.
3. Freeze success, completed semantic error, outer frontier error,
   bootstrap/observation Need, cancellation, equality, and validity algebra.
   Preserve current anchor-before-source order and the exact public
   registration/error/Display/source surface.
4. Keep root event authority exclusively with the observed root producer.
   The anchor boundary may project/associate its result and epoch but must not
   compute the legacy root key, store another event batch, re-evaluate MODULE
   files, or reconstruct Host demands.
5. Retain at most the accepted semantic-result Arc and Arc-backed
   `PathObservationEpoch`, or a borrowing/projection wrapper over them.
   Freeze clone/`Dupe`/`Allocative` boundaries, DICE lifetime,
   invalidation/equality cutoff, cancellation release, and proof that no
   evaluator, event batch, source bytes, horizon, ancestry, transaction, or
   loading result is retained.
6. Decide the exact future Rust file allowlist, per-file production/test/total
   caps, physical ceilings, proof matrix, and cleanup trigger. Select only one
   bounded Bzlmod-side carrier implementation or one proven smaller docs-only
   prerequisite. Loading consumption remains a later packet.

Admitted serial acyclic MODULE/include behavior, diagnostics, events, public
anchor values, and exact Host observation values remain exact. Certificate
association, projection, and identity are Slug-native. Loading consumption,
package source, BUILD/`.bzl`/glob closure, core final validation,
overlapping publication, routed/materialized repositories, and exact Bazel
identity bytes remain unsupported/deferred.

## Evidence, authority, and caps

Use accepted source/proof from `2640d1c0`; no new Bazel oracle is needed
because this packet changes no exact behavior. Inspect the live DICE ownership
rules before selecting an implementation owner.

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest; and
- `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Read only this packet and owner section, the plan-authoring guide,
`docs/developers/dice.md`, the Buck2 utility-reuse skill and matching
Stage 9 Arc/`Dupe`/`Allocative` row, Bzlmod
`src/{host_module,lib}.rs`, loading `src/bzl_module.rs`, workspace
`src/{lib,path_observation}.rs`, root `Cargo.toml`, app
`slug_{workspace,bzlmod,loading}_v2/Cargo.toml`, and directly referenced
focused tests.

Ledger caps are 40 canonical, 280 current, 260 Stage 2, and 580 total net
lines. No Rust, Cargo, test, oracle, or fixture write is authorized.

## STOP / REPLAN

STOP on every code/oracle write; public user-facing API, wire, output, or
diagnostic changes; loading/core caller edits; another observation producer;
legacy root or anchor behavior; second event owner; direct/reconstructed or
historical Host reads; new graph/store/container/interner; retained evaluator,
event, source, transaction, horizon, ancestry, or loading result; package
source, BUILD/`.bzl`/glob, routed/materialized repository, watcher, JVM,
or combined consumer work.

REPLAN if the frontier cannot cross the existing Bzlmod-to-loading dependency
direction without a reverse edge or user-facing API; success and completed
errors cannot share one exact carrier boundary; Need/error/event order would
change; the anchor would need to recompute either root key or duplicate event
authority; or the smallest implementation cannot be bounded independently.

## Immediate successor

On acceptance, activate only one bounded Bzlmod-side anchor-carrier
implementation or one proven smaller docs-only prerequisite. Do not combine
loading activation, package-source aggregation, BUILD/`.bzl`/glob, or
core publication.
