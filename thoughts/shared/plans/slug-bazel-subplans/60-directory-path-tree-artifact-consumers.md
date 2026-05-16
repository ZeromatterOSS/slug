# Plan 60: DirectoryPathInfo and tree-artifact consumers

> Parent: [2026-01-21-slug-bazel-compatible-build-tool.md](../2026-01-21-slug-bazel-compatible-build-tool.md)
>
> Discovered while continuing the SDK parity loop after Plan 57 removed
> build-time `MODULE.bazel.lock` mutation. The full `//sdk:sdk` smoke advanced
> past the previous rules_rust build-script frontier and then repeatedly waited
> on `//sdk:ffi_cpp_headers`.

## Status: IN PROGRESS

## Failure class

`//sdk:ffi_cpp_headers` is:

```python
directory_path(
    name = "ffi_cpp_headers_path",
    directory = "//sdk/zeromatter_ffi:build_script",
    path = "headers",
)

copy_to_directory(
    name = "ffi_cpp_headers",
    srcs = [":ffi_cpp_headers_path"],
    replace_prefixes = {
        "zeromatter_ffi/build_script.out_dir/headers": "",
    },
)
```

`@bazel_lib//lib:directory_path.bzl` returns a user provider whose fields are
a TreeArtifact and a nested path. `@bazel_lib//lib:copy_to_directory.bzl`
then detects `DirectoryPathInfo in t`, reads the tree artifact's `path`,
`root.path`, `short_path`, `owner`, and `workspace_name`, and registers a
`CopyToDirectory` action with the tree artifact as an input and declared
directory as output.

The repeated wait is therefore not owned by the SDK label. The owning Slug
abstractions are:

- Bazel-compatible tree-artifact `File` attributes (`is_directory`, `path`,
  `short_path`, `root.path`, `owner`, and command-line/input behavior).
- User provider membership/indexing on target values (`DirectoryPathInfo in t`
  and `t[DirectoryPathInfo]`).
- `ctx.actions.run` input registration for tree artifacts used through user
  providers rather than `DefaultInfo.files`.

## Non-fixes

Do not special-case `//sdk:ffi_cpp_headers`, `DirectoryPathInfo`, `headers`, or
`copy_to_directory` labels. A correct fix belongs either in the generic File
tree-artifact surface, target/provider access, or action input registration.

Do not rewrite `replace_prefixes` or SDK paths. If Slug's artifact paths differ
from Bazel's, fix the artifact root/path computation at the File abstraction.

## Current evidence

2026-05-15:

- Full non-instrumented Slug SDK smoke
  `p54-depset-perf-baseline-1` progressed past earlier build-script targets and
  repeatedly logged:
  `Waiting on workspace//sdk:ffi_cpp_headers (...) -- running analysis [evaluate_rule], and 32 other actions`.
- The target is a `bazel_lib` `directory_path` + `copy_to_directory` consumer
  over the `zeromatter_ffi` build-script tree output.
- Earlier memory instrumentation showed high depset construction volume, but
  this later focused frontier is a stronger classifier for the next slice.

## Next checks

1. Run a focused Slug build of `//sdk:ffi_cpp_headers` with a fresh isolation
   directory and compare whether the same target stalls without the rest of
   `//sdk:sdk`.
2. If it stalls, instrument the user-rule analysis path around provider
   membership/indexing and artifact attribute reads used by
   `copy_to_directory_bin_action`.
3. If it fails, encode the failure as a focused regression at the owning
   abstraction before patching.
4. After a systemic fix, run focused tests, `cargo build -p slug -j 1`, then a
   fresh `//sdk:sdk` smoke. Verify `MODULE.bazel.lock` hash remains the Bazel
   regenerated hash and that Slug does not write it.

