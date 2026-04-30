# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# Plan 28: Bundled Bazel-Compatible Builtins.
#
# This file is the entry point of the @kuro_builtins bundled cell. The
# kuro interpreter auto-loads it into every BUILD and `.bzl` evaluation
# context (per `bazel_builtins_autoload` in
# `app/kuro_interpreter_for_build/src/interpreter/interpreter_for_dir.rs`).
#
# The export contract follows Plan 28's design (mirrored after Bonanza's
# `builtins_core/exports.bzl`):
#
#   - `exported_toplevels`: symbols visible at the top level of every
#     BUILD and `.bzl` file. Each entry must have a Bazel 9 parity
#     citation (or a `_kuro_*` prefix indicating it is kuro-internal,
#     e.g. probes for tests).
#   - `rule_implementation_wrapper` / `aspect_implementation_wrapper` /
#     `subrule_implementation_wrapper`: identity wrappers that
#     Phase 28.4 will route Starlark rule analysis through, so
#     subsequent stages can move `ctx`-method bodies into Starlark
#     without touching the Rust analysis call site again.
#
# Adding a symbol here means committing to:
#   1. A Bazel 9 parity citation (or `_kuro_*` naming).
#   2. A single owner per Plan 28.7 (Rust primitive, Starlark export, or
#      external ruleset — never two of the three).

# -----------------------------------------------------------------------
# Private helpers (not exported, hidden by leading underscore).
# -----------------------------------------------------------------------

# Phase 28.2 probe symbol. Not a Bazel builtin — exists solely to verify
# that the autoload mechanism reaches external `.bzl` files. Will be
# removed once Phase 28.3 starts moving real compatibility logic.
_kuro_builtins_probe_value = "kuro-28-2-loader-ok"

# Phase 28.4 Stage 1: identity rule-impl wrapper. Stage 2 will wire
# `kuro_analysis::run_analysis` to call `_invoke_rule(impl, ctx)`
# instead of `impl(ctx)`. Once routed, subsequent stages can swap the
# body to install a Starlark `ctx` facade and migrate methods like
# `ctx.target_platform_has_constraint` / `ctx.runfiles` / `ctx.var` out
# of Rust. Defined here, not yet referenced by analysis — kept off the
# `exported_toplevels` dict because it is a kuro-internal hook, not a
# user-visible builtin.
def _invoke_rule(implementation, raw_ctx):
    return implementation(raw_ctx)

# -----------------------------------------------------------------------
# Plan 28 export contract.
# -----------------------------------------------------------------------

# Symbols visible at the top level of every BUILD and `.bzl` file. The
# autoload in `interpreter_for_dir.rs::create_env` iterates this dict
# and copies each (name, value) into the consuming module's env.
# Visibility-control lives here, not in the interpreter — adding a name
# is an explicit decision in this file.
exported_toplevels = {
    # Phase 28.2 probe; kept under a `kuro_builtins_*` name to flag that
    # it is not a Bazel builtin. Used by
    # `tests/core/analysis/test_native_rules.py::test_28_2_kuro_builtins_visible_in_external_bzl`.
    "kuro_builtins_probe": _kuro_builtins_probe_value,
}

# Phase 28.4 wrapper hook. Not in `exported_toplevels` — analysis pulls
# it directly via the bundled module, not via the user-visible env.
rule_implementation_wrapper = _invoke_rule
