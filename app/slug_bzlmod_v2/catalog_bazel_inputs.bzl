"""Expose pinned bazel_tools bytes as inputs without loading upstream BUILD rules."""

def slug_builtin_catalog_files():
    native.filegroup(
        name = "slug_builtin_catalog_files",
        srcs = ["BUILD"] + native.glob(
            ["**"],
            exclude = ["BUILD", "BUILD.bazel"],
            allow_empty = True,
        ),
        visibility = ["//app/slug_bzlmod_v2:__pkg__"],
    )
