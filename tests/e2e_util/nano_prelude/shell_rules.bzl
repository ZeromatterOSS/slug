# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# @nolint

# Shell rules for nano_prelude. Mirrors the behavior of the now-removed
# kuro native sh_binary / sh_test / sh_library rules so existing fixtures
# continue to work after Plan 27.2 converts the native names to
# removed-rule stubs. Semantics intentionally match what the deleted
# `analyze_sh_*` helpers in kuro_analysis did:
#
# - `sh_library` collects `srcs` (and merged dep outputs) as DefaultInfo.files.
# - `sh_binary` exposes the first src as DefaultInfo(executable=...).
# - `sh_test` is `sh_binary` plus ExternalRunnerTestInfo(command=["bash", first_src]),
#   so the test runner doesn't require +x bits on the script.

def _sh_library_impl(ctx):
    files = list(ctx.files.srcs)
    for f in ctx.files.deps:
        files.append(f)
    return [DefaultInfo(default_outputs = files)]

sh_library = rule(
    implementation = _sh_library_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True, default = []),
        "deps": attr.label_list(allow_files = True, default = []),
        "data": attr.label_list(allow_files = True, default = []),
    },
)

def _sh_binary_impl(ctx):
    if not ctx.files.srcs:
        return [DefaultInfo()]
    first = ctx.files.srcs[0]
    return [DefaultInfo(default_output = first)]

sh_binary = rule(
    implementation = _sh_binary_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True, default = []),
        "deps": attr.label_list(allow_files = True, default = []),
        "data": attr.label_list(allow_files = True, default = []),
    },
)

def _sh_test_impl(ctx):
    if not ctx.files.srcs:
        return [
            DefaultInfo(),
            ExternalRunnerTestInfo(type = "sh", command = []),
        ]
    first = ctx.files.srcs[0]
    return [
        DefaultInfo(default_output = first),
        ExternalRunnerTestInfo(type = "sh", command = ["bash", first]),
    ]

sh_test = rule(
    implementation = _sh_test_impl,
    test = True,
    attrs = {
        "srcs": attr.label_list(allow_files = True, default = []),
        "deps": attr.label_list(allow_files = True, default = []),
        "data": attr.label_list(allow_files = True, default = []),
        "env": attr.string_dict(default = {}),
    },
)
