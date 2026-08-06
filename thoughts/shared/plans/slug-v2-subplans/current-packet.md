# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-query-unit-preparation-restart-repair-design`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: an accepted, evidence-backed bounded repair or terminal REPLAN for the
sole clean-baseline failure blocking the 28-case query crate-mode target.

## Goal

Determine why
`external_restricted_visible_uses_canonical_fake_caller_without_a_second_route`
returns `QueryErrorKind::PreparationRestart`, freeze Bazel 9.2 authority and
the exact live owner, and select the smallest valid successor without editing
Rust.

## Required design

Read `docs/developers/dice.md` before auditing the Need/restart path. Reuse or
reproduce the clean Cargo/Bazel 27/28 result, then trace the external
Restricted-visibility fake-caller request through query loading, source
preparation, repository mapping, DICE keys, Need propagation, and retry
ownership. Establish the accepted Bazel 9.2 semantic result and distinguish a
stale expectation from a missing route, incorrect canonical caller identity,
or invalid restart lifecycle. Inventory equality/invalidation/event/error and
cold/warm/edit/delete/recreate consumers affected by any proposed owner.
Freeze exact files, tests, downstream/platform validation, line caps, and stop
conditions for one implementation packet, or record terminal REPLAN if no
bounded exact Rust correction exists.

## Allowed paths

- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Record the exact clean failure and accepted Bazel 9.2 oracle/pinned-source
authority, live key/request/caller/Need source anchors, the complete proposed
evidence matrix and line arithmetic, and independent DICE/identity design
review. Run structure, scope, cap, credential-pattern, and `git diff --check`
gates; no implementation suite is authorized in this packet.

## Stop conditions

Stop with REPLAN on a second route that duplicates source preparation, a
callerless/direct-filesystem/fresh-graph bypass, lock held across DICE compute,
changed global query identity, erased Need/restart semantics, filtered or
expected-failure tests, fixture/host-tool coupling, unbounded redesign, or any
query/cquery/aquery formatter expansion, execution/cache, self-hosting,
Java/JVM delegation, Bazel 8, WORKSPACE, rc, or credential dependency.

## Diff budget

- At most 240 net documentation lines. No Rust, BUILD, Cargo, lock, fixture,
  generated-source, CI, or unrelated change.
