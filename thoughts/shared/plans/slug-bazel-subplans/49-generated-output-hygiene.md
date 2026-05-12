# Plan 49: Generated Output Hygiene

> Parent: [2026-01-21-slug-bazel-compatible-build-tool.md](../2026-01-21-slug-bazel-compatible-build-tool.md)
>
> Related: [Plan 44](./44-workspace-layout-parity.md), which owns the
> actual workspace/output layout.

## Status: PROPOSED

## Scope

Prevent generated workspace/output artifacts from being accidentally
committed again.

Generated paths covered by this plan:

- `bazel-bin`
- `bazel-out`
- `bazel-testlogs`
- `bazel-*`
- `execroot/`
- `external/`
- `bazel-external/`

## Work

1. Audit `.gitignore`, `.bazelignore`, and any fixture-local ignore files
   so generated layout artifacts are ignored everywhere they can appear.
2. Add a lightweight repository hygiene check that fails if
   `git status --short` contains generated output paths outside an
   explicitly allowlisted fixture.
3. Document the allowlist rule: fixtures may track generated-looking names
   only when the test asserts behavior involving that exact path.
4. Wire the check into the same local/CI path used by other doc or
   repository hygiene checks.

## Verification

- Positive test: a deliberately created generated output path is reported.
- Negative test: allowed fixture paths and normal source files are ignored
  by the hygiene check.
- Manual verification: run a build that creates `bazel-*` / `execroot`
  paths, then run the hygiene check and confirm it reports only generated
  artifacts.
