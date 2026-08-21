# Current Slug V2 Packet

Packet: `WP-6-7A-host-prepared-module-extension-inputs-observation-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling and Rust base: `50881fc0`

## Goal and authority

Design only the private observed sibling for
`HostPreparedModuleExtensionInputsKey`, now that its two immediate semantic
children have accepted cross-crate Result/epoch carriers. Do not implement or
activate the sibling, pure invocations or any upper owner.

Audit Rust and tests read-only. Preserve the existing prepared-input owner and
trace its exact legacy child order, local join/coercion semantics, errors,
events, retained value and sole production consumer. Specify one bounded
matching-family Result-Arc+transaction-local-epoch sibling and its proof; add no
adapter, umbrella owner or public caller.

Write authority is exactly:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this Stage 6 subplan;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`; and
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`.

Net caps are <=40/<=220/<=180/<=30 respectively and <=470 aggregate. Every
Rust file, test, fixture, oracle, Cargo/BUILD target, API and other plan is
read-only. The design may authorize at most one implementation successor.

## Accepted children and owner boundary

Treat these as accepted and non-writable:

- `e82057f2` plus `50881fc0`: selected evaluation-input requests and their
  doc-hidden observation key/carrier/opaque outer;
- `3a68afa5`: loaded module-extension definitions observation; and
- all lower request, root-file and Host-Bzl observation/event owners.

The frontier audit proves `HostPreparedModuleExtensionInputsKey` is the first
and sole production semantic join of evaluation inputs and loaded definitions.
It owns request/count/order alignment, tag-schema/class validation,
repository-aware attribute coercion and prepared tag grouping/order. Its sole
production consumer is `HostPureModuleExtensionInvocationsKey`.

Pure invocation remains a later owner: it reacquires and validates the
implementation, builds evaluation context, runs Starlark, owns print events and
repository-rule call/result validation. Instantiated/validated repositories,
root mapping, generated/public publication, commands and bootstrap are later or
parallel and remain inactive.

## Design questions

Resolve from the live prepared key/consumer/tests:

1. Freeze exact child order: observed evaluation-input requests first and
   observed loaded definitions second. Specify where each Complete epoch merges
   left-first relative to compute, child semantic and local prepared semantics.
2. Classify every reachable Need, child outer, child semantic and local
   prepared terminal. Decide the minimum typed opaque outer stages and exact
   carrierless/prefixed behavior without exposing Bzlmod or loading internals.
3. Preserve legacy semantic values/errors/order exactly while sharing one
   Legacy/Observed driver and moving the exact local Result Arc into the legacy
   projection.
4. Prove duplicate-Arc preference, conflict/operation mismatch, exact child
   family/order, first-terminal suppression, warm/cancel recovery and held
   semantic A -> B -> A with transaction-local frontier association.
5. Confirm the prepared parent owns no event batch; accepted child events remain
   at request/root/Bzl owners and warm reuse does not replay them.
6. Bound retained lifetime to one local prepared Result Arc plus compact epoch.
   Child carriers/results, loaded modules, frozen Starlark heap, coercion/join
   scratch, event data, locks and tasks must remain compute-local.
7. Define exact all-key/source nonactivation for pure, instantiated, validated,
   root-mapping, canonical/generated/public and command owners.
8. Set one-file implementation/proof caps from the live loading source and keep
   every helper/test below 200. Reuse accepted Bazel 9.2 evidence; add no oracle
   unless a demonstrated exact-compatibility gap exists.

## Compatibility and evidence

Existing prepared-input values/errors/order, schema/class validation, attribute
coercion and child events remain exact Bazel 9 compatibility. The private
observed key/carrier/typed outer and transaction-local Result/epoch association
are Slug-native. Pure/instantiated/validated/root-mapping/generated/public/
bootstrap activation, M8/M7B and exact identity bytes remain deferred.

Read `docs/developers/dice.md` before freezing key ownership, retention or event
behavior. Reuse accepted lower tests and source evidence; add no docs-only
oracle or proof code. Run `git diff --check` on the four records before terminal
review.

## Terminal and stops

Terminate with exactly one bounded prepared-observation implementation packet
or formal `REPLAN` if a private Result/epoch sibling cannot preserve the owner
without widening semantics.

STOP Rust/test/API/export/caller implementation, pure/upper activation, a
second key/adapter/owner, reverse crate dependency, event movement, retained
Starlark heap, lock across DICE, proof waiver, milestone closure, M8/M7B and
exact identity-byte work. M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Implementation `50881fc0`, from audit/design bases `d17637fd` and `a6abf250`,
exposes only the existing evaluation-input observation key/carrier and one
opaque outer through three doc-hidden Bzlmod reexports. The private driver and
typed errors remain unchanged; the key wraps only `Complete(Err)`. Accounting
is 59 semantic lines at 11,687/415/29 physical. Focused/smoke/full Bzlmod,
direct loading, formatting and diff gates pass; independent review returned
`ACCEPT`. No prepared-input caller has been added.
