# Current Slug V2 Work Packet

Packet: WP-4-6-7A-named-execution-group-runtime-audit-r1

Status: docs/source audit selected after terminal acceptance of imported native
genrule loading. No new Rust implementation is authorized by this audit.

## Immediate predecessor and authentic boundary

The imported-native-genrule implementation is independently ACCEPTED at
170 production/255 proof/425 aggregate gross Rust additions. Full loading,
query-library and analysis-library gates, CLI build, formatting and scope checks
pass. The targeted root-alias replay finishes with exit 2 in 62.74 seconds:
all six authenticated rules_java helper/genrule calls return before the next
top-level statement at toolchains/BUILD:138 calls cc_library.

The next terminal is:

```text
@@rules_java+//toolchains:BUILD:138 -> cc_library
@@rules_cc+//cc:cc_library.bzl:19 -> _cc_library(**kwargs)
target invocation for named execution-group semantics is unsupported
```

No package publishes; no genrule/generated output is selected for analysis and
Java runtime rule analysis does not begin. The former BUILD:365 prediction was
unproven and is superseded by this observed earlier boundary. Complete replay
provenance and validation are recorded in Stage 4's imported-genrule section.

The selected rules_cc archive is 0.2.17, SHA-256
`283fa1cdaaf172337898749cf4b9b1ef5ea269da59540954e51fba0e7b8f277a`.
Its 19-line cc/cc_library.bzl wrapper hashes to
`f78cea09a88fff65a5409d7e2b725cf9fcbdbf6d409a2b4b94216565be79b2dc`
and imports cc_library from the generated compatibility proxy.
The tracked root lock and built-in MODULE regression agree on 0.2.17.
Do not downgrade to 0.2.4 based on contradictory historical prose.

## Observable audit result

Authenticate the proxy's complete selected cc_library declaration, identify
which declared groups and named exec transitions trigger the existing generic
guard, and design the complete generic named-execution-group runtime category
needed by that owner. Inventory automatic groups separately; do not silently
admit or erase them. The result is a bounded cross-stage design or an explicit
REPLAN identifying the prerequisite owner, not a rules_cc special case.

The accepted Stage 6 execution-group declaration design explicitly reserves
removing this pre-publication guard for a reviewed cross-stage packet owning
group-specific transitions, constraint/toolchain resolution, ctx.exec_groups,
exec properties and action routing. Do not simply move the guard later, erase
group names, fall back to the default platform, or wire the unused
toolchains/exec_groups.rs prototype into live analysis.

## Authority and inspection

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` remains the
compatibility authority. Read its applicable source and discriminating tests:

- packages/DeclaredExecGroup.java;
- analysis/ExecGroupCollection.java;
- analysis/starlark/StarlarkExecGroupCollection.java;
- starlarkbuildapi/ExecGroupApi.java and platform/ExecGroupCollectionApi.java;
- analysis/StarlarkExecGroupTest.java and AutoExecGroupsTest.java;
- skyframe/toolchains/ToolchainResolutionFunctionTest.java and
  SingleToolchainResolutionFunctionTest.java;
- starlark/StarlarkRuleClassFunctionsTest.java.

These are paths under src/main/java/com/google/devtools/build/lib or
src/test/java/com/google/devtools/build/lib at that commit. Record which tests
apply, which belong to deferred automatic/aspect breadth, and why. Reuse
accepted source/oracle evidence before adding a new oracle.

Inspect the live Stage 4 declaration/target boundary, Stage 6 configured
transition and toolchain owners, configured action-group identity, and
existing default-context tests. Read docs/developers/dice.md before proposing
key or ownership changes. Buck2/DICE are ownership/utility guidance only, never
Bazel compatibility authority. Preserve the Rust-native architecture.

## Required design and non-decisions

The audit must cover declaration-to-target retention, structural identity and
equality, request/configuration projections, target versus execution platform,
constraint and toolchain ownership, named dependency transitions, provider/ctx
views, action-group selection and failure ordering. Identify natural producers
and retained lifetimes; no command-side semantic reconstruction.

Specify source/configuration/platform A/B/A, overlapping requests, cancellation
and failure publication, and default-group nonregression. Preserve distinct
configuration, output-path, ActionKey and REAPI digest domains. All new owners,
fields, keys and async boundaries require explicit review and memory accounting.

Exact means only source/oracle-established named surfaces. DICE/Host integrity
and structural identities remain Slug-native. Unselected automatic/aspect
behavior, later C++/Java initializer/provider/action semantics and execution
remain unsupported/deferred unless separately selected. This audit itself
activates none of them.

## Scope, review and stops

Allowed edits: this manifest, canonical Live Status and the relevant Stage 4
and Stage 6 owner sections; Stage 5 only if provenance changes. No Rust,
fixtures, vendored sources, harness, dependency, runtime or repository changes.
Keep audit additions under 300 text lines, excluding replacement of this
manifest. Do not add chronological status copies or speculative APIs.

Before implementation, apply the plan-authoring guide and obtain independent
architecture/retained-state review. Freeze exact files, caps, compact owners,
Bazel tests, focused validation, compatibility classes, complexity decisions
and explicit stops. A new DICE/global/retained owner, unsupported transition,
default-platform fallback or incomplete category is a design decision or
REPLAN, not implicit implementation authority.

The stopped checkout-wide replay must not restart. Tests over one minute are
a red flag requiring investigation; fifteen minutes is the absolute maximum.
The completed 62.74-second gate followed short diagnostics confirming global
path-epoch revalidation fanout. Preserve that separate performance concern;
do not hide it with source overrides, longer timeouts, disabled provenance or
a fresh graph. Prefer deterministic focused tests during iteration.
