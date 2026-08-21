# Current Slug V2 Packet

Packet: `WP-6-7A-extension-definition-evaluation-observation-frontier-audit-resume`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling and Rust base: `3a68afa5`

## Goal and authority

Resume the read-only extension definition/evaluation frontier after accepting
both parallel immediate carriers: selected evaluation-input requests in Bzlmod
and loaded module-extension definitions in loading. Identify the uniquely
smallest next reusable semantic owner or one uniquely smaller carrier-
visibility/evidence prerequisite. Do not implement or activate it.

Audit Rust and tests read-only. Trace the exact first consumers of
`HostSelectedExtensionEvaluationInputRequestsKey` and
`HostLoadedModuleExtensionDefinitionsKey`, beginning with
`HostPreparedModuleExtensionInputsKey`, then pure invocations, instantiated and
validated repositories. Inspect root mapping, canonical/generated repository
definitions, command/publication and bootstrap only far enough to reject false
prerequisites or umbrella ownership.

Write authority is exactly:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this Stage 6 subplan;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`; and
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`.

Net caps are <=40/<=220/<=180/<=30 respectively and <=470 aggregate. Every
Rust file, test, fixture, oracle, Cargo/BUILD target, API and other plan is
read-only. The audit may authorize at most one bounded design successor.

## Accepted frontier

Treat these as accepted and non-writable:

- `094ba075` selected extension definition-load-request observation;
- `e82057f2` selected extension evaluation-input-request observation;
- `99c23033` doc-hidden request-carrier promotion; and
- `3a68afa5` loaded module-extension-definition observation.

The loaded-definition owner preserves request -> root Bzl label -> observed
Host-Bzl module -> named export order, left-first Complete epoch merging,
first-terminal suppression, child-only event batches, compact Result/epoch
retention and semantic lifecycle/cancellation/nonactivation proof. The
evaluation-input owner preserves request -> root-files -> pure input projection
and its own exact epoch/error/event boundary. Do not reopen either owner.

## Audit questions

Resolve with live key/consumer traces and ownership evidence:

1. Is `HostPreparedModuleExtensionInputsKey` the first complete reusable join
   of the two accepted parallel carriers, or does one smaller missing observed
   child/visibility seam precede it?
2. Which exact semantics belong to prepared inputs versus
   `HostPureModuleExtensionInvocationsKey`—especially repository rules,
   extension metadata, evaluation context and invocation grouping?
3. What are the exact child order, Need/outer/error precedence, Complete epoch
   merge order, duplicate-Arc policy, event owner and retained lifetime at the
   selected boundary?
4. Which upper instantiated/validated/root-mapping/generated/public/bootstrap
   consumers are independent or strictly later, and therefore must remain
   inactive?
5. Which behavior remains exact Bazel 9 compatibility, which private
   Result/epoch association is Slug-native, and which identity/public/bootstrap
   work remains deferred?

Use direct consumer counts and source ownership. Reject an umbrella owner that
combines independent prepared, pure, instantiated or validated semantics. If a
cross-crate type is unavailable, prove whether a visibility-only prerequisite
is uniquely smaller before scheduling it.

## Evidence and validation

Reuse accepted Bazel 9.2 loading/Bzlmod evidence and existing lower tests. Add
no oracle or proof code. Read `docs/developers/dice.md` before classifying any
DICE owner, retention or event boundary. Check `git diff --check` on the four
records before terminal review.

## Terminal and stops

Terminate with exactly one of:

- one bounded owner design packet;
- one uniquely smaller carrier-visibility/evidence packet before that design;
  or
- formal `REPLAN` if no bounded Rust-native owner exists.

STOP implementation, Rust/API/export/caller change, second simultaneous
successor, event movement, retained Starlark heap, proof waiver, milestone
closure, M8/M7B or exact identity-byte work. M7 remains partial and
M7A -> M8 -> M7B remains.

## Immediate predecessor

Implementation `3a68afa5`, from loaded-definition design base `0a8e1220` and
the accepted serial proof corrections through `3388a8fd`, completes the private
loaded-definition observed owner in `slug_loading_v2`. Final accounting is
`+1,518/-112`, 8,288 physical. Exact focused parent/lower tests, full loading,
direct core, formatting and diff gates pass; independent terminal review
returned `ACCEPT`. This audit resumes the prepared/evaluation frontier promised
by the accepted design without activating an upper consumer.
