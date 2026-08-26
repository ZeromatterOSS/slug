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
use starlark::values::Freeze;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::list::AllocList;
use starlark::values::list::ListRef;
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
impl<'v> StarlarkValue<'v> for CcInternalModule {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(cc_internal_methods)
    }
}

#[derive(Debug, Trace, Freeze, ProvidesStaticType, NoSerialize, Allocative)]
struct EmptyCcHeaderInfoGen<V> {
    empty_headers: V,
}

type EmptyCcHeaderInfo<'v> = EmptyCcHeaderInfoGen<Value<'v>>;
type FrozenEmptyCcHeaderInfo = EmptyCcHeaderInfoGen<FrozenValue>;
starlark::starlark_complex_values!(EmptyCcHeaderInfo);

impl<V> fmt::Display for EmptyCcHeaderInfoGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HeaderInfo")
    }
}

#[starlark_value(type = "HeaderInfo")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for EmptyCcHeaderInfoGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = FrozenEmptyCcHeaderInfo;

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "header_module" | "pic_header_module" | "separate_module" | "separate_pic_module" => {
                Some(Value::new_none())
            }
            "modular_public_headers"
            | "modular_private_headers"
            | "textual_headers"
            | "separate_module_headers" => Some(self.empty_headers.to_value()),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        [
            "header_module",
            "pic_header_module",
            "modular_public_headers",
            "modular_private_headers",
            "textual_headers",
            "separate_module_headers",
            "separate_module",
            "separate_pic_module",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }
}

#[starlark_module]
fn cc_internal_methods(builder: &mut MethodsBuilder) {
    fn freeze<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        value: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let Some(list) = ListRef::from_value(value) else {
            anyhow::bail!("cc_internal.freeze currently supports only empty lists");
        };
        if !list.is_empty() {
            anyhow::bail!("cc_internal.freeze currently supports only empty lists");
        }
        Ok(eval.frozen_heap().alloc(AllocList::EMPTY).to_value())
    }

    fn create_header_info<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let empty_headers = eval.frozen_heap().alloc(AllocList::EMPTY).to_value();
        Ok(eval
            .heap()
            .alloc_complex(EmptyCcHeaderInfoGen { empty_headers }))
    }
}

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
