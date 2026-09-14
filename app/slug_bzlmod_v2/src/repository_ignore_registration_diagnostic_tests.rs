use std::path::PathBuf;
use std::sync::Arc;

use super::*;

fn render(error: HostRepositoryIgnoreError) -> String {
    let mut text = String::new();
    error.write_registration_diagnostic(&mut text).unwrap();
    text
}

#[test]
fn repository_ignore_diagnostic_reuses_request_projection() {
    let text = render(HostRepositoryIgnoreError::RepositorySource(
        RepositorySourceFileError::WrongKind {
            repo_relative_path: Arc::new(PathBuf::from(".bazelignore")),
            actual: PathNodeKind::Directory,
        },
    ));
    assert_eq!(
        text,
        "RepositorySource.Request.WrongKind path=hex:2e62617a656c69676e6f7265 actual=Directory"
    );
}

#[test]
fn repository_ignore_diagnostic_omits_unbounded_native_message() {
    let text = render(HostRepositoryIgnoreError::NativeInvalid {
        logical_path: NormalizedAbsolutePath::new("/workspace/POISON/.bazelignore").unwrap(),
        message: "POISON".into(),
    });
    assert_eq!(
        text,
        "NativeInvalid path=hex:2f776f726b73706163652f504f49534f4e2f2e62617a656c69676e6f7265"
    );
    assert!(!text.contains("POISON"));
}
