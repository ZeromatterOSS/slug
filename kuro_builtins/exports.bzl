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
# Plan 28.4 Stage 6: Starlark replacement for the deleted Rust impl
# of `ctx.package_relative_label` in
# `app/kuro_build_api/src/interpreter/rule_defs/context.rs`.
# Resolves a label string against the BUILD file's package (the
# target's package), distinct from the `Label()` builtin which
# resolves against the *file* where it appears. Same input/output
# contract as the previous Rust impl.
#
# When `raw_ctx.label` is `None` (dynamic_output / BXL contexts),
# fall through to the file-cell-resolving `Label()` builtin — which
# is what the old Rust path also did via `BazelLabel::parse(input)`.
def _kuro_package_relative_label(raw_ctx, label_str):
    label = raw_ctx.label
    if label == None:
        return Label(label_str)
    cell = label.cell
    pkg = label.package
    if label_str.startswith("@"):
        # Already fully qualified; pass through unchanged.
        return Label(label_str)
    if label_str.startswith("//"):
        return Label("@" + cell + label_str)
    target = label_str[1:] if label_str.startswith(":") else label_str
    return Label("@" + cell + "//" + pkg + ":" + target)

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
# adding a corresponding line in `_make_rule_facade` below; the
# kuro_facade_drift_guard test
# (tests/core/analysis/test_native_rules.py) compares `dir(raw_ctx)`
# against this list and fails loudly when they diverge.
#
# `kind` distinguishes which wrapper produced the facade — Stage 5's
# subrule wrapper reuses the same field set but tags itself
# differently so acceptance tests can prove which dispatch path ran.
def _make_rule_facade(raw_ctx, kind):
    # Closure binding `raw_ctx` for `package_relative_label`, which
    # needs to read `raw_ctx.label` at call time but takes only the
    # label string from the user — mirrors the Rust impl's signature.
    def _package_relative_label_bound(label_str):
        return _kuro_package_relative_label(raw_ctx, label_str)

    return struct(
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
        package_relative_label = _package_relative_label_bound,
        # ---- AnalysisContext methods passed through (bound to raw_ctx) ----
        coverage_instrumented = raw_ctx.coverage_instrumented,
        tokenize = raw_ctx.tokenize,
        runfiles = raw_ctx.runfiles,
        expand_make_variables = raw_ctx.expand_make_variables,
        resolve_tools = raw_ctx.resolve_tools,
        resolve_command = raw_ctx.resolve_command,
        new_file = raw_ctx.new_file,
        expand_location = raw_ctx.expand_location,
        # ---- Acceptance markers (kuro_*-prefixed). Used by Stage 3/5
        #      tests to prove which wrapper produced the facade. Not
        #      Bazel builtins; not part of the rule-author contract.
        kuro_facade_active = True,
        kuro_facade_kind = kind,
    )

def _invoke_rule(implementation, raw_ctx):
    return implementation(_make_rule_facade(raw_ctx, "rule"))

# Plan 28.4 Stage 5: subrule-side wrapper. Subrules are invoked from
# inside a rule impl; the dispatch site
# (`app/kuro_interpreter_for_build/src/subrule.rs`) reaches the
# wrapper via TLS set by `RuleSpec::Impl::invoke`. Subrule impls have
# the shape `def _impl(ctx, **kwargs)`, so the wrapper signature is
# `wrapper(impl, ctx, **kwargs)` — kwargs forward verbatim.
#
# Subrule contexts are the same `AnalysisContext` type as the
# enclosing rule, so the facade shares `_make_rule_facade`. Only the
# `kuro_facade_kind` tag differs so tests can confirm which dispatch
# path produced the struct.
def _invoke_subrule(implementation, raw_ctx, **kwargs):
    return implementation(_make_rule_facade(raw_ctx, "subrule"), **kwargs)

# Plan 28.4 Stage 4: aspect-side facade. Mirrors
# `_invoke_rule` but for `AspectContext`. Aspect impls are called as
# `impl(target, ctx)` (two positional args) so the wrapper signature is
# `wrapper(impl, target, raw_ctx)`. The dispatch site for aspects lives
# in `app/kuro_analysis/src/analysis/aspect_calculation.rs` (see Stage 4
# wiring in this commit).
#
# Field set is the AspectContext public surface in
# `app/kuro_build_api/src/interpreter/rule_defs/aspect/context.rs`.
# Smaller than rule context — no `attrs`, `outputs`, `executable`, etc.
# `target_platform_has_constraint` was deleted in Stage 3 from the Rust
# AspectContext too; here we install the same Starlark shim the rule
# facade uses, which means aspects can now answer the question
# meaningfully (instead of the previous unconditional `False`).
def _invoke_aspect(implementation, target, raw_ctx):
    return implementation(target, struct(
        # ---- AspectContext attributes (#[starlark(attribute)]) ----
        attr = raw_ctx.attr,
        actions = raw_ctx.actions,
        label = raw_ctx.label,
        rule = raw_ctx.rule,
        fragments = raw_ctx.fragments,
        host_fragments = raw_ctx.host_fragments,
        toolchains = raw_ctx.toolchains,
        features = raw_ctx.features,
        disabled_features = raw_ctx.disabled_features,
        bin_dir = raw_ctx.bin_dir,
        genfiles_dir = raw_ctx.genfiles_dir,
        configuration = raw_ctx.configuration,
        aspect_ids = raw_ctx.aspect_ids,
        build_file_path = raw_ctx.build_file_path,
        workspace_name = raw_ctx.workspace_name,
        # ---- AspectContext methods served from Starlark ----
        target_platform_has_constraint = _kuro_target_platform_has_constraint,
        # ---- AspectContext methods passed through (bound to raw_ctx) ----
        coverage_instrumented = raw_ctx.coverage_instrumented,
        # ---- Stage 4 acceptance marker (kuro_*-prefixed). Same shape as
        #      Stage 3's rule-facade marker but disambiguated so the
        #      acceptance test can prove which wrapper ran.
        kuro_facade_active = True,
        kuro_facade_kind = "aspect",
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

# Phase 28.4 Stage 4 aspect-wrapper hook. Picked up by
# `aspect_calculation.rs::execute_aspect`; same not-exported semantics
# as `rule_implementation_wrapper`.
aspect_implementation_wrapper = _invoke_aspect

# Phase 28.4 Stage 5 subrule-wrapper hook. Picked up by
# `kuro_interpreter_for_build::subrule::FrozenStarlarkSubruleCallable::invoke`
# via TLS set in `RuleSpec::Impl::invoke`. Same not-exported semantics
# as the rule/aspect hooks.
subrule_implementation_wrapper = _invoke_subrule
