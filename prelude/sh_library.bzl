# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

def sh_library_impl(ctx: AnalysisContext) -> list[Provider]:
    # Collect all sources
    srcs = ctx.attrs.srcs

    # Collect all outputs from deps
    dep_outputs = []
    for dep in ctx.attrs.deps:
        info = dep[DefaultInfo]
        dep_outputs.extend(info.default_outputs)
        dep_outputs.extend(info.other_outputs)

    return [
        DefaultInfo(
            default_outputs = srcs,
            other_outputs = dep_outputs,
        ),
    ]
