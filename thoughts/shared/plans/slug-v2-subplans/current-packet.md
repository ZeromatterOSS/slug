# Current Slug V2 Packet

Packet: `WP-2A-m1-host-glob-frontier-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Accepted predecessors: `5816e435`, `daf5eef9`
Result: audit and freeze the next complete natural owner above the accepted
private observed root-package loading frontier.

## Design task

Start from the live checkout and enumerate every direct and transitive consumer
of `RootPackageLoadKey`, including any loading lookup, package projection,
request/certificate, core bridge and event-publication boundary already accepted
by the canonical plan. Trace where the semantic package Arc is retained,
projected or republished and where the new observation epoch would currently be
erased.

Select the uniquely smallest complete owner that can consume
`RootPackageLoadObservationKey` without reconstructing Host observations. If an
already accepted private loading-side or publication-side owner is the complete
boundary, freeze its observed sibling/shared driver. If one smaller prerequisite
is required, record only that prerequisite. Do not activate or edit a caller in
this docs-only packet and do not publish a partial certificate.

## Required audit

- map every live root-package consumer and its semantic/event authority;
- identify the exact natural owner of the next retained semantic Result Arc and
  whether it already has an observation/certificate carrier;
- preserve existing dependency and event order, semantic errors, Need, typed
  outer errors, cancellation and terminal publication;
- freeze strict Legacy/Observed family isolation without one sibling computing
  the other or duplicating loading;
- classify all retained state versus compute-local request, projection, event,
  child and union scratch;
- specify complete-only equality/validity, exact-Arc union, warm/edit/delete/
  recreate/A-B-A, cancellation and activation/nonactivation proof; and
- classify changed behavior as exact, Slug-native or unsupported/deferred and
  cite accepted evidence or pinned Bazel 9.2 source for exact claims.

Read only the bounded loading/publication/core owners and their directly related
tests, plus accepted predecessor implementations and plan evidence. Read
`docs/developers/dice.md` before proposing any key or compute ownership change.
Do not run or add an oracle unless the audit demonstrates a specific parity gap.

## Authority and caps

This packet is docs-only. Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest; and
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Completion scheduling is limited to 180 aggregate net lines. The design packet
itself remains under 40 canonical, 320 manifest, 280 Stage 2 and 640 aggregate
net lines. Require exact ledger accounting, plan consistency, independent design
review and `git diff --check`.

## Compatibility boundary

Accepted package evaluation, Host-glob matching/results/order/diagnostics and
event behavior remain exact. Synchronous replay, frontier association,
deterministic first-Arc union and typed outer errors remain Slug-native. Public
or core activation, repository package globbing/materialization, native-Windows
raw-byte ordering and exact Bazel identity bytes remain unsupported/deferred in
this packet.

## STOP / REPLAN

STOP on Rust, Cargo, fixture or oracle writes; any caller/public/core/repository/
materializer activation; a partial certificate; reconstructed or historical
Host reads; duplicated loading or event ownership; changed request/dependency/
error/event order; a new retained standard collection, cache, graph, store,
lock or task; or cap excess.

`REPLAN` to exactly one smaller docs-only natural-owner prerequisite if the next
consumer cannot receive the observed root-package carrier locally, an accepted
intermediate frontier is incomplete, family isolation would duplicate loading,
or proof needs another code owner/seam/oracle. Record an unsupported boundary if
no bounded Rust-native owner exists.

## Immediate successor

On independent design acceptance schedule exactly one bounded implementation or
one uniquely smaller docs-only prerequisite. Do not combine public/core cutover,
repository package globbing or materialization.
