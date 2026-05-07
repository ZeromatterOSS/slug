# Plan 47: Aspect Completion

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Refines [Plan 06: Aspects](./06-aspects.md). Plan 06 delivered core
> aspect execution and propagation, but several Bazel parity criteria
> remain open.

## Status: PROPOSED

## Scope

Close the remaining aspect parity gaps that are too broad to hide inside
the historical Plan 06 phase files:

1. DICE-keyed aspect result caching.
2. Incremental recomputation when aspect implementation, attributes, or
   dep providers change.
3. `requires` ordering and aspect-on-aspect chains.
4. Aspect toolchain resolution and exec groups.
5. `apply_to_generating_rules` for output-file inputs.
6. rules_cc `graph_structure_aspect` end-to-end verification.

Out of scope: new rule APIs, module-extension behavior, or non-aspect
toolchain resolution. Those remain with their owning plans.

## Bazel Source of Truth

Before implementation closure, cite the relevant Bazel 9 sources/tests:

- `src/main/java/com/google/devtools/build/lib/analysis/AspectResolver.java`
- `src/main/java/com/google/devtools/build/lib/skyframe/AspectFunction.java`
- `src/test/java/com/google/devtools/build/lib/analysis/AspectTest.java`
- rules_cc aspect usages under `@rules_cc//cc/...`

## Verification

- Unit tests for each remaining semantic: caching key stability,
  invalidation, `requires`, toolchain selection, exec groups, and
  `apply_to_generating_rules`.
- Integration tests under `tests/core/aspects/` for aspect chains and
  generated-file inputs.
- Real-world verification: `@rules_cc`'s `graph_structure_aspect`
  executes on a rules_cc target without falling back to kuro-specific
  behavior.
- Manual-only risk must be recorded before marking complete.
