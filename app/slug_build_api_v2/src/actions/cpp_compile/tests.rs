use slug_configuration_v2::CppFragmentProjection;
use slug_identity_v2::CanonicalLabel;

use super::*;
use crate::ActionKind;
use crate::ActionSpec;
use crate::AnalysisConfiguredTargetKey;
use crate::AnalysisDepset;
use crate::ArtifactInputSource;
use crate::CtxActions;
use crate::DepsetOrder;
use crate::ProviderIdentity;
use crate::ProviderOccurrence;
use crate::ReapiCommandProjection;
use crate::RetainedArtifactInputs;
use crate::artifact_root::test_support as roots;
use crate::cc_features::*;
use crate::cc_variables::*;

fn source(name: &str) -> AnalysisArtifact {
    AnalysisArtifact::Source(CanonicalLabel::parse(&format!("@@//pkg:{name}")).unwrap())
}
fn inputs(names: &[&str]) -> ArtifactInputs {
    ArtifactInputs::new(
        names
            .iter()
            .map(|name| ArtifactInputSource::Direct(source(name)))
            .collect::<Vec<_>>(),
    )
}
fn variables(text: &str) -> CcToolchainVariables {
    CcToolchainVariables::new(CcVariablesModel {
        kind: CcVariablesKind::Map,
        occurrence: CcVariablesOccurrence::new(),
        parent: None,
        locals: vec![(
            "unused".into(),
            AnalysisValue::list(vec![AnalysisValue::string(text)]),
        )]
        .into(),
        cache: Box::new([]),
    })
}
fn context(fields: Vec<(&str, AnalysisValue)>) -> AnalysisValue {
    AnalysisValue::provider(ProviderOccurrence::new(
        ProviderIdentity::builtin("CcCompilationContext"),
        fields,
    ))
}
fn features(unused: &str) -> CcFeatureConfiguration {
    features_requested(unused, vec!["c++-compile".into()])
}
fn features_requested(unused: &str, requested: Vec<CompactString>) -> CcFeatureConfiguration {
    let tool = CcTool::new(
        Some("cc"),
        None,
        CcSet::new(),
        [CompactString::from("requires-special")]
            .into_iter()
            .collect(),
        CcExecOs::Linux,
    )
    .unwrap();
    CcToolchainFeatures::new(
        vec![CcFeature {
            name: unused.into(),
            enabled: false,
            flag_sets: Box::new([]),
            env_sets: Box::new([]),
            requires: Box::new([]),
            implies: Box::new([]),
            provides: Box::new([]),
        }],
        vec![CcActionConfig {
            action_name: "c++-compile".into(),
            enabled: true,
            tools: vec![tool].into(),
            flag_sets: Box::new([]),
            implies: Box::new([]),
        }],
        vec![],
        "tools",
        CcExecOs::Linux,
    )
    .unwrap()
    .configure_features(requested)
    .unwrap()
}
fn data() -> CppCompileData {
    let root = roots::root();
    CppCompileData {
        construction: CppCompileConstruction {
            source: source("source.cc"),
            outputs: vec![
                ActionOutput::new(root.clone(), "pkg/source.o", ActionOutputKind::File),
                ActionOutput::new(root.clone(), "pkg/source.d", ActionOutputKind::File),
            ]
            .into(),
            action_name: "c++-compile".into(),
            feature_configuration: features("unused-a"),
            mandatory_spawn_inputs: inputs(&["cc", "builtin.h", "extra", "source.cc"]),
            cache_key_inputs: inputs(&["cache"]),
            declared_headers: inputs(&["header.h"]),
            builtin_inputs: inputs(&["builtin.h"]),
            additional_prunable_inputs: inputs(&["prunable.h"]),
            environment: RetainedActionEnvironment::default(),
            execution_requirements: CanonicalStringMap::from_pairs([("requires-special", "")]),
            selected_tool: "tools/cc".into(),
            toolchain_cpp: RetainedCppFragment::new(
                CppFragmentProjection::new(root.configuration().clone()).unwrap(),
            ),
            builtin_include_directories: vec!["/include".into()].into(),
            use_pic: false,
            needs_include_validation: true,
        },
        variables: variables("unchanged"),
        compilation_context: context(vec![]),
    }
}
fn paths(inputs: &ArtifactInputs) -> Vec<String> {
    let mut paths = Vec::new();
    inputs
        .visit(|artifact| paths.push(artifact.path().into_owned()))
        .unwrap();
    paths
}

#[test]
fn cpp_compile_retains_input_roles_outputs_and_rejects_command_projection() {
    let spec = CppCompileSpec::new(data()).unwrap();
    assert_eq!(
        paths(spec.mandatory_spawn_inputs()),
        ["pkg/cc", "pkg/builtin.h", "pkg/extra", "pkg/source.cc"]
    );
    assert_eq!(paths(spec.cache_key_inputs()), ["pkg/cache"]);
    assert_eq!(paths(spec.declared_headers()), ["pkg/header.h"]);
    assert_eq!(spec.object_output().root_relative_path(), "pkg/source.o");
    assert_eq!(spec.dotd_output().root_relative_path(), "pkg/source.d");
    assert!(std::ptr::eq(spec.data(), spec.dupe().data()));
    assert_eq!(
        std::mem::size_of::<CppCompileSpec>(),
        std::mem::size_of::<usize>()
    );
    let action = ActionSpec::cpp_compile(spec);
    assert_eq!(action.kind(), &ActionKind::CppCompile);
    assert_eq!(action.mnemonic(), "CppCompile");
    assert!(action.spawn_spec().is_none());
    assert!(action.cpp_compile_spec().is_some());
    assert_eq!(action.outputs().len(), 2);
    assert_eq!(
        action.render_argv().unwrap_err(),
        "CppCompile command expansion is not admitted"
    );
    assert_eq!(
        ReapiCommandProjection::from_action(&action).unwrap_err(),
        "CppCompile REAPI projection is not admitted"
    );
}

#[test]
fn cpp_compile_publication_compares_unused_recipe_and_policy_payloads() {
    let baseline = CppCompileSpec::new(data()).unwrap();
    assert_eq!(baseline, CppCompileSpec::new(data()).unwrap());
    let changes: Vec<Box<dyn FnOnce(&mut CppCompileData)>> = vec![
        Box::new(|d| d.variables = variables("different-unused")),
        Box::new(|d| {
            d.compilation_context = context(vec![("unused", AnalysisValue::boolean(true))])
        }),
        Box::new(|d| d.construction.feature_configuration = features("unused-b")),
        Box::new(|d| d.construction.use_pic = true),
        Box::new(|d| d.construction.needs_include_validation = false),
        Box::new(|d| d.construction.builtin_include_directories = vec!["/other".into()].into()),
        Box::new(|d| d.construction.cache_key_inputs = inputs(&["other"])),
        Box::new(|d| d.construction.declared_headers = inputs(&["other.h"])),
        Box::new(|d| d.construction.builtin_inputs = inputs(&["other.h"])),
        Box::new(|d| d.construction.additional_prunable_inputs = inputs(&["other.h"])),
        Box::new(|d| {
            d.construction.execution_requirements =
                CanonicalStringMap::from_pairs([("requires-special", ""), ("tag", "")])
        }),
        Box::new(|d| {
            d.construction.toolchain_cpp = RetainedCppFragment::new(
                CppFragmentProjection::new(
                    roots::root_for(b"unused-config").configuration().clone(),
                )
                .unwrap(),
            )
        }),
    ];
    for change in changes {
        let mut changed = data();
        change(&mut changed);
        assert_ne!(baseline, CppCompileSpec::new(changed).unwrap());
    }
}

#[test]
fn cpp_compile_publication_pairs_variables_context_and_depset_aliases_together() {
    fn aliased(split_variables: bool, split_depset: bool) -> CppCompileSpec {
        let mut d = data();
        let headers = || {
            AnalysisDepset::new(
                DepsetOrder::Default,
                vec![AnalysisValue::artifact(source("shared.h"))],
                vec![],
            )
            .unwrap()
        };
        let shared = headers();
        d.construction.declared_headers = ArtifactInputs::new(vec![ArtifactInputSource::Depset(
            RetainedArtifactInputs::new(shared.clone()).unwrap(),
        )]);
        d.compilation_context = context(vec![
            (
                "variables",
                AnalysisValue::cc_toolchain_variables(if split_variables {
                    variables("unchanged")
                } else {
                    d.variables.clone()
                }),
            ),
            (
                "headers",
                AnalysisValue::depset(if split_depset { headers() } else { shared }),
            ),
            (
                "features",
                AnalysisValue::cc_feature_configuration(d.feature_configuration.clone()),
            ),
            ("cpp", AnalysisValue::cpp_fragment(d.toolchain_cpp.clone())),
        ]);
        CppCompileSpec::new(d).unwrap()
    }
    let a = aliased(false, false);
    assert_eq!(a, aliased(false, false));
    for b in [aliased(true, false), aliased(false, true)] {
        assert_ne!(a, b);
        assert_ne!(b, a);
    }
}

#[test]
fn cpp_compile_rejects_malformed_outputs_source_context_and_unconfigured_action() {
    let changes: Vec<Box<dyn FnOnce(&mut CppCompileData)>> = vec![
        Box::new(|d| d.construction.outputs = vec![d.outputs[0].clone()].into()),
        Box::new(|d| d.construction.outputs[1] = d.outputs[0].clone()),
        Box::new(|d| {
            d.construction.outputs[1] = ActionOutput::new(
                roots::root_for(b"foreign"),
                "pkg/source.d",
                ActionOutputKind::File,
            )
        }),
        Box::new(|d| {
            d.construction.outputs[1] =
                ActionOutput::new(roots::root(), "pkg/source.d", ActionOutputKind::Directory)
        }),
        Box::new(|d| {
            d.construction.outputs[1] =
                ActionOutput::new(roots::root(), "pkg/../source.d", ActionOutputKind::File)
        }),
        Box::new(|d| {
            d.construction.source = AnalysisArtifact::Derived {
                owner: AnalysisConfiguredTargetKey::new(
                    CanonicalLabel::parse("@@//pkg:owner").unwrap(),
                    b"key".as_slice(),
                ),
                output: ActionOutput::new(roots::root(), "pkg/tree", ActionOutputKind::Directory),
            }
        }),
        Box::new(|d| d.compilation_context = AnalysisValue::none()),
        Box::new(|d| d.construction.action_name = "absent".into()),
        Box::new(|d| d.construction.selected_tool = "wrong".into()),
    ];
    for change in changes {
        let mut invalid = data();
        change(&mut invalid);
        assert!(CppCompileSpec::new(invalid).is_err());
    }
}

#[test]
fn cpp_compile_finalization_replaces_atomically_and_preserves_interleaving() {
    let mut actions = CtxActions::new(roots::root());
    let first = actions.declare_file("first").unwrap();
    actions.write(first, "first", false).unwrap();
    let last = actions.declare_file("last").unwrap();
    actions.write(last, "last", false).unwrap();
    let before = actions.clone();
    let cpp = ActionSpec::cpp_compile(CppCompileSpec::new(data()).unwrap());
    let first = before.registry().actions()[0].clone();
    let last = before.registry().actions()[1].clone();
    actions
        .replace_actions(vec![first.clone(), cpp.clone(), cpp.clone(), last.clone()])
        .unwrap_err();
    assert_eq!(actions, before);
    let conflict = ActionSpec::new(
        ActionKind::Write {
            content: "conflict".into(),
            is_executable: false,
        },
        "FileWrite",
        vec![cpp.outputs()[0].clone()],
    );
    for batch in [
        vec![cpp.clone(), conflict.clone()],
        vec![conflict, cpp.clone()],
    ] {
        actions.replace_actions(batch).unwrap_err();
        assert_eq!(actions, before);
    }
    actions
        .replace_actions(vec![first.clone(), cpp.clone(), last.clone()])
        .unwrap();
    assert_eq!(actions.registry().actions(), [first, cpp, last]);
    assert_eq!(
        actions
            .registry()
            .output_owner(&roots::exec_path("pkg/source.o")),
        Some(1)
    );
}

#[test]
fn cpp_compile_cannot_be_forged_through_legacy_action_constructor() {
    let forged = ActionSpec::new(
        ActionKind::CppCompile,
        "CppCompile",
        data().outputs.to_vec(),
    )
    .with_argv(vec!["not-a-compiler".into()]);
    assert!(forged.cpp_compile_spec().is_none());
    assert_eq!(
        forged.render_argv().unwrap_err(),
        "CppCompile command expansion is not admitted"
    );
    assert_eq!(
        ReapiCommandProjection::from_action(&forged).unwrap_err(),
        "CppCompile REAPI projection is not admitted"
    );
    let mut registry = crate::ActionRegistry::new();
    assert!(matches!(
        registry.register(forged.clone()),
        Err(crate::ActionError::InvalidActionPayload { .. })
    ));
    assert!(matches!(
        registry.register_batch([forged]),
        Err(crate::ActionError::InvalidActionPayload { .. })
    ));
    assert!(registry.actions().is_empty());
}

#[test]
fn cpp_compile_rejects_deferred_features_at_eager_and_owned_boundaries() {
    for feature in [
        "parse_showincludes",
        "use_header_modules",
        "compiler_param_file",
    ] {
        let mut d = data();
        d.construction.feature_configuration =
            features_requested(feature, vec!["c++-compile".into(), feature.into()]);
        assert!(
            CppCompileSpec::validate_construction(&d.construction)
                .unwrap_err()
                .contains(feature)
        );
        assert!(CppCompileSpec::new(d).unwrap_err().contains(feature));
    }
}
