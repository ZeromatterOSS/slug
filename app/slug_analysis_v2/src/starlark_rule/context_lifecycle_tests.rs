//! The actual rule ctx and its action handles expire at invocation exit.
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_identity_v2::CanonicalLabel;
use starlark::environment::Globals;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;

fn configuration() -> crate::ConfigurationKey {
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap()
    .with_action_environment_host(
        slug_configuration_v2::native::host::ActionEnvironmentHost::without_environment(
            slug_configuration_v2::native::host::ActionEnvironmentHostOs::Linux,
        ),
    );
    crate::ConfigurationKey::from_slug(SlugConfiguration::default_target(&host).unwrap())
}

fn declaration_sink(key: &ConfiguredTargetKey) -> Arc<SynchronousAnalysisActionSink> {
    let root = DerivedArtifactRoot::bin(key.configuration().slug_configuration().unwrap().clone())
        .unwrap();
    let context = Arc::new(ConfiguredActionOwnerContext::unresolved_default(key.clone()).unwrap());
    let group = crate::execution_groups::ConfiguredExecGroupRow::new(
        ConfiguredExecGroup::Default,
        Arc::from([]),
        Arc::from([]),
        context,
    )
    .unwrap();
    Arc::new(SynchronousAnalysisActionSink {
        root: root.clone(),
        actions: Arc::new(Mutex::new(CtxActions::new(root))),
        declared_outputs: Mutex::new(SmallMap::new()),
        package_path: "pkg".to_owned(),
        owner: key.artifact_owner(),
        typed_configuration: Ok(None),
        executable_provenance: Arc::new(ExecutableArtifactProvenance::default()),
        execution_tags: CanonicalStringMap::default(),
        target_package: Arc::new(RunfilesPackageMetadata::new(
            key.label().package().clone(),
            Arc::new(slug_build_api_v2::RunfilesRepositoryMapping::empty()),
        )),
        exec_groups: ConfiguredExecGroupCollection::new(
            vec![group],
            Arc::from([]),
            crate::result::ToolchainTopology::new(Vec::new(), None).unwrap(),
            false,
        )
        .unwrap(),
    })
}

fn check_rule_context_teardown(fail: bool) {
    let module = Module::new();
    let key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//pkg:subject").unwrap(),
        configuration(),
    );
    let declarations = declaration_sink(&key);
    let sink: Arc<dyn AnalysisActionSink> = declarations.clone();
    let evaluation = AnalysisEvaluationContext::new(
        Arc::from([]),
        [],
        key.label().clone(),
        sink.clone(),
        FrozenValue::new_none(),
        Arc::from([]),
    );
    let token = evaluation.root_token();
    let file = AnalysisValue::artifact(AnalysisArtifact::Source(
        CanonicalLabel::parse("@@//pkg:input.h").unwrap(),
    ));
    let (attribute_file, attribute_files) = {
        let mut materializer = AnalysisValueMaterializer::new(module.frozen_heap());
        (
            materializer
                .value(&AnalysisValue::strukt([
                    ("input", file.clone()),
                    ("optional", AnalysisValue::none()),
                ]))
                .unwrap(),
            materializer
                .value(&AnalysisValue::strukt([
                    ("input", AnalysisValue::list(vec![file])),
                    ("optional", AnalysisValue::list(Vec::new())),
                ]))
                .unwrap(),
        )
    };
    let ctx = module.heap().alloc(AnalysisContextGen {
        workspace_status: Arc::new(slug_build_api_v2::WorkspaceStatusArtifacts::new(
            slug_workspace_v2::NormalizedAbsolutePath::new("/fixture/workspace").unwrap(),
        )),
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
        attribute_file,
        attribute_files,
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
                "def implementation(ctx):\n    saved.append(ctx.actions)\n    captured.extend([ctx.file, ctx.files, ctx.files.input, ctx.info_file, ctx.version_file])\n    {}\ndef mutate():\n    captured[2].append(None)\nsaved = []\ncaptured = []\n",
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
    let capability = ctx
        .request_value::<&dyn slug_loading_v2::cc_context::CcCompileOutputContext>()
        .unwrap();
    let config = key.configuration().slug_configuration().unwrap();
    let compile = ctx
        .request_value::<&dyn slug_loading_v2::cc_context::CcCompileActionContext>()
        .unwrap();
    let foreign = config.to_exec().unwrap();
    // These deliberately invalid later fields must never be inspected: the
    // genuine capability validates its active token and supplied root first.
    let mut compile_request = slug_loading_v2::cc_compile_action::CcCompileRequest {
        configuration: &foreign,
        compilation_context: Value::new_none(),
        toolchain: Value::new_none(),
        features: Value::new_none(),
        variables: Value::new_none(),
        source: Value::new_none(),
        output: Value::new_none(),
        dotd: Value::new_none(),
        additional_inputs: None,
        additional_inputs_set: None,
        cache_key_inputs: None,
        build_info_headers: None,
        action_name: None,
        should_scan_includes: None,
        use_pic: false,
        needs_include_validation: true,
        toolchain_type: "//tools:cpp",
    };
    let error = compile
        .prepare_cc_compile(&compile_request)
        .err()
        .expect("foreign root rejected");
    assert!(
        error
            .to_string()
            .contains("must equal the active ctx output root configuration"),
        "{error}"
    );
    compile_request.configuration = config;
    let location = CanonicalLabel::parse("@@//pkg/child:other/part").unwrap();
    // A supplied configuration is not silently replaced by the active root.
    // Rejection precedes all declaration state, including directory conflicts.
    let error = capability
        .declare_compile_output(&location, "out.o", &config.to_exec().unwrap())
        .unwrap_err();
    assert!(error.to_string().contains("must equal"), "{error}");
    assert!(declarations.declared_outputs.lock().unwrap().is_empty());
    for invalid in [
        "../../../../../escape.o",
        "/absolute.o",
        "back\\slash.o",
        "nul\0byte.o",
    ] {
        assert!(
            capability
                .declare_compile_output(&location, invalid, config)
                .is_err()
        );
        assert!(declarations.declared_outputs.lock().unwrap().is_empty());
    }
    let output = capability
        .declare_compile_output(&location, "tmp/../out.o", config)
        .unwrap();
    let repeated = capability
        .declare_compile_output(&location, "out.o", config)
        .unwrap();
    assert_eq!(output.artifact(), repeated.artifact());
    let AnalysisArtifact::Derived {
        owner,
        output: file,
    } = output.artifact()
    else {
        panic!("derived File")
    };
    assert_eq!(owner, &key.artifact_owner());
    assert_eq!(file.root().configuration(), config);
    assert_eq!(
        file.root_relative_path(),
        "pkg/child/_objs/other/part/out.o"
    );
    assert_eq!(file.kind(), ActionOutputKind::File);
    assert_eq!(declarations.declared_outputs.lock().unwrap().len(), 1);
    // Native declarations and ordinary ctx.actions share kind/conflict authority.
    declarations
        .declare_directory("child/_objs/other/part/tree.o")
        .unwrap();
    assert!(
        capability
            .declare_compile_output(&location, "tree.o", config)
            .unwrap_err()
            .to_string()
            .contains("different type")
    );
    assert_eq!(declarations.declared_outputs.lock().unwrap().len(), 2);
    assert!(
        declarations
            .actions
            .lock()
            .unwrap()
            .registry()
            .actions()
            .is_empty()
    );
    let bin = ctx.get_attr("bin_dir", module.heap()).unwrap().unwrap();
    assert!(slug_loading_v2::artifact_root_value(bin).is_some());
    for field in ["info_file", "version_file"] {
        let file = ctx.get_attr(field, module.heap()).unwrap().unwrap();
        assert!(
            eval.eval_function(file, &[], &[]).is_err(),
            "File is not callable"
        );
    }
    let result = eval.eval_function(module.get("implementation").unwrap(), &[ctx], &[]);
    assert_eq!(result.is_err(), fail);
    if let Ok(label) = result {
        assert_eq!(label.to_str(), "@@//pkg:subject");
    }
    drop(guard);
    for field in ["info_file", "version_file"] {
        assert!(
            ctx.get_attr(field, module.heap()).unwrap().is_none(),
            "expired ctx retrieval must fail"
        );
    }
    let error = compile
        .prepare_cc_compile(&compile_request)
        .err()
        .expect("expired context rejected");
    assert!(
        error
            .to_string()
            .contains("outside of its own implementation function"),
        "{error}"
    );
    let before = declarations.declared_outputs.lock().unwrap().len();
    let error = capability
        .declare_compile_output(&location, "expired.o", config)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("outside of its own implementation")
    );
    assert_eq!(declarations.declared_outputs.lock().unwrap().len(), before);
    let ctx = module.get("original").unwrap();
    assert!(token.require_active("label", "rule context").is_err());
    assert!(ctx.get_attr("label", module.heap()).unwrap().is_none());
    assert!(ctx.get_attr("actions", module.heap()).unwrap().is_none());
    assert!(ctx.get_attr("file", module.heap()).unwrap().is_none());
    assert!(ctx.get_attr("files", module.heap()).unwrap().is_none());
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

    // Captured file collections are ordinary data, not expired capabilities.
    let captured = module.get("captured").unwrap();
    let values = ListRef::from_value(captured).unwrap();
    let file = values.content()[0]
        .get_attr("input", module.heap())
        .unwrap()
        .unwrap();
    assert_eq!(
        AnalysisArtifactValue::from_starlark(file)
            .unwrap()
            .artifact(),
        &AnalysisArtifact::Source(CanonicalLabel::parse("@@//pkg:input.h").unwrap())
    );
    assert!(
        values.content()[0]
            .get_attr("optional", module.heap())
            .unwrap()
            .unwrap()
            .is_none()
    );
    assert!(
        values.content()[1]
            .get_attr("input", module.heap())
            .unwrap()
            .unwrap()
            .ptr_eq(values.content()[2])
    );
    assert!(
        eval.eval_function(module.get("mutate").unwrap(), &[], &[])
            .is_err()
    );
    let retained = AnalysisValueLowerer::from_module(&module)
        .lower(captured, "captured attribute files")
        .unwrap();
    drop(eval);
    // Freeze the producer, then dispose it: the retained publication owns all
    // Files independently, without retaining the inactive ctx or its token.
    drop(module.freeze().unwrap());
    let restored = Module::new();
    let value = AnalysisValueMaterializer::new(restored.frozen_heap())
        .value(&retained)
        .unwrap();
    restored.set("captured", value.to_value());
    let frozen = restored.freeze().unwrap();
    let mut gc = starlark::assert::Assert::new();
    gc.disable_static_typechecking();
    gc.module_add("retained", frozen);
    gc.pass(
        r#"
load("retained", "captured")
garbage_collect()
assert_eq(type(captured[0]), "struct")
assert_eq(type(captured[1]), "struct")
assert_eq(captured[0].input, captured[1].input[0])
assert_eq(captured[0].input.path, "pkg/input.h")
assert_eq(captured[0].optional, None)
assert_eq(captured[1].optional, [])
assert_eq(captured[2], captured[1].input)
assert_eq(dir(captured[0]), ["input", "optional"])
assert_eq(dir(captured[1]), ["input", "optional"])
assert_eq(captured[3].path, "bazel-out/stable-status.txt")
assert_eq(captured[4].path, "bazel-out/volatile-status.txt")
assert_eq(captured[3].owner, None)
assert_eq(captured[4].owner, None)
assert_eq(captured[3].root, captured[4].root)
assert_eq(captured[3] == captured[4], False)
assert_eq(len({captured[3]: 1, captured[4]: 2}), 2)
"#,
    );
}

#[test]
fn real_rule_context_and_escaped_actions_expire_after_success() {
    check_rule_context_teardown(false);
}

#[test]
fn real_rule_context_and_escaped_actions_expire_after_error() {
    check_rule_context_teardown(true);
}
