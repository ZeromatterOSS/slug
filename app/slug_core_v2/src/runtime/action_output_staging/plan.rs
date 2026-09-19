//! Attempt-owned publication groups derived from authoritative producer bindings.

use slug_build_api_v2::AnalysisArtifact;
use starlark_map::small_map::SmallMap;

use super::*;
use crate::runtime::PreparedActionChainInputs;
use crate::runtime::PreparedActionPlan;

mod runfiles;
use crate::runtime::configured_output::ConfiguredOutputOwner;

/// Private destinations for one planned producer's selected output subset.
/// The ordinal refers to PreparedActionPlan::actions(), never a global action ID.
#[derive(Debug)]
pub struct PlannedActionOutputStaging {
    action_index: usize,
    staging: ActionOutputStaging,
    backing: Vec<SourceBacking>,
    covered: Vec<PublishedPlannedActionOutputs>,
}
impl PlannedActionOutputStaging {
    pub fn action_index(&self) -> usize {
        self.action_index
    }
    pub fn staging(&self) -> &ActionOutputStaging {
        &self.staging
    }
    pub(in crate::runtime) fn seal(&mut self) -> io::Result<()> {
        for backing in &mut self.backing {
            backing.seal()?;
        }
        self.staging.seal()
    }
    pub(in crate::runtime) fn publish_all(
        stages: &mut [Self],
    ) -> io::Result<Arc<[PublishedPlannedActionOutputs]>> {
        // Complete the batch preflight before the first visible replacement.
        // Retain every stage, including retired entries, until outside the
        // native revision lock. A later rename failure can still be partial.
        for stage in stages.iter() {
            for backing in &stage.backing {
                backing.preflight()?;
            }
            stage.staging.preflight()?;
        }
        for stage in stages.iter_mut() {
            for backing in &mut stage.backing {
                backing.publish()?;
            }
        }
        let mut published = Vec::new();
        // Stages are sorted so all backing artifacts precede physical trees.
        for stage in stages {
            published.push(PublishedPlannedActionOutputs {
                action_index: stage.action_index,
                outputs: stage.staging.publish()?,
            });
            published.extend(stage.covered.iter().cloned());
        }
        Ok(published.into())
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
    #[cfg(test)]
    pub(in crate::runtime) fn stage_planned_outputs(
        &self,
        plan: &PreparedActionPlan<'_>,
    ) -> io::Result<Vec<PlannedActionOutputStaging>> {
        self.stage_outputs_with_inputs(plan, None)
    }

    pub(in crate::runtime) fn stage_prepared_outputs(
        &self,
        inputs: &PreparedActionChainInputs,
    ) -> io::Result<Vec<PlannedActionOutputStaging>> {
        let plan = inputs
            .plan()
            .map_err(|error| io::Error::other(error.to_string()))?;
        self.stage_outputs_with_inputs(&plan, Some(inputs))
    }

    fn stage_outputs_with_inputs(
        &self,
        plan: &PreparedActionPlan<'_>,
        inputs: Option<&PreparedActionChainInputs>,
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
        let trees = runfiles::expand_groups(plan, inputs, &mut groups)?;
        // Exclude every selected path component, including other producers'
        // not-yet-created package parents, from each hidden sibling allocation.
        let reserved = groups
            .values()
            .flat_map(|outputs| outputs.iter())
            .flat_map(|output| output.path().split('/'))
            .collect();
        let mut stages = groups
            .iter()
            .filter(|(index, _)| !trees.iter().any(|tree| tree.index == **index))
            .map(|(index, outputs)| {
                let outputs = outputs.iter().cloned().collect::<Vec<_>>();
                Ok(PlannedActionOutputStaging {
                    action_index: *index,
                    staging: self.stage_output_subset(
                        plan.actions()[*index].action(),
                        &outputs,
                        &reserved,
                    )?,
                    backing: Vec::new(),
                    covered: Vec::new(),
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        runfiles::stage_trees(self, plan, inputs, trees, &reserved, &mut stages)?;
        Ok(stages)
    }
}

#[cfg(all(test, target_os = "linux", target_env = "gnu"))]
#[path = "plan_tests.rs"]
mod tests;
