# Current Slug V2 Packet

Packet: `WP-6-7A-host-selected-extension-evaluation-input-requests-observation-carrier-promotion-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `87256749`
Rust base: `3a68afa5`

## Goal and authority

Design only the uniquely smaller visibility prerequisite selected by the
prepared/evaluation frontier audit. Make the accepted evaluation-input
observation carrier usable by its natural prepared-input consumer in
`slug_loading_v2` without changing semantics or activating that consumer.

Audit Rust and tests read-only. Freeze the existing evaluation-input driver,
legacy key, Result/epoch association, Need/outer/error algebra, equality,
validity, eventlessness, retention and lifecycle behavior. Specify the minimum
doc-hidden Bzlmod -> loading API and one external-crate compile smoke. Do not
implement it.

Write authority is exactly:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this Stage 6 subplan;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`; and
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`.

Net caps are <=40/<=220/<=180/<=30 respectively and <=470 aggregate. Every
Rust file, test, fixture, oracle, Cargo/BUILD target, API and other plan is
read-only. The design may authorize at most one visibility-only implementation
successor.

## Accepted frontier and audit finding

Treat `e82057f2`'s evaluation-input observation,
`99c23033`'s definition-request carrier promotion and `3a68afa5`'s loaded-
definition observation as accepted and non-writable.

`HostPreparedModuleExtensionInputsKey` is the first and only production
semantic consumer of both legacy carriers. Its observed loaded-definition
sibling is already in the same loading module, but the evaluation-input
observation key, carrier and typed outer are private to `slug_bzlmod_v2`.
Therefore a visibility-only promotion is uniquely smaller than prepared-input
ownership. Pure invocation, repository instantiation, validation, root mapping,
generated publication, commands and bootstrap remain later or parallel.

## Design questions

1. Confirm the minimum nominal surface: the existing observation key, observed
   carrier and one opaque public observation-error wrapper, with only the key
   constructor and carrier `result()`/`observations()` accessors public.
2. Decide the exact three doc-hidden crate-root reexports and external-crate API
   smoke needed to prove loading can construct the key and inspect the public
   Result Arc/epoch/opaque outer without exposing private observation stages.
3. Freeze the associated `Key::Value` wrapping boundary so same-module Bzlmod
   tests/children can unwrap locally while external consumers see only the
   opaque wrapper. Add no adapter key, copied carrier or reverse dependency.
4. Preserve exact Display/key identity, Complete-only validity/equality,
   Result-Arc/transaction-local-epoch association, carrierless Need/outer,
   child event ownership, warm/cancel behavior and retained lifetime.
5. Bound future implementation authority and proof caps from the live
   11,676-line `selected_repo_spec.rs`, 409-line `lib.rs` and the existing
   definition-request promotion smoke pattern. Require a dependent loading
   compile check and unchanged focused/full Bzlmod evidence; add no oracle.

## Later prepared boundary, not active

The later prepared owner consumes observed evaluation inputs first and observed
loaded definitions second. It owns only request aggregate/count/order joins,
tag-schema/class validation, repository-aware attribute coercion and prepared
tag grouping/order. Pure invocation separately owns implementation
reacquisition/drift checks, evaluation context and Starlark execution,
repository-rule call capture, print events and result validation.

For that later owner, each Complete child epoch merges left-first before child
semantics and join/coercion. Need and typed child outer are carrierless;
semantic terminals retain the completed prefix appropriate to their context.
The parent remains eventless and retains only its local semantic Result Arc plus
compact cumulative epoch. This packet neither designs nor activates it.

## Compatibility and evidence

Existing evaluation-input values/errors/order/root metadata/tags and child
events remain exact Bazel 9 compatibility. The hidden observation API, opaque
typed outer and shared-Arc transaction-local epoch association are Slug-native.
Prepared/pure/instantiated/validated/root-mapping/generated/public/bootstrap
activation, M8/M7B and exact identity bytes remain deferred.

Reuse the accepted evaluation-input observation tests and the prior
definition-request external API-smoke pattern. Add no oracle or proof code in
this docs-only packet. Read `docs/developers/dice.md` before fixing the DICE
surface. Check `git diff --check` on the four records before terminal review.

## Terminal and stops

Terminate with exactly one bounded visibility-only implementation packet or
formal `REPLAN` if the opaque usable carrier requires a new semantic owner or
unbounded transitive API.

STOP Rust/API/export/caller/test implementation, prepared-input design or
activation, a second key/adapter/owner, mapping/root-file internal exposure,
event or retention movement, proof waiver, milestone closure, M8/M7B and exact
identity-byte work. M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

The audit at scheduling base `87256749` traced one production direct consumer
for each accepted legacy carrier: `HostPreparedModuleExtensionInputsKey`.
Prepared then has one production consumer (`HostPureModuleExtensionInvocationsKey`),
followed one-to-one by instantiation and validation; generated repository
definition is the sole non-test validated-spec consumer. Root mapping consumes
selected extension mappings independently. The missing cross-crate
evaluation-input observation surface is therefore the sole smaller prerequisite.
