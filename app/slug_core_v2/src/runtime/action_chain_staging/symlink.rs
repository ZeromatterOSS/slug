//! Pure alias metadata from the retained action and certified source observations.

use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::SymlinkTarget;

use super::*;
use crate::runtime::action_prerequisites::symlink::target;
use crate::runtime::dice::runfiles_manifest::paths;

#[derive(Clone, Debug)]
pub struct PreparedArtifactSymlink {
    output: ActionOutput,
    input: AnalysisArtifact,
    require_executable: bool,
    source_executable: Option<bool>,
}
impl PreparedArtifactSymlink {
    pub fn output(&self) -> &ActionOutput {
        &self.output
    }
    pub fn input(&self) -> &AnalysisArtifact {
        &self.input
    }
    pub fn require_executable(&self) -> bool {
        self.require_executable
    }
    pub fn source_executable(&self) -> Option<bool> {
        self.source_executable
    }
}

impl PreparedActionChainInputs {
    /// A pure projection, not permission to read sources, execute, or publish.
    pub fn prepare_artifact_symlink(
        &self,
        action: &slug_analysis_v2::ConfiguredAction,
    ) -> Result<Option<PreparedArtifactSymlink>, Arc<str>> {
        let Some(spec) = action.symlink_spec() else {
            return Ok(None);
        };
        let input = target(action, spec)?;
        let SymlinkTarget::Artifact {
            require_executable, ..
        } = spec.target()
        else {
            unreachable!()
        };
        let source_executable = if let AnalysisArtifact::Source(label) = input {
            let source = self
                .sources()
                .find(|source| source.label() == label)
                .ok_or_else(|| error("symlink source is absent from prepared observations"))?;
            Some(self.source_permissions(source)? & 0o100 != 0)
        } else {
            None
        };
        if *require_executable && source_executable == Some(false) {
            return Err(error("artifact symlink target is not executable"));
        }
        Ok(Some(PreparedArtifactSymlink {
            output: spec.output().clone(),
            input: input.clone(),
            require_executable: *require_executable,
            source_executable,
        }))
    }

    pub(in crate::runtime) fn artifact_symlink_target(
        &self,
        plan: &PreparedActionPlan<'_>,
        index: usize,
    ) -> Result<(String, Option<usize>), Arc<str>> {
        let step = &plan.actions()[index];
        let action = step.action();
        let alias = self
            .prepare_artifact_symlink(action)?
            .ok_or_else(|| error("expected an artifact symlink action"))?;
        let path = match alias.input() {
            AnalysisArtifact::Source(_) => self.target_path(alias.input(), &Default::default())?,
            AnalysisArtifact::Derived { owner, output } => {
                let input = step
                    .inputs()
                    .iter()
                    .find(|input| input.artifact() == alias.input())
                    .ok_or_else(|| error("alias target has no exact planned input"))?;
                let producer = input
                    .producer()
                    .and_then(|index| plan.actions().get(index))
                    .ok_or_else(|| error("alias target has no planned producer"))?
                    .action();
                if producer.context().owner().artifact_owner().configuration()
                    != owner.configuration()
                    || !producer.outputs().contains(output)
                {
                    return Err(error("alias target differs from exact producer binding"));
                }
                let configuration = producer
                    .context()
                    .owner()
                    .configuration()
                    .slug_configuration()
                    .ok_or_else(|| error("alias target requires structural configuration"))?;
                paths::absolute_string(
                    &crate::runtime::configured_output_root(self.workspace_path(), configuration)
                        .join(output.path()),
                )?
            }
        };
        let materialized = if let AnalysisArtifact::Source(label) = alias.input() {
            self.sources().enumerate().find_map(|(index, source)| {
                (source.label() == label
                    && matches!(
                        source.namespace(),
                        slug_workspace_v2::PathObservationNamespace::Materialization(_)
                    ))
                .then_some(index)
            })
        } else {
            None
        };
        Ok((path, materialized))
    }
}
