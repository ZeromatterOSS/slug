use std::path::PathBuf;
use std::sync::Arc;

use super::*;

fn render(error: HostRouteRepoFileError) -> String {
    let mut text = String::new();
    error.write_registration_diagnostic(&mut text).unwrap();
    text
}

#[test]
fn route_repo_file_diagnostic_reuses_request_projection() {
    let text = render(HostRouteRepoFileError::Source(
        RepositorySourceFileError::WrongKind {
            repo_relative_path: Arc::new(PathBuf::from("REPO.bazel")),
            actual: slug_workspace_v2::PathNodeKind::Directory,
        },
    ));
    assert_eq!(
        text,
        "Source.Request.WrongKind path=hex:5245504f2e62617a656c actual=Directory"
    );
}

#[test]
fn route_repo_file_diagnostic_omits_unbounded_evaluation_message() {
    let text = render(HostRouteRepoFileError::Evaluation(
        HostRepoFileError::Syntax {
            logical_path: NormalizedAbsolutePath::new("/workspace/POISON/REPO.bazel").unwrap(),
            message: "POISON".into(),
        },
    ));
    assert_eq!(
        text,
        "Evaluation.Syntax path=hex:2f776f726b73706163652f504f49534f4e2f5245504f2e62617a656c"
    );
    assert!(!text.contains("POISON"));
}
