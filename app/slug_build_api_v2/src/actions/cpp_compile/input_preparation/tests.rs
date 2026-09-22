use slug_configuration_v2::CommandConfigurationOccurrence;
use slug_configuration_v2::CommandConfigurationOverlay;
use slug_configuration_v2::NativeCommandOption;
use slug_identity_v2::CanonicalLabel;

use super::super::expansion::tests::configuration;
use super::super::expansion::tests::group;
use super::*;
use crate::ActionOutput;
use crate::ActionOutputKind;
use crate::AnalysisArtifact;
use crate::AnalysisConfiguredTargetKey;
use crate::AnalysisDepset;
use crate::AnalysisValue;
use crate::ArtifactInputSource;
use crate::ArtifactInputs;
use crate::CppCompileData;
use crate::DepsetOrder;
use crate::DerivedArtifactRoot;
use crate::ProviderIdentity;
use crate::ProviderOccurrence;
use crate::cc_variables::CcToolchainVariables;
use crate::cc_variables::CcVariableCacheEntry;
use crate::cc_variables::CcVariablesKind;
use crate::cc_variables::CcVariablesModel;
use crate::cc_variables::CcVariablesOccurrence;

fn strings(values: &[&str]) -> AnalysisValue {
    AnalysisValue::list(
        values
            .iter()
            .map(|v| AnalysisValue::string(*v))
            .collect::<Vec<_>>(),
    )
}
fn directories(values: &[&str]) -> AnalysisValue {
    AnalysisValue::depset(
        AnalysisDepset::new(
            DepsetOrder::Default,
            values.iter().map(|v| AnalysisValue::string(*v)).collect(),
            vec![],
        )
        .unwrap(),
    )
}
fn context(includes: &[&str], quote: &[&str], system: &[&str], external: &[&str]) -> AnalysisValue {
    AnalysisValue::provider(ProviderOccurrence::new(
        ProviderIdentity::builtin("CcCompilationContext"),
        [
            ("includes", directories(includes)),
            ("quote_includes", directories(quote)),
            ("system_includes", directories(system)),
            ("external_includes", directories(external)),
            // Framework paths are deliberately not part of this validation slice.
            ("framework_includes", directories(&["/framework/outside"])),
        ],
    ))
}
fn variables(
    locals: Vec<(&str, AnalysisValue)>,
    cache: Vec<(&str, CcVariableCacheEntry<AnalysisValue>)>,
) -> CcToolchainVariables {
    CcToolchainVariables::new(CcVariablesModel {
        kind: CcVariablesKind::Map,
        occurrence: CcVariablesOccurrence::new(),
        parent: None,
        locals: locals.into_iter().map(|(k, v)| (k.into(), v)).collect(),
        cache: cache.into_iter().map(|(k, v)| (k.into(), v)).collect(),
    })
}
fn data(copts: &[&str], options: &[&str]) -> CppCompileData {
    let mut data = super::super::tests::data();
    data.compilation_context = context(&[], &[], &[], &[]);
    data.variables = variables(vec![("user_compile_flags", strings(copts))], vec![]);
    let groups = if options.is_empty() {
        vec![]
    } else {
        vec![group(options)]
    };
    data.construction.feature_configuration = configuration(groups, &[]);
    data
}
fn recipe(copts: &[&str], options: &[&str]) -> CppCompileSpec {
    CppCompileSpec::new(data(copts, options)).unwrap()
}
fn error(spec: &CppCompileSpec) -> String {
    let before = spec.clone();
    let first = spec.prepare_inputs_posix().unwrap_err();
    assert_eq!(spec.prepare_inputs_posix().unwrap_err(), first);
    assert_eq!(spec, &before);
    first
}

#[test]
fn preparation_checks_ordinary_quote_system_order_and_returns_exact_input_roles() {
    let mut data = data(
        &["-Id/../../ordinary", "-iquote/quote"],
        &["-isystem/system"],
    );
    data.compilation_context = context(&["/context"], &[], &[], &[]);
    assert!(error(&CppCompileSpec::new(data.clone()).unwrap()).contains("'/context'"));
    data.compilation_context = context(&[], &[], &[], &[]);
    assert!(error(&CppCompileSpec::new(data).unwrap()).contains("'../ordinary'"));
    assert!(error(&recipe(&["-iquote/quote"], &["-isystem/system"])).contains("'/quote'"));
    assert!(error(&recipe(&[], &["-isystem/system"])).contains("'/system'"));
    let spec = recipe(&["-Ihere/../ok", "-iquote", "quote"], &["-isystemsystem"]);
    let before = spec.clone();
    for _ in 0..2 {
        let prepared = spec.prepare_inputs_posix().unwrap();
        assert_eq!(prepared.inputs(), &spec.input_sets().unwrap());
        assert!(prepared.warnings().is_empty());
        assert_eq!(spec, before);
    }
}

#[test]
fn raw_combined_ordinary_flags_are_distinct_from_feature_and_forced_flags() {
    // Standalone -I does not consume the following word. Feature -I/-iquote,
    // framework and forced-include options are outside this specific extraction.
    let spec = recipe(
        &[
            "-I",
            "../ignored",
            "/I",
            "/outside",
            "-i/ignored",
            "/i/ignored",
            "-include",
            "/forced",
            "-FI/forced",
        ],
        &[
            "-I/feature",
            "-iquote/feature",
            "-include",
            "/forced",
            "/FI/forced",
        ],
    );
    spec.prepare_inputs_posix().unwrap();
    for flag in ["-I/ordinary", "/I/ordinary"] {
        assert!(error(&recipe(&[flag], &[])).contains("'/ordinary'"));
    }
    for flag in ["-IMSVC/outside", "/IMsVc/outside"] {
        // msvc spellings never become ordinary paths, even on POSIX.
        recipe(&[flag], &[]).prepare_inputs_posix().unwrap();
        assert!(error(&recipe(&[flag], &[flag])).contains("'/outside'"));
    }
}

#[test]
fn joined_split_dangling_and_ascii_msvc_options_follow_source_scanning() {
    let options = [
        "-isystem join ",
        "-isystem",
        "split ",
        "/ImSvC\tmsvc\r",
        "-IMsVc",
        "next",
        "-isystem",
    ]
    .into_iter()
    .map(CompactString::from)
    .collect::<Vec<_>>();
    let mut warnings = Vec::new();
    assert_eq!(
        option_paths(&options, FlagKind::System, &mut warnings).unwrap(),
        ["join", "split ", "msvc", "next"]
    );
    assert_eq!(warnings, ["dangling -isystem include option"]);
    let spec = recipe(&["-iquote"], &["/ImSvC"]);
    assert_eq!(
        spec.prepare_inputs_posix().unwrap().warnings(),
        [
            "dangling /ImSvC include option",
            "dangling -iquote include option"
        ]
    );
    // The split argument is consumed as a path, not reparsed as another flag.
    recipe(
        &["-iquote", "-iquote/ignored"],
        &["-isystem", "-isystem/ignored"],
    )
    .prepare_inputs_posix()
    .unwrap();
    let mut warnings = Vec::new();
    assert_eq!(
        option_paths(
            &[
                "-iquote  joined\t".into(),
                "-iquote".into(),
                " split ".into()
            ],
            FlagKind::Quote,
            &mut warnings
        )
        .unwrap(),
        ["joined", " split "]
    );
}

#[test]
fn java_trim_component_boundaries_and_normalized_external_paths_are_exact() {
    for flag in ["-isystem\t../outside\r", "-isystem /absolute "] {
        assert!(error(&recipe(&[], &[flag])).contains("outside of the execution root"));
    }
    // NBSP is not Java trim whitespace. Split suffixes are not trimmed either.
    recipe(
        &["-iquote\u{a0}../relative"],
        &["-isystem\u{a0}../relative", "-isystem", " ../relative "],
    )
    .prepare_inputs_posix()
    .unwrap();
    let mut data = data(
        &["-Iexternal/repo/ok", "-Iexternal", "-I./ok/.."],
        &["-isystem/allowed/child"],
    );
    data.construction.builtin_include_directories = vec!["/allowed".into()].into();
    CppCompileSpec::new(data.clone())
        .unwrap()
        .prepare_inputs_posix()
        .unwrap();
    data.construction.feature_configuration =
        configuration(vec![group(&["-isystem/allowed-other/child"])], &[]);
    assert!(error(&CppCompileSpec::new(data).unwrap()).contains("'/allowed-other/child'"));
    assert!(error(&recipe(&["-Iexternal/../../escape"], &[])).contains("'../escape'"));
    assert!(error(&recipe(&[], &["-isystem", "bad\0path"])).contains("NUL"));
}

#[test]
fn original_cached_discovery_options_and_fresh_execution_argv_diverge_both_ways() {
    for (current, cached, valid) in [("safe", "../stale", false), ("../fresh", "safe", true)] {
        let mut data = data(&[], &["-isystem%{obj.field}"]);
        data.variables = variables(
            vec![(
                "obj",
                AnalysisValue::strukt([("field", AnalysisValue::string(current))]),
            )],
            vec![(
                "obj.field",
                CcVariableCacheEntry::Value(AnalysisValue::string(cached)),
            )],
        );
        let spec = CppCompileSpec::new(data).unwrap();
        let before = spec.clone();
        for _ in 0..2 {
            assert_eq!(spec.prepare_inputs_posix().is_ok(), valid);
            assert_eq!(
                spec.expand_command_line().unwrap().last().unwrap(),
                &format!("-isystem{current}")
            );
            assert_eq!(spec, before);
        }
        if !valid {
            assert!(error(&spec).contains("'../stale'"));
        }
    }
}

#[test]
fn validation_disabled_still_expands_and_normalizes_system_options_only() {
    let mut data = data(&[], &["-isystem/outside", "-isystem"]);
    data.construction.needs_include_validation = false;
    data.compilation_context = AnalysisValue::provider(ProviderOccurrence::empty(
        ProviderIdentity::builtin("CcCompilationContext"),
    ));
    data.variables = variables(
        vec![("user_compile_flags", AnalysisValue::integer(7))],
        vec![],
    );
    let spec = CppCompileSpec::new(data.clone()).unwrap();
    assert_eq!(
        spec.prepare_inputs_posix().unwrap().warnings(),
        ["dangling -isystem include option"]
    );
    data.construction.feature_configuration = configuration(vec![group(&["%{missing}"])], &[]);
    assert!(error(&CppCompileSpec::new(data.clone()).unwrap()).contains("missing"));
    data.construction.feature_configuration =
        configuration(vec![group(&["-isystembad\0path"])], &[]);
    assert!(error(&CppCompileSpec::new(data.clone()).unwrap()).contains("NUL"));
    data.construction.feature_configuration = configuration(vec![], &[]);
    data.construction.declared_headers = ArtifactInputs::new(vec![ArtifactInputSource::Direct(
        AnalysisArtifact::Derived {
            owner: AnalysisConfiguredTargetKey::new(
                CanonicalLabel::parse("@@//pkg:owner").unwrap(),
                b"config".as_slice(),
            ),
            output: ActionOutput::new(
                data.outputs[0].root().clone(),
                "pkg/tree",
                ActionOutputKind::Directory,
            ),
        },
    )]);
    assert!(error(&CppCompileSpec::new(data).unwrap()).contains("regular Files"));
}

#[test]
fn consumed_copts_and_context_fail_closed_with_source_error_timing() {
    let mut data = data(&[], &[]);
    data.variables = variables(vec![], vec![]);
    CppCompileSpec::new(data.clone())
        .unwrap()
        .prepare_inputs_posix()
        .unwrap();
    for value in [
        AnalysisValue::integer(1),
        AnalysisValue::string("scalar"),
        AnalysisValue::list(vec![AnalysisValue::integer(2)]),
    ] {
        data.variables = variables(vec![("user_compile_flags", value)], vec![]);
        assert!(
            CppCompileSpec::new(data.clone())
                .unwrap()
                .prepare_inputs_posix()
                .is_err()
        );
    }
    data.compilation_context = AnalysisValue::provider(ProviderOccurrence::empty(
        ProviderIdentity::builtin("CcCompilationContext"),
    ));
    assert!(error(&CppCompileSpec::new(data.clone()).unwrap()).contains("system_includes"));
    data.construction.feature_configuration = configuration(vec![group(&["%{missing}"])], &[]);
    assert!(error(&CppCompileSpec::new(data.clone()).unwrap()).contains("missing"));
    data.construction.feature_configuration = configuration(vec![], &[]);
    data.variables = variables(vec![], vec![]);
    for (field, value) in [
        ("includes", strings(&[])),
        ("quote_includes", AnalysisValue::none()),
        ("external_includes", directories(&["bad\0dir"])),
    ] {
        data.compilation_context = AnalysisValue::provider(ProviderOccurrence::new(
            ProviderIdentity::builtin("CcCompilationContext"),
            [
                "system_includes",
                "external_includes",
                "includes",
                "quote_includes",
            ]
            .into_iter()
            .map(|key| {
                (
                    key,
                    if key == field {
                        value.clone()
                    } else {
                        directories(&[])
                    },
                )
            }),
        ));
        assert!(
            CppCompileSpec::new(data.clone())
                .unwrap()
                .prepare_inputs_posix()
                .is_err()
        );
    }
}

#[test]
fn strict_system_policy_does_not_change_preexecution_ignored_directories() {
    let mut data = data(
        &["-I/system/child", "-iquote/external/child"],
        &["-isystem/system/child"],
    );
    data.compilation_context = context(&[], &[], &["/system"], &["/external"]);
    let config = data.outputs[0].root().configuration();
    let overlay: CommandConfigurationOverlay = vec![CommandConfigurationOccurrence::native(
        NativeCommandOption::StrictSystemIncludes,
        Some("true"),
        false,
    )]
    .into();
    let root = DerivedArtifactRoot::bin(
        config
            .with_command_configuration(config.starlark_options().clone(), &overlay)
            .unwrap(),
    )
    .unwrap();
    for output in &mut data.construction.outputs {
        *output = ActionOutput::new(root.clone(), output.root_relative_path(), output.kind());
    }
    let spec = CppCompileSpec::new(data).unwrap();
    assert_eq!(
        spec.prepare_inputs_posix().unwrap().inputs(),
        &spec.input_sets().unwrap()
    );
}
