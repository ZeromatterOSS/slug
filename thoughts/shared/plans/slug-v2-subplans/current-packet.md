# Current Slug V2 Work Packet

Packet: WP-4-7A-configured-conflict-path-observation-typed-invalidation-provenance-r1
Status: accepted typed provenance result; diagnostic chain stopped

## Accepted predecessor and boundary

The activation audit is accepted at a coherent 220/220/219 handoff cutoff. It
observed 2,488 legacy-delivered evaluated `PathObservationKey` callbacks, all
with an immediate exact `PathObservationShardKey` dependency. Reused and
no-direct cells were zero. Observer health, exact PID/reap, `RootCompute`
deadline and cleanup gates passed. All temporary artifacts were removed and the
receipt is durable at `914161858`.

That aggregate result establishes no unique identity, per-window pairing,
invalidation source, necessity, cost or avoidability. Another callback/event
counter is forbidden. Independent review permits a materially different typed
invalidation-provenance packet and, after full design and preexecution review,
one fresh replay of the same authentic selector.

## Typed observable

Use the existing public DICE
`DiceComputations::get_invalidation_paths()` API. Its documented invalidated
path order is source first and current dependency last. Capture only the normal
priority dependency-context snapshot inside `PathObservationKey::compute`,
after one selected dependency resolves and before returning the unchanged
`PathOutcome`:

- after a successful `PathObservationShardKey` compute, capture before both the
  present-result and shard-missing `Need` returns; an invalidated path must be
  nonempty, at most 64 entries, and end in an exact shard key;
- after a shard compute error and successful `PathObservationEpochKey` fallback,
  capture before both the present-result and epoch-missing `Need` returns; an
  invalidated path must be nonempty, at most 64 entries, and end in the exact
  epoch key;
- if both dependency computations fail, preserve the existing `Need` result and
  store a malformed diagnostic summary when capture is armed;
- `Clean` and `Unknown` snapshots contain no traversed identity and are retained
  as those exact fixed-size categories.

For a valid invalidated path, downcast only the first entry and retain one
fixed-size identity-free category: source exact shard, source exact epoch, or
source other. Empty, over-64 or wrong-terminal paths retain `Malformed`.
`get_invalidation_path()` necessarily allocates a `Vec` before its length can be
bounded, and DICE boxes evaluation data. Only the fixed-size enum survives as
evaluation data. Preserve the 2 GiB supervisor limit and make no allocation,
cost or timing inference.

Add one doc-hidden workspace marker to `UserComputationData` only when the exact
probe observer is attached. With the marker absent, execute the original
branches and returns without calling either invalidation API or evaluation-data
API. With it present, ignore any `store_evaluation_data` error so the key result
cannot change. The legacy tracker accepts only the exact summary type on exact
evaluated `PathObservationKey` callbacks. Missing, wrong or `Malformed` payload,
including a duplicate-store outcome, sets sticky diagnostic error and cannot
produce an accepted cutoff. Reused legacy callbacks and all rich-only callbacks
remain outside this provenance census.

## Six-word protocol and lifecycle

Reuse words 51--53 and 60--63 for the accepted installed-merge/next-commit
lifecycle, flags and callback claim. Use the only six aggregate cells as:

| Word | Aggregate |
|---:|---|
| 54 | total valid typed evaluated callbacks (`T`) |
| 55 | clean dependency snapshot (`C`) |
| 56 | unknown dependency snapshot (`U`) |
| 57 | invalidated with exact shard source (`S`) |
| 58 | invalidated with exact epoch source (`E`) |
| 59 | invalidated with another typed source (`O`) |

After exact key, evaluated-kind and armed/disabled checks, acquire word 63
before reading evaluation data and recheck state. Precheck the selected category
and total for overflow, publish category first and total last, then release the
claim last. Contention, malformed/missing/wrong payload, lifecycle error,
counter overflow or invalid claim sets sticky word-62 state and refuses valid
publication. Wrong keys, reused callbacks and disarmed/disabled callbacks are
neutral.

At cutoff, checked-add `C + U + S + E + O` and require it to equal `T > 0`.
Require `P = committed > 0`, pending zero, active one, closed plus one equal to
committed, flags zero and claim zero. There is no valid global activation bound.
Every partial category-before-total or killed callback fails by arithmetic or
claim. A positive `S` proves only that DICE reported an exact shard as the
normal-priority source of invalidated data in the selected dependency context
for that evaluated callback. It does not prove necessity, cost or avoidability.

## Proof and one replay

Add one exact workspace proof using real DICE version updates. Prove documented
source-to-terminal order and terminal checks for shard success, shard-missing,
shard update, unrelated update, epoch fallback, clean and unknown snapshots,
other source, empty/over-cap/wrong terminal rejection, marker off/on and
unchanged results on diagnostic storage failure.

Add one exact feature-enabled Core proof. Cover the doc-hidden marker install,
exact evaluation-data downcast, every valid category, missing/wrong/malformed
payload, reused and rich-only exclusion, rich/root and demand/effect behavior,
armed/disarmed/disabled neutrality, lifecycle, contention, all overflows, every
category-before-total partial, claim-last release and checked decoder equations.
Prepare each exact proof under 60 seconds and run only its exact selector under
12/15 seconds.

Derive one 0644 excluded supervisor from the last accepted base. Extend only its
words 51--63 decoder/result and self-check for the five-way sum, total, lifecycle,
claim, both flags, zero total, every partial, checked overflow, output cap and
normal/deadline/exception cleanup. Preserve namespace isolation, descriptor
ownership, exact reads, wall deadline, kill/reap finalization and resource caps.

Compile only the feature-enabled `cli` integration target. Only exact nonignored
selector `configured_action_conflicts::one_shot_build_conflict_is_atomic_and_recovers`
adopts the inherited observer and external fixture. Preflight exact listing,
verify a fresh fixture, freeze all artifacts, and obtain independent
preexecution review before exactly one 12/15-second portable replay. There is no
retry.

## Scope, caps and stops

Temporary production owners are limited to the workspace path-observation
owner/export, Core user-data installation/tracker/observer/mapping, CLI library
gate and selected test adapter. Proof owners are one workspace proof, one Core
proof, the CLI adapter and one excluded supervisor. No DICE crate, loading,
analysis, fixture or oracle input may change. Caps are 230 gross production,
310 gross proof and 140 changed supervisor lines.

Durable edits remain limited to status/scheduling sections of the canonical
plan, this manifest, Stage 4, bootstrap readiness and configured CLI ledger.
Restore every temporary source, supervisor, fixture and log artifact before the
result receipt. Run exact proofs, plan-status and diff checks and independently
review both preexecution and result.

The corrected design, implementation preparation and sole replay completed
their independent reviews. Do not replay again, run another aggregate
activation/event audit, F3 or a sibling selector. Do not broaden to
`ResolvedPathObservationKey`, change DICE semantics, retain identity or version
material, infer optimization work, merge the combined stack or push the review
branch.

## Frozen preparation checkpoint

The final temporary change fits the 230/310/140 caps at 221 production
additions, 310 proof additions and 76 changed supervisor lines. The exact
workspace and feature-enabled Core proofs each pass 1/1. The corrected 0644
supervisor passes syntax plus normal/deadline/exception cleanup and decoder
self-checks. Its compiler completed the sole `cli` integration target with zero
errors in 56.53 seconds; Cargo's required non-test `slug` binary is retained as
the integration harness dependency. The exact nonignored selector lists once.

The fresh fixture retains 28 objects, 177 registry metadata files, 8,004,740
source bytes and inventory
`4337d0756cefc0971a76e12bbeea54ee40c24beb0ff943a4c3bdc60d88ed764f`.
Frozen SHA-256 values are: base supervisor
`d9a012989d595e6da275094d4ce020a8e8a8214e1714df79e8a3285282191179`,
typed supervisor
`b81d12e42bc3801126e9ac837ec9ffd8727adf01421831826628d637930c5b2e`,
exact Rust diff
`4bf84230b7e5a299d35a102eeb3c7605551922c1e05cf1311648ce40e3116606`,
workspace proof binary
`7a87acf55b2de44903975b698928029064be1b0a07bb76b965177f7f78cd1a7a`,
Core proof binary
`d31095972f62ea0976e172bf4d60d597d360b45d058ba2ff57aa39935aa55c13`,
CLI harness
`63e77c3247a107a2bce13af79a05345c6e1194728e82d77daf4c53c4335b4e56`
and required `slug` binary
`c26d13fb1798ca22f4cd44c5d7851edbba6af7f8fccdd53703e70504a51108af`.
Independent preexecution rereview authorized exactly one no-retry replay. It
reached the expected 12.013-second `RootCompute` deadline with one exact
installed/reaped observer PID and complete cleanup. Lifecycle was coherent at
222 produced, 222 committed and 221 closed, with pending zero, active one,
flags zero and claim zero. The checked typed census was
`T=2541=C0+U0+S2541+E0+O0`; global overflow, dropped samples and activity claim
were zero. Independent result review returned `ACCEPT`.

This proves only that 2,541 valid legacy-delivered evaluated exact
`PathObservationKey` callbacks observed in aggregate armed windows had a
normal-priority dependency-context invalidation path whose source downcast to
`PathObservationShardKey`; the classifier also required its terminal to be the
expected shard-key type. It identifies no shard, path or version, pairs no
callback to a merge/window, proves no evaluation cause, and establishes no
necessity, cost, avoidability or comparison with the predecessor count. Stop
the diagnostic chain. Any semantic change requires a new docs-first
architecture packet based on source invariants and ordinary correctness proofs.
