//! Evaluator-local ownership, not another analysis context representation.
use starlark::environment::FrozenModule;
use starlark::environment::GlobalsBuilder;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::dict::DictMut;
use starlark::values::list::AllocList;

use super::*;
use crate::package::loading_globals;
use crate::provider::BzlEvaluationContext;

fn context() -> AnalysisEvaluationContext {
    AnalysisEvaluationContext::new(
        Arc::from([]),
        [],
        CanonicalLabel::parse("@@//:subject").unwrap(),
        Arc::new(crate::cc_common::solib_tests::RecordingSink::default()),
        FrozenValue::new_none(),
        Arc::from([]),
    )
}

fn actions<'v>(
    module: &'v Module,
    context: &AnalysisEvaluationContext,
    token: AnalysisCallToken,
) -> Value<'v> {
    module.heap().alloc_simple(AnalysisActions::new(
        context.payload.action_sink.clone(),
        token,
        "rule context",
        AnalysisActionCallScope::Root,
    ))
}

#[starlark_module]
fn recovery_globals(builder: &mut GlobalsBuilder) {
    fn recover<'v>(
        actions: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        actions_to_context(actions, eval)
    }
}

fn internal() -> FrozenModule {
    let module = Module::new();
    let context = BzlEvaluationContext::new("@@rules_cc+//cc/private:context_test.bzl");
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    eval.eval_module(
        AstModule::parse(
            "context_test.bzl",
            "i = cc_common.internal_DO_NOT_USE()".to_owned(),
            &Dialect::Bazel,
        )
        .unwrap(),
        &loading_globals(),
    )
    .unwrap();
    drop(eval);
    module.freeze().unwrap()
}

#[test]
fn recovery_rejects_missing_foreign_and_occupied_registration() {
    let module = Module::new();
    let a = context();
    let b = context();
    let a_actions = actions(&module, &a, a.root_token());
    let b_actions = actions(&module, &b, b.root_token());
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&a);
    assert!(
        actions_to_context(a_actions, &eval)
            .unwrap_err()
            .to_string()
            .contains("registered")
    );
    assert!(
        actions_to_context(Value::new_none(), &eval)
            .unwrap_err()
            .to_string()
            .contains("genuine")
    );
    let original = module.heap().alloc(AllocList([7]));
    let guard = a.enter_root_context(&module, original).unwrap();
    assert!(a.enter_root_context(&module, original).is_err());
    assert!(b.enter_root_context(&module, original).is_err());
    assert!(
        actions_to_context(a_actions, &eval)
            .unwrap()
            .ptr_eq(original)
    );
    assert!(
        actions_to_context(b_actions, &eval)
            .unwrap_err()
            .to_string()
            .contains("another evaluation")
    );
    // Same-number token and evaluator agree, but the registered slot belongs to A.
    eval.extra = Some(&b);
    assert!(
        actions_to_context(b_actions, &eval)
            .unwrap_err()
            .to_string()
            .contains("registration belongs")
    );
    drop(guard);
    assert!(crate::evaluator_roots::analysis_context(&module).is_none());
    eval.extra = Some(&a);
    assert!(
        actions_to_context(a_actions, &eval)
            .unwrap_err()
            .to_string()
            .contains("expired")
    );
    assert!(a.enter_root_context(&module, original).is_err());
}

#[test]
fn suspended_root_and_subrule_frames_recover_one_locked_outer_context() {
    let module = Module::new();
    let context = context();
    let root = context.root_token();
    let root_actions = actions(&module, &context, root.clone());
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
    let subrule_token = AnalysisCallToken {
        token: 1,
        stack: context.stack.clone(),
    };
    let subrule_guard = CallFrameGuard {
        token: subrule_token.clone(),
    };
    let subrule_actions = module.heap().alloc_simple(AnalysisActions::new(
        context.payload.action_sink.clone(),
        subrule_token.clone(),
        "subrule context",
        AnalysisActionCallScope::Subrule(identity),
    ));
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    for actions in [root_actions, subrule_actions] {
        assert!(actions_to_context(actions, &eval).unwrap().ptr_eq(original));
    }
    assert!(root.require_active("label", "rule context").is_err());
    subrule_token
        .require_active("label", "subrule context")
        .unwrap();
    drop(subrule_guard);
    root.require_active("label", "rule context").unwrap();
    assert!(
        actions_to_context(root_actions, &eval)
            .unwrap()
            .ptr_eq(original)
    );
    assert!(
        actions_to_context(subrule_actions, &eval)
            .unwrap_err()
            .to_string()
            .contains("expired")
    );
}

#[test]
fn root_cleanup_runs_on_success_and_error_without_freezing_context() {
    for fail in [false, true] {
        let module = Module::new();
        let context = context();
        let token = context.root_token();
        let result: anyhow::Result<()> = (|| {
            let _guard =
                context.enter_root_context(&module, module.heap().alloc(AllocList([7])))?;
            token.require_active("label", "rule context")?;
            if fail {
                anyhow::bail!("implementation failed");
            }
            Ok(())
        })();
        assert_eq!(result.is_err(), fail);
        assert!(crate::evaluator_roots::analysis_context(&module).is_none());
        assert!(token.require_active("label", "rule context").is_err());
        let frozen = module.freeze().unwrap();
        let roots = TupleRef::from_value(frozen.extra_value().unwrap().to_value()).unwrap();
        assert!(roots.content()[2].is_none());
    }
}

#[test]
fn registered_context_survives_real_gc_with_other_loading_roots() {
    let module = Module::new();
    let context = context();
    // After this statement only the module extra slot roots this moving list.
    let guard = context
        .enter_root_context(&module, module.heap().alloc(AllocList([7])))
        .unwrap();
    module.set("actions", actions(&module, &context, context.root_token()));
    crate::evaluator_roots::set_empty_depsets(
        &module,
        module.heap().alloc(AllocTuple(["empty-depset-root"])),
    );
    {
        let mut registry =
            DictMut::from_value(crate::evaluator_roots::callable_registry(&module)).unwrap();
        registry.aref.insert_hashed(
            module
                .heap()
                .alloc_str("capture")
                .to_value()
                .get_hashed()
                .unwrap(),
            module.heap().alloc(AllocList([42])),
        );
    }
    let globals = GlobalsBuilder::standard().with(recovery_globals).build();
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    eval.eval_module(AstModule::parse("pressure.bzl", "def churn():\n    for i in range(1000):\n        temporary = [str(i)] * 256\nchurn()\ncollected = True\n".to_owned(), &Dialect::Bazel).unwrap(), &globals).unwrap();
    assert!(
        module.heap().allocated_bytes() < module.heap().peak_allocated_bytes(),
        "must actually collect"
    );
    eval.eval_module(
        AstModule::parse(
            "recover.bzl",
            "recover(actions).append(8)\nRESULT = recover(actions)\n".to_owned(),
            &Dialect::Bazel,
        )
        .unwrap(),
        &globals,
    )
    .unwrap();
    assert_eq!(module.get("RESULT").unwrap().to_repr(), "[7, 8]");
    let recovered = actions_to_context(module.get("actions").unwrap(), &eval).unwrap();
    assert!(recovered.ptr_eq(module.get("RESULT").unwrap()));
    assert_eq!(
        crate::evaluator_roots::empty_depsets(&module)
            .unwrap()
            .to_repr(),
        "(\"empty-depset-root\",)"
    );
    assert_eq!(
        crate::evaluator_roots::callable_registry(&module).to_repr(),
        "{\"capture\": [42]}"
    );
    drop(guard);
    assert_eq!(
        crate::evaluator_roots::callable_registry(&module).to_repr(),
        "{\"capture\": [42]}"
    );
    assert!(crate::evaluator_roots::empty_depsets(&module).is_some());
}

#[test]
fn actions2ctx_binding_is_positional_genuine_and_has_no_caller_gate() {
    for (expression, succeeds) in [
        ("i.actions2ctx_cheat(actions)", true),
        ("i.actions2ctx_cheat(actions = actions)", false),
        ("i.actions2ctx_cheat(None)", false),
        ("i.actions2ctx_cheat(struct(actions = actions))", false),
        ("i.actions2ctx_cheat()", false),
        ("i.actions2ctx_cheat(actions, actions)", false),
    ] {
        let module = Module::new();
        module.import_public_symbols(&internal());
        let context = context();
        let original = module.heap().alloc(AllocList([7]));
        let _guard = context.enter_root_context(&module, original).unwrap();
        module.set("original", original);
        module.set("actions", actions(&module, &context, context.root_token()));
        let mut eval = Evaluator::new(&module);
        eval.extra = Some(&context);
        let result = eval.eval_module(
            AstModule::parse(
                "ordinary.bzl",
                format!("RESULT = {expression}"),
                &Dialect::Bazel,
            )
            .unwrap(),
            &loading_globals(),
        );
        assert_eq!(result.is_ok(), succeeds, "{expression}: {result:?}");
        if succeeds {
            assert!(
                module
                    .get("RESULT")
                    .unwrap()
                    .ptr_eq(module.get("original").unwrap())
            );
        }
    }
}
