# Current Slug V2 Packet

Packet: `WP-2A-m1-host-glob-frontier-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: design the smallest complete callerless observed Host-glob frontier, or
record one uniquely smaller prerequisite, without implementing code or
activating package loading, core publication, or public overlap.

## Accepted predecessor and implementation evidence

Commit `b9fda97d` accepts the loading-private observed Host `.bzl` frontier.
It has one mode-aware driver, exact source/child epoch union, family-tagged
legacy/observed cycle identities, rotated `cycle.keys` source reacquisition,
an invalid poison dependency, selected-key event ownership, and no loading or
public consumer.

Exact accounting against `78eb0ea0` is:

- `bzl_module.rs`: 343 production + 480 tests = 823 net lines and 5,915
  physical lines;
- `cycle_detector.rs`: 46 production + zero tests = 46 net lines and 598
  physical lines; and
- aggregate: 389 production + 480 tests = 869 net Rust lines.

Focused observed proof passed 4/4. Full `slug_loading_v2` passed 114 library
and 66 integration tests (180 total); direct `slug_core_v2` check,
`cargo fmt --all -- --check`, artifact scan and `git diff --check` passed.
Strict Clippy remained blocked first in unchanged `allocative_derive`; the
archive checker reported only its inherited archive baseline. Independent
proof, ownership and cleanup review accepted the implementation. In
particular, real-driver Input and Freeze each prove exactly one observed
`@@//:ext.bzl` activation with one empty completed event batch; poison
recomputation, simultaneous family separation, cancellation/drop recovery and
cycle-prefix ownership are directly covered.

## Live source map and missing boundary

The loading-private Host-glob chain is currently:

1. `compute_host_glob_request` in `host_glob/adapter.rs` validates one
   request and computes `HostGlobTraversalKey`.
2. `HostGlobTraversalKey` in `host_glob/traversal.rs` owns breadth-first
   pattern traversal, candidate ordering, package-boundary stops, grouped Need,
   first-ranked semantic error and final sorted matches.
3. `HostGlobSegmentCandidatesKey` in `host_glob/mod.rs` owns directory
   listing plus per-candidate path-resolution/symlink classification.
4. `HostRootPackageBoundaryKey` decides ignored directories and package
   boundaries while glob traversal proceeds.
5. `evaluate_host_package_attempts` in `bzl_module.rs` is the first dormant
   caller: it iteratively evaluates a BUILD attempt, obtains one glob request,
   awaits its prepared result, and restarts evaluation.

These legacy values retain semantic matches/errors but erase the exact Host
observation epochs needed for final reobservation. The accepted
`PathObservationEpoch`, observed path-resolution, repository-ignore,
package-marker, root-package-source and recursive Host-`.bzl` producers are
available for composition. No observed directory-listing, segment-candidate,
package-boundary, traversal or adapter contract is yet accepted.

## Required design decisions

Freeze one bounded architecture answering all of the following:

1. Identify the natural owner and visibility of the first retained observed
   glob key/carrier. Prefer loading-private ownership and the existing
   workspace -> Bzlmod -> loading dependency direction. Do not create a public
   user API or reverse dependency.
2. Prove the complete mutable predecessor frontier for one traversal terminal.
   Include exact directory-listing results, selected and negative
   path-resolution/symlink probes, package-boundary observations, and every
   earlier traversal state that can decide the same success or completed error.
3. Decide whether existing lower keys can expose exact epochs through
   mode-aware sibling projections. If any required lower owner lacks a complete
   observed value, select exactly one smaller prerequisite rather than
   reconstructing demands above its owner.
4. Preserve traversal order: breadth-first state ordinals, raw candidate order,
   boundary checks, operation semantics, recursive-wildcard progress, grouped
   Need, first-ranked complete error and final sorted/deduplicated paths.
   Separate dependency-owned speculative child cache state from the decisive
   prefix retained by the parent.
5. Freeze outer `ObservedPathFrontierError` versus inner legacy semantic
   errors, Need, unsupported-host and cancellation polarity. Need, cancellation
   and outer failure publish no parent carrier or event data.
6. Reuse `PathObservationEpoch::from_shared` with accumulated entries first,
   deterministic structural equality, first exact-Arc retention for equal
   duplicates, and typed mismatch/conflict. Do not add another retained
   container, provenance map, cache, interner, graph or store.
7. Freeze success and completed-error carrier ownership, complete-only DICE
   equality/validity, Arc clone boundaries, `Dupe`/`Allocative` accounting,
   and command/DICE lifetime. Retain no traversal queue, visited set, child
   carrier, matcher scratch, evaluator, transaction or event batch.
8. Decide the exact event boundary. The callerless glob producer should own no
   evaluation data; the later package-attempt owner must remain deferred unless
   this design proves otherwise without combining implementation.
9. Bound Unix proof for literal/wildcard/recursive traversal, directory
   changes, symlink retarget/cycle/error, boundary ignored/package transitions,
   negative probes, Need/error ordering, exact Arc retention, equality,
   warm/A-B/A and cancellation. Record non-Unix availability rather than
   weakening the frontier.
10. Name exactly one future Rust implementation allowlist and production/test/
    total/physical caps, or exactly one smaller docs-only prerequisite. Do not
    combine the observed glob producer with BUILD evaluation or final package
    loading.

## Compatibility boundary

Preserve admitted serial Host-glob validation, traversal, match/error ordering,
package-boundary behavior, Need behavior and existing event behavior exactly.
Existing admitted Host observation values remain exact. Frontier aggregation,
exact-Arc association, sibling identity and final-validation readiness are
Slug-native. BUILD evaluation retry, final package-load aggregation, public
overlap, core request revision, external/routed repository globbing,
repository/materializer work, native-Windows byte ordering and exact Bazel
identity bytes remain unsupported/deferred.

## Proof and validation for this design packet

Use live source and focused tests to prove:

- the complete traversal and segment-candidate dependency graph;
- whether exact listing and path-resolution results remain available without
  duplicate observation;
- whether package-boundary classification already has a complete observed
  predecessor or requires one smaller sibling;
- deterministic decisive-prefix union across traversal success/error/Need;
- one semantic Result Arc plus one Arc-backed epoch as the only retained shape;
- no event owner, evaluator, transaction or traversal scratch in the carrier;
- one-way visibility and no legacy caller/equality change; and
- credible file-specific caps and mandatory cohesion review for any file over
  2,000 physical lines.

No Bazel oracle is required for a callerless representation design. If a
changed exact behavior cannot be resolved from accepted evidence, STOP and
schedule one focused pinned Bazel 9.2 source/oracle packet rather than claiming
parity.

## Authority and caps

Write exactly:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest; and
- `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Read only:

- `AGENTS.md`, `docs/developers/dice.md`, the canonical/current/Stage 2
  ledgers and directly referenced focused tests;
- `app/slug_loading_v2/src/host_glob/{mod,adapter,traversal}.rs`,
  `app/slug_loading_v2/src/bzl_module.rs` and
  `app/slug_loading_v2/src/package.rs`;
- `app/slug_bzlmod_v2/src/host_package.rs` and
  `app/slug_bzlmod_v2/src/host_package_boundary/mod.rs`;
- `app/slug_workspace_v2/src/{path_observation,path_resolution}.rs` and the
  loading/Bzlmod/workspace Cargo manifests; and
- `.codex/skills/slug-buck2-utility-reuse/SKILL.md`, the matching Stages 3/6
  row of `slug-v2-subplans/09-v1-extraction-ledger.md`, and only the directly
  relevant `dupe`, `allocative`, `small_map` and `small_set` sources
  already used by the live owners.

Ledger caps are 40 canonical, 320 current, 280 Stage 2 and 640 total net lines.
No correction is authorized.

## STOP / REPLAN

STOP on any Rust, Cargo, oracle or generated write; a public export, consumer or
behavior change; BUILD/package-load activation; a generic certificate
framework; a new graph/store/cache/interner/container; reconstructed, direct or
historical Host reads; retained evaluator/transaction/queue/visited/event state;
loading/core/public overlap; external repository/materializer/watcher/JVM work;
or ledger cap excess.

REPLAN if exact directory-listing or path-resolution observations cannot be
projected by their natural owner; package-boundary semantics lack a complete
finite predecessor frontier; dynamic traversal cannot retain a deterministic
complete decisive prefix; grouped Need/error ordering would change; one
semantic Arc plus the existing epoch is insufficient; one-way visibility is
impossible; or the smallest implementation cannot be bounded independently.

## Immediate successor

On acceptance schedule exactly one bounded callerless Host-glob frontier
implementation, or exactly one smaller docs-only observed-predecessor design.
Do not combine BUILD evaluation, final package loading, loading consumption,
core publication, public overlap or another consumer.
