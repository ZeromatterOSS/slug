# Plan 52: root bazel_dep apparent aliases

## Status: IMPLEMENTED (2026-05-08)

## Context

ZeroMatter's `//sdk:sdk_contents` currently fails during BUILD loading:

```text
unknown cell alias: `bazel_lib`
```

The failing load is:

```starlark
load("@bazel_lib//lib:copy_to_directory.bzl", "copy_to_directory")
```

`MODULE.bazel` declares `bazel_dep(name = "bazel_lib", version = "3.2.2")`,
so Bazel 9 makes `@bazel_lib` visible in the root module. Plan 38 fixed
extension spoke registration, but this is a root-module repository mapping
gap: kuro was only emitting aliases for `bazel_dep(repo_name = "...")`, and
assumed the selected cell identity always matched the apparent dependency
name.

## Fix

1. Preserve `module(repo_name = "...")` instead of accepting and discarding it.
   Bazel documents this as the module's own repository name as seen by the
   module itself.
2. For every root `bazel_dep`, register its Bazel apparent name
   (`repo_name` when present, otherwise `name`) as an alias when kuro's selected
   cell identity differs from that apparent name.
3. Resolve alias targets through the selected module graph, including
   disambiguated `name+version` cell identities used by kuro for selected
   modules.

## Verification

- `cargo test -p kuro_bzlmod parser::tests::test_parse_module_with_compatibility_level --lib`
- `cargo check -p kuro_common`

ZeroMatter verification is run separately after rebuilding `target/debug/kuro`.
