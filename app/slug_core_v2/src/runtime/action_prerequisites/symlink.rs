//! Artifact-target aliases retain one exact dependency; unresolved paths are separate.

use slug_build_api_v2::SymlinkSpec;
use slug_build_api_v2::SymlinkTarget;

use super::*;

pub(in crate::runtime) fn target<'a>(
    action: &ConfiguredAction,
    spec: &'a SymlinkSpec,
) -> Result<&'a AnalysisArtifact, Arc<str>> {
    if !cfg!(all(target_os = "linux", target_env = "gnu")) {
        return Err("artifact symlinks require Linux GNU".into());
    }
    if spec.output().kind() != ActionOutputKind::File || action.outputs() != [spec.output().clone()]
    {
        return Err("artifact symlink must declare exactly one regular File output".into());
    }
    for path in std::iter::once(spec.output().path()).chain(match spec.target() {
        SymlinkTarget::Artifact {
            input: AnalysisArtifact::Derived { output, .. },
            ..
        } => Some(output.path()),
        _ => None,
    }) {
        if path.split('/').count() > 256
            || path.split('/').any(|part| {
                part.is_empty() || matches!(part, "." | "..") || part.contains(['\\', '\0'])
            })
        {
            return Err("noncanonical artifact symlink path".into());
        }
    }
    if action
        .context()
        .owner()
        .configuration()
        .slug_configuration()
        .is_none()
    {
        return Err("artifact symlink requires structural configuration".into());
    }
    match spec.target() {
        SymlinkTarget::Artifact {
            input,
            use_exec_root_for_source: false,
            ..
        } if match input {
            AnalysisArtifact::Source(_) => true,
            AnalysisArtifact::Derived { output, .. } => output.kind() == ActionOutputKind::File,
        } =>
        {
            Ok(input)
        }
        _ => Err("unsupported artifact symlink target or source-root policy".into()),
    }
}
