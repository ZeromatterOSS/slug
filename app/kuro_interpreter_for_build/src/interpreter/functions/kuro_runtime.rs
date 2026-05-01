/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Plan 28.4 runtime hooks: Starlark globals that expose per-build
//! state to the bundled `@kuro_builtins//:exports.bzl` so that
//! `ctx`-method bodies migrated out of Rust can still reach
//! information that isn't carried on the analysis context (and
//! shouldn't be — that would just shift the duplication elsewhere).
//!
//! Every name in this module is `kuro_*`-prefixed so the bundled
//! module can identify it as a kuro-internal hook. End-user code is
//! technically able to call these (Starlark globals are flat) but the
//! contract is "internal to `@kuro_builtins`"; treat them as
//! private. If a hook's contract changes, both the Rust definition
//! here and the call site in `exports.bzl` must change together.

use starlark::environment::GlobalsBuilder;
use starlark::starlark_module;
use starlark::values::Heap;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::dict::AllocDict;
use starlark::values::list::AllocList;

#[starlark_module]
pub(crate) fn register_kuro_runtime(builder: &mut GlobalsBuilder) {
    /// Plan 28.4 Stage 8: returns the current value of the
    /// `--collect_code_coverage` flag for this build invocation.
    /// Consumed by `_kuro_coverage_instrumented` in
    /// `@kuro_builtins//:exports.bzl` (which serves
    /// `ctx.coverage_instrumented`). Mirrors
    /// `kuro_build_api::interpreter::rule_defs::build_config::get_collect_code_coverage`
    /// — same default (`false`), same per-build setter.
    fn kuro_collect_code_coverage() -> starlark::Result<bool> {
        Ok(kuro_build_api::interpreter::rule_defs::build_config::get_collect_code_coverage())
    }

    /// Plan 28.4 Stage 9: returns the Bazel CPU identifier for the
    /// host platform (e.g. "k8" for linux/x86_64). Consumed by
    /// `_kuro_make_substitutions` in `@kuro_builtins//:exports.bzl`
    /// to populate the `TARGET_CPU` Make variable. Mirrors
    /// `kuro_build_api::interpreter::rule_defs::context::host_target_cpu`
    /// (which is still used by `cc_common`'s ctx_cheat path).
    fn kuro_host_target_cpu() -> starlark::Result<String> {
        Ok(kuro_build_api::interpreter::rule_defs::context::host_target_cpu().to_owned())
    }

    /// Plan 28.4 Stage 9: returns the default C compiler path for the
    /// host platform. Consumed by `_kuro_make_substitutions` to
    /// populate the `CC` Make variable.
    fn kuro_host_cc_path() -> starlark::Result<String> {
        Ok(kuro_build_api::interpreter::rule_defs::context::host_cc_path().to_owned())
    }

    /// Plan 28.4 Stage 9: returns the per-build `--define KEY=VALUE`
    /// entries as a Starlark dict. Consumed by
    /// `_kuro_make_substitutions` as the lowest-priority layer of
    /// the `$(VAR)` substitution table. Mirrors
    /// `build_config::get_all_defines`.
    fn kuro_get_all_defines<'v>(heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let map = kuro_build_api::interpreter::rule_defs::build_config::get_all_defines();
        let entries: Vec<(String, String)> = map.into_iter().collect();
        Ok(heap.alloc(AllocDict(entries)))
    }

    /// Plan 28.4 Stage 9: resolves `COMPILATION_MODE` for the given
    /// configured target label. Reads the cfg's
    /// `@bazel_tools//tools/cpp:compilation_mode` build setting (the
    /// Plan 19.4 path) and falls back to the process-global
    /// `BUILD_CONFIG` entry when the cfg does not carry the setting
    /// (BXL top-level, anonymous targets). The cfg hash is not
    /// reachable from Starlark today, so this hook keeps the cfg
    /// lookup on the Rust side and returns just the resolved string.
    fn kuro_compilation_mode_for_label<'v>(label: Value<'v>) -> starlark::Result<String> {
        Ok(
            kuro_build_api::interpreter::rule_defs::context::compilation_mode_for_label_value(
                label,
            ),
        )
    }

    /// Plan 28.4 Stage 9: gathers `TemplateVariableInfo` variables
    /// from each dep in a `toolchains` attribute list. Returns a
    /// dict ready to merge into the `$(VAR)` substitution table.
    /// Mirrors Bazel's `RuleContext.getMakeVariables()` — keeping
    /// the provider-id lookup on the Rust side avoids exposing the
    /// `Provider in dep` operator and the instance's internal
    /// `variables` SmallMap to user `.bzl` files.
    fn kuro_collect_toolchains_template_vars<'v>(
        toolchains: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let pairs = kuro_build_api::interpreter::rule_defs::context::collect_toolchains_template_vars_from_list(
            toolchains,
        );
        Ok(heap.alloc(AllocDict(pairs)))
    }

    /// Plan 28.4 Stage 14: constructs a base Runfiles object from
    /// explicit `files`, `transitive_files`, `symlinks`, and
    /// `root_symlinks` args. Delegates to `create_runfiles` in
    /// `kuro_build_api::interpreter::rule_defs::provider::builtin::default_info`,
    /// which is already pub. Consumed by `_kuro_runfiles` in
    /// `@kuro_builtins//:exports.bzl`.
    fn kuro_create_runfiles<'v>(
        files: Value<'v>,
        transitive_files: Value<'v>,
        symlinks: Value<'v>,
        root_symlinks: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(
            kuro_build_api::interpreter::rule_defs::provider::builtin::default_info::create_runfiles(
                heap,
                files,
                transitive_files,
                symlinks,
                root_symlinks,
            )?,
        )
    }

    /// Plan 28.4 Stage 14: merges runfiles from a single attribute
    /// value (list of Dependency or a single Dependency) into `runfiles`
    /// and returns the updated Runfiles value. `want_data` selects
    /// `data_runfiles` vs `default_runfiles` on each dep's DefaultInfo.
    /// Delegates to the now-pub `collect_runfiles_from_value` in
    /// `kuro_build_api::interpreter::rule_defs::context`. Consumed by
    /// `_kuro_runfiles` in `@kuro_builtins//:exports.bzl`.
    fn kuro_collect_runfiles_into<'v>(
        mut runfiles: Value<'v>,
        attr_value: Value<'v>,
        want_data: bool,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        kuro_build_api::interpreter::rule_defs::context::collect_runfiles_from_value(
            attr_value,
            want_data,
            heap,
            &mut runfiles,
        )?;
        Ok(runfiles)
    }

    /// Plan 28.4 Stage 13: builds the label→paths pool consumed by
    /// `_kuro_expand_location` in `@kuro_builtins//:exports.bzl`.
    ///
    /// Iterates `targets` (the explicit list passed to
    /// `ctx.expand_location`) and the implicit attrs struct of
    /// `raw_ctx` (Dependency / artifact values from srcs / data /
    /// tools attrs), collecting `[label_str, [path1, ...]]` entries.
    /// Output-typed attrs (whose values are plain strings in ctx.attrs)
    /// are intentionally excluded — their lookup is deferred to
    /// `kuro_lookup_output_path` to avoid spurious artifact
    /// declarations.
    fn kuro_collect_location_pool<'v>(
        raw_ctx: Value<'v>,
        targets: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        use kuro_build_api::interpreter::rule_defs::context::AnalysisContext;
        let Some(ctx) = raw_ctx.downcast_ref::<AnalysisContext<'v>>() else {
            return Ok(heap.alloc(AllocList(Vec::<Value<'v>>::new())));
        };
        kuro_build_api::interpreter::rule_defs::context::collect_location_pool_for_ctx(
            ctx, targets, heap,
        )
    }

    /// Plan 28.4 Stage 13: lazily resolves an attr.output /
    /// attr.output_list label string to an artifact path. Called by
    /// `_kuro_expand_location` in `@kuro_builtins//:exports.bzl` when
    /// the pool built by `kuro_collect_location_pool` yields no match
    /// for the query label. Deferring this lookup avoids declaring
    /// unbound artifacts for every string-valued attribute (e.g.
    /// `name`, `tags`) — only the specific attr that actually contains
    /// the query label triggers an artifact declaration via CtxOutputs.
    ///
    /// Returns the full artifact path as a string, or `None` when no
    /// output attr matches `label_str`.
    fn kuro_lookup_output_path<'v>(
        raw_ctx: Value<'v>,
        label_str: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        use kuro_build_api::interpreter::rule_defs::context::AnalysisContext;
        let Some(ctx) = raw_ctx.downcast_ref::<AnalysisContext<'v>>() else {
            return Ok(Value::new_none());
        };
        kuro_build_api::interpreter::rule_defs::context::lookup_output_path_for_ctx(
            ctx, label_str, heap,
        )
    }
}
