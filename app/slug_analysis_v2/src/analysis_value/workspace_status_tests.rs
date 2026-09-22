use slug_build_api_v2::WorkspaceStatusArtifactKind;
use slug_build_api_v2::WorkspaceStatusArtifacts;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::*;

#[test]
fn workspace_status_file_publication_keeps_workspace_role_and_metadata_after_heap_disposal() {
    let workspace = NormalizedAbsolutePath::new("/fixture/real-workspace").unwrap();
    let descriptors = WorkspaceStatusArtifacts::new(workspace.clone());
    let values = AnalysisValue::tuple(vec![
        AnalysisValue::artifact(AnalysisArtifact::WorkspaceStatus(
            descriptors.stable().clone(),
        )),
        AnalysisValue::artifact(AnalysisArtifact::WorkspaceStatus(
            descriptors.volatile().clone(),
        )),
        AnalysisValue::artifact_root(slug_build_api_v2::AnalysisArtifactRoot::WorkspaceStatus(
            descriptors.stable().root().clone(),
        )),
    ]);
    let retained = {
        let source = FrozenHeap::new();
        let allocated = AnalysisValueMaterializer::new(&source)
            .value(&values)
            .unwrap();
        AnalysisValueLowerer::default()
            .lower(allocated.to_value(), "$source")
            .unwrap()
    };
    let heap = FrozenHeap::new();
    let restored = AnalysisValueMaterializer::new(&heap)
        .value(&retained)
        .unwrap();
    let retained_again = AnalysisValueLowerer::default()
        .lower(restored.to_value(), "$restored")
        .unwrap();
    assert!(values.publication_eq(&retained_again));
    let AnalysisValueKind::Tuple(parts) = retained_again.kind() else {
        panic!("tuple")
    };
    for (index, kind) in [
        WorkspaceStatusArtifactKind::Stable,
        WorkspaceStatusArtifactKind::Volatile,
    ]
    .into_iter()
    .enumerate()
    {
        let AnalysisValueKind::Artifact(AnalysisArtifact::WorkspaceStatus(file)) =
            parts[index].kind()
        else {
            panic!("workspace status File")
        };
        assert_eq!(file.root().workspace(), &workspace);
        assert_eq!(file.kind(), kind);
        assert_eq!(file.is_constant_metadata(), index == 1);
    }
    let AnalysisValueKind::ArtifactRoot(slug_build_api_v2::AnalysisArtifactRoot::WorkspaceStatus(
        root,
    )) = parts[2].kind()
    else {
        panic!("workspace status root")
    };
    assert_eq!(root.workspace(), &workspace);
}
