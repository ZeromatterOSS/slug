//! Call-scoped lookup of exact declared artifacts and their native target paths.

use slug_analysis_v2::ConfiguredNodeResult;

use super::*;

pub(super) type DeclaredOutputs<'a> = SmallMap<AnalysisArtifact, &'a ConfiguredTargetKey>;

pub(super) fn declared_outputs<'a>(
    owners: impl Iterator<Item = &'a ConfiguredNodeResult>,
) -> Result<DeclaredOutputs<'a>, Arc<str>> {
    let mut declared = SmallMap::new();
    for node in owners {
        if node.actions().is_empty() {
            continue;
        }
        let key = node
            .configured_target_key()
            .ok_or_else(|| error("runfiles output producer has no configured owner"))?;
        let owner = key.artifact_owner();
        for action in node.actions() {
            if action.context().owner() != key {
                return Err(error(
                    "runfiles output producer context differs from its configured owner",
                ));
            }
            for output in action.outputs() {
                let artifact = AnalysisArtifact::Derived {
                    owner: owner.clone(),
                    output: output.clone(),
                };
                if declared.insert(artifact, key).is_some() {
                    return Err(error("ambiguous exact runfiles output producer"));
                }
            }
        }
    }
    Ok(declared)
}

pub(super) fn generated_target(
    workspace: &NormalizedAbsolutePath,
    declared: &DeclaredOutputs<'_>,
    artifact: &AnalysisArtifact,
) -> Result<String, Arc<str>> {
    let AnalysisArtifact::Derived { output, .. } = artifact else {
        return Err(error(
            "runfiles generated target requires a derived artifact",
        ));
    };
    let key = declared.get(artifact).ok_or_else(|| {
        error(format!(
            "runfiles artifact has no exact declared producer: {artifact:?}"
        ))
    })?;
    let configuration = key
        .configuration()
        .slug_configuration()
        .ok_or_else(|| error("runfiles output producer has no structural Slug configuration"))?;
    let path = crate::runtime::configured_output_root(workspace.as_path(), configuration)
        .join(output.path());
    absolute_string(&path)
}

pub(super) fn absolute_string(path: &std::path::Path) -> Result<String, Arc<str>> {
    if !path.is_absolute() {
        return Err(error("runfiles target path is not absolute"));
    }
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| error("runfiles target path is not valid Unicode"))
}
