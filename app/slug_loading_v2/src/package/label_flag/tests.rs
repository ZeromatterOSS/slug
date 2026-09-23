//! Pinned Bazel 9.2 LabelBuildSettings / LabelBuildSettingTest declaration cases.

use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;

fn evaluate_in(
    source: &str,
    repo: CanonicalRepoName,
    mapping: &[(ApparentRepoName, CanonicalRepoName)],
) -> anyhow::Result<LegacyLoadedPackage> {
    let mut recorder =
        PackageRecorder::new(PackageListing::new(vec![], vec![], vec![], vec![]), "pkg");
    recorder.package_identifier = PackageIdentifier::new(repo, PackagePath::parse("pkg").unwrap());
    recorder.repository_mapping = PackageRecorderRepositoryMapping::Legacy(mapping.into());
    let module = Module::new();
    // A genuine Label supplied by a loaded .bzl is accepted alongside strings.
    module.set(
        "owned_label",
        module
            .heap()
            .alloc(crate::starlark_label::StarlarkLabel::new(
                CanonicalLabel::parse("@@dep+//other:owned").unwrap(),
            )),
    );
    {
        let mut eval = Evaluator::new(&module);
        eval.extra = Some(&recorder);
        eval.eval_module(
            AstModule::parse("pkg/BUILD.bazel", source.to_owned(), &Dialect::Standard)
                .map_err(starlark::Error::into_anyhow)?,
            &build_file_loading_globals(),
        )
        .map_err(starlark::Error::into_anyhow)?;
    }
    // No live Starlark object is part of the declaration or its schema snapshot.
    drop(module);
    Ok(recorder.finish_legacy(
        "/workspace/pkg".into(),
        "/workspace/pkg/BUILD.bazel".into(),
        Arc::from([]),
        Arc::from([]),
        [0; 32],
        Arc::from([]),
    ))
}

fn evaluate(source: &str) -> anyhow::Result<LegacyLoadedPackage> {
    evaluate_in(source, CanonicalRepoName::root(), &[])
}

fn flag(package: &LegacyLoadedPackage, index: usize) -> &LabelFlagDeclaration {
    let PackageTargetKind::LabelFlag(flag) = &package.targets[index].kind else {
        panic!("genuine native label_flag")
    };
    flag
}

#[test]
fn label_flag_globals_preserve_native_schema_and_scope_provenance() {
    let package = evaluate(
        "label_flag(name='a', build_setting_default=':input')\n\
         native.label_flag(name='b', build_setting_default=':input', scope='universal', help='choose input')\n",
    ).unwrap();
    assert_eq!(flag(&package, 0).default().to_string(), "@@//pkg:input");
    assert_eq!(flag(&package, 0).scope(), BuildSettingScope::Default);
    assert_eq!(flag(&package, 1).scope(), BuildSettingScope::Universal);
    assert_eq!(flag(&package, 1).help(), "choose input");
    assert_eq!(
        package.targets[0].kind.build_setting_declaration().unwrap(),
        Some(flag(&package, 0).declaration().clone()),
    );
    assert_eq!(
        flag(&package, 0).declaration().definition(),
        BuildSettingDefinition::Label { flag: true }
    );
    assert_eq!(
        package.targets[0].rule_capability().unwrap().rule_class,
        "label_flag"
    );
    let attributes = package.native_attributes_at(0).unwrap();
    let rows = attributes.iter().collect::<Vec<_>>();
    assert_eq!(rows.len(), 20);
    assert_eq!(
        rows[16..]
            .iter()
            .map(|(schema, _)| schema.query_name())
            .collect::<Vec<_>>(),
        [":alias", "scope", "build_setting_default", "help"]
    );
    assert_eq!(rows[16].1.provenance, AttributeProvenance::Implicit);
    assert_eq!(rows[17].1.provenance, AttributeProvenance::Default);
    assert_eq!(rows[18].1.provenance, AttributeProvenance::Explicit);
    assert_eq!(rows[19].1.provenance, AttributeProvenance::Default);
    let second = package
        .native_attributes_at(1)
        .unwrap()
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(second[17].1.provenance, AttributeProvenance::Explicit);
    assert_eq!(second[19].1.provenance, AttributeProvenance::Explicit);
}

#[test]
fn label_flag_defaults_use_declaration_repository_and_canonical_value_identity() {
    let dep = CanonicalRepoName::new("dep+").unwrap();
    let mapping = [
        (ApparentRepoName::new("dep").unwrap(), dep.clone()),
        (ApparentRepoName::new("other_name").unwrap(), dep.clone()),
    ];
    let package = evaluate_in(
        "label_flag(name='a', build_setting_default=':input')\n\
         label_flag(name='b', build_setting_default='//other:input')\n\
         label_flag(name='c', build_setting_default='@dep//other:owned')\n\
         label_flag(name='d', build_setting_default=owned_label)\n\
         label_flag(name='e', build_setting_default='@other_name//other:owned')\n",
        dep,
        &mapping,
    )
    .unwrap();
    assert_eq!(flag(&package, 0).default().to_string(), "@@dep+//pkg:input");
    assert_eq!(
        flag(&package, 1).default().to_string(),
        "@@dep+//other:input"
    );
    assert_eq!(flag(&package, 2), flag(&package, 3));
    assert_eq!(flag(&package, 2), flag(&package, 4));
    assert!(flag(&package, 2).default().mapping_id().is_none());
    let missing =
        evaluate("label_flag(name='a', build_setting_default='@invisible//:x')").unwrap_err();
    assert!(
        missing
            .to_string()
            .contains("no repository visible as '@invisible'"),
        "{missing:#}"
    );
}

#[test]
fn label_flag_binding_rejects_missing_configurable_invalid_and_unknown_attributes() {
    for source in [
        "label_flag(name='a')",
        "label_flag('a', ':input')",
        "label_flag(name='a', build_setting_default=None)",
        "label_flag(name='a', build_setting_default=42)",
        "label_flag(name='a', build_setting_default=[':input'])",
        "label_flag(name='a', build_setting_default=select({'//conditions:default': ':input'}))",
        "label_flag(name='a', build_setting_default=':input', scope=select({'//conditions:default': 'universal'}))",
        "label_flag(name='a', build_setting_default=':input', scope='target')",
        "label_flag(name='a', build_setting_default=':input', scope='project')",
        "label_flag(name='a', build_setting_default=':input', scope='Universal')",
        "label_flag(name='a', build_setting_default=':input', help=7)",
        "label_flag(name='a', build_setting_default=':input', help=select({'//conditions:default': 'x'}))",
        "label_flag(name='a', build_setting_default=':input', build_setting_help='wrong')",
        "label_flag(name='a', build_setting_default=':input', **{':alias': ':other'})",
        "label_flag(name='a', build_setting_default=':input', unknown=True)",
        "label_setting(name='a', build_setting_default=':input')",
    ] {
        assert!(evaluate(source).is_err(), "unexpectedly admitted: {source}");
    }
}

#[test]
fn label_flag_retains_package_defaults_generator_and_full_declaration_equality() {
    let package = evaluate(
        "package(default_visibility=['//visibility:public'], default_testonly=True)\n\
         def declare(name):\n    native.label_flag(name=name, build_setting_default=':input')\n\
         declare(name='a')\n",
    )
    .unwrap();
    assert_eq!(
        package.effective_visibility(&package.targets[0]),
        Some(RuleVisibility::Public)
    );
    let attrs = package.native_attributes_at(0).unwrap();
    let value = |name| {
        attrs
            .iter()
            .find(|(schema, _)| schema.query_name() == name)
            .unwrap()
            .1
    };
    assert_eq!(
        value("testonly").value,
        CoercedAttributeValue::Boolean(true)
    );
    assert_eq!(
        value("generator_function").value,
        CoercedAttributeValue::String("declare".into())
    );
    let omitted = evaluate("label_flag(name='a', build_setting_default=':input')").unwrap();
    let same = evaluate("label_flag(name='a', build_setting_default='//pkg:input')").unwrap();
    assert_eq!(omitted, same);
    let none =
        evaluate("label_flag(name='a', build_setting_default=':input', help=None, scope=None)")
            .unwrap();
    assert_eq!(omitted, none);
    for extra in [", help=''", ", help='other'", ", scope='universal'"] {
        let changed = evaluate(&format!(
            "label_flag(name='a', build_setting_default=':input'{extra})"
        ))
        .unwrap();
        assert_ne!(omitted, changed);
    }
    assert_eq!(
        PackageTargetKind::ExportedFile
            .build_setting_declaration()
            .unwrap(),
        None
    );
}
