use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::depset::DepsetTraversalError;
use slug_build_api_v2::depset::traverse_depset_bounded;
use slug_identity_v2::CanonicalLabel;
use starlark::eval::ExecutionBudget;
use starlark::eval::ExecutionLimit;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::float::StarlarkFloat;

use super::EvaluatorDepset;
use super::StarlarkDepset;
use super::starlark_label_ref;
use crate::subrule_invocation::AnalysisArtifactValue;

pub(super) fn flatten<'v>(
    value: Value<'v>,
    budget: &ExecutionBudget,
) -> starlark::Result<Vec<Value<'v>>> {
    traverse_depset_bounded(
        &EvaluatorDepset(value),
        |work, bytes| Ok::<_, starlark::Error>(budget.charge(work, bytes)?),
        |value| {
            admit_leaf(*value, budget, Use::Hash)?;
            value
                .get_hashed()
                .map(|value| u64::from(value.hash().get()))
        },
        |left, right| {
            if left.ptr_eq(*right) {
                budget.charge(1, 0)?;
                return Ok(true);
            }
            admit_leaf(*left, budget, Use::Equality)?;
            admit_leaf(*right, budget, Use::Equality)?;
            left.equals(*right)
        },
    )
    .map_err(|error| match error {
        DepsetTraversalError::Operation(error) => error,
        DepsetTraversalError::SizeOverflow => budget.refuse(ExecutionLimit::AllocationBytes).into(),
        DepsetTraversalError::AllocationFailed => {
            budget.refuse(ExecutionLimit::AllocationFailed).into()
        }
    })
}

#[derive(Clone, Copy)]
enum Use {
    Hash,
    Equality,
}

fn admit_leaf(
    value: Value<'_>,
    budget: &ExecutionBudget,
    usage: Use,
) -> Result<(), ExecutionLimit> {
    budget.charge(1, 0)?;
    if let Some(text) = value.unpack_str() {
        return budget.charge_text_work(text.len());
    }
    if let Some(label) = starlark_label_ref(value) {
        return admit_label(label, budget);
    }
    if let Some(file) = AnalysisArtifactValue::from_starlark(value) {
        return admit_artifact(file.artifact(), budget, usage);
    }
    if value.is_none()
        || value.unpack_bool().is_some()
        || value.unpack_i32().is_some()
        || value.downcast_ref::<StarlarkFloat>().is_some()
        || StarlarkDepset::occurrence_from_value(value).is_some()
    {
        return Ok(());
    }
    Err(budget.refuse(ExecutionLimit::UnsupportedOperation {
        operation: "depset leaf identity",
        value_type: value.get_type(),
    }))
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
    usage: Use,
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
            if let Use::Equality = usage {
                budget.charge(config.option_count(), 0)?;
                budget.charge_text_work(config.canonical_bytes().len())?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use slug_build_api_v2::AnalysisDepsetOccurrence;
    use slug_build_api_v2::DepsetOrder;
    use starlark::environment::Globals;
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;
    use starlark::values::list::ListRef;

    use super::*;
    use crate::provider::StarlarkDepsetSuccessorGen;
    use crate::provider::alloc_starlark_depset_parts;

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
    fn bounded_native_to_list_refuses_scratch_and_unmodeled_leaves() {
        let budget = Arc::new(ExecutionBudget::new(10_000, 0, 16));
        let module = Module::new_with_execution_budget(budget.clone());
        install(&module, "File", [file(&module, "@@//pkg:a.rs")]);
        assert!(eval(&module).is_err());
        assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));

        let budget = Arc::new(ExecutionBudget::new(10_000, 10_000, 16));
        let module = Module::new_with_execution_budget(budget.clone());
        install(&module, "tuple", [module.frozen_heap().alloc((1, 2))]);
        assert!(eval(&module).is_err());
        assert_eq!(
            budget.check(),
            Err(ExecutionLimit::UnsupportedOperation {
                operation: "depset leaf identity",
                value_type: "tuple",
            })
        );
    }
}
