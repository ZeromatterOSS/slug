//! Attempt-owned publication groups derived from authoritative producer bindings.

use slug_build_api_v2::AnalysisArtifact;
use starlark_map::small_map::SmallMap;

use super::*;
use crate::runtime::PreparedActionPlan;
use crate::runtime::configured_output::ConfiguredOutputOwner;

/// Private destinations for one planned producer's selected output subset.
/// The ordinal refers to PreparedActionPlan::actions(), never a global action ID.
#[derive(Debug)]
pub struct PlannedActionOutputStaging {
    action_index: usize,
    staging: ActionOutputStaging,
}
impl PlannedActionOutputStaging {
    pub fn action_index(&self) -> usize {
        self.action_index
    }
    pub fn staging(&self) -> &ActionOutputStaging {
        &self.staging
    }
    pub(in crate::runtime) fn seal(&mut self) -> io::Result<()> {
        self.staging.seal()
    }
    pub(in crate::runtime) fn publish_all(
        stages: &mut [Self],
    ) -> io::Result<Arc<[PublishedPlannedActionOutputs]>> {
        // Complete the batch preflight before the first visible replacement.
        // Retain every stage, including retired entries, until outside the
        // native revision lock. A later rename failure can still be partial.
        for stage in stages.iter() {
            stage.staging.preflight()?;
        }
        stages
            .iter_mut()
            .map(|stage| {
                Ok(PublishedPlannedActionOutputs {
                    action_index: stage.action_index,
                    outputs: stage.staging.publish()?,
                })
            })
            .collect::<io::Result<Vec<_>>>()
            .map(Arc::from)
    }
}

/// Accepted publication metadata with its exact producer ordinal.
#[derive(Clone, Debug)]
pub struct PublishedPlannedActionOutputs {
    action_index: usize,
    outputs: PublishedActionOutputs,
}
impl PublishedPlannedActionOutputs {
    pub fn action_index(&self) -> usize {
        self.action_index
    }
    pub fn outputs(&self) -> &PublishedActionOutputs {
        &self.outputs
    }
}

impl ConfiguredOutputOwner {
    pub(in crate::runtime) fn stage_planned_outputs(
        &self,
        plan: &PreparedActionPlan<'_>,
    ) -> io::Result<Vec<PlannedActionOutputStaging>> {
        let mut groups: SmallMap<usize, SmallSet<ActionOutput>> = SmallMap::new();
        match plan {
            PreparedActionPlan::Selected(selected) => {
                let index = selected
                    .actions()
                    .len()
                    .checked_sub(1)
                    .ok_or_else(|| io::Error::other("selected action plan is empty"))?;
                groups.insert(
                    index,
                    selected.actions()[index]
                        .action()
                        .outputs()
                        .iter()
                        .cloned()
                        .collect(),
                );
            }
            PreparedActionPlan::Requested(requested) => {
                for (artifact, producer) in requested
                    .selection()
                    .artifacts()
                    .iter()
                    .zip(requested.artifact_producers())
                {
                    let AnalysisArtifact::Derived { owner, output } = artifact else {
                        continue;
                    };
                    let index = producer.ok_or_else(|| {
                        io::Error::other("selected derived artifact has no producer")
                    })?;
                    let action = requested
                        .actions()
                        .get(index)
                        .ok_or_else(|| io::Error::other("selected artifact producer is absent"))?
                        .action();
                    if owner.configuration()
                        != action.context().owner().artifact_owner().configuration()
                        || !action.outputs().contains(output)
                    {
                        return Err(io::Error::other(
                            "selected artifact differs from producer declaration or configuration",
                        ));
                    }
                    groups
                        .entry(index)
                        .or_insert_with(SmallSet::new)
                        .insert(output.clone());
                }
            }
        }
        // Exclude every selected path component, including other producers'
        // not-yet-created package parents, from each hidden sibling allocation.
        let reserved = groups
            .values()
            .flat_map(|outputs| outputs.iter())
            .flat_map(|output| output.path().split('/'))
            .collect();
        groups
            .iter()
            .map(|(index, outputs)| {
                let outputs = outputs.iter().cloned().collect::<Vec<_>>();
                Ok(PlannedActionOutputStaging {
                    action_index: *index,
                    staging: self.stage_output_subset(
                        plan.actions()[*index].action(),
                        &outputs,
                        &reserved,
                    )?,
                })
            })
            .collect()
    }
}

#[cfg(all(test, target_os = "linux", target_env = "gnu"))]
#[path = "plan_tests.rs"]
mod tests;
