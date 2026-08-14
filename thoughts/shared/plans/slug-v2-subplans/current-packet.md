# Current Slug V2 Packet

Packet: `WP-2A-m1-host-repo-file-frontier-key-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: implement and prove exactly one callerless Bzlmod-private observed
`REPO.bazel` sibling key. Preserve every legacy key, caller, event, diagnostic,
and public behavior. Completion schedules only the repository-ignore frontier
design.

## Accepted design

Commit `7d7f0d25` activates the design after the hierarchical audit. Independent
source and ownership review accepts one-file implementation in
`app/slug_bzlmod_v2/src/repo_file.rs`.

Freeze these private types:

- `ObservedHostRepoFile` derives `Debug`, `Clone`, `PartialEq`, `Eq`,
  `Allocative`, and `Dupe`; it retains exactly
  `Arc<Result<HostRepoFileValue, HostRepoFileError>>` plus
  `PathObservationEpoch`, and exposes only borrowed result/epoch accessors.
- `HostRepoFileObservationKey` has only the normalized workspace identity,
  derives the existing private-key traits, has a distinct observed Display,
  and returns
  `PathOutcome<Result<ObservedHostRepoFile, ObservedPathFrontierError>>`.
  Equality is `complete_eq`; validity is `is_complete`.

The live retained utility sources are `gazebo/dupe/src/lib.rs` and
`allocative/allocative/src/lib.rs`; the design's stale
`third-party/buck2/` prefixes are replaced as authorized live equivalents.
`Dupe` means cheap Arc-backed bumps only. `Allocative` preserves memory
accounting. `PathObservationEpoch` remains the sole compact retained map.

## Implementation ownership

Read `CaptureEvaluationEvents`, then preserve legacy policy-first ordering.
Factor one synchronous, stack-only private finalization adapter around the
existing `evaluate_repo_file`/reporter path. It may accept either a completed
semantic terminal or present bytes plus UTF-8 mode, returns one
`Arc<Result<HostRepoFileValue, HostRepoFileError>>`, and stores exactly one
completed EventBatch when capture is enabled. It performs no DICE compute,
await, lock, or callback beyond the existing synchronous evaluator/reporters.

Both legacy and observed keys call the same adapter. Each key owns its DICE
computes and calls no other REPO-file key. Do not duplicate Starlark evaluation
or change the legacy value/equality.

Preserve this observed-key terminal order:

1. compute `RootRepoFileSemanticsProjectionKey`;
2. policy error completes with the unchanged inner semantic error, an empty
   epoch, one empty captured batch if enabled, and zero Host-file activation;
3. compute only
   `HostFileBytesObservationKey(workspace/REPO.bazel)`;
4. Need forwards unchanged and stores no events;
5. a completed outer `ObservedPathFrontierError` forwards with no carrier,
   semantic evaluation, or stored events;
6. completed Host-file resolution/wrong-kind/FileBytes errors become inner
   `HostRepoFileError::HostFile`, retain the exact dependency epoch, and store
   an empty batch if captured;
7. missing becomes the existing empty semantic success with the exact
   dependency epoch and an empty captured batch;
8. present bytes are borrowed/Arc-cloned only through synchronous evaluation;
   success and parse/UTF-8/restricted/compile/evaluation errors retain the exact
   dependency epoch and one semantic-result Arc; and
9. cancellation/drop retains no carrier, evaluator, reporter, batch, or scratch.

A small pure private shaping seam may be used to force outer-error/terminal
tests. It must not introduce another value family, retained container, fallback,
or production activation.

## Compatibility and memory

Existing serial root `REPO.bazel` missing/present/error/evaluation behavior,
diagnostics, event order, and exact Host observation results remain exact
regression invariants. The new aggregate carrier identity/equality and future
batch validation are Slug-native. No new Bazel parity is claimed.

The carrier is a callerless DICE-retained semantic value. It retains only one
semantic-result Arc and the existing Arc-backed epoch. Present source bytes are
not separately retained beyond any existing semantic result. Event batches,
reporters, evaluators, transactions, workers, locks, accepted snapshots, and
command state remain outside the carrier and release at compute/activation
boundaries. No manual lock spans a DICE compute.

## Write/read authority and caps

Write only:

- `app/slug_bzlmod_v2/src/repo_file.rs`;
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`,
  `slug-v2-subplans/current-packet.md`, and
  `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md` at completion.

Read only the active packet, `docs/developers/dice.md`,
`.codex/skills/slug-buck2-utility-reuse/SKILL.md`, the matching Stages-3/6 row
of `slug-v2-subplans/09-v1-extraction-ledger.md`,
`gazebo/dupe/src/lib.rs`, `allocative/allocative/src/lib.rs`, Bzlmod
`src/{host_file,repo_file,package_policy}.rs`, workspace
`src/path_observation.rs`, the Bzlmod/core manifests, and directly referenced
focused tests inside those files.

Formatted Rust caps are 200 production, 370 in-module test, and 570 total net
added lines. The physical `repo_file.rs` ceiling is 2,328 lines. Completion
ledgers are capped at 180 net lines. No cap correction is reserved.

If the formatted file crosses 2,000 physical lines, require an independent
cohesion/cleanup review. Keeping the sibling beside the sole evaluator,
reporter, legacy key, and activation tests is acceptable only if that review
finds no separable second responsibility or duplicated orchestration.

## Required proof and validation

Focused in-module proof must cover:

- policy failure: empty epoch, empty captured batch, zero Host activation;
- present, missing, wrong-kind, resolution error, FileBytes error,
  parse/restricted/evaluation error, and success with legacy semantic parity;
- exact final FileBytes observation Arc and observed semantic-result Arc
  retention;
- outer error passthrough with no carrier/evaluation/events;
- Need invalidity/no event publication and cancellation-neutral scratch;
- A/B/A equality, warm reuse, and restored events;
- capture-on/off event parity and exactly one completed batch; and
- zero `HostRepoFileKey` and `HostFileBytesKey` activation through the
  observed sibling.

Run focused tests, full `slug_bzlmod_v2`, a direct `slug_core_v2` compile
check, formatting, strict Clippy, `scripts/v2_archive_status.sh`,
`git diff --check`, scope/artifact/cfg-aware cap accounting, and independent
ownership plus AI-cleanup review. Record inherited failures without calling
them passes. No Bazel oracle is required because no existing exact/public
surface changes and the new carrier is private Slug-native representation.

## STOP / REPLAN

STOP on every other Rust file; Cargo/BUILD/oracle/generated evidence;
repository-ignore, routed/nonroot repository, or materializer activation;
package/MODULE/lockfile/BUILD/`.bzl`/loading/core/public caller; legacy
key/value/equality/public event/output change; public export/API; second
observed key; new retained container/cache/interner/graph/store; reconstructed
or historical Host reads; watcher; JVM; sibling-to-legacy DICE compute;
duplicated evaluator; retained event/evaluator/transaction; unbounded scratch;
or cap/physical ceiling excess.

REPLAN if shared finalization changes legacy diagnostics/events, outer errors
cannot stay separate from semantic errors, an exact epoch requires a second
container or copied observation results, tests require visibility outside the
one file, or the post-format cohesion review finds a real split prerequisite.

## Immediate successor

On acceptance, activate docs-only
`WP-2A-m1-host-repository-ignore-frontier-design`. It may design the private
union of this REPO epoch, ordered observed `.bazelignore` negative/selected
probes, and platform-normalization observations. It must not implement that
consumer or activate routed/materialized branches in the same packet.
