//! Runtime wrapper for the finite Rust toolchain recipe. Code is phase-owned.
use std::fmt;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use slug_build_api_v2::RustToolchainCallable;
use starlark::any::ProvidesStaticType;
use starlark::collections::StarlarkHashValue;
use starlark::collections::StarlarkHasher;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::eval::ParametersSpec;
use starlark::eval::ParametersSpecParam;
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
use starlark::values::tuple::AllocTuple;
use starlark::values::tuple::TupleRef;

use crate::bzl_module::rust_toolchain_helper::PreparedRustToolchainHelperBinding;

#[derive(Debug, ProvidesStaticType, NoSerialize, Trace, Freeze, Allocative)]
struct RustCallableGen<V> {
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    model: RustToolchainCallable,
    captures: V,
    helper: FrozenValue,
}

type RustCallable<'v> = RustCallableGen<Value<'v>>;
type FrozenRustCallable = RustCallableGen<FrozenValue>;
starlark::starlark_complex_values!(RustCallable);
impl<V> fmt::Display for RustCallableGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<function make_ccinfo>")
    }
}
impl<V> StarlarkFunctionIdentity for RustCallableGen<V> {
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
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for RustCallableGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = FrozenRustCallable;
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn StarlarkFunctionIdentity>(self);
    }
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(other
            .request_value::<&dyn StarlarkFunctionIdentity>()
            .and_then(|other| other.issued_identity())
            .is_some_and(|identity| Arc::ptr_eq(&self.model.model().identity, identity)))
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
        let params = ParametersSpec::<FrozenValue>::new_parts(
            "make_ccinfo",
            [],
            ["label", "actions", "allocator_library", "std"]
                .map(|name| (name, ParametersSpecParam::Required)),
            false,
            [],
            false,
        );
        let [label, actions, allocator, std] = params
            .collect_into::<4>(args, eval.heap())?
            .map(|value| value.expect("required parameter"));
        let factory = crate::subrule_invocation::AnalysisActions::from_starlark(actions)
            .ok_or_else(|| {
                starlark::Error::new_other(anyhow::anyhow!(
                    "make_ccinfo requires genuine current ctx.actions"
                ))
            })?;
        crate::subrule_invocation::actions_to_context(actions, eval)
            .map_err(starlark::Error::new_other)?;
        factory
            .require_active_rust_callback()
            .map_err(starlark::Error::new_other)?;
        let captures = self.captures.to_value();
        let captures = TupleRef::from_value(captures)
            .expect("four Rust captures")
            .content();
        eval.eval_function(
            self.helper.to_value(),
            &[],
            &[
                ("cc_toolchain", captures[0]),
                ("feature_configuration", captures[1]),
                ("experimental_link_std_dylib", captures[2]),
                ("rust_std", captures[3]),
                ("label", label),
                ("actions", actions),
                ("allocator_library", allocator),
                ("std", std),
            ],
        )
    }
}

pub fn rust_toolchain_callable<'v>(value: Value<'v>) -> Option<(RustToolchainCallable, Value<'v>)> {
    match RustCallable::from_value(value)? {
        starlark::__macro_refs::Either::Left(value) => Some((value.model.clone(), value.captures)),
        starlark::__macro_refs::Either::Right(value) => {
            Some((value.model.clone(), value.captures.to_value()))
        }
    }
}
pub fn alloc_frozen_rust_toolchain_callable(
    heap: &FrozenHeap,
    model: RustToolchainCallable,
    captures: [FrozenValue; 4],
    binding: &PreparedRustToolchainHelperBinding,
) -> Result<FrozenValue, String> {
    if binding.authority().as_ref() != model.model().source.as_ref() {
        return Err("Rust callable helper binding authority mismatch".to_owned());
    }
    let helper = binding.install(heap);
    Ok(heap.alloc(FrozenRustCallable {
        model,
        captures: heap.alloc(AllocTuple(captures)),
        helper,
    }))
}

#[cfg(test)]
mod tests;
