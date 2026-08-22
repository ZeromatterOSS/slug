# Slug V2 Plan Authoring Guide

## Purpose and authority

Use this guide whenever a Slug V2 packet, replan, roadmap, or donor review is
created or materially revised. The canonical plan owns milestone priority, the
compact `slug-v2-subplans/current-packet.md` manifest owns the active packet,
and stage plans own subsystem decisions and reusable evidence. This guide owns
plan readiness and hygiene; it does not authorize implementation or widen an
active packet.

Keep worker prompts short. Put durable architecture and evidence requirements
in plans, then route workers to the accepted packet. Historical chronology is
evidence, not scheduling authority.

## Required packet record

Every implementation packet must state the following, omitting a section only
when the packet explains why it is inapplicable:

1. **Learned facts and research basis**
   - Name the relevant Bazel 9.2 source and tests before implementation.
   - For DICE behavior, name the relevant Buck2 DICE documentation or tests
     covering dependency recording, invalidation, equality cutoff,
     projections, cycles, transactions, duplicate work, cancellation, or
     publication.
   - For a donor implementation, classify each candidate as leaf reuse,
     concept/test only, or avoid.
2. **Decision and non-decisions**
   - State what the packet chooses, what it deliberately does not choose, and
     what observation will prove the decision.
   - Preserve the canonical `exact`, `Slug-native`, and
     `unsupported/deferred` compatibility classes.
3. **Natural semantic owner**
   - Name the producer, DICE key or tracked dependency, and retained value that
     own every changed semantic fact.
   - Prefer a producer-owned fact over a command-side repair, replay cache,
     global registry, path inference, or fallback scan.
   - State whether command result sets remain request-local or are reusable
     semantic facts.
4. **Request and revision behavior**
   - For command options, environment, source reads, repositories, lockfiles,
     watchers, or daemon work, state the immutable request projection,
     observed inputs, final validation boundary, and overlapping-request
     behavior.
   - Do not assume that a mutable host filesystem supplies historical snapshot
     reads. Unavailable historical state is unsupported rather than guessed.
5. **Memory and asynchronous ownership**
   - Classify new memory as service/container, DICE-retained semantic,
     service-retained nonsemantic cache, command-retained,
     phase/action/RPC scratch, or transfer-owned async memory.
   - State publication, equality-cutoff, invalidation, eviction, cancellation,
     task-join, and shutdown release boundaries that apply.
   - A retained value must not borrow command scratch or an evaluator heap.
6. **Evidence and fixture provenance**
   - Reuse accepted discriminating evidence before adding a fixture.
   - Every new fixture follows the Stage 1 `fixture.toml` provenance contract
     and records exact, message-shape, or structured-semantic comparison.
   - Record why a relevant upstream test was skipped: unsupported phase,
     implementation-detail assertion, obsolete Bazel behavior, or stronger
     existing coverage.
7. **Fallback ledger**
   - A temporary bridge or fallback names the violated invariant, exact
     deletion condition, owning future packet, and regression that prevents it
     from becoming permanent.
   - A fallback without those four fields is not plan-ready.
8. **Scope and stops**
   - Name an observable result, exact file allowlist, exclusions, production
     and test growth caps where useful, validation, residual risk, and explicit
     `REPLAN` conditions.
   - Do not broaden a packet merely because adjacent work is convenient.

## Upstream-test and donor policy

Bazel source and tests remain the compatibility oracle. Zabel, Buck2, V1, and
other implementations may supply design ideas and fixture themes, but cannot
replace Bazel 9.2 evidence for an exact claim.

For an exact byte-algorithm donor, the packet may classify a bounded encoder or
fingerprint helper as leaf reuse/concept input only after it lists every ordered
byte input, conditional field, framing rule, and domain separator. Donor output
vectors are regression candidates, not acceptance authority: replace the exact
claim with pinned Bazel 9.2 source anchors and fresh discriminating oracle
evidence. The packet must also state which semantic owner supplies each input
and which consumers may use the derived projection.

For a migrated test theme:

- identify the upstream Bazel class, method, or shell test that establishes
  the observable behavior;
- keep a short adaptation note in the plan and fixture manifest;
- prefer public BUILD, MODULE, Starlark, command, or REAPI behavior over Java
  implementation details; and
- start with a focused workspace before promoting a real-workspace stress
  case.

A prior-art review should explicitly record:

| Classification | Permitted use |
|----------------|---------------|
| Leaf reuse | Small isolated utility or protocol code after ownership, license, and compatibility review |
| Concept/test only | Architecture contract, failure lesson, benchmark method, or fixture theme reimplemented behind Slug owners |
| Avoid | Donor scheduler/runtime, semantic side store, fallback repair, unverified identity output, or monolithic orchestration |

## Complexity and document hygiene

The packet author inspects physical size and responsibility boundaries before
adding to a large file. These are review triggers, not automatic rewrite
orders:

- a touched production file above 2,000 lines;
- a touched function above 150 lines;
- a file that combines semantic production, command presentation, persistence,
  and transport;
- repeated key registration, validation, error translation, or traversal; or
- an active plan accumulating completed chronological evidence.

When triggered, the packet records either a bounded split or a concrete reason
the existing owner remains cohesive. Do not create a central file that owns
every key family, action kind, builtin, or command. Tests may be colocated when
that improves ownership, but no policy should force tens of thousands of test
lines into one physical production module.

Keep the canonical Live Status and current packet compact. Move completed
chronology into capped companion evidence files or Git history. A stage owner
should describe current architecture and reusable decisions; it should not be
the only archive of every worker turn.

On packet rollover, replace the manifest with the new active contract plus at
most one compact immediate-predecessor summary; do not prepend another contract
to an unbounded historical tail. Existing oversized history is cleaned only by
a bounded docs-only compaction that preserves Git reachability and cannot alter
milestone state, packet authority, compatibility, evidence, or the selected
successor. Document compaction never delays an otherwise ready semantic packet
unless the active contract itself is ambiguous.

## Performance decision discipline

Apply performance gates only to a demonstrated hot path or retained-memory
change. Correctness and exact outputs are prerequisites.

- Compare an exact control and candidate with alternating runs, preferably
  A/B/B/A or a longer balanced sequence.
- Record workload, fresh/warm state, exact output and RPC invariants, retired
  instructions, cycles, wall time, and RSS where available.
- State an acceptance threshold before measuring.
- Keep rejected experiments and their measurements in a compact ledger so
  later work does not repeat them.
- Reject an optimization that merely moves cost to another phase or regresses
  the declared primary metric outside noise.

## Plan-ready checklist

- [ ] Canonical milestone and current-packet relationship is explicit.
- [ ] Bazel 9.2 source/tests and any Buck2 DICE evidence are named.
- [ ] Learned facts, decision, non-decisions, and proof are separate.
- [ ] Exact, Slug-native, and unsupported/deferred surfaces are classified.
- [ ] Natural producer/key/value ownership is named.
- [ ] Request overlay, input observation, and overlapping-session behavior are
      covered when applicable.
- [ ] Retained, command, scratch, cache, and async-transfer lifetimes are
      covered when applicable.
- [ ] Fixture provenance and skipped-upstream-test reasons are recorded.
- [ ] Every fallback has an invariant, deletion condition, owner, and test.
- [ ] File allowlist, growth caps, validation, residual risk, and stops are
      bounded.
- [ ] Complexity triggers and hot-path measurement needs were reviewed.
- [ ] Completed chronology has an evidence destination rather than expanding
      the active scheduling surface.
