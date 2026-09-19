//! Borrowed dependency planning over a validated configured closure. No execution.

use std::sync::Arc;

use slug_analysis_v2::ConfiguredAction;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::AnalysisArtifact;
use starlark_map::small_map::SmallMap;

use super::configured_action_closure::ValidatedActionClosure;
use super::configured_action_closure::scalar_file_write;
use super::dice::source_staging::declared_artifacts;

type Coordinate = (usize, usize);

/// A declared input and, for generated inputs, its producer in `actions()`.
/// Artifact leaves are projection-owned; their retained depset graphs are not copied.
#[derive(Debug)]
pub struct PlannedActionInput {
    artifact: AnalysisArtifact,
    producer: Option<usize>,
}
impl PlannedActionInput {
    pub fn artifact(&self) -> &AnalysisArtifact {
        &self.artifact
    }
    pub fn producer(&self) -> Option<usize> {
        self.producer
    }
}

#[derive(Debug)]
pub struct PlannedAction<'a> {
    action: &'a ConfiguredAction,
    inputs: Vec<PlannedActionInput>,
}
impl<'a> PlannedAction<'a> {
    pub fn action(&self) -> &'a ConfiguredAction {
        self.action
    }
    pub fn inputs(&self) -> &[PlannedActionInput] {
        &self.inputs
    }
}

/// Prerequisite-first declared action plan. The selected execution representative
/// is last. This borrows a validated evaluation, not current execution authority.
#[derive(Debug)]
pub struct ActionPrerequisitePlan<'a> {
    actions: Vec<PlannedAction<'a>>,
}
impl<'a> ActionPrerequisitePlan<'a> {
    pub fn actions(&self) -> &[PlannedAction<'a>] {
        &self.actions
    }

    pub(super) fn new(
        closure: &'a ValidatedActionClosure,
        selected_owner: &ConfiguredTargetKey,
        selected_action: usize,
    ) -> Result<Self, Arc<str>> {
        let owners = closure.owners();
        let selected = owners
            .iter()
            .position(|owner| owner.configured_target_key() == Some(selected_owner))
            .ok_or("selected action owner is absent from validated closure")?;
        if owners[selected].actions().get(selected_action).is_none() {
            return Err("selected action ordinal is absent from validated closure".into());
        }
        // One owner projection per node, using Analysis's artifact producer.
        let artifact_owners = owners
            .iter()
            .map(|owner| owner.configured_target_key().unwrap().artifact_owner())
            .collect::<Vec<_>>();
        let mut owner_index = SmallMap::new();
        let mut declared = SmallMap::new();
        let mut physical = SmallMap::new();
        for (owner, node) in owners.iter().enumerate() {
            if owner_index.insert(&artifact_owners[owner], owner).is_some() {
                return Err("ambiguous retained artifact owner in validated closure".into());
            }
            for (action, value) in node.actions().iter().enumerate() {
                for output in value.outputs() {
                    if declared
                        .insert((&artifact_owners[owner], output), (owner, action))
                        .is_some()
                    {
                        return Err("ambiguous generated output producer".into());
                    }
                    if closure.execution_representative(owner, action) {
                        physical.insert(
                            (artifact_owners[owner].configuration(), output),
                            (owner, action),
                        );
                    }
                }
            }
        }
        // Canonicalize only coordinates already admitted as shared by the
        // closure. Exact declared owner/output lookup always precedes this.
        let canonical = |coordinate: Coordinate| -> Result<Coordinate, Arc<str>> {
            let (owner, action) = coordinate;
            if closure.execution_representative(owner, action) {
                return Ok(coordinate);
            }
            let value = &owners[owner].actions()[action];
            if !scalar_file_write(value) {
                return Err("unsupported shared action prerequisite".into());
            }
            physical
                .get(&(artifact_owners[owner].configuration(), &value.outputs()[0]))
                .copied()
                .ok_or_else(|| "shared action representative is absent".into())
        };
        type Inputs = Vec<(AnalysisArtifact, Option<Coordinate>)>;
        enum Visit {
            Enter(Coordinate),
            Exit(Coordinate, Inputs),
        }
        let mut pending = vec![Visit::Enter(canonical((selected, selected_action))?)];
        // None means on the current DFS path; Some is the emitted plan index.
        let mut states: SmallMap<Coordinate, Option<usize>> = SmallMap::new();
        let mut actions = Vec::new();
        while let Some(visit) = pending.pop() {
            match visit {
                Visit::Enter(coordinate) => {
                    match states.get(&coordinate) {
                        Some(Some(_)) => continue,
                        Some(None) => return Err("cycle in generated action prerequisites".into()),
                        None => {}
                    }
                    states.insert(coordinate, None);
                    let action = &owners[coordinate.0].actions()[coordinate.1];
                    if action.outputs().iter().any(|output| {
                        !matches!(
                            output.kind(),
                            ActionOutputKind::File | ActionOutputKind::Directory
                        )
                    }) {
                        return Err("unsupported generated prerequisite output kind".into());
                    }
                    let artifacts = if let Some(spawn) = action.spawn_spec() {
                        declared_artifacts(spawn)?
                    } else if scalar_file_write(action) {
                        Default::default()
                    } else {
                        return Err("unsupported action family in generated prerequisites".into());
                    };
                    let mut inputs = Vec::with_capacity(artifacts.len());
                    for artifact in artifacts {
                        let producer = match &artifact {
                            AnalysisArtifact::Source(_) => None,
                            AnalysisArtifact::Derived { owner, output } => {
                                if !matches!(
                                    output.kind(),
                                    ActionOutputKind::File | ActionOutputKind::Directory
                                ) {
                                    return Err(
                                        "unsupported generated prerequisite artifact kind".into()
                                    );
                                }
                                let declared =
                                    declared.get(&(owner, output)).copied().ok_or_else(|| {
                                        Arc::<str>::from(format!(
                                            "missing generated producer for {}:{} ({:?})",
                                            owner.label(),
                                            output.path(),
                                            output.kind()
                                        ))
                                    })?;
                                Some(canonical(declared)?)
                            }
                        };
                        inputs.push((artifact, producer));
                    }
                    let prerequisites = inputs
                        .iter()
                        .filter_map(|(_, producer)| *producer)
                        .collect::<Vec<_>>();
                    pending.push(Visit::Exit(coordinate, inputs));
                    pending.extend(prerequisites.into_iter().rev().map(Visit::Enter));
                }
                Visit::Exit(coordinate, inputs) => {
                    let inputs = inputs
                        .into_iter()
                        .map(|(artifact, producer)| PlannedActionInput {
                            artifact,
                            producer: producer.map(|key| {
                                states
                                    .get(&key)
                                    .copied()
                                    .flatten()
                                    .expect("prerequisite emitted before consumer")
                            }),
                        })
                        .collect();
                    states.insert(coordinate, Some(actions.len()));
                    actions.push(PlannedAction {
                        action: &owners[coordinate.0].actions()[coordinate.1],
                        inputs,
                    });
                }
            }
        }
        Ok(Self { actions })
    }
}

#[cfg(test)]
mod tests;
