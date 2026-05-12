/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Runtime hooks: Starlark globals that expose per-build state to the
//! bundled `@slug_builtins//:exports.bzl` so that `ctx`-method bodies
//! served from Starlark can still reach information that isn't carried
//! on the analysis context.
//!
//! Every name in this module is `slug_*`-prefixed so the bundled
//! module can identify it as a slug-internal hook. End-user code is
//! technically able to call these (Starlark globals are flat) but the
//! contract is "internal to `@slug_builtins`"; treat them as
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
pub(crate) fn register_slug_runtime(builder: &mut GlobalsBuilder) {
    /// Returns the current value of the `--collect_code_coverage`
    /// flag. Consumed by `_slug_coverage_instrumented` in
    /// `@slug_builtins//:exports.bzl` to serve
    /// `ctx.coverage_instrumented`.
    fn slug_collect_code_coverage() -> starlark::Result<bool> {
        Ok(slug_build_api::interpreter::rule_defs::build_config::get_collect_code_coverage())
    }

    /// Returns the Bazel CPU identifier for the host platform (e.g.
    /// "k8" for linux/x86_64). Consumed by `_slug_make_substitutions`
    /// to populate the `TARGET_CPU` Make variable.
    fn slug_host_target_cpu() -> starlark::Result<String> {
        Ok(slug_build_api::interpreter::rule_defs::context::host_target_cpu().to_owned())
    }

    /// Returns the default C compiler path for the host platform.
    /// Consumed by `_slug_make_substitutions` to populate the `CC`
    /// Make variable.
    fn slug_host_cc_path() -> starlark::Result<String> {
        Ok(slug_build_api::interpreter::rule_defs::context::host_cc_path().to_owned())
    }

    /// Returns the per-build `--define KEY=VALUE` entries as a
    /// Starlark dict. Consumed by `_slug_make_substitutions` as the
    /// lowest-priority layer of the `$(VAR)` substitution table.
    fn slug_get_all_defines<'v>(heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let map = slug_build_api::interpreter::rule_defs::build_config::get_all_defines();
        let entries: Vec<(String, String)> = map.into_iter().collect();
        Ok(heap.alloc(AllocDict(entries)))
    }

    /// Resolves `COMPILATION_MODE` for the given configured target
    /// label. Reads the cfg's
    /// `@bazel_tools//tools/cpp:compilation_mode` build setting and
    /// falls back to the process-global `BUILD_CONFIG` entry when the
    /// cfg does not carry the setting (BXL top-level, anonymous
    /// targets). The cfg hash is not reachable from Starlark today, so
    /// this hook keeps the cfg lookup on the Rust side and returns
    /// just the resolved string.
    fn slug_compilation_mode_for_label<'v>(label: Value<'v>) -> starlark::Result<String> {
        Ok(
            slug_build_api::interpreter::rule_defs::context::compilation_mode_for_label_value(
                label,
            ),
        )
    }

    /// Gathers `TemplateVariableInfo` variables from each dep in a
    /// `toolchains` attribute list. Returns a dict ready to merge into
    /// the `$(VAR)` substitution table. Mirrors Bazel's
    /// `RuleContext.getMakeVariables()` — keeping the provider-id
    /// lookup on the Rust side avoids exposing the `Provider in dep`
    /// operator and the instance's internal `variables` SmallMap to
    /// user `.bzl` files.
    fn slug_collect_toolchains_template_vars<'v>(
        toolchains: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let pairs = slug_build_api::interpreter::rule_defs::context::collect_toolchains_template_vars_from_list(
            toolchains,
        );
        Ok(heap.alloc(AllocDict(pairs)))
    }

    /// Constructs a base Runfiles object from explicit `files`,
    /// `transitive_files`, `symlinks`, and `root_symlinks` args.
    /// Consumed by `_slug_runfiles` in `@slug_builtins//:exports.bzl`.
    fn slug_create_runfiles<'v>(
        files: Value<'v>,
        transitive_files: Value<'v>,
        symlinks: Value<'v>,
        root_symlinks: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(
            slug_build_api::interpreter::rule_defs::provider::builtin::default_info::create_runfiles(
                heap,
                files,
                transitive_files,
                symlinks,
                root_symlinks,
            )?,
        )
    }

    /// Merges runfiles from a single attribute value (list of
    /// Dependency or a single Dependency) into `runfiles` and returns
    /// the updated Runfiles value. `want_data` selects `data_runfiles`
    /// vs `default_runfiles` on each dep's DefaultInfo. Consumed by
    /// `_slug_runfiles` in `@slug_builtins//:exports.bzl`.
    fn slug_collect_runfiles_into<'v>(
        mut runfiles: Value<'v>,
        attr_value: Value<'v>,
        want_data: bool,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        slug_build_api::interpreter::rule_defs::context::collect_runfiles_from_value(
            attr_value,
            want_data,
            heap,
            &mut runfiles,
        )?;
        Ok(runfiles)
    }

    /// Builds the label→paths pool consumed by `_slug_expand_location`
    /// in `@slug_builtins//:exports.bzl`.
    ///
    /// Iterates `targets` (the explicit list passed to
    /// `ctx.expand_location`) and the implicit attrs struct of
    /// `raw_ctx` (Dependency / artifact values from srcs / data /
    /// tools attrs), collecting `[label_str, [path1, ...]]` entries.
    /// Output-typed attrs (whose values are plain strings in ctx.attrs)
    /// are intentionally excluded — their lookup is deferred to
    /// `slug_lookup_output_path` to avoid spurious artifact
    /// declarations.
    fn slug_collect_location_pool<'v>(
        raw_ctx: Value<'v>,
        targets: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        use slug_build_api::interpreter::rule_defs::context::AnalysisContext;
        let Some(ctx) = raw_ctx.downcast_ref::<AnalysisContext<'v>>() else {
            return Ok(heap.alloc(AllocList(Vec::<Value<'v>>::new())));
        };
        slug_build_api::interpreter::rule_defs::context::collect_location_pool_for_ctx(
            ctx, targets, heap,
        )
    }

    /// Lazily resolves an attr.output / attr.output_list label string
    /// to an artifact path. Called by `_slug_expand_location` when the
    /// pool built by `slug_collect_location_pool` yields no match for
    /// the query label. Deferring this lookup avoids declaring unbound
    /// artifacts for every string-valued attribute (e.g. `name`,
    /// `tags`) — only the specific attr that actually contains the
    /// query label triggers an artifact declaration via CtxOutputs.
    ///
    /// Returns the full artifact path as a string, or `None` when no
    /// output attr matches `label_str`.
    fn slug_lookup_output_path<'v>(
        raw_ctx: Value<'v>,
        label_str: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        use slug_build_api::interpreter::rule_defs::context::AnalysisContext;
        let Some(ctx) = raw_ctx.downcast_ref::<AnalysisContext<'v>>() else {
            return Ok(Value::new_none());
        };
        slug_build_api::interpreter::rule_defs::context::lookup_output_path_for_ctx(
            ctx, label_str, heap,
        )
    }
}
