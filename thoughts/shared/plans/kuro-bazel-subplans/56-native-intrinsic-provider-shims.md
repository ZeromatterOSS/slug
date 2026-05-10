# Plan 56: Native/Intrinsic Provider Shims

**Status:** Proposed follow-up.

## Motivation

Some Bazel 9 APIs are not ordinary Starlark rule implementations even when
their call sites are reached through Starlark. Bazel builds these surfaces in
Java or another intrinsic engine layer, then exposes provider-shaped objects to
Starlark. Kuro must model those as explicit NativeShim boundaries, not as
normal dependency analysis of an implementation target and not as one-off
per-label shortcuts.

The immediate example is the C++ toolchain provider exposed through
`ctx.toolchains["@bazel_tools//tools/cpp:toolchain_type"]`. Bazel resolves the
toolchain normally, but the provider object itself is an intrinsic C++ API
surface. Kuro's current C++ workaround fixes the analysis cycle only for that
surface; this plan generalizes the model and naming.

## Naming

- Use `Native` for behavior implemented by Kuro's native engine layer.
- Use `Intrinsic` for Bazel behavior that is inherently built in rather than
  authored as ordinary Starlark.
- Use `NativeShim` for a Kuro object that presents a Bazel-intrinsic provider or
  API to Starlark while preserving Bazel 9 semantics.
- Do not use misleading placeholder terminology for valid Bazel-intrinsic
  provider facades.

## Non-goals

- Do not weaken depset validation, provider validation, or toolchain resolution.
- Do not make C++ provider handling a public alias for Buck/Kuro internals.
- Do not reintroduce Bazel 8 compatibility.
- Do not mask Bazel 9 failures with "never error" placeholders.
- Do not special-case only `CcToolchainInfo`; the mechanism must cover every
  intrinsic provider/API boundary we rely on for SDK builds.

## Required Work

1. Inventory intrinsic Bazel API boundaries currently approximated in Kuro.
   Initial candidates:
   - C++ toolchain provider and `cc_common` helper objects.
   - Python internal provider/helper surfaces used by `rules_python`.
   - Config fragments, feature-configuration objects, and toolchain context
     provider values exposed through `ctx`.
   - Default outputs, runfiles, and file/depset provider facades produced by
     native rule analysis.
   - Repository-generated tool repos where Bazel creates content from native
     module extension or repository-rule machinery.
2. Define a small NativeShim registry keyed by intrinsic API surface, not by
   arbitrary labels. The registry should make the boundary obvious in code,
   logs, and tests.
3. Rename the current C++ NativeShim implementation artifacts to the approved
   terminology:
   - provider type and helper functions;
   - checkpoint/status names;
   - plan notes and future run labels.
4. Preserve normal Bazel toolchain resolution. NativeShim construction starts
   only after the resolved toolchain identity is known and only at the provider
   object boundary Bazel exposes to Starlark.
5. Add Bazel 9 probes before tightening behavior. Each NativeShim surface needs
   focused tests for:
   - provider presence and field/method shape;
   - errors for unsupported operations;
   - depset/hashability behavior of values it returns;
   - interaction with `ctx.toolchains`, action inputs, runfiles, and command
     argument expansion where applicable.
6. Revisit the current `//sdk:sdk_contents` blocker:
   `rules_rust+0.69.0/rust/private/rustc.bzl:1374 deps = depset(deps)` reports
   a mutable depset element. Identify whether that mutable value crossed a
   NativeShim boundary, then fix the systemic freezing/hashability path rather
   than adding a local allowlist.
7. Verify performance and memory after each expansion:
   - focused Rust tests for the touched provider/API surface;
   - `cargo check -p kuro_analysis` or the affected crate;
   - bounded `//sdk:sdk_contents` smoke with `KURO_MEMORY_CHECKPOINTS=1`;
   - compare depset flattening checkpoints before and after the change.

## Bazel Sources of Truth

- C++ intrinsic provider semantics:
  `src/main/java/com/google/devtools/build/lib/rules/cpp/`.
- Toolchain context and provider exposure:
  `src/main/java/com/google/devtools/build/lib/analysis/`.
- Config fragments and Starlark `ctx` exposure:
  `src/main/java/com/google/devtools/build/lib/analysis/starlark/`.
- Repository/module extension materialization:
  `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/` and
  `src/main/java/com/google/devtools/build/lib/rules/repository/`.

Use installed Bazel 9 probes when source behavior is ambiguous, and record the
exact Bazel version with the focused test or progress note.

## Exit Criteria

- `//sdk:sdk_contents` advances without per-label provider hacks.
- Approved Native/Intrinsic/NativeShim terminology is used in new code, logs,
  and plan notes for these provider boundaries.
- Existing C++ provider handling is folded into the shared NativeShim model.
- Each implemented NativeShim has focused Bazel 9 parity tests or a documented
  source citation.
- No TransitiveSet streaming behavior changes as part of this plan.
