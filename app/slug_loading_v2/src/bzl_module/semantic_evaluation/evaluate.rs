//! Run the original evaluator only on a semantic-key miss.
use super::SemanticBzlEvaluationError as Error;
use super::*;

pub(super) fn evaluate(
    inputs: Arc<PreparedSemanticBzlEvaluation>,
    direct_presentation: bool,
) -> SemanticBzlEvaluationResult {
    let capture = events::SemanticPrintCapture::new(
        inputs.budget.records,
        inputs.budget.bytes,
        direct_presentation,
    );
    let (result, event_batch) = match capture {
        Err(error) => (
            Err(Error::Resource(error.into())),
            Arc::new(EventBatch::empty()),
        ),
        Ok(capture) => {
            let result = evaluate_module(&inputs, &capture);
            match capture.finish() {
                Ok(events) => (result, events),
                Err(error) => (
                    Err(Error::Resource(error.into())),
                    Arc::new(EventBatch::empty()),
                ),
            }
        }
    };
    SemanticBzlEvaluationResult {
        inputs,
        result,
        events: event_batch,
    }
}

fn evaluate_module(
    inputs: &Arc<PreparedSemanticBzlEvaluation>,
    capture: &events::SemanticPrintCapture,
) -> Result<FrozenBzlModule, Error> {
    let root = &inputs.rows[0];
    let source = std::str::from_utf8(&root.source)
        .map_err(|error| Error::Parse(error.to_string().into()))?;
    let ast = AstModule::parse_with_string_encoding(
        &root.key.source_name,
        source.to_owned(),
        &Dialect::Bazel,
        StringEncoding::BazelInternal,
    );
    let ast = ast.map_err(|error| Error::Parse(error.to_string().into()))?;
    let manifest = BzlLoadManifest::new(
        root.key.identity.clone(),
        digest(std::str::from_utf8(&root.source).expect("prepared UTF-8 source")),
        inputs.children.iter().map(|child| &child.module),
    );
    let owner = crate::function_owner::bzl_evaluation_owner(
        &manifest,
        ast.file_span(ast.statement().span).file,
        matches!(root.key.context, BzlModuleContext::Bzlmod),
        true,
        inputs.children.iter().map(|child| {
            child
                .module
                .module
                .client_data()
                .and_then(|data| {
                    crate::starlark_label::defining_module_function_owner(data.to_value())
                })
                .cloned()
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "original loaded .bzl module has no admitted structural function owner"
                    )
                })
        }),
    )
    .map_err(|error| Error::Resource(error.to_string().into()))?;
    let module = Module::new();
    crate::starlark_label::attach_defining_module_owner(&module, &manifest, owner.clone())
        .expect("a new .bzl module has no defining client data");
    let evaluation_context = BzlEvaluationContext::from_manifest(&manifest);
    let globals = root.key.context.globals(&manifest.root.label);
    let loader = LocalBzlLoader {
        modules: inputs
            .children
            .iter()
            .map(|child| (child.raw.as_ref(), child.module.module.dupe()))
            .collect(),
    };
    {
        let mut evaluator = Evaluator::new(&module);
        evaluator
            .set_function_symbol_session(crate::function_owner::function_symbol_session(owner))
            .expect("fresh evaluator");
        evaluator
            .set_function_export_mode(starlark::eval::FunctionExportMode::TopLevelAssignments)
            .expect("fresh evaluator");
        evaluator.extra = Some(&evaluation_context);
        evaluator.set_loader(&loader);
        evaluator.set_print_handler(capture);
        let result = evaluator.eval_module(ast, &globals).map(|_| ());
        drop(evaluator);
        result.map_err(|error| Error::Evaluation(error.to_string().into()))?;
    }
    let frozen = module
        .freeze()
        .map_err(|error| Error::Freeze(format!("{error:?}").into()))?;
    let retained_bzl_modules =
        flattened_module_lifetime_closure(inputs.children.iter().map(|child| &child.module)).into();
    Ok(FrozenBzlModule {
        module: frozen,
        path: manifest.root.workspace_path.clone(),
        loads: inputs
            .children
            .iter()
            .map(|child| child.raw.to_string())
            .collect(),
        retained_bzl_modules,
        bzl_load_visibility: evaluation_context.bzl_load_visibility(),
        manifest,
        semantic_inputs: Some(inputs.clone()),
    })
}
