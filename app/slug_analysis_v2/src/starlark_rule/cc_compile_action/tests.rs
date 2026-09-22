//! Pinned builder policy over real configured owner rows and retained depsets.
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::ActionEnvironmentHost;
use slug_configuration_v2::native::host::ActionEnvironmentHostOs;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_identity_v2::CanonicalLabel;

use super::*;
use crate::execution_groups::ConfiguredExecGroupRow;
use crate::result::ConfiguredActionAspectProvenance;
use crate::result::PlatformSemanticFact;
use crate::result::ToolchainTopology;

fn groups(auto: bool) -> ConfiguredExecGroupCollection {
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap()
    .with_action_environment_host(ActionEnvironmentHost::without_environment(
        ActionEnvironmentHostOs::Linux,
    ));
    let configuration = SlugConfiguration::default_target(&host).unwrap();
    let owner = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@consumer//pkg:rule").unwrap(),
        crate::ConfigurationKey::from_slug(configuration.clone()),
    );
    let platform = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:platform").unwrap(),
        crate::ConfigurationKey::from_slug(configuration.to_exec().unwrap()),
    );
    let mut rows = Vec::new();
    for group in [
        ConfiguredExecGroup::Default,
        ConfiguredExecGroup::automatic(CanonicalLabel::parse("@@canonical//tools:cpp").unwrap()),
    ] {
        let context = ConfiguredActionOwnerContext::new(
            owner.clone(),
            group.clone(),
            platform.clone(),
            PlatformSemanticFact {
                exec_properties: Arc::from([]),
                missing_toolchain_error: None,
            },
            &Default::default(),
            &Default::default(),
            Vec::new(),
            None,
            ConfiguredActionAspectProvenance::Absent,
        )
        .unwrap();
        rows.push(
            ConfiguredExecGroupRow::new(group, Arc::from([]), Arc::from([]), Arc::new(context))
                .unwrap(),
        );
    }
    ConfiguredExecGroupCollection::new(
        rows,
        Arc::from([]),
        ToolchainTopology::new(Vec::new(), None).unwrap(),
        auto,
    )
    .unwrap()
}

#[test]
fn native_compile_owner_uses_canonical_auto_group_with_default_fallback() {
    let groups = groups(true);
    let expected = selected_group(&groups, "@@canonical//tools:cpp").unwrap();
    assert!(expected.is_some());
    assert_eq!(
        selected_group(&groups, "@canonical//tools:cpp").unwrap(),
        expected
    );
    // An apparent spelling is not resolved through the caller's mapping.
    assert_eq!(selected_group(&groups, "@alias//tools:cpp").unwrap(), None);
    assert_eq!(selected_group(&groups, "//tools:cpp").unwrap(), None);
    assert_eq!(
        toolchain_label("//tools:cpp").unwrap().to_string(),
        "@@//tools:cpp"
    );
    assert_eq!(
        selected_group(&groups, "@@missing//tools:cpp").unwrap(),
        None
    );
    assert_eq!(
        selected_group(&self::groups(false), "@canonical//tools:cpp").unwrap(),
        None
    );
    assert!(toolchain_label(":relative").is_err());
    assert_eq!(
        selected_group(&self::groups(false), ":not_parsed").unwrap(),
        None
    );
}

fn source(path: &str) -> AnalysisArtifact {
    AnalysisArtifact::Source(CanonicalLabel::parse(&format!("@@//:{path}")).unwrap())
}
fn set(paths: &[&str]) -> AnalysisDepset {
    union(
        DepsetOrder::Default,
        paths.iter().map(|path| source(path)).collect(),
        Vec::new(),
    )
    .unwrap()
}

#[test]
fn native_compile_input_union_preserves_postorder_and_deduplicates_shared_inputs() {
    // Builder mandatoryInputs: additional direct files, then additional-set
    // and compiler-file nested sets. The final compile-order wrapper adds
    // non-code inputs transitively, and builtin/source files directly.
    let mandatory = union(
        DepsetOrder::Default,
        vec![source("additional")],
        vec![set(&["extra", "header"]), set(&["header", "compiler"])],
    )
    .unwrap();
    let union = union(
        DepsetOrder::Postorder,
        vec![source("header"), source("builtin"), source("source.cc")],
        vec![mandatory, set(&["noncode", "header"])],
    )
    .unwrap();
    let retained = inputs(union).unwrap();
    let mut actual = Vec::new();
    retained
        .visit(|file| actual.push(file.path().into_owned()))
        .unwrap();
    assert_eq!(
        actual,
        [
            "extra",
            "header",
            "compiler",
            "additional",
            "noncode",
            "builtin",
            "source.cc"
        ]
    );
}

#[test]
fn native_compile_specific_tool_selection_keeps_eager_fallback_validation() {
    let heap = starlark::values::FrozenHeap::new();
    let alloc = |paths: &[&str]| {
        slug_loading_v2::provider::alloc_starlark_depset(&heap, set(paths), Vec::new()).to_value()
    };
    let compiler = alloc(&["compiler"]);
    let all = alloc(&["all"]);
    let without = alloc(&["without"]);
    let empty = alloc(&[]);
    let fields = |without, compiler| {
        vec![
            (
                CompactString::new("_compiler_files_without_includes"),
                without,
            ),
            (CompactString::new("_compiler_files"), compiler),
            (CompactString::new("all_files"), all),
        ]
    };
    for specific in [false, true] {
        let rows = fields(without, compiler);
        let (value, selected) =
            compiler_inputs(&rows, specific, &mut AnalysisValueLowerer::default()).unwrap();
        assert!(value.ptr_eq(if specific { compiler } else { all }));
        assert_eq!(
            selected.to_list(),
            set(&[if specific { "compiler" } else { "all" }]).to_list()
        );
        assert!(
            compiler_inputs(
                &fields(Value::new_none(), compiler),
                specific,
                &mut AnalysisValueLowerer::default()
            )
            .is_err()
        );
        assert!(
            compiler_inputs(
                &fields(empty, Value::new_none()),
                specific,
                &mut AnalysisValueLowerer::default()
            )
            .is_err()
        );
    }
    // Nonempty without-includes avoids fallback; all_files selection must not
    // inspect the unused malformed compiler field when specific tools is false.
    assert!(
        compiler_inputs(
            &fields(without, Value::new_none()),
            false,
            &mut AnalysisValueLowerer::default()
        )
        .is_ok()
    );
}

#[test]
fn native_compile_scanning_override_precedes_configured_requested_conjunction() {
    for configured in [false, true] {
        for requested in [false, true] {
            assert_eq!(
                should_scan(None, configured, requested),
                configured && requested
            );
            assert!(!should_scan(Some(false), configured, requested));
            assert!(should_scan(Some(true), configured, requested));
        }
    }
}
