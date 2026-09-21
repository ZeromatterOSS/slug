//! The actual rule ctx and its action handles expire at invocation exit.
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_identity_v2::CanonicalLabel;
use starlark::environment::Globals;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;

#[derive(Debug)]
struct UnusedSink;
impl AnalysisActionSink for UnusedSink {
    fn declare_file(&self, _: &str) -> anyhow::Result<AnalysisArtifactValue> {
        unreachable!()
    }
    fn write(&self, _: Value<'_>, _: Value<'_>, _: bool) -> anyhow::Result<()> {
        unreachable!()
    }
    fn run_shell(&self, _: Value<'_>, _: &str, _: Value<'_>) -> anyhow::Result<()> {
        unreachable!()
    }
    fn run(&self, _: AnalysisRunRequest<'_>) -> anyhow::Result<()> {
        unreachable!()
    }
    fn artifact_symlink(
        &self,
        _: Value<'_>,
        _: Value<'_>,
        _: bool,
        _: Option<&str>,
    ) -> anyhow::Result<()> {
        unreachable!()
    }
    fn absolute_symlink(&self, _: Value<'_>, _: &str, _: Option<&str>) -> anyhow::Result<()> {
        unreachable!()
    }
}

fn configuration() -> crate::ConfigurationKey {
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap();
    crate::ConfigurationKey::from_slug(SlugConfiguration::default_target(&host).unwrap())
}

fn check_rule_context_teardown(fail: bool) {
    let module = Module::new();
    let key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//pkg:subject").unwrap(),
        configuration(),
    );
    let sink: Arc<dyn AnalysisActionSink> = Arc::new(UnusedSink);
    let evaluation = AnalysisEvaluationContext::new(
        Arc::from([]),
        [],
        key.label().clone(),
        sink.clone(),
        FrozenValue::new_none(),
        Arc::from([]),
    );
    let token = evaluation.root_token();
    let ctx = module.heap().alloc(AnalysisContextGen {
        action_sink: sink,
        token: token.clone(),
        retained_owner: key.artifact_owner(),
        output_root: DerivedArtifactRoot::bin(
            key.configuration().slug_configuration().unwrap().clone(),
        )
        .unwrap(),
        cc_context: CcContextProjection::unconfigured(Arc::new(
            ConfiguredActionOwnerContext::unresolved_default(key.clone()).unwrap(),
        )),
        target_label: key.label().clone(),
        package_path: "pkg".to_owned(),
        dependencies: Arc::from([]),
        executables: Arc::from([]),
        resolved_attributes: Arc::from([]),
        predeclared_outputs: Arc::from([]),
        build_setting_value: None,
        fragments: Value::new_none(),
        toolchain: None,
        exec_groups: None,
    });
    module.set("original", ctx);
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&evaluation);
    eval.eval_module(
        AstModule::parse(
            "context.bzl",
            format!(
                "def implementation(ctx):\n    saved.append(ctx.actions)\n    {}\nsaved = []\n",
                if fail {
                    "fail('intentional failure')"
                } else {
                    "return ctx.label"
                }
            ),
            &Dialect::Bazel,
        )
        .unwrap(),
        &Globals::standard(),
    )
    .unwrap();
    let guard = evaluation.enter_root_context(&module, ctx).unwrap();
    let bin = ctx.get_attr("bin_dir", module.heap()).unwrap().unwrap();
    assert!(slug_loading_v2::artifact_root_value(bin).is_some());
    let result = eval.eval_function(module.get("implementation").unwrap(), &[ctx], &[]);
    assert_eq!(result.is_err(), fail);
    if let Ok(label) = result {
        assert_eq!(label.to_str(), "@@//pkg:subject");
    }
    drop(guard);
    let ctx = module.get("original").unwrap();
    assert!(token.require_active("label", "rule context").is_err());
    assert!(ctx.get_attr("label", module.heap()).unwrap().is_none());
    assert!(ctx.get_attr("actions", module.heap()).unwrap().is_none());
    assert!(ctx.get_attr("bin_dir", module.heap()).unwrap().is_none());
    assert!(
        ctx.get_attr("genfiles_dir", module.heap())
            .unwrap()
            .is_none()
    );
    let saved = ListRef::from_value(module.get("saved").unwrap()).unwrap();
    let actions = AnalysisActions::from_value(saved.content()[0]).unwrap();
    let error = actions
        .register_absolute_symlink(Value::new_none(), "unused", None)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("outside of its own implementation function"),
        "{error}"
    );
    // The old ctx remains the exact object, merely inactive; cleanup must not
    // replace or freeze it to make escaped accesses fail.
    assert!(module.get("original").unwrap().ptr_eq(ctx));
}

#[test]
fn real_rule_context_and_escaped_actions_expire_after_success() {
    check_rule_context_teardown(false);
}

#[test]
fn real_rule_context_and_escaped_actions_expire_after_error() {
    check_rule_context_teardown(true);
}
