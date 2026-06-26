# Stage 6: Analysis, Toolchains, and Actions

## Goal

Analyze configured targets with Bazel-compatible providers, depsets,
configuration transitions, toolchain resolution, and action declarations.

## Scope

- configured target keys and analysis DICE keys;
- user-defined providers and native providers;
- depset semantics and validation;
- `ctx`, `ctx.attr`, `ctx.files`, `ctx.actions`, outputs, runfiles, and
  default providers;
- configuration fragments, build settings, and transitions;
- platform and toolchain resolution;
- aspects and aspect propagation after base analysis is stable.

## V1 Extraction Candidates

- depset/providing tests from `slug_build_api_tests`;
- `rule(implementation=...)` tests;
- selected `cc_common` and provider surfaces;
- action declaration plumbing only after Stage 3 path semantics are clean.

## Acceptance Criteria

- Custom Starlark rule fixtures produce the same providers/actions as Bazel.
- Toolchain and platform fixtures match Bazel for focused public examples.
- Action declarations produce REAPI-ready command/input/output structures.
- No analysis shortcut depends on Buck cells or direct filesystem scans outside
  DICE-tracked inputs.

## Validation

```bash
cargo test -p slug_analysis_v2
slug-v2-oracle run --fixture custom-rule-analysis
slug-v2-oracle run --fixture toolchain-resolution-basic
```
