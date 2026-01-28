# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

"""
Bootstrap file for bazel_tools globals.

Symbols exported here will be injected as globals in BUILD files by the
interpreter. This allows defining cc_common, proto_common_do_not_use, and
other Bazel-compatible globals in Starlark rather than native Rust code.

This file is loaded automatically by the interpreter during startup.
Symbols in BAZEL_TOOLS_GLOBALS will be injected into the global namespace
for BUILD files.

Usage:
    # The interpreter loads this file at startup:
    # load("@bazel_tools//tools/build_defs:globals.bzl", "BAZEL_TOOLS_GLOBALS")
    # for name, value in BAZEL_TOOLS_GLOBALS.items():
    #     env.set(name, value)
"""

load("@bazel_tools//tools/cpp:cc_common.bzl", "cc_common")
load("@bazel_tools//tools/build_defs/proto:proto_common.bzl", "proto_common_do_not_use")

# Export symbols that should be globals in BUILD files.
# The interpreter will iterate over this dict and inject each symbol.
BAZEL_TOOLS_GLOBALS = {
    "cc_common": cc_common,
    "proto_common_do_not_use": proto_common_do_not_use,
}
