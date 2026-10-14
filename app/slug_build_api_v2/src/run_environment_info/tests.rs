use super::*;
use crate::AnalysisValue;
use crate::AnalysisValueKind;
use crate::AnalysisValueType;
use crate::ProviderCollection;
use crate::ProviderValue;

fn info(pairs: &[(&str, &str)], inherited: &[&str]) -> RunEnvironmentInfo {
    RunEnvironmentInfo::new(
        pairs
            .iter()
            .map(|(key, value)| (CompactString::new(key), CompactString::new(value)))
            .collect(),
        inherited
            .iter()
            .map(|name| CompactString::new(name))
            .collect(),
    )
}
fn hash(info: &RunEnvironmentInfo) -> u64 {
    let mut state = StarlarkHasher::default();
    info.hash_fields(&mut state);
    state.finish()
}

#[test]
fn raw_order_and_duplicates_survive_field_equality_and_ordered_publication() {
    let a = info(&[("B", "two"), ("A", "one")], &["B", "X", "B"]);
    let reordered = info(&[("A", "one"), ("B", "two")], &["B", "X", "B"]);
    let fresh = info(&[("B", "two"), ("A", "one")], &["B", "X", "B"]);
    assert_eq!(
        a.environment()
            .keys()
            .map(CompactString::as_str)
            .collect::<Vec<_>>(),
        ["B", "A"]
    );
    assert_eq!(a.inherited_environment(), ["B", "X", "B"]);
    assert!(a.field_eq(&reordered));
    assert_eq!(hash(&a), hash(&reordered));
    assert_ne!(a, reordered);
    assert_eq!(a, fresh);
    for different in [
        info(&[("B", "changed"), ("A", "one")], &["B", "X", "B"]),
        info(&[("B", "two"), ("A", "one")], &["X", "B", "B"]),
        info(&[("B", "two"), ("A", "one")], &["B", "X"]),
    ] {
        assert!(!a.field_eq(&different));
        assert_ne!(a, different);
    }
    assert_eq!(RunEnvironmentInfo::empty(), RunEnvironmentInfo::empty());
    assert!(RunEnvironmentInfo::empty().field_eq(&info(&[], &[])));
}

#[test]
fn nested_native_values_separate_starlark_fields_from_publication_and_share_owners() {
    fn allocative<T: Allocative>() {}
    fn dupe<T: Dupe>() {}
    allocative::<RunEnvironmentInfo>();
    dupe::<RunEnvironmentInfo>();
    let original = info(&[("B", "two"), ("A", "one")], &["B", "B"]);
    let copy = original.dupe();
    assert!(Arc::ptr_eq(&original.0, &copy.0));
    let inherited = original.0.inherited_environment.clone();
    assert!(Arc::ptr_eq(&inherited, &copy.0.inherited_environment));
    let value = AnalysisValue::run_environment_info(original.dupe());
    assert_eq!(value.value_type(), AnalysisValueType::RunEnvironmentInfo);
    assert!(matches!(
        value.kind(),
        AnalysisValueKind::RunEnvironmentInfo(_)
    ));
    assert!(value.is_starlark_immutable());
    assert!(value.is_starlark_hashable());
    let reordered =
        AnalysisValue::run_environment_info(info(&[("A", "one"), ("B", "two")], &["B", "B"]));
    assert_eq!(value, reordered);
    assert_eq!(
        value.starlark_hash().unwrap(),
        reordered.starlark_hash().unwrap()
    );
    assert!(!value.publication_eq(&reordered));
    let restored =
        AnalysisValue::run_environment_info(info(&[("B", "two"), ("A", "one")], &["B", "B"]));
    assert!(
        AnalysisValue::tuple(vec![value.clone(), value])
            .publication_eq(&AnalysisValue::tuple(vec![restored.clone(), restored]))
    );
    let providers =
        ProviderCollection::from_values(vec![ProviderValue::RunEnvironmentInfo(original)], false)
            .unwrap();
    assert_eq!(providers.run_environment_info(), Some(&copy));
    let reordered_providers = ProviderCollection::from_values(
        vec![ProviderValue::RunEnvironmentInfo(info(
            &[("A", "one"), ("B", "two")],
            &["B", "B"],
        ))],
        false,
    )
    .unwrap();
    assert_ne!(providers, reordered_providers);
}

#[test]
fn launch_environment_participates_in_runfiles_support_publication() {
    use slug_configuration_v2::CanonicalStringMap;
    use slug_configuration_v2::CanonicalStringSet;
    use slug_configuration_v2::RetainedActionEnvironment;
    use slug_identity_v2::CanonicalLabel;

    use crate::AnalysisArtifact;
    use crate::RetainedRunfiles;
    use crate::RunfilesSupport;

    let artifact = |label: &str| AnalysisArtifact::Source(CanonicalLabel::parse(label).unwrap());
    let channels = |fallback: &str, inherited: &[&str]| {
        RetainedActionEnvironment::from_channels(
            CanonicalStringMap::from_pairs([("BOTH", fallback)]),
            CanonicalStringSet::from_values(inherited.iter().copied()),
        )
    };
    let original = RunfilesSupport {
        runfiles: RetainedRunfiles::empty(),
        tree: artifact("@@//:tool.runfiles"),
        input_manifest: artifact("@@//:tool.runfiles_manifest"),
        manifest: None,
        repo_mapping_manifest: None,
        launch_environment: channels("fallback", &["BOTH", "DYNAMIC"]),
    };
    assert_eq!(
        original.launch_environment.fixed().get("BOTH"),
        Some("fallback")
    );
    assert!(original.launch_environment.inherited().contains("BOTH"));
    let mut changed = original.clone();
    changed.launch_environment = channels("changed", &["BOTH", "DYNAMIC"]);
    assert_ne!(original, changed);
    changed.launch_environment = channels("fallback", &["DYNAMIC"]);
    assert_ne!(original, changed);
    changed.launch_environment = channels("fallback", &["BOTH", "DYNAMIC"]);
    assert_eq!(original, changed);
}
