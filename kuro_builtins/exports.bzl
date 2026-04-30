# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# Plan 28: Bundled Bazel-Compatible Builtins.
#
# Public symbols here become visible at the top level of every BUILD and
# `.bzl` file in any cell whose interpreter has the `bazel_builtins_autoload`
# wired up — that is, every kuro project, with no MODULE.bazel registration
# required from the user.
#
# Per Plan 28.7 (migration discipline), each exported symbol must be a
# Bazel 9 builtin (cited inline) and have a single owner: either Rust
# primitive, Starlark builtins export, or external ruleset. Names added
# here MUST have a Bazel 9 source citation in the comment.

# Phase 28.2 probe symbol. Not a Bazel builtin — exists solely to verify
# that the autoload mechanism reaches external `.bzl` files. Will be
# removed once Phase 28.3 starts moving real compatibility logic into
# this package.
kuro_builtins_probe = "kuro-28-2-loader-ok"
