//! Test-only genuine constructor shared by refusal gates. No compiler executes.
use slug_build_api_v2::cc_variables::*;
use slug_build_api_v2::*;
use slug_configuration_v2::CppFragmentProjection;
use slug_identity_v2::CanonicalLabel;

pub fn action(root: DerivedArtifactRoot) -> ActionSpec {
    let tool = CcTool::new(
        Some("cc"),
        None,
        CcSet::new(),
        CcSet::new(),
        CcExecOs::Linux,
    )
    .unwrap();
    let features = CcToolchainFeatures::new(
        vec![],
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
    .configure_features(vec!["c++-compile".into()])
    .unwrap();
    let source = AnalysisArtifact::Source(CanonicalLabel::parse("@@//:not-read.cc").unwrap());
    let empty = || ArtifactInputs::new(Vec::new());
    ActionSpec::cpp_compile(
        CppCompileSpec::new(CppCompileData {
            construction: CppCompileConstruction {
                source: source.clone(),
                outputs: vec![
                    ActionOutput::new(root.clone(), "out.o", ActionOutputKind::File),
                    ActionOutput::new(root.clone(), "out.d", ActionOutputKind::File),
                ]
                .into(),
                action_name: "c++-compile".into(),
                feature_configuration: features,
                mandatory_spawn_inputs: ArtifactInputs::new(vec![ArtifactInputSource::Direct(
                    source,
                )]),
                cache_key_inputs: empty(),
                declared_headers: empty(),
                builtin_inputs: empty(),
                additional_prunable_inputs: empty(),
                environment: Default::default(),
                execution_requirements: Default::default(),
                selected_tool: "tools/cc".into(),
                toolchain_cpp: RetainedCppFragment::new(
                    CppFragmentProjection::new(root.configuration().clone()).unwrap(),
                ),
                builtin_include_directories: Box::new([]),
                use_pic: false,
                needs_include_validation: true,
            },
            variables: CcToolchainVariables::new(CcVariablesModel {
                kind: CcVariablesKind::Map,
                occurrence: CcVariablesOccurrence::new(),
                parent: None,
                locals: Box::new([]),
                cache: Box::new([]),
            }),
            compilation_context: AnalysisValue::provider(ProviderOccurrence::empty(
                ProviderIdentity::builtin("CcCompilationContext"),
            )),
        })
        .unwrap(),
    )
}
