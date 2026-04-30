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

load(":_host_constants.bzl", "HOST_CONSTRAINT_LABELS")

# -----------------------------------------------------------------------
# Private helpers (not exported, hidden by leading underscore).
# -----------------------------------------------------------------------

# Phase 28.2 probe symbol. Not a Bazel builtin — exists solely to verify
# that the autoload mechanism reaches external `.bzl` files. Will be
# removed once Phase 28.3 starts moving real compatibility logic.
_kuro_builtins_probe_value = "kuro-28-2-loader-ok"

# Plan 28.4 Stage 3: Starlark replacement for the deleted Rust impl in
# `app/kuro_build_api/src/interpreter/rule_defs/context.rs`. Until kuro
# has a full target-platform constraint resolver (Plan 19 territory), we
# answer `ctx.target_platform_has_constraint(c)` against the host's
# OS/CPU labels, mirroring the previous Rust shortcut byte-for-byte. The
# host labels are baked at kuro build time by
# `app/kuro_external_cells_bundled/build.rs::imp` and arrive here via
# `_host_constants.bzl`.
def _kuro_target_platform_has_constraint(constraint_value):
    # ConstraintValueInfo exposes the constraint's canonical label as
    # `.label`. Anything else (None, missing attr) maps to False, just
    # like the Rust impl.
    label_attr = getattr(constraint_value, "label", None)
    if label_attr == None:
        return False
    label_str = str(label_attr)
    for candidate in HOST_CONSTRAINT_LABELS:
        if not candidate:
            # Tombstone for unsupported host OS/CPU at build.rs time.
            continue
        if label_str == candidate:
            return True
        no_at = candidate[1:] if candidate.startswith("@") else candidate
        if label_str == no_at:
            return True
        idx = no_at.find("//")
        if idx >= 0 and label_str.endswith(no_at[idx:]):
            return True
    return False

# Plan 28.4 Stage 3: install a Starlark facade around `raw_ctx` so
# individual `ctx`-method bodies can move from Rust into Starlark
# without touching the analysis call site. The facade is a `struct`
# that mirrors every public field on the underlying `AnalysisContext`,
# with the migrated methods replaced by Starlark closures.
#
# Two invariants this code relies on:
#
#   1. For user-defined `rule()` impls (the only callers of this
#      wrapper — see `RuleSpec::invoke` in
#      `app/kuro_analysis/src/analysis/env.rs`), every attribute below
#      is available without raising. The "not available for
#      `dynamic_output` or BXL" attribute paths are not reachable here.
#
#   2. Bound-method values returned by `raw_ctx.<method>` for
#      non-migrated methods (e.g. `runfiles`, `expand_make_variables`)
#      are first-class Starlark values that re-bind to `raw_ctx` when
#      called. Storing them as struct fields preserves call semantics.
#
# Adding a new ctx field anywhere in
# `app/kuro_build_api/src/interpreter/rule_defs/context.rs` requires
# adding a corresponding line below; the kuro_facade_drift_guard test
# (tests/core/analysis/test_native_rules.py) compares `dir(raw_ctx)`
# against this list and fails loudly when they diverge.
def _invoke_rule(implementation, raw_ctx):
    return implementation(struct(
        # ---- AnalysisContext attributes (#[starlark(attribute)]) ----
        attrs = raw_ctx.attrs,
        actions = raw_ctx.actions,
        label = raw_ctx.label,
        plugins = raw_ctx.plugins,
        attr = raw_ctx.attr,
        split_attr = raw_ctx.split_attr,
        workspace_name = raw_ctx.workspace_name,
        build_file_path = raw_ctx.build_file_path,
        fragments = raw_ctx.fragments,
        host_fragments = raw_ctx.host_fragments,
        toolchains = raw_ctx.toolchains,
        outputs = raw_ctx.outputs,
        features = raw_ctx.features,
        disabled_features = raw_ctx.disabled_features,
        configuration = raw_ctx.configuration,
        files = raw_ctx.files,
        file = raw_ctx.file,
        executable = raw_ctx.executable,
        bin_dir = raw_ctx.bin_dir,
        genfiles_dir = raw_ctx.genfiles_dir,
        version_file = raw_ctx.version_file,
        info_file = raw_ctx.info_file,
        exec_groups = raw_ctx.exec_groups,
        var = raw_ctx.var,
        build_setting_value = raw_ctx.build_setting_value,
        # ---- AnalysisContext methods served from Starlark ----
        target_platform_has_constraint = _kuro_target_platform_has_constraint,
        # ---- AnalysisContext methods passed through (bound to raw_ctx) ----
        coverage_instrumented = raw_ctx.coverage_instrumented,
        tokenize = raw_ctx.tokenize,
        runfiles = raw_ctx.runfiles,
        expand_make_variables = raw_ctx.expand_make_variables,
        package_relative_label = raw_ctx.package_relative_label,
        resolve_tools = raw_ctx.resolve_tools,
        resolve_command = raw_ctx.resolve_command,
        new_file = raw_ctx.new_file,
        expand_location = raw_ctx.expand_location,
        # ---- Stage 3 acceptance marker (kuro_*-prefixed). Used by the
        #      facade-in-call-path test to assert the wrapper actually
        #      installed a struct over raw_ctx. Not a Bazel builtin.
        kuro_facade_active = True,
    ))

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
