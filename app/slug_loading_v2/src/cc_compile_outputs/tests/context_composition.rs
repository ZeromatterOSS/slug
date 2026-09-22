//! Pinned native boundaries used by the unchanged compilation-context helper.
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisConfiguredTargetKey;
use slug_build_api_v2::DerivedArtifactRoot;

use super::*;
use crate::subrule_invocation::AnalysisArtifactValue;

fn tree_query(expression: &str) -> anyhow::Result<String> {
    let module = Module::new();
    module.import_public_symbols(&internal());
    let label = CanonicalLabel::parse("@@//pkg:owner").unwrap();
    module.set(
        "source",
        module
            .heap()
            .alloc_simple(AnalysisArtifactValue::new(AnalysisArtifact::Source(
                label.clone(),
            ))),
    );
    for (name, kind) in [
        ("regular", ActionOutputKind::File),
        ("tree", ActionOutputKind::Directory),
    ] {
        let root = DerivedArtifactRoot::bin(config(false)).unwrap();
        module.set(
            name,
            module
                .heap()
                .alloc_simple(AnalysisArtifactValue::new(AnalysisArtifact::Derived {
                    owner: AnalysisConfiguredTargetKey::new(label.clone(), Arc::<[u8]>::from([])),
                    // Identical path spelling deliberately distinguishes kind only.
                    output: ActionOutput::new(root, "pkg/looks_like_a_file.cc", kind),
                })),
        );
    }
    let mut eval = Evaluator::new(&module);
    eval.eval_module(
        AstModule::parse(
            "tree.bzl",
            format!("result = {expression}"),
            &Dialect::Bazel,
        )
        .map_err(starlark::Error::into_anyhow)?,
        &loading_globals(),
    )
    .map_err(starlark::Error::into_anyhow)?;
    Ok(module.get("result").unwrap().to_repr())
}

#[test]
fn native_tree_query_classifies_genuine_artifacts_with_positional_only_binder() {
    assert_eq!(
        tree_query(
            "[i.is_tree_artifact(source), i.is_tree_artifact(regular), i.is_tree_artifact(tree)]"
        )
        .unwrap(),
        "[False, False, True]"
    );
    for expression in [
        "i.is_tree_artifact()",
        "i.is_tree_artifact(artifact=tree)",
        "i.is_tree_artifact(tree, tree)",
        "i.is_tree_artifact(tree, extra=True)",
        "i.is_tree_artifact(None)",
        "i.is_tree_artifact('pkg/tree')",
        "i.is_tree_artifact(struct(is_directory=True))",
    ] {
        assert!(tree_query(expression).is_err(), "{expression}");
    }
}

fn layout_query(label: Option<&str>, source: &str) -> anyhow::Result<String> {
    let module = Module::new();
    module.set("CONFIG", alloc_configuration(module.heap(), config(true))?);
    let context = label.map(|label| {
        let identity = crate::BzlModuleIdentity {
            label: CanonicalLabel::parse(label).unwrap(),
            workspace_path: std::path::PathBuf::from("layout.bzl"),
            repository_mapping: Arc::from([]),
        };
        BzlEvaluationContext::macro_runtime_context(
            identity.clone(),
            Arc::from([(
                compact_str::CompactString::new("layout.bzl"),
                crate::BzlModuleSourceProvenance::new(identity, [0; 32]),
            )]),
        )
    });
    let mut eval = Evaluator::new(&module);
    if let Some(context) = &context {
        eval.extra = Some(context);
    }
    eval.eval_module(
        AstModule::parse("layout.bzl", source.to_owned(), &Dialect::Bazel)
            .map_err(starlark::Error::into_anyhow)?,
        &loading_globals(),
    )
    .map_err(starlark::Error::into_anyhow)?;
    Ok(module.get("result").unwrap().to_repr())
}

#[test]
fn fixed_nonsibling_layout_uses_current_default_allowlist_and_preserves_coverage() {
    let code = "def read(c):\n    return [c.is_sibling_repository_layout(), c.coverage_enabled]\nresult = read(CONFIG)";
    assert_eq!(
        layout_query(Some("@@rules_cc+//cc/private:layout.bzl"), code).unwrap(),
        "[False, True]"
    );
    let denied = layout_query(Some("@@//ordinary:layout.bzl"), code).unwrap_err();
    assert!(
        denied.to_string().contains("cannot use private API"),
        "{denied:#}"
    );
    assert!(
        layout_query(None, code)
            .unwrap_err()
            .to_string()
            .contains("source-sensitive builtin")
    );
    for code in [
        "result = CONFIG.is_sibling_repository_layout()",
        "def read(c):\n    return c.is_sibling_repository_layout(True)\nresult = read(CONFIG)",
        "def read(c):\n    return c.is_sibling_repository_layout(extra=True)\nresult = read(CONFIG)",
        "CONFIG.is_sibling_repository_layout = False\nresult = None",
    ] {
        assert!(
            layout_query(Some("@@rules_cc+//cc/private:layout.bzl"), code).is_err(),
            "{code}"
        );
    }
}

#[test]
fn sibling_layout_flag_is_not_an_admitted_native_configuration_input() {
    const FLAG: &str = "experimental_sibling_repository_layout";
    assert_eq!(NativeCommandOption::from_name(FLAG), None);
    let error = config(false)
        .matches_config_setting(&[(FLAG.into(), "true".into())], &[])
        .unwrap_err();
    assert!(
        matches!(error, slug_configuration_v2::native::NativeConfigSettingMatchError::UnknownOption(name) if name == FLAG)
    );
    assert!(
        !slug_configuration_v2::native::NATIVE_OPTION_DESCRIPTORS
            .iter()
            .any(|entry| entry.canonical_name == FLAG)
    );
}
