use super::*;
use crate::AnalysisArtifact;
use crate::AnalysisArtifactRoot;
use crate::AnalysisDepset;
use crate::AnalysisValue;
use crate::DepsetOrder;

#[test]
fn workspace_status_identity_metadata_and_projections_are_structural() {
    let pair = WorkspaceStatusArtifacts::new(NormalizedAbsolutePath::new("/workspace").unwrap());
    let equivalent =
        WorkspaceStatusArtifacts::new(NormalizedAbsolutePath::new("/workspace/child/..").unwrap());
    let other = WorkspaceStatusArtifacts::new(NormalizedAbsolutePath::new("/other").unwrap());
    assert_eq!(pair, equivalent);
    assert_ne!(pair, other);
    assert_eq!(pair.stable().root(), pair.volatile().root());
    assert_ne!(pair.stable(), pair.volatile());
    assert!(!pair.stable().is_constant_metadata());
    assert!(pair.volatile().is_constant_metadata());
    assert_eq!(pair.stable().kind(), WorkspaceStatusArtifactKind::Stable);
    assert_eq!(
        pair.volatile().kind(),
        WorkspaceStatusArtifactKind::Volatile
    );
    assert_eq!(
        pair.stable().root().workspace().as_path(),
        std::path::Path::new("/workspace")
    );
    for value in [pair.stable(), pair.volatile()] {
        let artifact = AnalysisArtifact::WorkspaceStatus(value.clone());
        assert_eq!(artifact.path(), value.path());
        assert_eq!(artifact.short_path(), value.short_path());
        assert_eq!(artifact.dirname(), "bazel-out");
        assert_eq!(
            artifact.root(),
            AnalysisArtifactRoot::WorkspaceStatus(value.root().clone())
        );
        assert_eq!(artifact.root().path(), "bazel-out");
    }
    assert_eq!(pair.stable().path(), other.stable().path());
    assert_ne!(pair.stable().root(), other.stable().root());
    let values = [
        pair.stable(),
        equivalent.stable(),
        pair.volatile(),
        other.stable(),
    ]
    .into_iter()
    .map(|v| AnalysisValue::artifact(AnalysisArtifact::WorkspaceStatus(v.clone())))
    .collect();
    let depset = AnalysisDepset::new(DepsetOrder::Default, values, vec![]).unwrap();
    assert_eq!(depset.to_list().len(), 3);
}

#[cfg(unix)]
#[test]
fn workspace_status_preserves_nonunicode_workspace_identity() {
    use std::os::unix::ffi::OsStringExt;
    let make = |byte| {
        WorkspaceStatusArtifacts::new(
            NormalizedAbsolutePath::new(std::path::PathBuf::from(std::ffi::OsString::from_vec(
                vec![b'/', byte],
            )))
            .unwrap(),
        )
    };
    assert_eq!(make(0xff), make(0xff));
    assert_ne!(make(0xff), make(0xfe));
    assert_eq!(make(0xff).stable().path(), make(0xfe).stable().path());
}

#[test]
fn workspace_status_runfiles_refuse_before_resolver() {
    let status = AnalysisArtifact::WorkspaceStatus(
        WorkspaceStatusArtifacts::new(NormalizedAbsolutePath::new("/workspace").unwrap())
            .stable()
            .clone(),
    );
    let mut runfiles = crate::RetainedRunfiles::empty();
    runfiles.files = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::artifact(status.clone())],
        vec![],
    )
    .unwrap();
    // The support output slots are irrelevant to source-manifest projection;
    // status is deliberately present as a leaf in the ordinary runfiles set.
    let support = crate::RunfilesSupport {
        runfiles,
        tree: status.clone(),
        input_manifest: status,
        manifest: None,
        repo_mapping_manifest: None,
    };
    let layout = support.layout().unwrap();
    let result =
        layout.source_manifest_bytes::<String>(|_| panic!("must reject before resolution"));
    assert_eq!(
        result.unwrap_err(),
        crate::SourceManifestError::WorkspaceStatusUnsupported
    );
}
