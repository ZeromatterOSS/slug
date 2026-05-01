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
}
