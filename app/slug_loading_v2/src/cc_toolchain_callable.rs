//! Native implementations of the finite, source-authenticated toolchain recipes.
//! Authentication and capture lowering belong to the provider publication owner.
use std::fmt;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use slug_build_api_v2::CcToolchainCallable;
use slug_build_api_v2::CcToolchainCallableKind;
use slug_build_api_v2::DepsetOrder;
use starlark::any::ProvidesStaticType;
use starlark::collections::StarlarkHashValue;
use starlark::collections::StarlarkHasher;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::values::Demand;
use starlark::values::Freeze;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::function::StarlarkFunctionIdentity;
use starlark::values::starlark_value;

#[cfg(test)]
mod tests;

#[derive(Debug, ProvidesStaticType, NoSerialize, Trace, Freeze, Allocative)]
struct ToolchainCallableGen<V> {
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    model: CcToolchainCallable,
    capture: V,
}
type ToolchainCallable<'v> = ToolchainCallableGen<Value<'v>>;
type FrozenToolchainCallable = ToolchainCallableGen<FrozenValue>;
starlark::starlark_complex_values!(ToolchainCallable);

impl<V> fmt::Display for ToolchainCallableGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self.model.model().kind {
            CcToolchainCallableKind::SupportsPic => "_needs_pic_for_dynamic_libraries",
            CcToolchainCallableKind::AlwaysPic => "lambda",
            CcToolchainCallableKind::StaticRuntime => "static_runtime_lib_func",
            CcToolchainCallableKind::DynamicRuntime => "dynamic_runtime_lib_func",
        };
        write!(f, "<function {name}>")
    }
}
impl<V> StarlarkFunctionIdentity for ToolchainCallableGen<V> {
    fn issued_identity(&self) -> Option<&Arc<()>> {
        Some(&self.model.model().identity)
    }
    fn issue_identity(&self) -> Arc<()> {
        self.model.model().identity.clone()
    }
    fn name_hash(&self) -> StarlarkHashValue {
        StarlarkHashValue::new_unchecked(self.model.model().name_hash)
    }
}

#[starlark_value(type = "function")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for ToolchainCallableGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = FrozenToolchainCallable;
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn StarlarkFunctionIdentity>(self);
    }
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(other
            .request_value::<&dyn StarlarkFunctionIdentity>()
            .and_then(|other| other.issued_identity())
            .is_some_and(|other| Arc::ptr_eq(&self.model.model().identity, other)))
    }
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        hasher.write_u32(self.model.model().name_hash);
        Ok(())
    }
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        args.no_positional_args(eval.heap())?;
        let named = args.names_map()?;
        let feature = named
            .iter()
            .find_map(|(name, value)| (name.as_str() == "feature_configuration").then_some(*value));
        if named.len() != 1 || feature.is_none() {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "expected exactly one keyword-only argument: feature_configuration"
            )));
        }
        let kind = self.model.model().kind;
        if kind == CcToolchainCallableKind::AlwaysPic {
            return Ok(Value::new_bool(true));
        }
        let feature = feature.unwrap();
        let method = feature
            .get_attr("is_enabled", eval.heap())?
            .ok_or_else(|| {
                starlark::Error::new_other(anyhow::anyhow!(
                    "feature_configuration has no attribute is_enabled"
                ))
            })?;
        let feature_name = match kind {
            CcToolchainCallableKind::SupportsPic => "supports_pic",
            _ => "static_link_cpp_runtimes",
        };
        let enabled = eval.eval_function(
            method,
            &[eval.heap().alloc_str(feature_name).to_value()],
            &[],
        )?;
        if kind == CcToolchainCallableKind::SupportsPic {
            return Ok(enabled);
        }
        if !enabled.to_bool() {
            return Ok(crate::provider::empty_evaluator_depset(
                DepsetOrder::Default,
                eval,
            ));
        }
        let capture = self.capture.to_value();
        if capture.is_none() {
            let attribute = if kind == CcToolchainCallableKind::StaticRuntime {
                "static_runtime_lib"
            } else {
                "dynamic_runtime_lib"
            };
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "Toolchain supports embedded runtimes, but didn't provide {attribute} attribute."
            )));
        }
        Ok(capture)
    }
}

pub fn cc_toolchain_callable<'v>(value: Value<'v>) -> Option<(CcToolchainCallable, Value<'v>)> {
    match ToolchainCallable::from_value(value)? {
        starlark::__macro_refs::Either::Left(value) => Some((value.model.clone(), value.capture)),
        starlark::__macro_refs::Either::Right(value) => {
            Some((value.model.clone(), value.capture.to_value()))
        }
    }
}
pub fn alloc_frozen_cc_toolchain_callable(
    heap: &FrozenHeap,
    model: CcToolchainCallable,
    capture: FrozenValue,
) -> FrozenValue {
    heap.alloc(FrozenToolchainCallable { model, capture })
}
