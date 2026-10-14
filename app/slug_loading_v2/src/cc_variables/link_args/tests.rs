//! Pinned LinkCommandLine/FrozenArgs behavior through the genuine private binder.
use slug_build_api_v2::RetainedArgCall;
use slug_build_api_v2::RetainedArgsRecipe;
use slug_build_api_v2::cc_features::*;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;
use crate::package::loading_globals;
use crate::provider::BzlEvaluationContext;

fn execute(module: &Module, source: &str) -> anyhow::Result<()> {
    Evaluator::new(module)
        .eval_module(
            AstModule::parse("ordinary.bzl", source.to_owned(), &Dialect::Bazel)
                .map_err(starlark::Error::into_anyhow)?,
            &loading_globals(),
        )
        .map_err(starlark::Error::into_anyhow)?;
    Ok(())
}
fn fixture(flags: &[&str], iterate: Option<&str>) -> Module {
    let origin = Module::new();
    let context = BzlEvaluationContext::new("@@rules_cc+//cc/private:link_args_test.bzl");
    let mut eval = Evaluator::new(&origin);
    eval.extra = Some(&context);
    eval.eval_module(
        AstModule::parse(
            "source.bzl",
            "i = cc_common.internal_DO_NOT_USE()\n".to_owned(),
            &Dialect::Bazel,
        )
        .unwrap(),
        &loading_globals(),
    )
    .unwrap();
    drop(eval);
    let imported = origin.freeze().unwrap();
    let module = Module::new();
    module.import_public_symbols(&imported);
    let feature = CcFeature {
        name: "flags".into(),
        enabled: true,
        flag_sets: vec![CcFlagSet {
            actions: [CompactString::new("archive")].into_iter().collect(),
            with_features: CcSet::new(),
            flag_groups: vec![CcFlagGroup {
                expandables: flags
                    .iter()
                    .map(|flag| CcExpandable::Flag(parse_cc_string(flag).unwrap().into()))
                    .collect(),
                iterate_over: iterate.map(Into::into),
                expand_if_available: None,
                expand_if_not_available: None,
                expand_if_true: None,
                expand_if_false: None,
                expand_if_equal: None,
            }]
            .into(),
        }]
        .into(),
        env_sets: Box::new([]),
        requires: Box::new([]),
        implies: Box::new([]),
        provides: Box::new([]),
    };
    let config = CcToolchainFeatures::new(vec![feature], vec![], vec![], "tools", CcExecOs::Linux)
        .unwrap()
        .configure_features(vec!["flags".into()])
        .unwrap();
    module.set(
        "fc",
        crate::cc_features::alloc_frozen_cc_feature_configuration(module.frozen_heap(), config)
            .to_value(),
    );
    execute(&module, "vars = i.cc_toolchain_variables(vars = {})").unwrap();
    module
}
fn create(module: &Module, format: &str) -> anyhow::Result<()> {
    execute(
        module,
        &format!(
            "out = i.get_link_args(action_name = 'archive', feature_configuration = fc, build_variables = vars, parameter_file_type = {format})"
        ),
    )
}
fn snapshot<'v>(value: Value<'v>, heap: Heap<'v>) -> anyhow::Result<EvaluatorArgsSnapshot<'v>> {
    let adapted = prepare(value, heap)?;
    Ok(StarlarkArgs::snapshot(adapted).unwrap())
}
fn render<'v>(value: Value<'v>, heap: Heap<'v>) -> anyhow::Result<Vec<String>> {
    let snapshot = snapshot(value, heap)?;
    let calls = snapshot
        .calls
        .into_iter()
        .map(|call| match call {
            EvaluatorArgCallGen::Scalar(scalar) => RetainedArgCall::Scalar(scalar),
            _ => panic!("link arguments must lower to scalar recipes"),
        })
        .collect::<Vec<_>>();
    Ok(RetainedArgsRecipe::new(calls, snapshot.format).render()?)
}

#[test]
fn link_args_requires_four_named_genuine_inputs_and_refuses_other_formats() {
    let module = fixture(&["kept"], None);
    for call in [
        "i.get_link_args()",
        "i.get_link_args(feature_configuration=fc, build_variables=vars, parameter_file_type=None)",
        "i.get_link_args(action_name='archive', build_variables=vars, parameter_file_type=None)",
        "i.get_link_args(action_name='archive', feature_configuration=fc, parameter_file_type=None)",
        "i.get_link_args('archive', fc, vars, None)",
        "i.get_link_args(action_name='archive', feature_configuration=fc, build_variables=vars)",
        "i.get_link_args(action_name=1, feature_configuration=fc, build_variables=vars, parameter_file_type=None)",
        "i.get_link_args(action_name='archive', feature_configuration=struct(), build_variables=vars, parameter_file_type=None)",
        "i.get_link_args(action_name='archive', feature_configuration=fc, build_variables={}, parameter_file_type=None)",
    ] {
        assert!(execute(&module, call).is_err(), "{call}");
    }
    for format in ["True", "'invalid'", "'gcc_quoted'"] {
        assert!(create(&module, format).is_err());
    }
    for format in ["'WINDOWS'"] {
        let error = create(&module, format).unwrap_err();
        assert!(
            error.to_string().contains("unsupported/deferred"),
            "{error}"
        );
    }
    create(&module, "None").unwrap();
    let out = module.get("out").unwrap();
    assert!(is_link_args(out));
    assert_eq!(out.get_type(), "Args");
    assert_eq!(render(out, module.heap()).unwrap(), ["kept"]);
    let ordinary = module.heap().alloc_str("untouched").to_value();
    assert!(prepare(ordinary, module.heap()).unwrap().ptr_eq(ordinary));
}

#[test]
fn link_args_none_is_lazy_and_string_mode_expands_again_on_consumption() {
    let module = fixture(&["%{items}"], Some("items"));
    create(&module, "None").unwrap();
    assert!(render(module.get("out").unwrap(), module.heap()).is_err());
    for mode in ["'UNQUOTED'", "'GCC_QUOTED'"] {
        assert!(create(&module, mode).is_err());
    }
    execute(
        &module,
        "items = ['first']\nvars = i.cc_toolchain_variables(vars={'items': items})",
    )
    .unwrap();
    for mode in ["None", "'UNQUOTED'", "'GCC_QUOTED'"] {
        create(&module, mode).unwrap();
        execute(&module, "items.append('next')").unwrap();
        let expected = module.get("items").unwrap().to_repr();
        let rendered = render(module.get("out").unwrap(), module.heap()).unwrap();
        assert_eq!(format!("{rendered:?}"), expected);
        assert_eq!(
            render(module.get("out").unwrap(), module.heap()).unwrap(),
            rendered
        );
        execute(&module, "items.pop()").unwrap();
    }
}

#[test]
fn link_args_parameter_policy_escapes_percent_filters_all_occurrences_and_defaults_shell() {
    let module = fixture(
        &[
            "%%prefix%{linker_param_file}",
            "%{linker_param_file}-again",
            "keep",
        ],
        None,
    );
    execute(&module, "vars = i.cc_toolchain_variables(vars={'linker_param_file':'LINKER_PARAM_FILE_PLACEHOLDER'})").unwrap();
    for (mode, format) in [
        ("'UNQUOTED'", RetainedParamFileFormat::Multiline),
        ("'SHELL_QUOTED'", RetainedParamFileFormat::Shell),
        ("'GCC_QUOTED'", RetainedParamFileFormat::Gcc),
    ] {
        create(&module, mode).unwrap();
        let out = module.get("out").unwrap();
        let captured = snapshot(out, module.heap()).unwrap();
        assert_eq!(captured.format, format);
        assert_eq!(captured.param_file, Some(("%%prefix%s".into(), true)));
        assert_eq!(render(out, module.heap()).unwrap(), ["keep"]);
    }
    create(&module, "None").unwrap();
    let captured = snapshot(module.get("out").unwrap(), module.heap()).unwrap();
    assert!(captured.param_file.is_none());
    assert_eq!(captured.format, RetainedParamFileFormat::Shell);
    let no_placeholder = fixture(&["keep"], None);
    for mode in ["'UNQUOTED'", "'GCC_QUOTED'"] {
        create(&no_placeholder, mode).unwrap();
        let captured = snapshot(no_placeholder.get("out").unwrap(), no_placeholder.heap()).unwrap();
        assert!(captured.param_file.is_none());
        assert_eq!(captured.format, RetainedParamFileFormat::Shell);
    }
    let multiple = fixture(&["%{linker_param_file}/%{linker_param_file}"], None);
    execute(&multiple, "vars = i.cc_toolchain_variables(vars={'linker_param_file':'LINKER_PARAM_FILE_PLACEHOLDER'})").unwrap();
    for mode in ["'UNQUOTED'", "'GCC_QUOTED'"] {
        let error = create(&multiple, mode).unwrap_err();
        assert!(error.to_string().contains("exactly one %s"), "{error}");
    }
}

#[test]
fn link_args_param_lookup_precedes_expansion_and_empty_boolean_filters_use_existing_conversion() {
    let module = fixture(&["%{missing}"], None);
    execute(
        &module,
        "vars = i.cc_toolchain_variables(vars={'linker_param_file': []})",
    )
    .unwrap();
    create(&module, "None").unwrap();
    let error = render(module.get("out").unwrap(), module.heap()).unwrap_err();
    assert!(error.to_string().contains("linker_param_file"), "{error}");
    assert!(!error.to_string().contains("named 'missing'"), "{error}");
    for (raw, expected) in [
        ("''", vec![]),
        ("True", vec!["zero0".to_owned(), "keep".to_owned()]),
        ("False", vec!["one1".to_owned(), "keep".to_owned()]),
    ] {
        let module = fixture(&["one1", "zero0", "keep"], None);
        execute(
            &module,
            &format!("vars=i.cc_toolchain_variables(vars={{'linker_param_file':{raw}}})"),
        )
        .unwrap();
        create(&module, "None").unwrap();
        assert_eq!(
            render(module.get("out").unwrap(), module.heap()).unwrap(),
            expected
        );
    }
    let file = crate::cc_common::solib_tests::artifact();
    let path = file.path().into_owned();
    let module = fixture(&[path.as_str(), "keep"], None);
    module.set(
        "file",
        module
            .heap()
            .alloc_simple(crate::subrule_invocation::AnalysisArtifactValue::new(file)),
    );
    execute(
        &module,
        "vars=i.cc_toolchain_variables(vars={'linker_param_file':file})",
    )
    .unwrap();
    create(&module, "None").unwrap();
    assert_eq!(
        render(module.get("out").unwrap(), module.heap()).unwrap(),
        ["keep"]
    );
}

#[test]
fn link_args_rejects_all_mutations_and_hashing_then_freezes_original_variable_owner() {
    let module = fixture(&["%{items}"], Some("items"));
    execute(
        &module,
        "items=['before']\nvars=i.cc_toolchain_variables(vars={'items':items})",
    )
    .unwrap();
    create(&module, "None").unwrap();
    for call in [
        "out.add('x')",
        "out.add_all(['x'])",
        "out.add_joined(['x'], join_with=',')",
        "out.use_param_file('@%s')",
        "out.set_param_file_format('multiline')",
    ] {
        let error = execute(&module, call).unwrap_err();
        assert!(
            error.to_string().contains("cannot modify frozen value"),
            "{error}"
        );
    }
    assert!(module.get("out").unwrap().get_hashed().is_err());
    execute(
        &module,
        "items.append('after')\nout = out\nitems = items\nvars = vars",
    )
    .unwrap();
    let published = module.freeze().unwrap();
    let consumer = Module::new();
    consumer.import_public_symbols(&published);
    execute(&consumer, "frozen_args = out\nfrozen_items = items").unwrap();
    let out = consumer.get("frozen_args").unwrap();
    assert!(is_link_args(out));
    assert_eq!(render(out, consumer.heap()).unwrap(), ["before", "after"]);
    assert_eq!(render(out, consumer.heap()).unwrap(), ["before", "after"]);
    assert!(out.get_hashed().is_err());
    assert!(
        consumer
            .get("frozen_items")
            .unwrap()
            .unpack_frozen()
            .is_some()
    );
}

#[test]
fn link_args_traces_original_variables_through_real_gc() {
    let module = fixture(&["%{items}"], Some("items"));
    execute(
        &module,
        "items=['rooted']\nvars=i.cc_toolchain_variables(vars={'items':items})",
    )
    .unwrap();
    create(&module, "None").unwrap();
    // Only LinkArgs now retains the original moving variables/list objects.
    execute(&module, "items=None\nvars=None").unwrap();
    // Reuse the initialized evaluator pressure pattern from context_lifecycle_tests.
    // No Rust Value or snapshot survives across automatic collection.
    execute(&module, "def churn():\n    for i in range(1000):\n        temporary = [str(i)] * 256\nchurn()\ncollected = True\n").unwrap();
    assert!(
        module.heap().allocated_bytes() < module.heap().peak_allocated_bytes(),
        "must actually collect"
    );
    assert_eq!(
        render(module.get("out").unwrap(), module.heap()).unwrap(),
        ["rooted"]
    );
}
