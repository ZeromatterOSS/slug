/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use starlark::environment::GlobalsBuilder;
use starlark::starlark_module;
use starlark::values::StringValue;
use starlark::values::Value;
use starlark::values::none::NoneOr;

const READ_CONFIG_ERROR: &str = "read_config() is a Buck2-specific function not available in \
    Bazel-compatible mode. Use MODULE.bazel for dependency configuration, \
    select() for platform-conditional attributes, or module_extension() for \
    custom configuration. See https://bazel.build/external/module for details.";

const READ_ROOT_CONFIG_ERROR: &str = "read_root_config() is a Buck2-specific function not \
    available in Bazel-compatible mode. Use MODULE.bazel for dependency configuration, \
    select() for platform-conditional attributes, or module_extension() for \
    custom configuration. See https://bazel.build/external/module for details.";

#[starlark_module]
pub(crate) fn register_read_config(globals: &mut GlobalsBuilder) {
    /// Buck2-specific function not available in Bazel-compatible mode.
    ///
    /// In Bazel, configuration is handled through:
    /// - `MODULE.bazel` for dependency configuration
    /// - `select()` for platform-conditional attributes
    /// - `module_extension()` for custom configuration
    fn read_config<'v>(
        _section: StringValue,
        _key: StringValue,
        _default: Option<Value<'v>>,
    ) -> starlark::Result<Value<'v>> {
        Err(kuro_error::kuro_error!(kuro_error::ErrorTag::Input, "{}", READ_CONFIG_ERROR).into())
    }

    /// Buck2-specific function not available in Bazel-compatible mode.
    ///
    /// In Bazel, configuration is handled through:
    /// - `MODULE.bazel` for dependency configuration
    /// - `select()` for platform-conditional attributes
    /// - `module_extension()` for custom configuration
    fn read_root_config<'v>(
        #[starlark(require = pos)] _section: StringValue,
        #[starlark(require = pos)] _key: StringValue,
        #[starlark(require = pos, default = NoneOr::None)] _default: NoneOr<StringValue<'v>>,
    ) -> starlark::Result<NoneOr<StringValue<'v>>> {
        Err(
            kuro_error::kuro_error!(kuro_error::ErrorTag::Input, "{}", READ_ROOT_CONFIG_ERROR)
                .into(),
        )
    }
}
