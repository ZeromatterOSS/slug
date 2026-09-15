# Current Slug V2 Work Packet

Packet: WP-4-7A-configured-conflict-path-observation-typed-invalidation-provenance-r1
Status: corrected docs-first design pending independent acceptance

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

Do not implement or replay until this corrected design is independently
accepted. Do not run another aggregate activation/event audit, F3, a sibling
selector, or this selector outside the frozen typed instrumentation. Do not
broaden to `ResolvedPathObservationKey`, change DICE semantics, retain identity
or version material, infer optimization work, merge the combined stack or push
the review branch.
