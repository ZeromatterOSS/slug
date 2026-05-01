# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# Plan 28.6 PR 4: prelude.bzl is now empty. BUILD globals come from
# `register_all_natives` (Rust top-level globals) plus the bundled
# `@kuro_builtins//:exports.bzl` autoload. The legacy `native = struct(...)`
# scrape over `__kuro_builtins__` is gone. This file remains only so the
# `@prelude` cell load path resolves cleanly for workspaces that still
# register the cell; the `PreludePath` type-system entry will go in a
# follow-up once no workspace registers `@prelude`.
