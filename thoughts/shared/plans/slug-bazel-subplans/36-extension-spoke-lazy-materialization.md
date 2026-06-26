# Plan 36: Module-Extension Label Materialization

> Parent: [2026-01-21-slug-bazel-compatible-build-tool.md](../2026-01-21-slug-bazel-compatible-build-tool.md)
>
> Adjacent owners:
> - [10-module-extension-execution.md](./10-module-extension-execution.md) owns
>   baseline module extension execution.
> - [23-module-extension-realworld.md](./23-module-extension-realworld.md) owns
>   macro-wrapped repository-rule shapes and overlapping `repository_ctx`
>   method coverage.
> - [45-per-args-paramfile-and-cargo-runfiles.md](./45-per-args-paramfile-and-cargo-runfiles.md)
>   consumes this plan when public cargo-build-script validation is blocked by
>   extension tag/materialization behavior.

## Status: PARTIAL

Slug can synchronously materialize and resolve `Label` values reached through
`module_ctx.path`, `module_ctx.read`, and `module_ctx.execute`, including
dynamic extension aliases and precomputed `use_repo_rule` repos. Current
behavior is still hybrid: extension execution may also eagerly materialize
missing generated repos after an extension evaluates. That means the lazy-label
path is useful and tested, but not yet proof that only Label-referenced spokes
materialize in every path.

## Bazel Source Anchors

- `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/repository/starlark/StarlarkBaseExternalContext.java:1563-1643`:
  `path(Object)` and `read(Object, ...)` treat strings as paths relative to the
  extension/repository working directory, while `Label` values route through
  label resolution.
- `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/repository/starlark/StarlarkBaseExternalContext.java:2387-2414`:
  `getPathFromLabel(Label)` asks Skyframe for the package/file path and ensures
  materialization for remote external overlays.

## Current State

- `module_ctx.path(Label)` and `module_ctx.read(Label)` resolve labels through
  Slug's structured label filesystem resolver and can materialize referenced
  extension repos before returning paths.
- `module_ctx.execute([Label(...), ...])` and
  `repository_ctx.execute([Label(...), ...])` use the same resolved-label
  materialization path for executable arguments.
- The shared resolver maps dynamic extension apparent aliases, including
  `use_repo_rule` repos, to canonical `bazel-external/<canonical>` locations.
- `attr.label_keyed_string_dict` tag attrs now expose their keys as Starlark
  `Label` values inside module-extension implementations. This preserves the
  Bazel 9 boundary where raw strings remain working-directory paths while
  actual label-typed values are label-resolved.

## Accepted Evidence

- `cargo test -p slug_interpreter_for_build test_module_context_label_keyed_dict_attr_exposes_label_keys --lib -- --nocapture`
  - Passed: `1 passed`.
- `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py::test_module_ctx_read_label_keyed_dict_tag_attr_keys -s --tb=short`
  - Passed: `1 passed in 0.54s`.
- Public rules_rust 0.67.0 cargo-build-script smoke:
  `timeout 120 /var/mnt/dev/slug/target/debug/slug --isolation-dir plan45-rules-rust-public-smoke build //test/cargo_build_script/run_from_exec_root:rundir_build_rs --show-output`
  - Advances past rules_python `requirements_by_platform` `ctx.read(file)` and
    now stops later at missing registered Rust/C++ toolchains.

## Remaining Gaps

- Audit `repository_ctx.path(Label)` and `repository_ctx.read(Label)` for the
  same materialization guarantee across all current method paths.
- Backfill `repository_rule_attr` accessors surfaced by public extension use
  (`auth_patterns`, `_rules_python_workspace`, `vcs`, plus any additional
  accessors discovered by the audit).
- Replace downstream `No such file` errors from stubbed sub-extension repos
  with a direct extension-failure error.
- Separate lazy-label materialization evidence from any remaining eager
  generated-repo materialization path.

## Next Owner

1. Continue with the `repository_ctx` Label-path audit and add focused tests for
   any method still bypassing the shared resolver/materializer.
2. Route the public rules_rust cargo-build-script blocker that remains after
   this slice to the bzlmod/toolchain owner, not this plan.

## Out Of Scope

- Bazel versions before 9.0.
- REAPI executor-boundary proof, owned by Plan 34.
- Cargo-runfiles paramfile runner proof, owned by Plan 45.

## Sanitization Note

Older line-by-line external-workspace logs were intentionally removed from this
owner file. Keep future updates compact and use public or synthetic identifiers
only.
