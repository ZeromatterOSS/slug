# Current Slug V2 Packet

Packet: `WP-2A-m1-host-repository-ignore-frontier-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: implement and prove exactly one callerless Bzlmod-private observed
root repository-ignore sibling. Preserve every legacy key, caller, diagnostic,
event, matcher result, and public behavior. Completion schedules only the
package-marker frontier design.

## Accepted design

Commit `8ac5c30f` accepts the one-file repository-ignore frontier design on
exact predecessor `f2c7305f`. The lower observed path/Host-file frontier remains
accepted in `308b409a`.

The implementation is confined to `repo_file.rs`. It preserves the legacy
policy-first semantic and event behavior, computes only
`HostFileBytesObservationKey`, keeps Need incomplete, forwards outer frontier
errors before evaluation/event storage, and retains one semantic-result Arc
plus the exact child epoch. Focused observed-key proof is 4/4, the strengthened
resolution-prefix case is 1/1, all 564 Bzlmod unit/integration tests pass, and
the direct `slug_core_v2` compile check passes. Formatting and diff hygiene
pass. Strict Clippy stops first in unchanged `allocative_derive`; the archive
checker reproduces the inherited missing archive-ref/non-V2-thoughts baseline.

Formatted net growth is 158 production plus 365 in-module test lines, 523 total,
with 2,281 physical lines, within 200/370/570 and 2,328. Independent ownership
and AI-cleanup review accepts the large-file cohesion: the private sibling,
legacy/root/routed REPO owners, evaluator/reporters, event finalizer, and
activation tests are one responsibility, and a split would widen private seams
without isolating another owner.

## Implementation boundary

The earlier hierarchical audit `a6aaa844` established the root-only chain:

1. legacy `HostRepositoryIgnoreKey` consumes legacy `HostRepoFileKey`;
2. it then consumes immutable root repository-ignore policy;
3. it probes ordered package-root `.bazelignore` files, including negative
   selection observations and stopping at the existing selected terminal; and
4. platform parsing may add exact Host path-normalization observations.

The REPO predecessor is now representable through
`HostRepoFileObservationKey`; every `.bazelignore` file can use the accepted
`HostFileBytesObservationKey`. The policy projection is structural immutable
semantics, not a Host observation pair. The existing
`PathObservationEpoch` remains the only retained compact collection.

This packet implements a lower private producer only. It does not certify
package lookup, root MODULE includes, BUILD selection, loading, or a public
command. The eventual request-revision finalizer remains deferred until a
complete higher terminal frontier exists.

## Frozen design

Add one crate-private
`HostRepositoryIgnoreObservationKey { workspace: NormalizedAbsolutePath }`
with Display `bzlmod-observed-host-repository-ignore:{workspace}`. Its value is
`PathOutcome<Result<ObservedHostRepositoryIgnore,
ObservedPathFrontierError>>`; equality is `complete_eq` and validity is
`is_complete`, so Need remains the only incomplete state.

`ObservedHostRepositoryIgnore` derives `Debug`, `Clone`, `PartialEq`, `Eq`,
`Allocative`, and `Dupe`. It retains exactly one
`Arc<Result<RepositoryIgnoreMatcher, HostRepositoryIgnoreError>>` plus one
`PathObservationEpoch` and exposes only borrowed result/epoch accessors. Both
types stay `pub(crate)` in `repository_ignore.rs`; no export or dependency edge
is needed.

Preserve this exact compute and terminal order:

1. compute only `HostRepoFileObservationKey(workspace)`; forward Need and an
   outer frontier error without a carrier, or retain its exact epoch for REPO
   success and unchanged inner `RepoFile` error;
2. compute `RootRepositoryIgnoreInputsProjectionKey`; its package-root/vendor
   policy remains a structural DICE dependency, and `PolicyProjection` retains
   the already-complete REPO epoch;
3. visit package roots in policy order, append the existing vendor prefix, and
   compute only `HostFileBytesObservationKey(root/.bazelignore)`;
4. union every completed child epoch before interpreting it: Missing and
   directory WrongKind are negative probes that continue, other Host errors
   complete inner `HostFile`, and Present proceeds to the shared parser;
5. stop after the first successfully parsed present file, or return success
   after all negative probes; preserve matcher literal sorting/deduplication
   and REPO pattern order exactly.

Factor the existing parser into one shared observed-capable implementation.
Its stack-only `ObservedIgnoreParse` contains the unchanged semantic parse
result plus a temporary epoch. On Windows it appends each exact completed
`WindowsLongPath` demand/result Arc before interpreting it; a Need drops the
whole scratch frontier, and an error after earlier normalizations retains that
complete prefix. Legacy callers discard the temporary epoch and keep their
existing diagnostics/order. The observed path maps operation mismatch or
duplicate conflict to the outer `ObservedPathFrontierError` rather than a
panic or inner semantic error.

Use one private union helper over `PathObservationEpoch::from_shared` in
REPO, root, and parse order. Equal duplicate pairs coalesce and retain the first
supplied Arc; unequal duplicate results or operation mismatch are completed
outer errors with no partial carrier. No second retained collection,
provenance table, cache, graph, store, or reconstructed demand is introduced.

Repository-ignore owns no events. The observed REPO dependency remains the
sole owner of its captured batch; a later Need, outer error, or cancellation
adds no batch and publishes no parent carrier. Retained completion state is
only the semantic-result Arc and accepted Arc-backed epoch. Bytes, prepared
lines, normalization scratch, policy inputs, evaluators, reporters, event
batches, transactions, workers, and locks release before completion.

Focused proof must cover REPO success/error/Need/outer error; policy failure
with the exact REPO prefix; missing, directory, resolution, FileBytes, parse,
native-invalid, and selected-success terminals; ordered negative-to-selected
and create/edit/delete/recreate transitions; exact Arc retention and
duplicate-first-Arc/conflict behavior; zero legacy REPO/Host-file activation;
no parent event batch; A/B/A, warm reuse, policy-only change, Need, and
cancellation; plus Windows long-path completion, Need, and normalization replay
under `cfg(windows)`.

No new Bazel oracle is required: legacy serial ignore behavior and admitted Host
observation results remain exact regression invariants. Frontier aggregation,
identity, equality, and future batch validation are Slug-native. Public overlap,
package/MODULE/source/load/glob composition, routed/materialized repositories,
and exact Bazel identity bytes remain unsupported/deferred.

## Write/read authority and caps

Write only:

- `app/slug_bzlmod_v2/src/repository_ignore.rs`; and
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`,
  `slug-v2-subplans/current-packet.md`, and
  `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md` at completion.

Read only the active packet, this owner section, `docs/developers/dice.md`,
`.codex/skills/slug-buck2-utility-reuse/SKILL.md`, the matching Stages-3/6
row of `slug-v2-subplans/09-v1-extraction-ledger.md`,
`gazebo/dupe/src/lib.rs`, `allocative/allocative/src/lib.rs`, Bzlmod
`src/{lib,repository_ignore,repo_file,host_file,package_policy}.rs`, workspace
`src/path_observation.rs`, the Bzlmod/workspace manifests, and directly
referenced focused tests in those files.

Formatted Rust caps are 280 production, 450 in-module test, and 730 total net
added lines. The physical `repository_ignore.rs` ceiling is 2,821 lines from
the 2,091-line baseline. Require an independent cohesion/cleanup review once
the formatted file exceeds 2,400 lines. Completion ledgers are capped at 180
net lines. No correction is reserved.

## STOP / REPLAN

STOP on every other Rust file; a second observed key family; any package,
MODULE, lockfile, selected-source, BUILD, `.bzl`, glob, loading, core, or public
caller; routed/nonroot repository or materializer work; legacy key/value/error/
event/output changes; public export/API; Cargo/dependency/BUILD/oracle/generated
evidence; generic certificate framework; new retained container/cache/interner/
graph/store; reconstructed demands; direct/historical Host reads; a manual lock
across compute/evaluation; watcher; JVM; or cap/physical-ceiling excess.

REPLAN if shared parsing changes legacy diagnostics/order, a Windows
normalization result cannot retain its exact Arc without another workspace key,
an inner semantic error loses its complete prefix, Need/outer error publishes a
partial carrier, deterministic union needs another retained container,
visibility escapes the crate, tests require another Rust file, or the cohesion
review finds a real split prerequisite.

## Immediate successor

On acceptance, activate only docs-only
`WP-2A-m1-host-package-marker-frontier-design`. It may design one private
observed root package-marker/lookup sibling over immutable policy, the accepted
repository-ignore epoch, and ordered `BUILD.bazel`/`BUILD` resolution probes.
It must not implement that consumer or activate MODULE/loading/core/public work.
