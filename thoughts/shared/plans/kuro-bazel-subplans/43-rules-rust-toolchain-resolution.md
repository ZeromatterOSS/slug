# Plan 43: rules_rust toolchain resolution returns None

## Status: PROPOSED

## Context

After Plan 42 (`expand_template` deferred), zeromatter's
`//sdk:sdk_contents` build advances into rule analysis and fails
inside rules_rust:

```
* bazel-external/rules_rust+0.69.0/rust/private/rust_allocator_libraries.bzl:268, in _rust_allocator_libraries_impl
    return toolchain.make_libstd_and_allocator_ccinfo(
error: Object of type `NoneType` has no attribute `make_libstd_and_allocator_ccinfo`
```

`toolchain` here comes from `find_toolchain(ctx)` in
`rules_rust+0.69.0/rust/private/utils.bzl:62-71`:

```python
def find_toolchain(ctx):
    return ctx.toolchains[Label("//rust:toolchain_type")]
```

i.e. `ctx.toolchains[Label("@rules_rust//rust:toolchain_type")]`. Kuro
returns `None` from this lookup, so the .NoneType-attribute access
blows up.

## Investigation summary

ZeroMatter's MODULE.bazel registers two toolchain repos:

```python
register_toolchains("@llvm_toolchains//:all")
register_toolchains("@default_rust_toolchains//:all")
```

`@default_rust_toolchains` is a rules_rs-flavored repo that
`declare_rustc_toolchains(version = "1.95.0", ...)` — generates a
package full of `toolchain(...)` rules.

`rules_rust+0.69.0/rust:BUILD.bazel` (lines 13-20):

```python
toolchain_type(name = "toolchain_type", ...)
alias(
    name = "toolchain",
    actual = "toolchain_type",
    deprecation = "instead use `@rules_rust//rust:toolchain_type`",
)
```

`bazel-external/rules_rust+rust+rust_toolchains/BUILD.bazel`
(materialized but possibly not the one zeromatter uses) registers
`toolchain(... toolchain_type = "@@rules_rust//rust:toolchain", ...)`
— the **deprecated alias name**, not the canonical
`:toolchain_type`.

Kuro's `app/kuro_analysis/src/analysis/toolchain_resolution.rs:240+`
matches by exact label after `normalize_constraint_label`:

```rust
let tc_type_norm = normalize_constraint_label(&tc_info.toolchain_type);
let req_type_norm = normalize_constraint_label(&req.type_label);
if tc_type_norm != req_type_norm {
    continue;
}
```

So `:toolchain` (registered) and `:toolchain_type` (requested) never
match. Even when alias resolution would produce identical canonical
targets, kuro doesn't follow them.

That's hypothesis (a). Hypothesis (b): rules_rs's
`declare_rustc_toolchains` may not register *any* toolchain matching
the linux-gnu-host target platform zeromatter is building under, or its
generated toolchains may not have the rules_rust-shaped
`ToolchainInfo` provider that `find_toolchain`'s callers expect. Need
to verify before implementing.

## Investigation steps

1. **What `toolchain()`s are registered?** Print/dump the list kuro
   builds from `register_toolchains` after package expansion. Confirm
   whether `:all` from `@default_rust_toolchains` and from
   `rules_rust+rust+rust_toolchains` is being walked, and what
   `toolchain_type` labels each one carries.

2. **Alias following.** Trace
   `app/kuro_analysis/src/analysis/toolchain_resolution.rs::resolve_toolchains`
   for the failing target. Does the `toolchain_type` field arrive
   pre- or post-alias? If it arrives as a literal `:toolchain` while
   the rule asks for `:toolchain_type`, alias-aware matching is the
   fix.

3. **Provider shape.** If a toolchain is found and returned but
   `make_libstd_and_allocator_ccinfo` is still missing, the
   rules_rs-generated `ToolchainInfo` is incomplete relative to
   rules_rust's expectations. That's a different fix (extend rules_rs
   toolchain output, or shim missing fields in kuro).

## Likely fix path

Phase 1 (small, high-value): make toolchain resolution alias-aware.
- During `set_registered_toolchains` or at resolution time, follow
  each registered `toolchain.toolchain_type` through aliases (using
  the cell resolver's alias map / a load of the BUILD file). Store
  the canonical `toolchain_type` on the resolved record.
- In `resolve_toolchains`, normalize the requested type label the
  same way before equality.

Phase 2 (provider parity): if Phase 1 unblocks resolution but the
returned `ToolchainInfo` is still missing rules_rust-specific fields,
two options:
- **Adapt rules_rs**: extend `declare_rustc_toolchains` to emit a
  rules_rust-shaped struct (the cleaner long-term path).
- **Shim in kuro**: detect rules_rs-shaped toolchains on lookup and
  wrap them in a synthesized `ToolchainInfo` with the
  rules_rust-shaped lambdas.

## Out of scope

- Actually compiling Rust (rules_rust uses cc_common.create_link_variables,
  rust_std fetches, etc.) — once the toolchain is found, those
  surface as the next layer of issues.
- Cross-compilation toolchain selection.
- `register_execution_platforms` semantics (separate domain).

## Verification

- Re-run zeromatter `//sdk:sdk_contents`; expect `find_toolchain` to
  return a real ToolchainInfo, then advance to the next layer
  (likely cc_common/link_variables or actual rustc invocation).
- `cargo test -p kuro_analysis --lib` (toolchain_resolution tests).
- `examples/multi_package` still builds.
