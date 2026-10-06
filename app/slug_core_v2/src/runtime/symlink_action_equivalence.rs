//! Borrowed comparison for the admitted platform-agnostic Artifact Symlink class.
//! Pinned Bazel 9.2 SymlinkAction excludes progress and owner platform facts;
//! full Slug artifact identity is a deliberately stricter comparison domain.

use slug_analysis_v2::ConfiguredAction;
use slug_analysis_v2::ConfiguredActionAspectProvenance;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::ActionOutputMetadata;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::SymlinkSpec;
use slug_build_api_v2::SymlinkTarget;

pub(super) fn admitted_artifact_symlink(action: &ConfiguredAction) -> Option<&SymlinkSpec> {
    let spec = action.symlink_spec()?;
    if !action.exec_properties().is_empty()
        || action.exec_group().is_some()
        || action.context().aspect() != ConfiguredActionAspectProvenance::Absent
        || spec.output().kind() != ActionOutputKind::File
        || spec.output().metadata() != ActionOutputMetadata::Ordinary
        || action.outputs() != std::slice::from_ref(spec.output())
    {
        return None;
    }
    match spec.target() {
        SymlinkTarget::Artifact {
            input,
            use_exec_root_for_source: false,
            ..
        } => match input {
            AnalysisArtifact::Source(_) => Some(spec),
            AnalysisArtifact::Derived { output, .. }
                if output.kind() == ActionOutputKind::File
                    && output.metadata() == ActionOutputMetadata::Ordinary =>
            {
                Some(spec)
            }
            _ => None,
        },
        _ => None,
    }
}

/// None preserves typed unsupported-family refusal; false is a proven conflict.
pub(super) fn equivalent_artifact_symlinks(
    left: &ConfiguredAction,
    right: &ConfiguredAction,
) -> Option<bool> {
    let left = admitted_artifact_symlink(left)?;
    let right = admitted_artifact_symlink(right)?;
    Some(left.output() == right.output() && left.target() == right.target())
}
