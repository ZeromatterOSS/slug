//! Default link facade binding and unchanged evaluator-local action authority.
use starlark::environment::FrozenModule;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::list::AllocList;

use super::wrap;
use crate::package::loading_globals;
use crate::provider::BzlEvaluationContext;
use crate::subrule_invocation::*;

fn context() -> (
    AnalysisEvaluationContext,
    Arc<crate::cc_common::solib_tests::RecordingSink>,
) {
    let sink = Arc::new(crate::cc_common::solib_tests::RecordingSink::default());
    let context = AnalysisEvaluationContext::new(
        Arc::from([]),
        [],
        CanonicalLabel::parse("@@//:subject").unwrap(),
        sink.clone(),
        FrozenValue::new_none(),
        Arc::from([]),
    );
    (context, sink)
}

fn actions<'v>(module: &'v Module, context: &AnalysisEvaluationContext) -> Value<'v> {
    module.heap().alloc_simple(AnalysisActions::new(
        context.payload.action_sink.clone(),
        context.root_token(),
        "rule context",
        AnalysisActionCallScope::Root,
    ))
}

fn execute(eval: &mut Evaluator<'_, '_, '_>, source: &str) -> anyhow::Result<()> {
    eval.eval_module(
        AstModule::parse("ordinary.bzl", source.to_owned(), &Dialect::Bazel)
            .map_err(starlark::Error::into_anyhow)?,
        &loading_globals(),
    )
    .map_err(starlark::Error::into_anyhow)?;
    Ok(())
}

fn internal() -> FrozenModule {
    let module = Module::new();
    let context = BzlEvaluationContext::new("@@rules_cc+//cc/private:link_actions_test.bzl");
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    execute(&mut eval, "i = cc_common.internal_DO_NOT_USE()\n").unwrap();
    drop(eval);
    module.freeze().unwrap()
}

#[test]
fn default_link_wrapper_binding_is_fresh_nested_and_narrow() {
    let module = Module::new();
    module.import_public_symbols(&internal());
    let (context, sink) = context();
    let original = module.heap().alloc(AllocList([7]));
    let _guard = context.enter_root_context(&module, original).unwrap();
    let base = actions(&module, &context);
    module.set("actions", base);
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    for expression in [
        "i.wrap_link_actions(actions)",
        "i.wrap_link_actions(actions, None)",
        "i.wrap_link_actions(actions, None, False)",
    ] {
        execute(&mut eval, &format!("wrapped = {expression}\n")).unwrap();
        let wrapped = module.get("wrapped").unwrap();
        assert!(!wrapped.ptr_eq(base));
        assert!(AnalysisActions::from_value(wrapped).is_none());
        let handle = AnalysisActions::from_starlark(wrapped).unwrap();
        let base_handle = AnalysisActions::from_starlark(base).unwrap();
        assert!(Arc::ptr_eq(&handle.action_sink, &base_handle.action_sink));
        assert!(Arc::ptr_eq(&handle.token.stack, &base_handle.token.stack));
        assert_eq!(handle.token.token, base_handle.token.token);
        assert_eq!(handle.context_name, base_handle.context_name);
        assert_eq!(handle.scope, base_handle.scope);
        assert!(actions_to_context(wrapped, &eval).unwrap().ptr_eq(original));
    }
    let first = module.get("wrapped").unwrap();
    execute(
        &mut eval,
        "nested = i.wrap_link_actions(wrapped)\nrecovered = i.actions2ctx_cheat(nested)\n",
    )
    .unwrap();
    let nested = module.get("nested").unwrap();
    assert!(!nested.ptr_eq(first));
    assert!(module.get("recovered").unwrap().ptr_eq(original));
    for extension in ["declare_shareable_artifact", "declare_shareable_directory"] {
        assert!(base.get_attr(extension, module.heap()).unwrap().is_none());
        assert!(nested.get_attr(extension, module.heap()).unwrap().is_some());
        let error = execute(&mut eval, &format!("nested.{extension}('unused')\n")).unwrap_err();
        assert!(
            error
                .to_string()
                .contains(&format!("link-actions {extension} is unsupported/deferred")),
            "{error}"
        );
    }
    for (expression, diagnostic) in [
        (
            "i.wrap_link_actions(actions, False)",
            "alternative build_configuration",
        ),
        (
            "i.wrap_link_actions(actions, struct())",
            "alternative build_configuration",
        ),
        (
            "i.wrap_link_actions(actions, None, True)",
            "sharable_artifacts=True",
        ),
    ] {
        let error = execute(&mut eval, expression).unwrap_err();
        assert!(error.to_string().contains(diagnostic), "{error}");
        assert!(
            error.to_string().contains("unsupported/deferred"),
            "{error}"
        );
    }
    for expression in [
        "i.wrap_link_actions()",
        "i.wrap_link_actions(actions, None, False, None)",
        "i.wrap_link_actions(actions=actions)",
        "i.wrap_link_actions(actions, build_configuration=None)",
        "i.wrap_link_actions(actions, None, sharable_artifacts=False)",
        "i.wrap_link_actions(actions, None, 1)",
        "i.wrap_link_actions(actions, None, None)",
        "i.wrap_link_actions(None)",
        "i.wrap_link_actions(struct(actions=actions))",
        "i.wrap_link_actions('actions')",
    ] {
        assert!(execute(&mut eval, expression).is_err(), "{expression}");
    }
    assert!(sink.0.lock().unwrap().is_empty());
}

#[test]
fn default_link_wrapper_rejects_missing_foreign_and_expired_owners() {
    let module = Module::new();
    let (a, sink) = context();
    let (b, foreign_sink) = context();
    let a_actions = actions(&module, &a);
    let b_actions = actions(&module, &b);
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&a);
    let error = wrap(a_actions, None, false, &mut eval).unwrap_err();
    assert!(error.to_string().contains("registered"), "{error}");
    let original = module.heap().alloc(AllocList([7]));
    let guard = a.enter_root_context(&module, original).unwrap();
    let error = wrap(b_actions, None, false, &mut eval).unwrap_err();
    assert!(error.to_string().contains("another evaluation"), "{error}");
    eval.extra = Some(&b);
    let error = wrap(b_actions, None, false, &mut eval).unwrap_err();
    assert!(
        error.to_string().contains("registration belongs"),
        "{error}"
    );
    eval.extra = Some(&a);
    let wrapped = wrap(a_actions, None, false, &mut eval).unwrap();
    drop(guard);
    for expired in [a_actions, wrapped] {
        let error = wrap(expired, None, false, &mut eval).unwrap_err();
        assert!(error.to_string().contains("expired"), "{error}");
    }
    assert!(sink.0.lock().unwrap().is_empty());
    assert!(foreign_sink.0.lock().unwrap().is_empty());
}

#[test]
fn suspended_link_wrappers_preserve_root_and_subrule_scope() {
    let module = Module::new();
    let (context, sink) = context();
    let base = actions(&module, &context);
    let original = module.heap().alloc(AllocList([7]));
    let _root_guard = context.enter_root_context(&module, original).unwrap();
    let identity = Arc::new(SubruleIdentity {
        defining_label: CanonicalLabel::parse("@@//:subrule.bzl").unwrap(),
        exported_name: "helper".into(),
    });
    context.stack.lock().unwrap().frames.push(CallFrame {
        token: 1,
        caller: Some(identity.clone()),
        direct: Arc::from([]),
    });
    let token = AnalysisCallToken {
        token: 1,
        stack: context.stack.clone(),
    };
    let subrule_guard = CallFrameGuard {
        token: token.clone(),
    };
    let subrule = module.heap().alloc_simple(AnalysisActions::new(
        context.payload.action_sink.clone(),
        token,
        "subrule context",
        AnalysisActionCallScope::Subrule(identity.clone()),
    ));
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    let root_wrapped = wrap(base, None, false, &mut eval).unwrap();
    let sub_wrapped = wrap(subrule, None, false, &mut eval).unwrap();
    for wrapped in [root_wrapped, sub_wrapped] {
        assert!(actions_to_context(wrapped, &eval).unwrap().ptr_eq(original));
    }
    assert_eq!(
        AnalysisActions::from_starlark(sub_wrapped).unwrap().scope,
        AnalysisActionCallScope::Subrule(identity)
    );
    AnalysisActions::from_starlark(sub_wrapped)
        .unwrap()
        .token
        .require_active("declare_file", "subrule context")
        .unwrap();
    let root_method = root_wrapped
        .get_attr("declare_file", module.heap())
        .unwrap()
        .unwrap();
    let error = eval
        .eval_function(
            root_method,
            &[module.heap().alloc_str("unused").to_value()],
            &[],
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("outside of its own implementation"),
        "{error}"
    );
    drop(subrule_guard);
    let error = wrap(sub_wrapped, None, false, &mut eval).unwrap_err();
    assert!(error.to_string().contains("expired"), "{error}");
    assert!(
        actions_to_context(root_wrapped, &eval)
            .unwrap()
            .ptr_eq(original)
    );
    assert!(sink.0.lock().unwrap().is_empty());
}

#[test]
fn escaped_link_wrapper_methods_reject_after_success_and_error() {
    for fail in [false, true] {
        let module = Module::new();
        let (context, sink) = context();
        let base = actions(&module, &context);
        let mut eval = Evaluator::new(&module);
        eval.extra = Some(&context);
        let result: anyhow::Result<()> = (|| {
            let _guard =
                context.enter_root_context(&module, module.heap().alloc(AllocList([7])))?;
            let wrapped = wrap(base, None, false, &mut eval)?;
            module.set("wrapped", wrapped);
            for (name, method) in [
                ("ordinary", "declare_file"),
                ("shareable", "declare_shareable_artifact"),
                ("directory", "declare_shareable_directory"),
            ] {
                module.set(
                    name,
                    wrapped
                        .get_attr(method, module.heap())
                        .map_err(starlark::Error::into_anyhow)?
                        .unwrap(),
                );
            }
            if fail {
                anyhow::bail!("intentional implementation failure");
            }
            Ok(())
        })();
        assert_eq!(result.is_err(), fail);
        for name in ["ordinary", "shareable", "directory"] {
            let error = eval
                .eval_function(
                    module.get(name).unwrap(),
                    &[module.heap().alloc_str("unused").to_value()],
                    &[],
                )
                .unwrap_err();
            let message = error.to_string();
            assert!(
                message.contains("outside of its own implementation function"),
                "{message}"
            );
            assert!(!message.contains("unsupported/deferred"), "{message}");
        }
        assert!(sink.0.lock().unwrap().is_empty());
    }
}
