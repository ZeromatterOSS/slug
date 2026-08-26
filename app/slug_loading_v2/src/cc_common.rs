/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;

use allocative::Allocative;
use starlark::any::ProvidesStaticType;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;

use crate::provider::BzlEvaluationContext;

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct CcCommonModule;
starlark::starlark_simple_value!(CcCommonModule);

impl fmt::Display for CcCommonModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cc_common")
    }
}

#[starlark_value(type = "cc_common")]
impl<'v> StarlarkValue<'v> for CcCommonModule {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(cc_common_methods)
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct CcInternalModule;
starlark::starlark_simple_value!(CcInternalModule);

impl fmt::Display for CcInternalModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cc_internal")
    }
}

#[starlark_value(type = "cc_internal")]
impl<'v> StarlarkValue<'v> for CcInternalModule {}

#[starlark_module]
fn cc_common_methods(builder: &mut MethodsBuilder) {
    #[allow(non_snake_case)]
    fn internal_DO_NOT_USE<'v>(
        #[starlark(this)] _cc_common: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<CcInternalModule> {
        let context = BzlEvaluationContext::from_evaluator(eval)?;
        let source = context.source_identity_for_call(eval)?;
        if !source
            .label
            .package()
            .repo()
            .as_str()
            .starts_with("rules_cc+")
        {
            let canonical = source.label.to_string();
            let canonical = if source.label.package().repo().is_root() {
                canonical.strip_prefix("@@").unwrap_or(&canonical)
            } else {
                &canonical
            };
            anyhow::bail!("file '{canonical}' cannot use private API");
        }
        Ok(CcInternalModule)
    }
}

pub(crate) fn cc_common_globals(builder: &mut GlobalsBuilder) {
    builder.set("cc_common", CcCommonModule);
}
