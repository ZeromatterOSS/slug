# Current Slug V2 Packet

Packet: `WP-2A-m1-root-module-frontier-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: design one callerless Bzlmod-private observed root-MODULE sibling that
retains the complete finite Host observation frontier without changing the
legacy key, events, loading callers, or public behavior.

## Accepted predecessor

Commit `53833591` gives `HostRootModuleFileKey` a finite private
`IncludeCycle` terminal for selected-logical-path active-ancestry recurrence.
It changes only `host_module.rs` by 60 production and 215 test lines (275
total; 3,194 physical), within 130/240/370 and 3,289. Focused Host-module proof
passes 16/16; the full Bzlmod crate passes 383 library tests and all integration
groups (576 total) plus doctests; direct loading/core checks, formatting, and
diff hygiene pass. Strict Clippy stops first in unchanged `allocative_derive`;
the archive checker reproduces only the inherited missing-ref/non-V2-thoughts
baseline. Independent source, ownership, schedule, and nine-category cleanup
reviews accept the implementation.

Pinned Bazel 9.2 source has no recurrence terminal. Existing admitted acyclic
MODULE/include behavior remains exact; selected-path ancestry and the finite
cycle terminal are Slug-native. The accepted implementation remains private
and carries no frontier or public caller.

## Design questions

Freeze the smallest implementation contract for exactly one callerless
Bzlmod-private observed sibling of `HostRootModuleFileKey`:

1. Preserve structural policy, root-file ordering, missing-root bootstrap
   Need, source validation, full-horizon preflight, grouped child-file Need
   union, source-order errors, active-ancestry recurrence, evaluation, and
   event ownership. Do not make either sibling compute the legacy key.
2. Consume only accepted observed producers: observed Host-file bytes for the
   root and include files and observed package-marker lookup for every include
   preflight occurrence. Determine the smallest shared driver/factoring that
   prevents duplicated Starlark evaluation or Host observation while leaving
   every legacy value/error/caller unchanged.
3. Define one crate-private carrier containing exactly one shared semantic
   `Result<HostRootModuleFileValue, HostRootModuleFileError>` and the accepted
   Arc-backed `PathObservationEpoch`. Freeze visibility, `Dupe`/`Allocative`,
   complete-only equality/validity, and borrowing accessors without a public or
   loading-facing export.
4. Union root bytes, every completed package-selection frontier, and every
   completed include-file frontier in deterministic occurrence/source order.
   Retain exact observation-result Arcs, including decisive negative probes.
   Equal duplicates coalesce with first-Arc ownership; mismatch/conflict is a
   completed outer `ObservedPathFrontierError`, never a panic or legacy error.
5. Seal only when the dynamic horizon reaches its finite terminal. Completed
   success and every completed semantic error, including `IncludeCycle`, must
   retain the exact observation prefix that decided it. Need, cancellation, or
   an outer child/union error publishes no parent carrier; completed child DICE
   observations remain dependency-owned cache state.
6. Preserve the legacy root event batch as the sole event owner. Decide how a
   sibling reuses the evaluator/finalizer without retaining or duplicating an
   evaluator, reporter, batch, transaction, ancestry chain, horizon, package
   matcher, or child carrier. The observed parent must add no competing event
   batch.
7. Freeze exact proof for root present/missing/error, finite nested/repeated
   includes, direct/indirect cycle, package and file errors, Need/cancellation,
   exact Arc union/conflict/mismatch, zero legacy activation, event parity,
   structural equality, warm reuse, recovery, and A/B/A restoration.
8. Name one exact future Rust allowlist, production/test/total/physical caps,
   retained-memory accounting, and the unique successor toward package-source
   or loading/core composition. If a complete carrier cannot be constructed in
   one bounded owner without changing legacy behavior, select exactly one
   smaller docs-only prerequisite or `REPLAN`.

Existing admitted serial root MODULE/include parsing, validation, diagnostics,
source-order errors, repeated occurrences, event order, and Host observation
values remain exact. Frontier aggregation, identity, sealing, equality, and
retry ownership are Slug-native. Lockfile/registry, package source,
BUILD/.bzl/glob evaluation, loading/core/public activation,
routed/materialized repositories, overlap/final validation, and exact Bazel
identity bytes remain unsupported/deferred.

## Authority and caps

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `slug-v2-subplans/current-packet.md`; and
- `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Read only this packet and owner section, the plan-authoring guide,
`docs/developers/dice.md`, the Buck2 utility-reuse skill and matching Stage 9
Arc/`Dupe`/`Allocative` row, Bzlmod
`src/{host_module,host_include,host_file,host_package,interim_module,module_eval,lib}.rs`,
workspace `src/{lib,path_observation,path_resolution}.rs`, loading
`src/bzl_module.rs`, root `Cargo.toml`, app
`slug_{workspace,bzlmod,loading}_v2/Cargo.toml`, and directly referenced focused
tests.

Ledger growth is capped at 40 canonical, 340 current-packet, 300 Stage 2, and
680 total net lines, with no correction.

## STOP / REPLAN

STOP on Rust, Cargo, oracle, fixture, public API/output, loading/core caller,
legacy key/value/error behavior, another certificate family, new graph/key/
store/container/interner, reconstructed or direct Host reads, retained
evaluator/event/transaction/ancestry/horizon, lockfile/registry/package-source/
BUILD/.bzl/glob, routed/materialized repository, watcher, JVM, or unrelated
cleanup work.

REPLAN if the complete dynamic frontier is unavailable before the finite
terminal, existing observed children cannot supply every decisive Host input,
event/evaluation sharing requires a public or reverse seam, exact observation
Arcs cannot be retained without a second container, a completed error would
retain a partial frontier, or no single future implementation can stay within
one bounded owner.

## Immediate successor

On design acceptance, activate exactly one bounded private observed
root-module implementation or one proven smaller docs-only prerequisite. Do
not combine package-source, loading/core publication, or another consumer.
