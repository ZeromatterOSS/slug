use std::sync::Arc;

use super::*;

fn render(inner: RepositoryPackageSourceErrorInner) -> String {
    let error = RepositoryPackageSourceError::new(inner);
    let mut text = String::new();
    error.write_registration_diagnostic(&mut text).unwrap();
    text
}

fn package() -> PackageIdentifier {
    PackageIdentifier::new(
        CanonicalRepoName::new("dep+").unwrap(),
        PackagePath::parse("pkg").unwrap(),
    )
}

#[test]
fn repository_package_source_diagnostic_projects_safe_leaf_identity() {
    let package = package();
    assert_eq!(
        render(RepositoryPackageSourceErrorInner::Deleted {
            package: package.clone(),
        }),
        "Deleted package=@@dep+//pkg"
    );
    assert_eq!(
        render(RepositoryPackageSourceErrorInner::NoBuildFile {
            package: package.clone(),
        }),
        "NoBuildFile package=@@dep+//pkg"
    );
    assert_eq!(
        render(RepositoryPackageSourceErrorInner::LookupCompute {
            package,
            message: "POISON".into(),
        }),
        "LookupCompute package=@@dep+//pkg"
    );
}

#[test]
fn repository_package_source_diagnostic_reuses_request_projection() {
    let logical_path = Arc::new(PathBuf::from("/POISON/build-file"));
    let error = RepositorySourceFileError::WrongKind {
        repo_relative_path: Arc::new(PathBuf::from("cc/private/toolchain/BUILD.bazel")),
        actual: PathNodeKind::Directory,
    };
    let text = render(RepositoryPackageSourceErrorInner::Source {
        logical_path,
        error,
    });
    assert_eq!(
        text,
        "Source path=hex:2f504f49534f4e2f6275696c642d66696c65 error=Request.WrongKind path=hex:63632f707269766174652f746f6f6c636861696e2f4255494c442e62617a656c actual=Directory"
    );
    assert!(!text.contains("POISON"));
}
