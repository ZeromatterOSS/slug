# Plan 43: dict.pop kwarg + toolchain impl-error surfacing

## Status: COMPLETE (2026-05-05)

## Original symptom

After Plan 42 (`expand_template` deferred), zeromatter's
`//example_engine:example_engine` (and `//sdk:sdk_contents`) build advances into
rule analysis and fails inside rules_rust:

```
* bazel-external/rules_rust+0.69.0/rust/private/rust_allocator_libraries.bzl:268, in _rust_allocator_libraries_impl
    return toolchain.make_libstd_and_allocator_ccinfo(
error: Object of type `NoneType` has no attribute `make_libstd_and_allocator_ccinfo`
```

## Misdiagnosis

The first hypothesis — that slug's toolchain resolver wasn't following
the `//rust:toolchain` → `//rust:toolchain_type` alias used by
`rules_rust+rust+rust_toolchains` — turned out to be wrong. Resolution
already finds a matching toolchain (rules_rs's
`@default_rust_toolchains` registers with the canonical
`@rules_rust//rust:toolchain_type`, not the deprecated alias). With
`BUCK_LOG=slug_analysis=debug` the daemon log clearly shows
`Toolchain resolution for ... 2/2 type(s) resolved`.

The `NoneType` error came from a **silent failure two layers down.**

## Actual root cause chain

1. Resolution selects
   `default_rust_toolchains//:linux_x86_64_1_95_0_rust_toolchain` for
   `@@rules_rust//rust:toolchain_type`. ✓
2. Slug tries to *analyze* that impl target. Its dep graph leads to
   `llvm//tools:llvm-profdata` →
   `llvm//toolchain:bootstrapped_toolchain` → loading
   `bazel-external/rules_cc+0.2.17/cc/toolchains/toolchain.bzl:164`:
   ```python
   cc_toolchain_visibility = kwargs.pop("visibility", default = None)
   ```
3. Slug's starlark `dict.pop` declares `default` as
   `#[starlark(require = pos)]`
   (`starlark/src/values/types/dict/methods.rs:172`). Bazel's starlark
   accepts it as a keyword. The .bzl load errors with
   `Found 'default' extra named parameter(s) for call to function`.
4. `app/slug_analysis/src/analysis/env.rs:1351-1359` catches the
   analysis error and stores `None` in `resolved_toolchains` (the
   `provider_value` arm). ✗ **swallowed.**
5. `ResolvedToolchains.at()`
   (`app/slug_build_api/src/interpreter/rule_defs/context.rs:1898`)
   returns `Value::new_none()` for `Some(None)` entries.
6. Rule code does `toolchain.make_libstd_and_allocator_ccinfo(...)` —
   NoneType error. The user never sees the real cause.

## Fixes applied

### Fix A: `dict.pop` accepts `default` as keyword

`starlark-rust/starlark/src/values/types/dict/methods.rs`: drop
`require = pos` from the `default` param of `dict.pop`. Bazel's starlark
accepts both positional and keyword forms; rules_cc relies on the
keyword form.

### Fix B: surface mandatory toolchain impl-analysis errors

`app/slug_analysis/src/analysis/env.rs` (block previously at
1296-1387): for *mandatory* toolchain types, propagate impl-analysis
errors with `with_buck_error_context` instead of swallowing them into
`None`. Optional toolchains still degrade to `None` (matches Bazel's
optional-toolchain semantics).

The test of "is this toolchain mandatory" reads from
`analysis_env.rule_spec.toolchain_types()` (Vec<(label, mandatory)>);
mandatory == true entries surface their failures.

## Verification

`cd ../zeromatter && /var/mnt/dev/slug/slug build //example_engine:example_engine`
now fails with a real, actionable error chain:

```
4: Failed to analyze mandatory toolchain impl
   'llvm_toolchains//:linux_x86_64_cc_toolchain' for toolchain type
   '@@bazel_tools//tools/cpp:toolchain_type'
...
13: package `llvm_toolchains//cc/toolchains/impl:` does not exist
    dir `llvm_toolchains//cc` does not exist. Did you mean one of [`rules_cc//cc`]?
```

That deeper failure (llvm cc toolchain config loading) is its own
issue, not part of plan 43's scope.

## Lessons / follow-ups

- Silent fallback to `None` masked a starlark-builtin gap and a
  toolchain-impl loader bug for two layers. Surfacing the mandatory
  case fixes the symptom of plan 43 *and* every future
  toolchain-impl-load failure for mandatory types.
- `kwargs.pop("foo", default = X)` may exist in other rules as well —
  generic Bazel-compat starlark fix, no targeted fixture needed.

## Out of scope (for this plan)

- Toolchain alias resolution. Still a real edge case
  (`rules_rust+rust+rust_toolchains` registers via the deprecated
  alias). Not exercised by the zeromatter build because rules_rs's
  `@default_rust_toolchains` registers via the canonical type and gets
  selected first. File a follow-up if/when a build hits the alias path.
- llvm cc toolchain config loading
  (`llvm_toolchains//cc/toolchains/impl:darwin_aarch64`) — the new
  surfaced error.
