# Current Slug V2 Work Packet

Packet: WP-6-7A-configured-action-output-conflict-design-r1

Status: independent review ACCEPTS the terminal implementation REPLAN and
selects this docs/source prerequisite design.
No Rust implementation, candidate restoration or runtime admission authorized.

## Learned facts and preserved evidence

WP-6-7A-selected-toolchain-request-implementation-r1 reached a contract stop.
Its candidate fixes parent-configuration retention, full/retained preference
identity, resolver priority and edge propagation. Focused analysis/identity and
core consumer tests pass, but the required two-preference closure discriminator
exposes a missing cross-owner output-conflict check.

Two parents select the same implementation under A/B preferences and unchanged
configuration. Nested leaf toolchains produce different FileWrite contents at
implementation.txt. Both full owners survive the action closure, with distinct
semantic action identities but the same configured output root and relative
path. The current closure view accepts both. CLI and server execute/materialize
them sequentially; materialization removes then rewrites the prior output.
The per-evaluation action registry cannot detect a different owner's conflict.

The candidate is preserved, not accepted:
- Base ca284c2ae.
- /tmp/slug-selected-request.yZYoSi/candidate.patch
- SHA-256 d184284baa4dd28e37c248a4c1fb94b69e26b64a1c54f6c66ef61247b617bfe0
- /tmp/slug-selected-request.yZYoSi/validation.txt records exact gate limitations.
- Rust worktree restored to base; git apply --check accepts the saved patch.
- Core red regression selected_toolchain_request_conflicting_outputs_require_preexecution_rejection
  fails only at the required rejection assertion, 0.04 seconds, exit 101.
  No network execution or actual overwrite was performed.

Stage 6's output-conflict stop records the consumer trace. Reuse the selected
request design and candidate; do not repeat the execution-group general audit,
archive downloads, authenticated replay or already discriminating proof.

## Observable result and reserved decision

Produce an independently reviewed contract for producer-owned cross-owner
configured action output conflict detection, including action equivalence/share
semantics, before restoring/activating the selected-toolchain correction.
The completed vertical must reject different A/B contents at one output before
any execution RPC or materialization, retain legitimate distinct paths and
source-established shareable actions, and preserve source/configuration A/B/A
and overlapping request isolation. Do not reject every duplicate owner or
rename output directories merely to avoid deciding the semantic category.

Trace the natural existing analysis/action-closure producer and immutable
validated value consumed by CLI, server, run and aquery. Decide whether conflict
validation is intrinsic analysis state or root-set-dependent closure state;
record every changed DICE input, equality/cutoff and error-publication boundary.
No command-side scan/repair, global output registry, cache, filesystem guess,
partial publication, silently skipped action or blanket preference rejection.

Bazel 9.2 source authority remains local git object
8220c6198837d5c13d53fea211cf3282aa12408a in /home/wgray/bazel.
Read actions/Actions.java (canBeShared, generating-action assignment and prefix
conflicts), skyframe/IncrementalArtifactConflictFinder.java, action graph/artifact
ownership and FileWriteAction inputs as needed. Adapt buildtool/
OutputArtifactConflictTest: testInvalidatedConflict, testNewTargetConflict,
testTwoOverlappingBuildsHasNoConflict, unusedActionsStillConflict,
repeatedConflictBuild, testConflictAfterNullBuild and relevant prefix cases.
Classify configured aspects, directory artifacts and nonadmitted action families
explicitly; do not import unsupported cases or Java implementation details.
Reuse docs/developers/dice.md and Stage 6's retained DICE worker tests.

## Implementation-readiness gates and scope

Freeze exact producer/carrier/consumer/proof files and gross caps only after the
cross-owner contract is complete. Cover all admitted consumers, structural
output identity versus displayed/configuration/path/ActionKey/REAPI domains,
equivalent-action sharing and deterministic error order. Do not use whole
owner-key inequality as a substitute for source-established action equivalence.
Exact Bazel configuration/output-path byte reproduction remains deferred; a
required Slug-native path representation change needs explicit review, not an
implicit suffix. No broader executor or transport family is authorized.

Read the plan-authoring guide; for retained representation use the utility skill
and matching Stage 9 rows. Freeze memory class, scratch versus DICE retention,
Allocative/sharing, eviction, cancellation, joins and shutdown. Account for
unavailable historical Host state and final source certificates. No lock over
DICE awaits. Record cohesion decisions for any large owner file and a bounded
split if a shared validator would otherwise mix unrelated responsibilities.

Allowed edits: this manifest, canonical Live Status and relevant Stage 4/6 owner
sections. At most 250
added docs lines excluding manifest replacement. No Rust/fixture/harness/
dependency/vendored/runtime edits or candidate restoration in this packet.
The preserved patch is not a fallback and must not be partially shipped.

Validation: source/structure checks and git diff --check; no new builds/tests,
network replay or materialization required for this design. The previous core
and REAPI commands hit 60 seconds during compilation, not test execution; core
compile-only completed within its investigated 180-second bound. Do not claim
the unrun REAPI, observed cancellation or full dependent gates pass.
Tests over one minute remain a red flag; fifteen minutes is the absolute cap.

After architecture acceptance, select one bounded implementation that completes
the conflict prerequisite and selected-toolchain request correction together,
reusing the saved candidate and red regression where still applicable. Then
resume the shared named/automatic execution-group contract. Loading invocation,
computed-default and C++/Java runtime guards remain unchanged.
