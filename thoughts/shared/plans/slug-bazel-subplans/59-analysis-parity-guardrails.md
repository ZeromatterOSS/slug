# Plan 59: Analysis Parity Guardrails

> Parent: [2026-01-21-slug-bazel-compatible-build-tool.md](../2026-01-21-slug-bazel-compatible-build-tool.md)
>
> Status: In progress.

## Goal

Stop repeated SDK analysis breakages from surfacing only through long
`//sdk:sdk_contents` smokes. The immediate goal is not to replace Plan 58's
rule-based C++ toolchain work; it is to add cross-cutting guardrails for the
classes of analysis bugs that have repeatedly blocked Plan 58 and adjacent
SDK parity work.

## Meta-Analysis

The recent failures are not random one-off issues. Most cluster around four
systemic boundaries:

1. **Bzlmod canonical/apparent repo identity is fragmented.** Label context,
   extension-spoke registration, dependency-owned `use_repo`, and toolchain
   matching have each implemented pieces of Bazel 9 canonical naming. The same
   apparent label can be normalized correctly in one subsystem and incorrectly
   in another.
2. **Toolchain resolution and `ctx.toolchains` remain too approximate at the
   provider boundary.** Slug now resolves real toolchains, but lookup keys,
   provider ownership, optional toolchains, and native C++ shims still have
   local assumptions that only large rules_rust/rules_cc graphs expose.
3. **Rule-based C++ toolchain behavior is patched from symptoms.** Plan 58 is
   moving Slug from native approximations to declared toolchain feature data,
   but feature defaults, compile/link variable inputs, and provider shapes need
   contract tests so Slug does not invent behavior Bazel 9 did not enable.
4. **Frozen Starlark values can cross analysis boundaries without their owner.**
   Any retained `FrozenValue` used after the original target analysis heap is
   gone must carry the owning frozen heap. Otherwise a Starlark provider closure
   can turn a semantic analysis bug into a daemon disappearance.

Side issues such as filesystem cleanup are still real bugs, but they are not
the dominant pattern.

## Integration With Existing Plans

| Area | Existing owner | Plan 59 responsibility |
| ---- | -------------- | ---------------------- |
| Bazel 9 parity decisions, removed globals, label grammar | [Plan 15](./15-bazel-9-parity.md) | Add regression contract tests when a parity class spans multiple subsystems. |
| Module extension execution and spoke identity | [Plan 10](./10-module-extension-execution.md), [Plan 36](./36-extension-spoke-lazy-materialization.md), [Plan 57](./57-module-extension-facts-reuse.md) | Ensure canonical repo facts have a single source of truth and are reused in label/toolchain contexts. |
| Toolchain resolution algorithm | [Plan 11](./11-toolchain-resolution.md) | Add cross-boundary tests that registered toolchain type labels and `ctx.toolchains[...]` lookup labels normalize the same way. |
| Native/intrinsic provider boundaries | [Plan 56](./56-native-intrinsic-provider-shims.md) | Require owner-retaining wrappers for frozen provider values and focused tests for captured provider closures. |
| Rule-based C++ feature parity | [Plan 58](./58-rule-based-cc-toolchain-feature-parity.md) | Add guardrail tests around feature absence, toolchain-owned feature graphs, and compile/link action variable inputs before continuing new symptom fixes. |
| SDK slow tail and post-build waits | [Plan 51](./51-slugd-memory-profiling.md) | Keep performance/stall classification separate from semantic parity failures, and make smokes emit enough state to tell the difference. |

Plan 59 should be pursued before returning to Plan 58 when a Plan 58 blocker
reveals one of these cross-cutting failure classes.

## Non-Goals

- Do not add SDK-specific labels, repository-name workarounds, or target-name
  special cases.
- Do not weaken depset mutable-value validation.
- Do not use Plan 59 to defer concrete Plan 58 C++ feature work after the
  guardrail for that failure class exists.
- Do not make successful `//sdk:sdk_contents` execution a substitute for Bazel
  9 action/output parity checks.

## Phase 1: Frozen Provider Ownership Guardrail

**Class boundary:** retained Starlark values crossing target-analysis
boundaries.

**Owning subsystem:** Native/Intrinsic provider shims and `ctx.toolchains`.

**Current evidence:** rules_rust's Rust `ToolchainInfo` captures a Starlark
closure that later calls into C++ toolchain data from another rule's analysis.
Slug's C++ toolchain target-platform overlay stored only a raw `FrozenValue`
from a provider collection. After the owning provider collection fell out of
scope, the daemon disappeared during analysis and the client reported an h2
broken pipe.

**Work:**

- Replace raw retained `FrozenValue` fields with owner-retaining wrappers
  (`FrozenProviderCollectionValue`, `OwnedFrozenValue`, or a dedicated typed
  owner wrapper) when the value can be used outside the current eval call.
- Add focused coverage for C++ `ctx.toolchains` overlay values remaining
  usable after the original provider collection local binding is gone.
- Audit other direct `FrozenValue` fields in analysis/provider shims and record
  whether they are safe frozen definitions, same-heap temporaries, or
  cross-analysis retained values.

**Exit criteria:**

- The `//lib/file_utils:file_utils --target-platforms=//bazel/platforms:linux-musl`
  repro no longer terminates the daemon during rules_rust analysis.
- Focused unit coverage exercises the owner-retaining overlay.
- Any remaining risky raw `FrozenValue` fields are either fixed or listed in
  this plan with an owning follow-up.

## Phase 2: Canonical Repo/Label Contract Tests

**Class boundary:** Bazel 9 canonical names across Bzlmod, label parsing, and
toolchain matching.

**Owning subsystems:** Bzlmod repo facts, label context, and toolchain
resolution.

**Work:**

- Add table-driven tests for:
  - root-owned extension repo names: `_main+<extension>+<repo>`;
  - dependency-owned extension repo names: `<owner>++<extension>+<repo>`;
  - module repo names with version suffixes;
  - apparent module labels that refer to extension-generated toolchain types;
  - label context inside `bazel-external/<canonical-with-++>/...`.
- Prefer one shared helper for canonical/apparent matching instead of
  duplicated string heuristics in Bzlmod, interpreter label context, analysis
  resolution, and `ctx.toolchains`.

**Exit criteria:**

- The `rules_rs++rules_rust+rules_rust` and
  `rules_rs++toolchains+default_rust_toolchains` cases are covered at each
  boundary that previously diverged.
- Toolchain-type normalization is explicitly limited to toolchain lookup, not
  accidentally generalized to constraint labels unless Bazel source/probes
  justify it.

## Phase 3: Rule-Based C++ Feature Contract Tests

**Class boundary:** C++ compile/link behavior driven by declared toolchain
feature data.

**Owning subsystem:** Plan 58 / `cc_common` feature configuration and action
construction.

**Work:**

- Add tests that absence of a feature remains absence, starting with
  `supports_dynamic_linker`.
- Add focused aquery-backed fixtures for:
  - compiler-rt crt begin/end actions;
  - glibc shared-library whole-archive inputs;
  - rules_rust process-wrapper compile flags under LLVM libc++;
  - musl generated include search-directory inputs.
- Assert action-config flag expansion for both link and compile paths.
- Assert artifact-valued compile/link variables are registered as action
  inputs, not just rendered into argv.

**Exit criteria:**

- Plan 58 C++ fixes can be validated by small targets before a full SDK smoke.
- No default C++ feature is added without a Bazel 9 source/probe citation.

## Phase 4: SDK Smoke Classification Harness

**Class boundary:** distinguishing semantic failure, daemon crash, slow
analysis tail, and post-build wait.

**Owning subsystem:** Plan 51 plus the SDK parity loop prompt.

**Work:**

- Keep the mandatory pre/post `slugd[...]` cleanup.
- Standardize smoke metadata: command, isolation dir, log path, exit status,
  build ID, event summary, `what-failed`, `what-up`, and final daemon state.
- When the client reports h2/broken-pipe, automatically classify whether the
  daemon exited during analysis, execution, materialization, or post-build log
  persistence by reading the event log tail.
- Preserve focused narrow repros for every full SDK blocker that takes more
  than a few minutes to surface.

**Exit criteria:**

- A future daemon disappearance is reported as an analysis/execution/post-build
  class, not as an opaque h2 failure.
- Long smokes are not needed to revalidate already classified local fixes.

## Phase 5: Bazel 9 Output/Action Parity Gate

**Class boundary:** successful Slug execution that still produces Bazel-divergent
actions or output content.

**Owning subsystems:** Plan 15 and the specific feature plan for each
divergence.

**Work:**

- For each SDK blocker class, keep the smallest Bazel 9 `aquery`/`cquery`
  comparison that proves the intended behavior.
- After `//sdk:sdk_contents` builds under Slug, compare the SDK manifest,
  file modes, file list, and representative binary/link contents against
  Bazel 9 before declaring parity.

**Exit criteria:**

- Plan 58 can return to SDK output parity work with concrete action/content
  comparisons instead of only "build succeeded" evidence.

## Current Slice: 2026-05-14

Pursuing Phase 1 before resuming Plan 58:

- `//lib/file_utils:file_utils --target-platforms=//bazel/platforms:linux-musl`
  previously killed the daemon during rules_rust analysis after Slug selected
  the generated Rust 2024 toolchain.
- The focused fix is to make the C++ toolchain target-platform overlay retain
  its owning provider collection, and to avoid returning a C++ shim value from
  a dropped frozen heap in the fallback path.
- The narrow repro now succeeds and leaves final `slugd` state `<none>`.
- The full `//sdk:sdk_contents` run was intentionally terminated after this
  plan pivot. It had reached execution with Rust actions; no h2 broken pipe
  recurred before termination.

Initial direct-`FrozenValue` audit:

- Fixed in this slice: `CcToolchainInfoTargetPlatformOverlay` no longer stores
  a bare `FrozenValue`; it stores the owner-retaining
  `FrozenProviderCollectionValue` and reborrows the inner `CcToolchainInfo`
  value only while serving Starlark attrs/methods.
- Also fixed in this slice: the fallback C++ toolchain shim no longer returns
  a value borrowed from a temporary frozen provider collection. It allocates
  the native shim directly on the caller heap.
- Remaining audit candidates are not all bugs. Frozen Starlark callables and
  provider payloads often live inside a module/provider collection that already
  owns the heap. Before any candidate is captured into another analysis result
  or returned from `ctx.toolchains`, verify that the owner is retained:
  `artifact_groups.rs::ArtifactGroup.depset`,
  `interpreter_for_build::{rule,aspect,module_extension,repository_rule,macro_callable}`
  implementation fields, provider callable fields, and builtin-provider
  `FrozenValueOfUnchecked` payloads.
