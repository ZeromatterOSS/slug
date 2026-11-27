use std::hash::Hash;

use compact_str::CompactString;
use slug_build_api_v2::ProviderId;
use starlark::__macro_refs::Either;
use starlark::eval::ExecutionBudget;
use starlark::eval::ExecutionLimit;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::structs::StructuralIdentity;
use starlark::values::structs::StructuralIdentityNative;
use starlark::values::structs::StructuralIdentityOp;
use starlark_map::StarlarkHasher;

use super::super::FrozenLoadingStarlarkUserProvider;
use super::super::LoadingProviderFieldsGen;
use super::super::LoadingStarlarkUserProvider;
use super::super::LoadingStarlarkUserProviderGen;
use super::admit_native_leaf;

pub(super) struct NativeIdentity;

impl<'v> StructuralIdentityNative<'v> for NativeIdentity {
    fn admit_atomic(
        &self,
        value: Value<'v>,
        op: StructuralIdentityOp,
        budget: &ExecutionBudget,
    ) -> Result<bool, ExecutionLimit> {
        admit_native_leaf(value, budget, op)
    }

    fn is_immutable(
        &self,
        ctx: &mut StructuralIdentity<'_, 'v, Self>,
        value: Value<'v>,
    ) -> starlark::Result<Option<bool>> {
        let Some(provider) = UserProvider::from_value(value) else {
            return Ok(None);
        };
        let fields = provider.ordered_fields(ctx.budget())?;
        Ok(Some(fields_immutable(ctx, &fields)?))
    }

    fn write_hash(
        &self,
        ctx: &mut StructuralIdentity<'_, 'v, Self>,
        value: Value<'v>,
        hasher: &mut StarlarkHasher,
        structural: bool,
    ) -> starlark::Result<bool> {
        let Some(provider) = UserProvider::from_value(value) else {
            return Ok(false);
        };
        let fields = provider.ordered_fields(ctx.budget())?;
        if !structural && !fields_immutable(ctx, &fields)? {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "unhashable type: provider"
            )));
        }
        let id = provider.id();
        admit_id(id, ctx.budget())?;
        id.hash(hasher);
        for (name, value) in fields {
            ctx.budget().charge(1, 0)?;
            ctx.budget().charge_text_work(name.len())?;
            name.hash(hasher);
            ctx.write_hash(value, hasher, true)?;
        }
        Ok(true)
    }

    fn equals(
        &self,
        ctx: &mut StructuralIdentity<'_, 'v, Self>,
        left: Value<'v>,
        right: Value<'v>,
    ) -> starlark::Result<Option<bool>> {
        let Some(left) = UserProvider::from_value(left) else {
            return Ok(None);
        };
        let Some(right) = UserProvider::from_value(right) else {
            return Ok(Some(false));
        };
        admit_id(left.id(), ctx.budget())?;
        admit_id(right.id(), ctx.budget())?;
        if left.id() != right.id() || left.len() != right.len() {
            return Ok(Some(false));
        }

        let left = left.ordered_fields(ctx.budget())?;
        let right = right.ordered_fields(ctx.budget())?;
        for ((left_name, left_value), (right_name, right_value)) in left.into_iter().zip(right) {
            ctx.budget().charge(1, 0)?;
            charge_name_bytes(left_name, right_name, ctx.budget())?;
            if left_name != right_name || !ctx.equals(left_value, right_value)? {
                return Ok(Some(false));
            }
        }
        Ok(Some(true))
    }
}

type Field<'v> = (&'v CompactString, Value<'v>);

struct UserProvider<'v>(
    Either<&'v LoadingStarlarkUserProvider<'v>, &'v FrozenLoadingStarlarkUserProvider>,
);

impl<'v> UserProvider<'v> {
    fn from_value(value: Value<'v>) -> Option<Self> {
        LoadingStarlarkUserProvider::from_value(value).map(Self)
    }

    fn id(&self) -> &'v ProviderId {
        match self.0 {
            Either::Left(provider) => &provider.id,
            Either::Right(provider) => &provider.id,
        }
    }

    fn len(&self) -> usize {
        match self.0 {
            Either::Left(provider) => field_count(&provider.fields),
            Either::Right(provider) => field_count(&provider.fields),
        }
    }

    fn ordered_fields(&self, budget: &ExecutionBudget) -> starlark::Result<Vec<Field<'v>>> {
        match self.0 {
            Either::Left(provider) => ordered_fields(provider, budget),
            Either::Right(provider) => ordered_fields(provider, budget),
        }
    }
}

fn field_count<V>(fields: &LoadingProviderFieldsGen<V>) -> usize {
    match fields {
        LoadingProviderFieldsGen::Schemaful { values, .. } => values.len(),
        LoadingProviderFieldsGen::Schemaless(fields) => fields.names.len().min(fields.values.len()),
    }
}

fn ordered_fields<'v, V: ValueLike<'v>>(
    provider: &'v LoadingStarlarkUserProviderGen<V>,
    budget: &ExecutionBudget,
) -> starlark::Result<Vec<Field<'v>>> {
    let count = field_count(&provider.fields);
    let mut fields = Vec::new();
    budget.charge(count, 0)?;
    budget.reserve_vec(&mut fields, count)?;
    match &provider.fields {
        LoadingProviderFieldsGen::Schemaful { schema, values } => {
            for (index, value) in values.iter() {
                let name = schema.get(*index as usize).ok_or_else(|| {
                    starlark::Error::new_other(anyhow::anyhow!("invalid provider schema index"))
                })?;
                fields.push((name, value.to_value()));
            }
        }
        LoadingProviderFieldsGen::Schemaless(values) => {
            for (name, value) in values.names.iter().zip(&values.values) {
                fields.push((name, value.to_value()));
            }
        }
    }
    // Names are unique in both layouts, so stability does not change field order.
    budget.sort_unstable_by(&mut fields, |left, right| {
        charge_name_bytes(left.0, right.0, budget)?;
        Ok(left.0.cmp(right.0))
    })?;
    Ok(fields)
}

fn fields_immutable<'v>(
    ctx: &mut StructuralIdentity<'_, 'v, NativeIdentity>,
    fields: &[Field<'v>],
) -> starlark::Result<bool> {
    for &(_, value) in fields {
        if !ctx.is_immutable(value)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn admit_id(id: &ProviderId, budget: &ExecutionBudget) -> Result<(), ExecutionLimit> {
    budget.charge(1, 0)?;
    budget.charge_text_work(id.source_label().len())?;
    budget.charge_text_work(id.exported_name().len())
}

fn charge_name_bytes(
    left: &CompactString,
    right: &CompactString,
    budget: &ExecutionBudget,
) -> Result<(), ExecutionLimit> {
    budget.charge_text_work(left.len().min(right.len()))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use slug_build_api_v2::AnalysisArtifact;
    use slug_build_api_v2::AnalysisDepsetOccurrence;
    use slug_build_api_v2::DepsetOrder;
    use slug_identity_v2::CanonicalLabel;
    use starlark::environment::Module;
    use starlark::values::FrozenValue;
    use starlark::values::list::AllocList;
    use starlark_map::small_map::SmallMap;

    use super::*;
    use crate::provider::StarlarkDepset;
    use crate::provider::StarlarkDepsetSuccessorGen;
    use crate::provider::alloc_starlark_user_provider;
    use crate::subrule_invocation::AnalysisArtifactValue;

    fn id(name: &str) -> ProviderId {
        ProviderId::new("@@rules_rust+//rust/private:providers.bzl", name).unwrap()
    }

    fn crate_info(module: &Module) -> FrozenValue {
        let heap = module.frozen_heap();
        let root = heap.alloc(AnalysisArtifactValue::new(AnalysisArtifact::Source(
            CanonicalLabel::parse("@@//pkg:lib.rs").unwrap(),
        )));
        alloc_starlark_user_provider(
            heap,
            id("CrateInfo"),
            true,
            [
                ("root".into(), root),
                ("name".into(), heap.alloc("support")),
            ],
        )
    }

    #[test]
    fn bounded_provider_identity_matches_native_hash_equality_and_depset_duplicates() {
        let module = Module::new();
        let heap = module.heap();
        let left = heap.alloc_complex(LoadingStarlarkUserProviderGen {
            id: id("DepVariantInfo"),
            fields: LoadingProviderFieldsGen::Schemaful {
                schema: Arc::from(["crate_info".into(), "dep_info".into(), "native".into()]),
                values: SmallMap::from_iter([
                    (2, Value::new_none()),
                    (0, crate_info(&module).to_value()),
                ]),
            },
        });
        let right = alloc_starlark_user_provider(
            module.frozen_heap(),
            id("DepVariantInfo"),
            true,
            [
                ("native".into(), FrozenValue::new_none()),
                ("crate_info".into(), crate_info(&module)),
            ],
        )
        .to_value();
        let different_id = alloc_starlark_user_provider(
            module.frozen_heap(),
            id("OtherDepInfo"),
            false,
            [
                ("native".into(), FrozenValue::new_none()),
                ("crate_info".into(), crate_info(&module)),
            ],
        )
        .to_value();

        let budget = ExecutionBudget::new(10_000, 10_000, 16);
        let mut ctx = StructuralIdentity::new(&budget, &NativeIdentity);
        assert!(!left.ptr_eq(right));
        for value in [left, right, different_id] {
            let ordinary = u64::from(value.get_hashed().unwrap().hash().get());
            assert_eq!(ctx.leaf_hash(value).unwrap(), ordinary);
        }
        for (first, second) in [(left, right), (right, left), (left, different_id)] {
            assert_eq!(
                ctx.equals(first, second).unwrap(),
                first.equals(second).unwrap()
            );
        }

        let set = heap.alloc_complex(StarlarkDepset {
            order: DepsetOrder::Default,
            element_type: Some("struct".into()),
            occurrence: AnalysisDepsetOccurrence::new(),
            retained: None,
            depth: 1,
            successors: vec![
                StarlarkDepsetSuccessorGen::Direct(left),
                StarlarkDepsetSuccessorGen::Direct(right),
                StarlarkDepsetSuccessorGen::Direct(different_id),
            ],
        });
        let flattened = super::super::flatten(set, &budget).unwrap();
        assert_eq!(flattened.len(), 2);
        assert!(flattened[0].ptr_eq(left));
        assert!(flattened[1].ptr_eq(different_id));
    }

    #[test]
    fn bounded_provider_reports_ordinary_unhashability_and_latches_storage_refusal() {
        let module = Module::new();
        let heap = module.heap();
        let value = heap.alloc_complex(LoadingStarlarkUserProviderGen {
            id: id("CrateInfo"),
            fields: LoadingProviderFieldsGen::Schemaful {
                schema: Arc::from(["mutable".into()]),
                values: SmallMap::from_iter([(0, heap.alloc(AllocList([1])))]),
            },
        });
        let budget = ExecutionBudget::new(1000, 1000, 16);
        let ordinary = value.get_hashed().unwrap_err();
        let mut ctx = StructuralIdentity::new(&budget, &NativeIdentity);
        let bounded = ctx.leaf_hash(value).unwrap_err();
        assert_eq!(
            bounded.without_diagnostic().to_string(),
            ordinary.without_diagnostic().to_string()
        );
        assert_eq!(budget.check(), Ok(()));

        let budget = ExecutionBudget::new(1000, 0, 16);
        assert!(
            StructuralIdentity::new(&budget, &NativeIdentity)
                .leaf_hash(value)
                .is_err()
        );
        assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));
    }
}
