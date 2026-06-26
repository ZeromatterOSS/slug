# Plan 46: Directory paths in `filegroup.srcs` (and similar `one_of(dep, source)` attrs)

> Parent: [2026-01-21-slug-bazel-compatible-build-tool.md](../2026-01-21-slug-bazel-compatible-build-tool.md)
>
> Discovered while implementing Plan 44 Phase 2.5 (per-action execroot
> for rules_rust runner compatibility). End-to-end verification of
> `crates__zerocopy-0.8.42//:_bs` was blocked because zeromatter's
> `llvm_toolchains//:linux_x86_64_cc_toolchain` analysis fails with
> `Unknown target lib/clang/22` from
> `llvm-toolchain-minimal-22.1.0-linux-amd64//`.

## Status: IMPLEMENTED LOCALLY; EXTERNAL LLVM SMOKE PENDING

## Context

Bazel's `filegroup.srcs` accepts directory path strings — e.g.
`srcs = ["lib/clang/22"]` includes every file under that directory. The
LLVM rules use this:

```python
# bazel-external/llvm+0.7.0/directory.bzl::headers_directory
native.filegroup(
    name = name + "_source_directory",
    srcs = [path],          # path = "lib/clang/22"
)
```

Slug's `filegroup` rule rejects this with
`Unknown target lib/clang/22 from package
llvm-toolchain-minimal-22.1.0-linux-amd64//`. The dep-coercion path
synthesizes a target label `:lib/clang/22` that doesn't exist.

This blocks the entire LLVM toolchain analysis chain and, by
extension, every cargo_build_script in zeromatter whose toolchain
selection traverses `llvm_toolchains//:linux_x86_64_cc_toolchain`.

## Bazel Source Anchors

- Bazel creates assumed input files for same-package labels referenced from
  label-typed attributes:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/packages/Package.java:876-959`.
- Bazel `attr.string_keyed_label_dict` accepts `allow_files` as
  bool/list/None and does not have `allow_single_file`:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/starlarkbuildapi/StarlarkAttrModuleApi.java:770-886`.
- Bazel `attr.label_keyed_string_dict` accepts the same `allow_files` shape:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/starlarkbuildapi/StarlarkAttrModuleApi.java:888-1011`.
- Bazel label defaults use package-context conversion; bare `foo/bar` strings
  are parsed as package-relative labels:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/packages/Attribute.java:693-710`,
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/packages/BuildType.java:413-429`,
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/cmdline/LabelParser.java:150-153`.

## Current State

- Earlier Plan 46 work landed the native `filegroup` fix, source/dep
  fallthrough for existing package files/directories, and
  `attr.label_list(allow_files = True)` directory-source support.
- 2026-06-26: the generic Bazel label-attr construction is now shared by
  `attr.label`, `attr.label_list`, `attr.label_keyed_string_dict`, and
  `attr.string_keyed_label_dict`. Dictionary label positions with
  `allow_files = True` can now coerce existing package directories into
  `CoercedPath::Directory` with the contained files recorded.
- `attr.string_keyed_label_dict` no longer exposes Slug's stale
  `allow_single_file` parameter. Both dictionary attrs accept the Bazel 9
  metadata parameters they currently ignore (`allow_rules`,
  `for_dependency_resolution`, `flags`, `configurable`, plus
  `skip_validations` for `label_keyed_string_dict`).
- The Python fixture proves Starlark user rules can accept directory paths in
  both dictionary label positions. At `ctx.attr` provider level Slug currently
  exposes the directory artifact itself (`include`) rather than expanded
  basenames; the owner-abstraction Rust tests prove coercion records the
  directory contents.

## Accepted Evidence

- `cargo test -p slug_interpreter_for_build_tests allow_files_accepts_directory -- --nocapture`
  passed with three directory-source coercion tests.
- `cargo test -p slug_interpreter_for_build_tests 'attr::' -- --nocapture`
  passed with 18 attr module tests, including the Bazel 9 signature guard and
  package-context bare-label default check.
- `cargo build -p slug` passed after the implementation change.
- `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q tests/core/analysis/test_attr_types.py -s --tb=short`
  passed with 10 Python integration tests.

## Remaining Gaps

- The external LLVM smoke has not been rerun from this checkout. The
  next owner should verify Slug now advances past
  `llvm-toolchain-minimal-22.1.0-linux-amd64//:lib/clang/22` and record the
  next blocker, if any.
- If a ruleset depends on `DefaultInfo.files.to_list()` expanding a source
  directory into individual file artifacts for Starlark user rules, confirm
  Bazel 9 behavior and either extend this plan or route the provider-shape work
  to the Starlark/provider owner.

## Next Owner

- Run the external LLVM smoke only if the checkout and validation
  budget make it reasonable; otherwise keep it as the first continuation item.
- If that smoke advances to a cargo build-script runner or execroot shape
  failure, route the next blocker to Plan 45 or Plan 44 rather than adding more
  directory-source special cases here.
