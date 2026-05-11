/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 */

//! `module_ctx.facts` and `module_ctx.extension_metadata(...)` support.

use std::fmt;

use allocative::Allocative;
use derive_more::Display;
use starlark::any::ProvidesStaticType;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::typing::Ty;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::dict::DictRef;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;

const MAX_FACTS_DEPTH: usize = 7;

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub struct FactsValue {
    #[allocative(skip)]
    facts: serde_json::Map<String, serde_json::Value>,
}

impl FactsValue {
    pub fn new(facts: serde_json::Value) -> Self {
        let facts = match facts {
            serde_json::Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        Self { facts }
    }

    fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }
}

impl fmt::Display for FactsValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<Facts>")
    }
}

starlark_simple_value!(FactsValue);

#[starlark_value(type = "Facts")]
impl<'v> StarlarkValue<'v> for FactsValue {
    fn at(&self, index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let Some(key) = index.unpack_str() else {
            return Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "Facts keys must be strings, got {}",
                index.get_type()
            )
            .into());
        };
        match self.facts.get(key) {
            Some(value) => Ok(crate::interpreter::natives::json_to_starlark_value(
                heap, value,
            )),
            None => Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "key not found in Facts: {}",
                key
            )
            .into()),
        }
    }

    fn is_in(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(other
            .unpack_str()
            .is_some_and(|key| self.facts.contains_key(key)))
    }

    fn to_bool(&self) -> bool {
        !self.is_empty()
    }

    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(facts_methods)
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::any()
    }
}

#[starlark_module]
fn facts_methods(builder: &mut MethodsBuilder) {
    fn get<'v>(
        this: &FactsValue,
        #[starlark(require = pos)] key: &str,
        #[starlark(require = pos, default = NoneType)] default: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Ok(match this.facts.get(key) {
            Some(value) => crate::interpreter::natives::json_to_starlark_value(eval.heap(), value),
            None => default,
        })
    }
}

#[derive(Debug, Clone, Display, ProvidesStaticType, NoSerialize, Allocative)]
#[display("<module_extension_metadata>")]
pub struct StarlarkModuleExtensionMetadata {
    #[allocative(skip)]
    metadata: kuro_bzlmod::module_extension_executor::ModuleExtensionMetadata,
}

impl StarlarkModuleExtensionMetadata {
    pub fn new(metadata: kuro_bzlmod::module_extension_executor::ModuleExtensionMetadata) -> Self {
        Self { metadata }
    }

    pub fn metadata(&self) -> &kuro_bzlmod::module_extension_executor::ModuleExtensionMetadata {
        &self.metadata
    }
}

starlark_simple_value!(StarlarkModuleExtensionMetadata);

#[starlark_value(type = "module_extension_metadata")]
impl<'v> StarlarkValue<'v> for StarlarkModuleExtensionMetadata {
    fn get_type_starlark_repr() -> Ty {
        Ty::any()
    }
}

pub fn empty_facts() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

pub fn validate_facts_value(value: Value<'_>) -> starlark::Result<serde_json::Value> {
    if value.is_none() {
        return Ok(empty_facts());
    }

    let Some(dict) = DictRef::from_value(value) else {
        return Err(kuro_error::kuro_error!(
            kuro_error::ErrorTag::Input,
            "facts must be a dict with string keys, got {}",
            value.get_type()
        )
        .into());
    };

    let mut object = serde_json::Map::new();
    for (key, value) in dict.iter() {
        let Some(key) = key.unpack_str() else {
            return Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "Facts keys must be strings, got {}",
                key.get_type()
            )
            .into());
        };
        let json = value.to_json_value().map_err(|e| {
            kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "'{}' ({}) is not supported in facts: {}",
                value.to_repr(),
                value.get_type(),
                e
            )
        })?;
        validate_json_depth(&json, MAX_FACTS_DEPTH)?;
        object.insert(key.to_owned(), json);
    }

    Ok(serde_json::Value::Object(object))
}

fn validate_json_depth(value: &serde_json::Value, remaining_depth: usize) -> starlark::Result<()> {
    if remaining_depth == 0 {
        return Err(kuro_error::kuro_error!(
            kuro_error::ErrorTag::Input,
            "Facts cannot be nested more than {} levels deep",
            MAX_FACTS_DEPTH
        )
        .into());
    }

    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                validate_json_depth(item, remaining_depth - 1)?;
            }
        }
        serde_json::Value::Object(entries) => {
            for value in entries.values() {
                validate_json_depth(value, remaining_depth - 1)?;
            }
        }
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use starlark::environment::Globals;
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;
    use starlark::values::dict::AllocDict;

    use super::*;

    #[test]
    fn facts_value_supports_membership_index_and_get() {
        let module = Module::new();
        let heap = module.heap();
        let facts = heap.alloc(FactsValue::new(serde_json::json!({
            "resource": "stored",
            "nested": {"checksum": "abc"},
        })));

        assert!(facts.is_in(heap.alloc("resource")).unwrap());
        assert!(!facts.is_in(heap.alloc("missing")).unwrap());
        assert_eq!(
            facts.at(heap.alloc("resource"), heap).unwrap().unpack_str(),
            Some("stored")
        );

        module.set("facts", facts);
        let ast = AstModule::parse(
            "facts.star",
            "facts.get('resource')".to_owned(),
            &Dialect::Standard,
        )
        .unwrap();
        let mut eval = Evaluator::new(&module);
        let result = eval.eval_module(ast, &Globals::standard()).unwrap();
        assert_eq!(result.unpack_str(), Some("stored"));
    }

    #[test]
    fn validate_facts_rejects_non_string_keys() {
        let module = Module::new();
        let heap = module.heap();
        let value = heap.alloc(AllocDict(vec![(1, "bad")]));

        let err = validate_facts_value(value).unwrap_err();
        assert!(err.to_string().contains("Facts keys must be strings"));
    }

    #[test]
    fn validate_facts_accepts_json_like_values() {
        let module = Module::new();
        let heap = module.heap();
        let value = heap.alloc(AllocDict(vec![
            ("name", heap.alloc("serde")),
            ("enabled", Value::new_bool(true)),
            ("versions", heap.alloc(vec![1, 2])),
        ]));

        assert_eq!(
            validate_facts_value(value).unwrap(),
            serde_json::json!({
                "name": "serde",
                "enabled": true,
                "versions": [1, 2],
            })
        );
    }
}
