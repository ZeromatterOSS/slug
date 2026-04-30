# Plan 28.4 Stage 3 Facade Blocker

> **Plan**: [Plan 28: Bazel Builtins Module Architecture](../plans/kuro-bazel-subplans/28-builtins-module-architecture.md)
>
> **Status**: research notes 2026-04-30. Stage 2 (no-op wrapper) is in
> main; Stage 3 (facade with overridden ctx methods) is blocked on a
> Starlark `struct(**dict)` field-loss issue. Pick up in a follow-up.

## Goal

Phase 28.4 Stage 3 wants the bundled `rule_implementation_wrapper` to
install a Starlark facade around `raw_ctx` so individual `ctx`-method
bodies (e.g. `target_platform_has_constraint`, `runfiles`,
`expand_make_variables`, `var`) can move from Rust to Starlark.

The wrapper is already wired (Stage 2, commit `b0ae3834`):
analysis calls `eval.eval_function(wrapper, &[rule_impl, ctx_val], &[])`
when `@kuro_builtins//:exports.bzl` exposes `rule_implementation_wrapper`.
Stage 3 wants to change the wrapper *body* from `return implementation(raw_ctx)`
to something like:

```starlark
def _invoke_rule(implementation, raw_ctx):
    facade = _make_facade(raw_ctx)        # struct mirroring raw_ctx + overrides
    return implementation(facade)
```

## What was tried

`kuro_builtins/exports.bzl` was extended (and reverted) with:

```starlark
def _invoke_rule(implementation, raw_ctx):
    overrides = {
        "target_platform_has_constraint": _target_platform_has_constraint,
    }
    fields = {}
    for name in dir(raw_ctx):
        if name.startswith("_"):
            continue
        if name in overrides:
            fields[name] = overrides[name]
        else:
            fields[name] = getattr(raw_ctx, name)
    fields["kuro_facade_active"] = True   # observable marker for the test
    return implementation(struct(**fields))
```

A test rule asserted that the resulting `ctx` has a
`kuro_facade_active` attribute. The wrapper IS in the call path
(stack trace shows `_invoke_rule` calling `struct(**fields)`), but the
marker is missing from the produced struct:

```
error: Object of type `struct` has no attribute `kuro_facade_active`
  --> wrapper_proof/has_constraint.bzl:21:14
   |
21 |     marker = ctx.kuro_facade_active
```

Variants tried, all reproducing the same `no attribute` error:

1. `fields = dict(kuro_facade_active = True)` then loop fills
   raw_ctx mirrors. (Marker may be missing because `dict(k=v)` kwargs
   was the issue.)
2. `fields = {}` then loop, then `fields["kuro_facade_active"] = True`
   AFTER the loop. (Marker should be set last, but still drops.)
3. Direct `ctx.kuro_facade_active` access (errors), not via `getattr`
   default. Same `no attribute` outcome confirms the field truly is
   missing from the struct, not just hidden from `getattr`.

## Hypotheses

The behavior is consistent with `struct(**dict)` quietly dropping
some entries in kuro's starlark-rust fork. Candidate root causes,
ranked by likelihood:

1. **`**dict` into `struct()` strips entries whose values are bound
   methods.** When `getattr(raw_ctx, "actions")` returns a bound
   method on `AnalysisContext`, packing it into a dict and then
   spreading via `**dict` to `struct()` may filter through a
   `names_map()` path that rejects non-frozen-able values. The marker
   `True` is not a method, but if the spread fails *partway* through
   building the struct, the marker entry could be lost too.
2. **`**` only forwards Starlark-native kwargs, not arbitrary dict
   keys.** Some Starlark forks distinguish between actual call
   keyword arguments and dict-spread arguments. If the spread path
   only forwards the dict's "value-typed-as-kwarg" entries, kuro
   ctx-method values may be filtered out in the spread itself.
3. **`struct()` only allows valid-identifier kwargs and silently
   drops the rest.** The `getattr(raw_ctx, ...)` results may include
   entries whose names look like methods rather than fields, and
   `struct(**...)` discards them. (`kuro_facade_active` is a valid
   identifier, so this alone doesn't explain its loss — unless the
   spread bails out earlier on a non-identifier name.)
4. **Frozen-heap lifetime issue.** `getattr(raw_ctx, name)` returns
   values rooted in raw_ctx's heap; passing through `dict` →
   `struct(**...)` may re-freeze them and drop entries that don't
   round-trip cleanly.

`starlark-rust/starlark/src/values/types/structs/structs.rs` (the
`struct()` builtin) is short — `Struct::new(args.names_map()?)` —
so `**dict` behavior lives in `Arguments::names_map`. That's the
right next read.

## Alternatives if the `**` issue can't be fixed cheaply

- **Enumerate ctx fields statically.** Drop `dir()`/`getattr()` and
  build the struct with explicit kwargs:
  ```starlark
  return struct(
      attr = raw_ctx.attr,
      attrs = raw_ctx.attrs,
      actions = raw_ctx.actions,
      label = raw_ctx.label,
      # ... ~30 more lines ...
      target_platform_has_constraint = _override_fn,
      kuro_facade_active = True,
  )
  ```
  More code, but every kwarg is visible at parse time and
  `args.names_map()` doesn't have to do dict-spread bookkeeping.
  Maintenance cost: every new `ctx` field needs a corresponding line
  here — but there's a guard test that breaks when `dir(raw_ctx)`
  diverges from the static list.
- **Bypass `struct()` entirely, write a Rust facade.** Add a
  `KuroFacadeCtx` Rust value that wraps `raw_ctx` and overrides
  specific methods. Defeats the "compatibility logic in Starlark"
  goal of Phase 28 but is the least surprising option.
- **Add a Starlark `mutate_attr(struct, name, value)` helper in
  Rust.** Would let the wrapper return a mutated raw_ctx without
  going through struct construction. Probably violates Starlark's
  immutability invariants, so unlikely to be acceptable.

## Performance note (separate concern)

For Stage 2 (no-op wrapper, currently shipped):
`@llvm-project//llvm:Support` cold build → analyze=190 ms across 183
actions. Wrapper overhead is one extra Starlark function call per rule
analysis (~5–50 µs each), so total wrapper cost is <10 ms — under 5%
of analyze time, under 0.1% of total wall time (17.7 s).

For a hypothetical Stage 3 facade with `dir`/`getattr` per rule:
~30 ctx fields × ~3 µs per `getattr` ≈ 100 µs per rule, ≈ 20 ms over
183 rules. Still under 1% of typical analyze time. **Not a perf
blocker; the issue is correctness, not cost.**

## Recommendation

Pick up Stage 3 fresh. Order of investigation:

1. Read `starlark::eval::Arguments::names_map` in starlark-rust to
   understand exactly what `**dict` does at struct() call time.
2. Reproduce the field loss in a starlark-rust unit test (no kuro
   needed) — `struct(**{"x": True})` vs `struct(x = True)`.
3. If the issue is real, file it upstream and use the static-kwarg
   alternative (enumerate ctx fields explicitly) for Stage 3
   landing. Add a unit test that compares `set(dir(raw_ctx))`
   against the static list to catch drift.
4. Once the facade lands, migrate
   `ctx.target_platform_has_constraint` (the simplest body — just
   string match against host-OS labels), delete the Rust impl, and
   verify `@llvm-project//llvm:Demangle` still builds.

## What did not change in main

- Stage 2 wrapper (no-op) is in place from `b0ae3834`. Every Starlark
  rule impl is called as `wrapper(impl, ctx)` instead of `impl(ctx)`,
  but the wrapper body is the identity, so behavior is byte-for-byte
  equivalent to pre-wrapper. The infrastructure is ready when the
  Stage 3 facade approach is unblocked.
- `kuro_builtins/exports.bzl` keeps the `rule_implementation_wrapper`
  as `_invoke_rule` (identity).
- No tests were left in a failing state by the Stage 3 attempt;
  reverted cleanly.
