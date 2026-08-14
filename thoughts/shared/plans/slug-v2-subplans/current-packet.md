# Current Slug V2 Packet

Packet: `WP-2A-m1-host-repository-ignore-frontier-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: freeze exactly one docs-only design for a callerless Bzlmod-private
observed root repository-ignore sibling. It must compose the accepted REPO
frontier with every mutable ignore-selection predecessor without changing or
activating a legacy/public consumer.

## Accepted predecessor

Commit `f2c7305f` accepts the callerless
`HostRepoFileObservationKey` vertical designed in `7d7f0d25` on top of the
observed path/Host-file frontier in `308b409a`.

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

## Learned facts and design boundary

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

This packet designs a lower private producer only. It does not certify package
lookup, root MODULE includes, BUILD selection, loading, or a public command.
The eventual request-revision finalizer remains deferred until a complete
higher terminal frontier exists.

## Required design outputs

Freeze all of the following:

1. the exact crate-private sibling key/carrier names, workspace identity,
   Display, visibility, value type, complete equality, and validity;
2. the legacy source order for observed REPO, policy projection, ordered
   `.bazelignore` probes, platform-normalization observations, and every
   early success/error/Need exit;
3. the complete frontier for semantic success and every completed semantic
   error, including selected and negative file probes and all exact lower
   resolution/FileBytes/path-normalization observation Arcs;
4. outer frontier construction/union failure versus unchanged inner repository
   semantic errors, with no panic, laundering, or partial carrier;
5. deterministic union/dedup/conflict behavior by reusing
   `PathObservationEpoch::from_shared`, including repeated dependency
   coalescing and exact first-Arc retention;
6. event ownership: the observed REPO predecessor preserves its existing
   captured batch, repository-ignore adds no competing event authority, and
   Need/outer error/cancellation publish no new batch or partial carrier;
7. memory ownership: one DICE value may retain one semantic-result Arc and the
   accepted epoch only; no source copy, evaluator, reporter, event batch,
   transaction, policy object, worker, lock, or scratch survives completion;
8. A/B/A, warm reuse, create/edit/delete/recreate, policy-only changes,
   negative-to-selected precedence changes, dependency activation, exact-Arc,
   complete-error, Need, cancellation, and platform-focused proof;
9. the exact future Rust/test allowlist, formatted production/test/total caps,
   physical ceiling, cleanup trigger, validation, and completion-ledger cap;
   prefer exactly `app/slug_bzlmod_v2/src/repository_ignore.rs` or REPLAN; and
10. one uniquely bounded implementation successor, or one smaller prerequisite
    if the complete root-ignore frontier cannot be produced in that file.

No new Bazel oracle is required: legacy serial ignore behavior and admitted Host
observation results remain exact regression invariants. Frontier aggregation,
identity, equality, and future batch validation are Slug-native. Public overlap,
package/MODULE/source/load/glob composition, routed/materialized repositories,
and exact Bazel identity bytes remain unsupported/deferred.

## Write/read authority and caps

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `slug-v2-subplans/current-packet.md`; and
- `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Read only the active packet, this owner section, `docs/developers/dice.md`,
`.codex/skills/slug-buck2-utility-reuse/SKILL.md`, the matching Stages-3/6
row of `slug-v2-subplans/09-v1-extraction-ledger.md`,
`gazebo/dupe/src/lib.rs`, `allocative/allocative/src/lib.rs`, Bzlmod
`src/{lib,repository_ignore,repo_file,host_file,package_policy}.rs`, workspace
`src/path_observation.rs`, the Bzlmod/workspace manifests, and directly
referenced focused tests in those files.

Ledger caps are 40 canonical, 300 current, 260 Stage 2, and 600 net total lines.
No correction is reserved. This packet performs no Rust, Cargo, BUILD, oracle,
or generated-evidence write.

## STOP / REPLAN

STOP on code; implementation; a second observed key family; any package,
MODULE, lockfile, selected-source, BUILD, `.bzl`, glob, loading, core, or
public caller; routed/nonroot repository or materializer work; legacy key/value/
error/event/output changes; public export/API; Cargo/dependency change; generic
certificate framework; new retained container/cache/interner/graph/store;
reconstructed path demands; direct/historical Host reads; compute/evaluation
under a manual lock; watcher; JVM; or combining the implementation successor.

REPLAN if the root-only legacy key has another mutable predecessor not covered
by the accepted observed siblings, platform normalization cannot expose its
exact observation pair without a new workspace key, policy changes cannot
remain structural DICE dependencies, completed errors cannot retain the full
prefix without changing legacy diagnostics/order, visibility requires a public
export or reverse edge, deterministic union needs another retained container,
or a one-file implementation cannot be bounded.

## Immediate successor

On acceptance, activate exactly one implementation packet in
`app/slug_bzlmod_v2/src/repository_ignore.rs` plus completion ledgers, using
the frozen caps and proof. If the design proves that boundary incomplete,
schedule exactly one docs-only prerequisite instead.
