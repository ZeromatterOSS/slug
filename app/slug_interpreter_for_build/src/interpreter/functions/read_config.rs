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
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::StringValue;
use starlark::values::Value;
use starlark::values::none::NoneOr;

use crate::interpreter::build_context::BuildContext;

#[starlark_module]
pub(crate) fn register_read_config(globals: &mut GlobalsBuilder) {
    /// Read a buckconfig value for the current cell.
    ///
    /// Returns the string value if found, or `default` (None by default) if not found.
    fn read_config<'v>(
        #[starlark(require = pos)] section: StringValue<'v>,
        #[starlark(require = pos)] key: StringValue<'v>,
        #[starlark(default = NoneOr::None)] default: NoneOr<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Limit the scope of build_context borrow so eval can be used after
        let value = {
            let build_ctx = BuildContext::from_context(eval)?;
            build_ctx
                .buckconfigs
                .lookup_current(section.as_str(), key.as_str())?
        };
        match value {
            Some(v) => Ok(eval.heap().alloc(v.as_ref())),
            None => match default {
                NoneOr::None => Ok(Value::new_none()),
                NoneOr::Other(v) => Ok(v),
            },
        }
    }

    /// Read a buckconfig value from the root cell's config.
    ///
    /// Returns the string value if found, or `default` (None by default) if not found.
    fn read_root_config<'v>(
        #[starlark(require = pos)] section: StringValue<'v>,
        #[starlark(require = pos)] key: StringValue<'v>,
        #[starlark(default = NoneOr::None)] default: NoneOr<StringValue<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneOr<StringValue<'v>>> {
        // Limit the scope of build_context borrow so eval can be used after
        let value = {
            let build_ctx = BuildContext::from_context(eval)?;
            build_ctx
                .buckconfigs
                .lookup_root(section.as_str(), key.as_str())?
        };
        match value {
            Some(v) => Ok(NoneOr::Other(eval.heap().alloc_str(v.as_ref()))),
            None => match default {
                NoneOr::None => Ok(NoneOr::None),
                NoneOr::Other(v) => Ok(NoneOr::Other(v)),
            },
        }
    }
}
