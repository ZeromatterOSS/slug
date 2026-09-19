//! Alias publication uses exact prepared targets and durable source backing.

use super::*;

pub(super) fn stage(
    owner: &ConfiguredOutputOwner,
    plan: &PreparedActionPlan<'_>,
    index: usize,
    inputs: Option<&PreparedActionChainInputs>,
    reserved: &SmallSet<&str>,
    seen_backing: &mut SmallSet<PathBuf>,
) -> io::Result<PlannedActionOutputStaging> {
    let action = plan.actions()[index].action();
    let inputs =
        inputs.ok_or_else(|| io::Error::other("alias publication requires prepared sources"))?;
    let (target, source) = inputs
        .artifact_symlink_target(plan, index)
        .map_err(|error| io::Error::other(error.to_string()))?;
    let mut backing = Vec::new();
    if let Some(index) = source {
        let source = inputs
            .sources()
            .nth(index)
            .ok_or_else(|| io::Error::other("missing alias source"))?;
        let permissions = inputs
            .source_permissions(source)
            .map_err(|error| io::Error::other(error.to_string()))?;
        let path = source_backing_path(inputs.workspace_path(), source.digest(), permissions)?;
        if seen_backing.insert(path) {
            backing.push(SourceBacking::new(
                inputs.workspace_path(),
                source.digest(),
                permissions,
                &mut inputs.open_source(index)?,
            )?);
        }
    }
    Ok(PlannedActionOutputStaging {
        action_index: index,
        staging: owner.stage_alias(action, &target, reserved)?,
        backing,
        covered: Vec::new(),
    })
}
