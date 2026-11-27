mod structural;

use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::depset::DepsetTraversalError;
use slug_build_api_v2::depset::traverse_depset_bounded;
use slug_identity_v2::CanonicalLabel;
use starlark::eval::ExecutionBudget;
use starlark::eval::ExecutionLimit;
use starlark::values::Value;
use starlark::values::structs::StructuralIdentity;
use starlark::values::structs::StructuralIdentityOp;

use self::structural::NativeIdentity;
use super::EvaluatorDepset;
use super::StarlarkDepset;
use super::starlark_label_ref;
use crate::cc_header_info::cc_header_info_ref;
use crate::subrule_invocation::AnalysisArtifactValue;

pub(super) fn flatten<'v>(
    value: Value<'v>,
    budget: &ExecutionBudget,
) -> starlark::Result<Vec<Value<'v>>> {
    let mut hash = StructuralIdentity::new(budget, &NativeIdentity);
    let mut equals = StructuralIdentity::new(budget, &NativeIdentity);
    traverse_depset_bounded(
        &EvaluatorDepset(value),
        |work, bytes| Ok::<_, starlark::Error>(budget.charge(work, bytes)?),
        |value| hash.leaf_hash(*value),
        |left, right| equals.equals(*left, *right),
    )
    .map_err(|error| match error {
        DepsetTraversalError::Operation(error) => error,
        DepsetTraversalError::SizeOverflow => budget.refuse(ExecutionLimit::AllocationBytes).into(),
        DepsetTraversalError::AllocationFailed => {
            budget.refuse(ExecutionLimit::AllocationFailed).into()
        }
    })
}

fn admit_native_leaf(
    value: Value<'_>,
    budget: &ExecutionBudget,
    operation: StructuralIdentityOp,
) -> Result<bool, ExecutionLimit> {
    if let Some(label) = starlark_label_ref(value) {
        admit_label(label, budget)?;
        return Ok(true);
    }
    if let Some(file) = AnalysisArtifactValue::from_starlark(value) {
        admit_artifact(file.artifact(), budget, operation)?;
        return Ok(true);
    }
    // Both objects use exact, fixed-size occurrence identity in Starlark.
    Ok(StarlarkDepset::occurrence_from_value(value).is_some()
        || cc_header_info_ref(value).is_some())
}

fn admit_label(label: &CanonicalLabel, budget: &ExecutionBudget) -> Result<(), ExecutionLimit> {
    let package = label.package();
    for component in [
        package.repo().as_str(),
        package.package().as_str(),
        label.target().as_str(),
        label.mapping_id().map_or("", |mapping| mapping.as_str()),
    ] {
        budget.charge_text_work(component.len())?;
    }
    Ok(())
}

fn admit_artifact(
    artifact: &AnalysisArtifact,
    budget: &ExecutionBudget,
    operation: StructuralIdentityOp,
) -> Result<(), ExecutionLimit> {
    match artifact {
        AnalysisArtifact::Source(label) => admit_label(label, budget),
        AnalysisArtifact::WorkspaceStatus(status) => budget.charge_text_work(
            status
                .root()
                .workspace()
                .as_path()
                .as_os_str()
                .as_encoded_bytes()
                .len(),
        ),
        AnalysisArtifact::Derived { owner, output } => {
            admit_label(owner.label(), budget)?;
            budget.charge_text_work(owner.configuration().len())?;
            if let Some(platform) = owner.toolchain_execution_platform() {
                admit_label(platform, budget)?;
            }
            budget.charge_text_work(output.path().len())?;
            budget.charge_text_work(output.root().path().len())?;
            let config = output.root().configuration();
            budget.charge_text_work(config.canonical_bytes().len())?;
            // Configuration hashing reads canonical bytes. Equality also visits
            // the ordered typed fields, which are framed in those same bytes.
            if let StructuralIdentityOp::Equality = operation {
                budget.charge(config.option_count(), 0)?;
                budget.charge_text_work(config.canonical_bytes().len())?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroI32;
    use std::sync::Arc;

    use slug_build_api_v2::AnalysisDepsetOccurrence;
    use slug_build_api_v2::CcHeaderInfo;
    use slug_build_api_v2::CcHeaderInfoInput;
    use slug_build_api_v2::DepsetOrder;
    use slug_build_api_v2::ProviderId;
    use starlark::environment::Globals;
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;
    use starlark::values::FrozenValue;
    use starlark::values::ValueLike;
    use starlark::values::list::ListRef;
    use starlark::values::range::Range;

    use super::*;
    use crate::cc_header_info::alloc_frozen_cc_header_info;
    use crate::provider::StarlarkDepsetSuccessorGen;
    use crate::provider::alloc_starlark_depset_parts;
    use crate::provider::alloc_starlark_user_provider;

    fn install<'v>(
        module: &'v Module,
        element_type: &str,
        leaves: impl IntoIterator<Item = starlark::values::FrozenValue>,
    ) {
        let value = alloc_starlark_depset_parts(
            module.frozen_heap(),
            DepsetOrder::Preorder,
            Some(element_type.into()),
            AnalysisDepsetOccurrence::new(),
            None,
            2,
            leaves
                .into_iter()
                .map(StarlarkDepsetSuccessorGen::Direct)
                .collect(),
        );
        module.set("data", value.to_value());
    }

    fn eval(module: &Module) -> starlark::Result<Value<'_>> {
        let ast = AstModule::parse(
            "depset-budget.star",
            "data.to_list()".to_owned(),
            &Dialect::Standard,
        )
        .unwrap();
        Evaluator::new(module).eval_module(ast, &Globals::standard())
    }

    fn file(module: &Module, label: &str) -> starlark::values::FrozenValue {
        module
            .frozen_heap()
            .alloc(AnalysisArtifactValue::new(AnalysisArtifact::Source(
                CanonicalLabel::parse(label).unwrap(),
            )))
    }

    #[test]
    fn bounded_native_to_list_preserves_file_order_and_equality() {
        let budget = Arc::new(ExecutionBudget::new(10_000, 10_000, 16));
        for module in [
            Module::new(),
            Module::new_with_execution_budget(budget.clone()),
        ] {
            let leaves = [
                file(&module, "@@//pkg:b.rs"),
                file(&module, "@@//pkg:a.rs"),
                file(&module, "@@//pkg:b.rs"),
            ];
            install(&module, "File", leaves);
            let result = eval(&module).unwrap();
            let output = ListRef::from_value(result).unwrap();
            let paths: Vec<_> = output
                .iter()
                .map(|value| {
                    AnalysisArtifactValue::from_starlark(value)
                        .unwrap()
                        .artifact()
                        .short_path()
                })
                .collect();
            assert_eq!(paths, ["pkg/b.rs", "pkg/a.rs"]);
        }
        assert_eq!(budget.check(), Ok(()));
    }

    #[test]
    fn bounded_native_to_list_deduplicates_structural_leaves_in_occurrence_order() {
        let budget = Arc::new(ExecutionBudget::new(10_000, 10_000, 16));
        let module = Module::new_with_execution_budget(budget.clone());
        let heap = module.frozen_heap();
        let provider = |name: &str, field: &str, value: FrozenValue| {
            alloc_starlark_user_provider(
                heap,
                ProviderId::new("@@//:defs.bzl", name).unwrap(),
                true,
                [(field.into(), value)],
            )
        };
        let header = || CcHeaderInfo::create(CcHeaderInfoInput::default(), false, &[]).unwrap();
        let leaf = |value| {
            provider(
                "DepVariantInfo",
                "cc_info",
                provider(
                    "CcInfo",
                    "_header_info",
                    alloc_frozen_cc_header_info(heap, value),
                ),
            )
        };
        let original = header();
        let first = leaf(original.clone());
        let distinct = leaf(header());
        let duplicate = leaf(original);
        assert!(!first.to_value().ptr_eq(duplicate.to_value()));
        assert!(first.to_value().equals(duplicate.to_value()).unwrap());
        assert!(!first.to_value().equals(distinct.to_value()).unwrap());

        install(&module, "struct", [first, distinct, duplicate]);
        let result = eval(&module).unwrap();
        let output = ListRef::from_value(result).unwrap();
        assert_eq!(output.len(), 2);
        for (actual, expected) in output.iter().zip([first, distinct]) {
            assert!(actual.ptr_eq(expected.to_value()));
        }
        assert_eq!(budget.check(), Ok(()));
    }

    #[test]
    fn bounded_native_to_list_refuses_scratch_and_unmodeled_leaves() {
        let budget = Arc::new(ExecutionBudget::new(10_000, 0, 16));
        let module = Module::new_with_execution_budget(budget.clone());
        install(&module, "File", [file(&module, "@@//pkg:a.rs")]);
        assert!(eval(&module).is_err());
        assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));

        let budget = Arc::new(ExecutionBudget::new(10_000, 10_000, 16));
        let module = Module::new_with_execution_budget(budget.clone());
        let range = module
            .frozen_heap()
            .alloc(Range::new(0, 2, NonZeroI32::new(1).unwrap()));
        install(&module, "range", [range]);
        assert!(eval(&module).is_err());
        assert!(matches!(
            budget.check(),
            Err(ExecutionLimit::UnsupportedOperation {
                value_type: "range",
                ..
            })
        ));
    }
}
