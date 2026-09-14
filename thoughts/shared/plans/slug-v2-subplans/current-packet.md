# Current Slug V2 Work Packet

Packet: WP-7A-execution-group-order-replan-r1
Status: ready; docs/source scheduling replan only

## Result and contradiction

Freeze an implementation-ready prerequisite order for the selected-toolchain
request correction, configured-action output conflicts, and shared rule
execution-group runtime. The bounded authentic F3 proof now reaches
rules_java `toolchains/BUILD:138`, then rules_cc `cc_library.bzl:19`, and fails
at the retained named execution-group target-invocation guard in 9.98 seconds.
This directly disproves the former schedule assumption that F3 could complete
before execution-group work.

The execution-group design already established that this selected rules_cc
owner declares named `cpp_link` and `_use_auto_exec_groups=True`. Its complete
runtime requires the selected-toolchain request correction. That correction
exposed the cross-owner output-conflict prerequisite; both remain together in
the unaccepted R2 candidate at `27e9e9c0c`, based on `97dffd5d4`. Canonical
scheduling previously blocked R2 on F3, creating the cycle now proven by the
authentic fixture.

## Decision to produce

Choose and document one atomic sequence that breaks this acceptance cycle while
preserving every semantic gate. Prefer an independently acceptable prerequisite
checkpoint only when its focused owner and consumer evidence does not claim F3
or shared group-runtime acceptance. Otherwise keep the combined candidate and
name the exact non-F3 evidence that can accept it before group activation.

The result must state:

- whether the current R2 candidate can be reconciled onto `main` and accepted
  before F3 using its source/configuration/output-conflict/consumer gates;
- which remaining baseline failures materially gate that acceptance, separating
  the two already attributed failures from unexamined full-core failures;
- the exact point at which the complete named and automatic group runtime can
  remove the loading guard, and the F3 replay that follows it;
- whether any candidate portion is independently cohesive. Do not split it only
  to bypass an unmet gate;
- the new active implementation packet, exact file allowlist, caps, commands,
  review requirements, and observable stop.

A second general execution-group audit, a new archive, or another unchanged F3
run cannot answer this scheduling decision.

## Evidence and authority

Reuse these records rather than reconstructing them:

- F3 fixture inventory SHA-256
  `4337d0756cefc0971a76e12bbeea54ee40c24beb0ff943a4c3bdc60d88ed764f`,
  28 objects, 8,004,740 source bytes and 177 metadata entries;
- the final F3 receipt: one selected/executed failed probe, native exit 1,
  9.98 seconds, valid observer and complete cleanup, with 47,625 starts,
  41,735 dependency checks and 7,432 computes;
- Stage 6 sections `Rule execution-group runtime prerequisite: named-only
  REPLAN` and the selected-toolchain/output-conflict owner contracts;
- candidate commit `27e9e9c0c`, its `review-evidence/validation.txt`, original
  patch SHA-256
  `90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`,
  and the clean-base attribution recorded there;
- Bazel 9.2 authority at local object
  `8220c6198837d5c13d53fea211cf3282aa12408a` and the existing source/test
  anchors already recorded in Stage 6.

The selected request, conflict, and execution-group semantics remain exact only
for the admitted Bazel surfaces. DICE ownership, structural keys and publication
boundaries remain Slug-native. Computed defaults, C++/Java rule behavior,
configured aspects, broader action families, and exact configuration/output
bytes remain deferred.

## Scope, review, and validation

Allowed edits are this manifest, canonical Live Status, the configured-fixture
ledger, bootstrap readiness, and the relevant current Stage 4/6 owner sections.
No Rust, fixture, harness, dependency, vendored source, branch integration, or
runtime change is authorized. Keep additions under 240 text lines excluding
this manifest replacement.

Inspect the preserved candidate and current `main` structurally; do not apply it
in this packet. Reconcile all landed prerequisites and identify conflicts by
file and owner. Read the plan-authoring guide and obtain independent design
review because the decision changes cross-stage semantic acceptance order.
Validation is source/structure inspection, `python3 scripts/v2_plan_status.py`,
and `git diff --check`; no build, network access, daemon, materialization, or F3
replay is needed.

Return `REPLAN` only if accepting any prerequisite before F3 would require a new
semantic owner or weakening an existing gate. In that case name the exact owner
and a bounded successor. Never erase group names, substitute the default
platform, move the loading guard early, waive output conflicts, use a command
side scan, or treat diagnostic progress as configured-source acceptance.

## Immediate predecessor

The package-attempt diagnostic projects the already-retained `LoadingError`
through a borrowed bounded view. All 561 active loading tests pass, with one
intentional ignored test. The authentic proof identifies the existing Stage 6
runtime guard and no additional payload. This diagnostic checkpoint changes no
loading, DICE, toolchain, action, or fixture semantics.
