/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use starlark::collections::SmallMap;
use starlark::environment::GlobalsBuilder;
use starlark::starlark_module;
use starlark::values::Value;
use starlark::values::none::NoneType;

const LOAD_SYMBOLS_ERROR: &str = "load_symbols() is a Buck2-specific function not available in \
    Bazel-compatible mode. Use top-level variable assignments and load() statements to \
    export symbols from .bzl files.";

#[starlark_module]
pub(crate) fn register_load_symbols(builder: &mut GlobalsBuilder) {
    /// Buck2-specific function not available in Bazel-compatible mode.
    ///
    /// In Bazel, use top-level variable assignments and `load()` statements
    /// to export symbols from .bzl files.
    fn load_symbols<'v>(_symbols: SmallMap<&'v str, Value<'v>>) -> starlark::Result<NoneType> {
        Err(kuro_error::kuro_error!(kuro_error::ErrorTag::Input, "{}", LOAD_SYMBOLS_ERROR).into())
    }
}
