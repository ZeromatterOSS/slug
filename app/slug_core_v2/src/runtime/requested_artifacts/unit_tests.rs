use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::AnalysisConfiguredTargetKey;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::DefaultInfo;
use slug_build_api_v2::OutputGroupInfo;
use slug_build_api_v2::ProviderValue;
use slug_identity_v2::CanonicalLabel;

use super::*;

fn files(artifacts: &[AnalysisArtifact]) -> AnalysisDepset {
    AnalysisDepset::new(
        DepsetOrder::Default,
        artifacts
            .iter()
            .cloned()
            .map(AnalysisValue::artifact)
            .collect(),
        vec![],
    )
    .unwrap()
}

#[test]
fn selection_deduplicates_identity_without_collapsing_owner_configuration_or_kind() {
    let derived = |label: &str, configuration: &[u8], kind| AnalysisArtifact::Derived {
        owner: AnalysisConfiguredTargetKey::new(
            CanonicalLabel::parse(label).unwrap(),
            Arc::<[u8]>::from(configuration),
        ),
        output: ActionOutput::new("out", kind),
    };
    let distinct = vec![
        AnalysisArtifact::Source(CanonicalLabel::parse("@@//:out").unwrap()),
        derived("@@//:owner", b"target", ActionOutputKind::File),
        derived("@@//:other", b"target", ActionOutputKind::File),
        derived("@@//:owner", b"exec", ActionOutputKind::File),
        derived("@@//:owner", b"target", ActionOutputKind::Directory),
        derived("@@//:owner", b"target", ActionOutputKind::RunfilesTree),
    ];
    assert!(distinct.iter().all(|artifact| artifact.path() == "out"));
    let default = files(&distinct[..3]);
    let explicit = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::artifact(distinct[3].clone())],
        vec![files(&distinct[2..3])],
    )
    .unwrap();
    let providers = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(DefaultInfo::from_files(default.clone()).unwrap()),
        ProviderValue::OutputGroupInfo(
            OutputGroupInfo::new([
                ("default", explicit.clone()),
                ("temp_files_INTERNAL_", files(&distinct[4..])),
                (
                    "unrequested",
                    files(&[AnalysisArtifact::Source(
                        CanonicalLabel::parse("@@//:excluded").unwrap(),
                    )]),
                ),
            ])
            .unwrap(),
        ),
    ])
    .unwrap();
    let mut artifacts = SmallMap::new();
    let groups = select_groups(&providers, &mut artifacts).unwrap();
    assert_eq!(
        groups.iter().map(|group| group.name()).collect::<Vec<_>>(),
        ["default", "temp_files_INTERNAL_"]
    );
    assert_eq!(groups[0].artifact_indices(), [0, 1, 2, 3]);
    assert_eq!(groups[1].artifact_indices(), [4, 5]);
    assert_eq!(artifacts.keys().cloned().collect::<Vec<_>>(), distinct);
    // A repeated request shares artifact-table entries without losing membership.
    let repeated = select_groups(&providers, &mut artifacts).unwrap();
    assert_eq!(repeated[0].artifact_indices(), groups[0].artifact_indices());
    assert_eq!(repeated[1].artifact_indices(), groups[1].artifact_indices());
    assert_eq!(artifacts.len(), 6);
    assert_eq!(providers.default_info().unwrap().files(), &default);
    assert_eq!(
        providers
            .output_group_info()
            .unwrap()
            .groups()
            .get("default"),
        Some(&explicit)
    );
}
