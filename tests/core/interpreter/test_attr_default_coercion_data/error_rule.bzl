# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

def _impl(ctx):
    pass

# Test that invalid default values for label attributes are properly rejected.
# Using Bazel-compatible attr.label() API.
error_rule = rule(
    implementation = _impl,
    attrs = {
        "someattr": attr.label(default = "notaproperlabel"),
    },
)
