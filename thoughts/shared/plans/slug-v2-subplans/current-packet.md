# Current Slug V2 Work Packet

Packet: WP-6-7A-selected-toolchain-request-implementation-r1

Status: independent architecture review ACCEPT; prerequisite implementation
selected within the frozen scope below. No Rust changed in the design milestone.

## Immediate predecessor and decision

The shared rule execution-group design returns REPLAN to this prerequisite.
Pinned Bazel 9.2 gives selected toolchain implementations the parent's unchanged
configuration plus a separate execution-platform preference in their key.
Slug currently substitutes an Exec configuration; one selected-toolchain test
asserts the resulting target-option loss. Correct this semantic defect before
activating named/automatic groups. Do not rerun the completed general audit.

Stage 6, "Selected-toolchain request correction before group activation
(2026-09-10)", is the frozen representation, propagation, evidence and proof
contract. It also retains resolved automatic-policy/action/alias decisions for
the subsequent complete group-runtime design. The authenticated rules_cc guard
and independent computed-default boundary stay unchanged; no package/Java
runtime success is claimed. Imported native genrule loading remains accepted
at de62232ad; the prior named-only audit is committed at 87e705ef7.

## Observable result and authority

Two platforms are registered A then B. A parent's selected toolchain requires
B; its implementation declares no toolchains and writes an output. The
implementation retains the parent's target configuration/settings while its
default action context selects B. Its ordinary Exec attributes still perform
the existing Exec transition. First observe the new regression fail, then fix
the complete direct-toolchain request category, not only that fixture.

Bazel authority is local git object 8220c6198837d5c13d53fea211cf3282aa12408a
in /home/wgray/bazel, not its working HEAD. Reuse the Stage 6 anchors in
DependencyProducer, PrerequisitesProducer, ConfiguredTargetKey,
TargetAndConfigurationProducer, DependencyResolver, ToolchainContextUtil,
UnloadedToolchainContextsProducer, ToolchainResolutionFunction and PlatformKeys.
Adapt the named forceExecutionPlatform tests and keepParentToolchainContext;
the latter is source/synthetic-key evidence, not a full end-to-end replay.
The C++ coverage-action variant is deferred with that unsupported action family.

Exact: admitted direct implementation configuration, preference priority and
edge-specific propagation. Slug-native: tracked DICE state, structural owner
identity, key framing and existing paths. Deferred: named/automatic activation,
test groups, rule inheritance, configured aspects/subrule contexts, manual
feature-flag trimming, new platform policies, exact Bazel configuration/path
bytes, broader execution and action families. No compatibility fallback added;
delete the old selected-implementation Exec substitution/assertions outright.

## Ownership, identity and bounded scope

Use existing configured-node and resolution DICE owners, an optional shared
canonical platform label in full/retained keys, one analysis-owned projection,
and the existing FileWrite owner encoder. Keep configuration payload bytes
separate. Default constructors clear the preference; same-node incoming rule
transitions preserve it, ordinary attributes including alias.actual do not.
Known-platform preference lookup cannot discover arbitrary labels. The optional
known-preference topology field preserves provenance without inventing a
registration candidate. Stage 6 freezes its zero/all-optional target-platform
case, alias lookup and suitable-first/fallback behavior.

Seven production files, exactly:

- app/slug_analysis_v2/src/key.rs
- app/slug_build_api_v2/src/analysis_value.rs
- app/slug_analysis_v2/src/dice.rs
- app/slug_analysis_v2/src/analysis_value.rs
- app/slug_analysis_v2/src/starlark_rule.rs
- app/slug_analysis_v2/src/result.rs
- app/slug_core_v2/src/runtime/file_write_identity.rs

Additional proof files, exactly:

- app/slug_build_api_v2/tests/analysis_value.rs
- app/slug_analysis_v2/tests/configured_target.rs
- app/slug_analysis_v2/tests/starlark_rule.rs
- app/slug_core_v2/src/runtime/dice.rs (proof only)
- app/slug_reapi_v2/tests/reapi.rs (proof only)

Inline tests in the seven production files count as proof. Gross caps, counting
moved lines: 900 production / 1,800 proof / 2,700 aggregate Rust additions.
Docs: this manifest, canonical Live Status and relevant Stage 4/6 owner sections.
No loading, configuration, query, REAPI transport or runtime-orchestration
production edits, fixtures, dependencies, vendored sources or harness changes.
Stage 6 records the concrete cohesion decision for large producer/evaluator
files, retained/scratch/view/async lifetimes, nullable Arc/Allocative reuse,
source certificates, cancellation and eviction. No locks across DICE awaits,
side cache, command reconstruction, fresh-graph workaround or new interner.

## Discriminating proof and validation

Use existing Rust source-derived workspace scaffolds; no new oracle fixture or
network replay. New regressions use the prefix selected_toolchain_request_.
Cover the Stage 6 matrix: full-key Eq/Hash/Ord, None/A/B discrimination and
unchanged config bytes; options, aliases, incoming transitions, ordinary and
nested edges; suitable preference before optional coverage, unavailable or
unsuitable fallback, no match and empty/all-optional known target preference;
same-DICE source/configuration/platform/property A/B/A and unchanged cutoff;
overlapping preference requests, cancellation/Need/error/recovery and atomic
publication; provider/derived-file owner identity, closure-resolved FileWrite
bytes and direct configured REAPI handoff. The REAPI digest still represents
the actual command/action payload, never the structural owner key itself.
Do not reconstruct full keys from only label/configuration in proof helpers.
Exercise two preference variants in one closure: retain distinct owners and
preserve existing output-conflict rejection; a required path redesign is REPLAN.

Run these commands serially from the checkout with PATH containing the pinned
nightly-2025-09-14-x86_64-unknown-linux-gnu toolchain. Every command starts with
a 60-second limit; investigate timeout before any targeted retry. Fifteen
minutes is the absolute per-test maximum, not a default timeout. A filter must
run its intended nonzero tests; inspect failures, not only the exit code.

```sh
timeout 60s cargo test -q -p slug_analysis_v2 --test starlark_rule selected_toolchain_request_
timeout 60s cargo test -q -p slug_analysis_v2 --test starlark_rule selected_toolchain_
timeout 60s cargo test -q -p slug_analysis_v2 --test starlark_rule root_toolchain_
timeout 60s cargo test -q -p slug_analysis_v2 --test starlark_rule default_exec_
timeout 60s cargo test -q -p slug_analysis_v2 --test starlark_rule rule_transition_
timeout 60s cargo test -q -p slug_analysis_v2 --test configured_target
timeout 60s cargo test -q -p slug_build_api_v2 --test analysis_value
timeout 60s cargo test -q -p slug_core_v2 --lib selected_toolchain_request_
timeout 60s cargo test -q -p slug_reapi_v2 --test reapi
timeout 60s cargo check -q -p slug_query_v2 -p slug_core_v2 -p slug_reapi_v2 -p slug_cli_v2
timeout 60s cargo fmt --all -- --check
git diff --check
```

At milestone, run affected owner suites once, serially and under the same
limits; rebuild CLI before any later binary validation. No checkout-wide
query, downloads, credential access or authentic replay is needed here.
Require independent identity/architecture and terminal review. A missing
retained consumer, new semantic/async owner, cap overflow, path redesign or
second material correction is REPLAN, not permission to broaden the allowlist.
After acceptance resume the shared named/automatic group-runtime contract;
never move the invocation guard or substitute a default group to make progress.
