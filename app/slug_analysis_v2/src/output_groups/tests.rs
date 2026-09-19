use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisValueKind;
use slug_build_api_v2::DefaultInfo;
use slug_build_api_v2::Depset;
use slug_build_api_v2::RetainedRunfiles;
use slug_identity_v2::CanonicalLabel;

use super::*;

fn files(order: DepsetOrder, names: &[&str]) -> AnalysisDepset {
    AnalysisDepset::new(
        order,
        names
            .iter()
            .map(|name| {
                AnalysisValue::artifact(AnalysisArtifact::Source(
                    CanonicalLabel::parse(&format!("@@//pkg:{name}")).unwrap(),
                ))
            })
            .collect(),
        vec![],
    )
    .unwrap()
}

fn paths(files: &AnalysisDepset) -> Vec<String> {
    files
        .to_list()
        .iter()
        .map(|value| match value.kind() {
            AnalysisValueKind::Artifact(artifact) => artifact.path().into_owned(),
            _ => panic!("non-artifact group"),
        })
        .collect()
}

#[test]
fn configured_completion_normalizes_stable_groups_without_replacing_default_files() {
    let explicit = files(DepsetOrder::Preorder, &["group_a", "group_b"]);
    let default_files = files(DepsetOrder::Default, &["default_file"]);
    let original = OutputGroupInfo::new([
        ("default", explicit.clone()),
        ("empty", AnalysisDepset::empty(DepsetOrder::Preorder)),
    ])
    .unwrap();
    let providers = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(DefaultInfo::from_files(default_files.clone()).unwrap()),
        ProviderValue::OutputGroupInfo(original.clone()),
    ])
    .unwrap();
    let result = complete_rule_output_groups(providers, vec![]).unwrap();
    assert_eq!(result.default_info().unwrap().files(), &default_files);
    let groups = result.output_group_info().unwrap().groups();
    assert_eq!(
        paths(groups.get("default").unwrap()),
        ["pkg/group_a", "pkg/group_b"]
    );
    assert_eq!(groups.get("default").unwrap().order(), DepsetOrder::Default);
    assert_eq!(groups.get("empty").unwrap().order(), DepsetOrder::Default);
    assert!(groups.get(HIDDEN).unwrap().is_empty());
    assert!(!groups.contains_key(VALIDATION));
    // The separately retained constructor/nested value is not rewritten.
    assert_eq!(original.groups().get("default").unwrap(), &explicit);
    assert_eq!(
        original.groups().get("empty").unwrap().order(),
        DepsetOrder::Preorder
    );
}

#[test]
fn empty_runfiles_names_and_data_files_are_not_hidden_artifacts() {
    let default_runfiles = RetainedRunfiles {
        empty_filenames: Depset::from_direct(DepsetOrder::Default, vec!["empty".to_owned()])
            .unwrap(),
        ..RetainedRunfiles::empty()
    };
    let data_runfiles = RetainedRunfiles {
        files: files(DepsetOrder::Default, &["data_only"]),
        ..RetainedRunfiles::empty()
    };
    let info = DefaultInfo::from_effective(
        AnalysisDepset::empty(DepsetOrder::Default),
        default_runfiles,
        data_runfiles,
        None,
    )
    .unwrap();
    let providers = ProviderCollection::new(vec![ProviderValue::DefaultInfo(info)]).unwrap();
    let result = complete_rule_output_groups(providers, vec![]).unwrap();
    let groups = result.output_group_info().unwrap().groups();
    assert_eq!(groups.len(), 1);
    assert!(groups.get(HIDDEN).unwrap().is_empty());
}
