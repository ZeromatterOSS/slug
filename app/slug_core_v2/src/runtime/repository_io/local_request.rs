//! Pure binding validation for normal Local requests; no filesystem authority.
use super::*;

pub(super) fn validate(
    workspace: &Path,
    request: &RepositoryMaterializationRequest,
) -> Result<(), RepositoryTransportError> {
    let label = &request.repo_spec.rule_id.bzl_file;
    if request.id.workspace.as_path() != workspace
        || label.mapping_id().is_some()
        || label.package().repo().as_str() != "bazel_tools"
        || label.package().package().as_str() != "tools/build_defs/repo"
        || label.target().as_str() != "local.bzl"
        || request.repo_spec.rule_id.rule_name != "local_repository"
    {
        return Err(unsupported("foreign native local repository request"));
    }
    let RepositoryMaterializationKind::Local { logical_root } = &request.kind else {
        return Err(unsupported(
            "native local repository request has wrong kind",
        ));
    };
    reject_extra_attributes(&request.repo_spec, &["path"])?;
    let path = Path::new(required_string(&request.repo_spec, "path")?);
    // Keep the Shared/legacy component domain: interior/trailing `.` is
    // normalized by Path, but leading relative `.` and any `..` are refused.
    // An absolute root alone is already admitted by CommandAbsolute.
    if path.as_os_str().is_empty()
        || path.components().any(|component| {
            !matches!(
                component,
                Component::Prefix(_) | Component::RootDir | Component::Normal(_)
            )
        })
    {
        return Err(unsupported(
            "native local repository path is not normalized",
        ));
    }
    let expected = NormalizedAbsolutePath::new(if path.is_absolute() {
        path.to_owned()
    } else {
        workspace.join(path)
    })
    .map_err(|_| unsupported("native local repository root is not absolute"))?;
    if logical_root != &expected {
        return Err(unsupported(
            "native local repository root differs from request",
        ));
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
#[path = "local_request/tests.rs"]
mod tests;
