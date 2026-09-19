//! Pure request projections of retained runfiles actions and certified source paths.

use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::RunfilesLayoutTarget;
use slug_build_api_v2::RunfilesSupport;
use slug_build_api_v2::RunfilesSupportActionSpec;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;

use super::*;
use crate::runtime::action_output_staging::source_backing_path;
use crate::runtime::dice::runfiles_manifest::paths;

/// Local support completion metadata. These bytes never enter a DICE value.
#[derive(Clone, Debug)]
pub enum PreparedRunfilesAction {
    Manifest {
        output: ActionOutput,
        bytes: Arc<[u8]>,
    },
    SymlinkTree {
        output: ActionOutput,
        bytes: Arc<[u8]>,
    },
    RunfilesTree {
        output: ActionOutput,
    },
}

pub(in crate::runtime) struct RunfilesLinks {
    pub workspace_name: String,
    pub entries: Vec<(String, Option<String>)>,
    pub materialized_sources: Vec<usize>,
}

impl PreparedActionChainInputs {
    pub(in crate::runtime) fn workspace_path(&self) -> &std::path::Path {
        self.workspace.as_path()
    }

    pub(in crate::runtime) fn source_permissions(
        &self,
        source: &SourceArtifactInput,
    ) -> Result<i32, Arc<str>> {
        let demand = PathObservationDemand::new(
            source.namespace(),
            source.real_path().clone(),
            PathObservationOperation::Lstat,
        );
        let Some(row) = self.observations.observations().get(&demand) else {
            return Err(error("runfiles source lacks an observed mode"));
        };
        match row.as_ref() {
            PathObservationResult::Lstat(PathOperationResult::Present(stat))
                if stat.permissions() >= 0 =>
            {
                Ok(stat.permissions())
            }
            _ => Err(error("runfiles source permissions are unavailable")),
        }
    }

    pub(super) fn target_path(
        &self,
        artifact: &AnalysisArtifact,
        declared: &paths::DeclaredOutputs<'_>,
    ) -> Result<String, Arc<str>> {
        match artifact {
            AnalysisArtifact::Source(label) => {
                let source = self
                    .sources()
                    .find(|source| source.label() == label)
                    .ok_or_else(|| error("runfiles source is absent from prepared observations"))?;
                match source.namespace() {
                    PathObservationNamespace::Host => {
                        paths::absolute_string(source.requested_path().as_path())
                    }
                    PathObservationNamespace::Materialization(_) => {
                        let path = source_backing_path(
                            self.workspace.as_path(),
                            source.digest(),
                            self.source_permissions(source)?,
                        )
                        .map_err(error)?;
                        paths::absolute_string(&path)
                    }
                }
            }
            AnalysisArtifact::Derived { .. } => {
                paths::generated_target(&self.workspace, declared, artifact)
            }
        }
    }

    /// Validate exact support ownership and all physical paths before any execution.
    pub fn runfiles_action(
        &self,
        index: usize,
    ) -> Result<Option<PreparedRunfilesAction>, Arc<str>> {
        let plan = self.plan()?;
        let action = plan
            .actions()
            .get(index)
            .ok_or_else(|| error("unknown planned action"))?
            .action();
        self.prepare_runfiles_action(action)
    }

    /// Pure projection from a step in this prepared plan; confers no execution authority.
    pub fn prepare_runfiles_action(
        &self,
        action: &slug_analysis_v2::ConfiguredAction,
    ) -> Result<Option<PreparedRunfilesAction>, Arc<str>> {
        let Some(spec) = action.runfiles_support_spec() else {
            return Ok(None);
        };
        let expected = match spec {
            RunfilesSupportActionSpec::RepoMappingManifest { support, .. } => {
                support.repo_mapping_manifest.as_ref()
            }
            RunfilesSupportActionSpec::SourceSymlinkManifest { support, .. } => {
                Some(&support.input_manifest)
            }
            RunfilesSupportActionSpec::SymlinkTree { support, .. } => support.manifest.as_ref(),
            RunfilesSupportActionSpec::RunfilesTree { support, .. } => Some(&support.tree),
        };
        if !matches!(expected, Some(AnalysisArtifact::Derived { owner, output })
            if owner == &action.context().owner().artifact_owner() && output == spec.output())
            || action.outputs() != [spec.output().clone()]
        {
            return Err(error(
                "runfiles action does not produce its exact support artifact",
            ));
        }
        self.validate_support(action, spec.support())?;
        // Includes late path conflicts even for metadata-only support actions.
        self.runfiles_links_for(spec.support())?;
        let output = spec.output().clone();
        let result = match spec {
            RunfilesSupportActionSpec::RepoMappingManifest { .. } => {
                PreparedRunfilesAction::Manifest {
                    output,
                    bytes: spec.repo_mapping_manifest_bytes().map_err(error)?.into(),
                }
            }
            RunfilesSupportActionSpec::SourceSymlinkManifest { .. } => {
                PreparedRunfilesAction::Manifest {
                    output,
                    bytes: self.source_manifest(spec.support())?.into(),
                }
            }
            RunfilesSupportActionSpec::SymlinkTree { .. } => PreparedRunfilesAction::SymlinkTree {
                output,
                bytes: self.source_manifest(spec.support())?.into(),
            },
            RunfilesSupportActionSpec::RunfilesTree { .. } => {
                PreparedRunfilesAction::RunfilesTree { output }
            }
        };
        Ok(Some(result))
    }

    fn validate_support(
        &self,
        action: &slug_analysis_v2::ConfiguredAction,
        support: &Arc<RunfilesSupport>,
    ) -> Result<(), Arc<str>> {
        let owner = action.context().owner();
        let expected_owner = owner.artifact_owner();
        let public = support
            .manifest
            .as_ref()
            .ok_or_else(|| error("runfiles public manifest is absent"))?;
        let mapping = support
            .repo_mapping_manifest
            .as_ref()
            .ok_or_else(|| error("runfiles repository manifest is absent"))?;
        let expected = [&support.input_manifest, public, mapping, &support.tree];
        for (kind, artifact) in expected.iter().enumerate() {
            let AnalysisArtifact::Derived {
                owner: artifact_owner,
                output,
            } = artifact
            else {
                return Err(error("runfiles support artifact is not derived"));
            };
            if artifact_owner != &expected_owner
                || output.kind()
                    != if kind == 3 {
                        ActionOutputKind::RunfilesTree
                    } else {
                        ActionOutputKind::File
                    }
            {
                return Err(error("runfiles support artifact owner or kind differs"));
            }
            let mut matching = self
                .evaluation()
                .map_err(error)?
                .analyses()
                .filter(|node| node.configured_target_key() == Some(owner))
                .flat_map(|node| node.actions())
                .filter(|candidate| candidate.outputs().contains(output));
            let candidate = matching
                .next()
                .ok_or_else(|| error("runfiles support producer is absent"))?;
            if matching.next().is_some()
                || candidate.context().owner() != owner
                || candidate.outputs() != [output.clone()]
            {
                return Err(error("runfiles support producer is ambiguous"));
            }
            let spec = candidate
                .runfiles_support_spec()
                .ok_or_else(|| error("runfiles artifact has the wrong producer family"))?;
            let expected_kind = matches!(
                (kind, spec),
                (0, RunfilesSupportActionSpec::SourceSymlinkManifest { .. })
                    | (1, RunfilesSupportActionSpec::SymlinkTree { .. })
                    | (2, RunfilesSupportActionSpec::RepoMappingManifest { .. })
                    | (3, RunfilesSupportActionSpec::RunfilesTree { .. })
            );
            if !expected_kind || spec.support() != support || spec.output() != output {
                return Err(error(
                    "runfiles artifacts do not share the exact retained support",
                ));
            }
        }
        let tree_path = support.tree.path();
        if public.path() != format!("{tree_path}/MANIFEST") {
            return Err(error(
                "runfiles public manifest is outside its support tree",
            ));
        }
        Ok(())
    }

    pub(super) fn warning_events(&self) -> Result<EventBatch, Arc<str>> {
        use slug_build_api_v2::RunfilesConflictPolicy;
        use slug_build_api_v2::RunfilesLayoutDiagnostic;
        use slug_events_v2::EvaluationDiagnosticLevel;
        use slug_events_v2::EvaluationEvent;
        let mut events = Vec::new();
        if self.evaluation().is_err() {
            return Ok(EventBatch::empty());
        }
        for step in self.plan()?.actions() {
            let Some(RunfilesSupportActionSpec::SourceSymlinkManifest { support, .. }) =
                step.action().runfiles_support_spec()
            else {
                continue;
            };
            for diagnostic in support.layout().map_err(error)?.diagnostics() {
                if let RunfilesLayoutDiagnostic::Obscured {
                    path,
                    artifact,
                    ancestor_path,
                    ancestor,
                    policy: RunfilesConflictPolicy::Warn,
                } = diagnostic
                {
                    events.push(EvaluationEvent::Diagnostic {
                        level: EvaluationDiagnosticLevel::Warning,
                        text: format!(
                            "runfiles symlink {path} -> {} obscured by {ancestor_path} -> {}",
                            artifact.path(),
                            ancestor.path()
                        )
                        .into(),
                    });
                }
            }
        }
        Ok(EventBatch::from_events(events))
    }

    fn source_manifest(&self, support: &RunfilesSupport) -> Result<Vec<u8>, Arc<str>> {
        let declared = paths::declared_outputs(self.evaluation().map_err(error)?.analyses())?;
        support
            .layout()
            .map_err(error)?
            .source_manifest_bytes(|artifact| self.target_path(artifact, &declared))
            .map_err(error)
    }

    pub(in crate::runtime) fn runfiles_links(
        &self,
        index: usize,
    ) -> Result<RunfilesLinks, Arc<str>> {
        let plan = self.plan()?;
        let spec = plan
            .actions()
            .get(index)
            .and_then(|step| step.action().runfiles_support_spec())
            .ok_or_else(|| error("runfiles tree has no support"))?;
        self.runfiles_links_for(spec.support())
    }

    fn runfiles_links_for(&self, support: &RunfilesSupport) -> Result<RunfilesLinks, Arc<str>> {
        let declared = paths::declared_outputs(self.evaluation().map_err(error)?.analyses())?;
        let layout = support.layout().map_err(error)?;
        for artifact in layout.constituents() {
            if let AnalysisArtifact::Derived { output, .. } = artifact {
                if !matches!(
                    output.kind(),
                    ActionOutputKind::File | ActionOutputKind::Directory
                ) {
                    return Err(error(
                        "nested runfiles trees and unresolved symlinks remain unsupported",
                    ));
                }
                paths::generated_target(&self.workspace, &declared, artifact)?;
            }
        }
        // The encoder owns diagnostic severity and unsupported-target admission.
        layout
            .source_manifest_bytes(|artifact| self.target_path(artifact, &declared))
            .map_err(error)?;
        let mut materialized_sources = SmallSet::new();
        let mut entries = Vec::with_capacity(layout.entries().len() + 1);
        for entry in layout.entries() {
            let target = match entry.target() {
                RunfilesLayoutTarget::Empty => None,
                RunfilesLayoutTarget::Artifact(artifact) => {
                    if let AnalysisArtifact::Source(label) = artifact {
                        let index = self
                            .sources()
                            .position(|source| source.label() == label)
                            .ok_or_else(|| error("runfiles source is absent"))?;
                        if matches!(
                            self.sources().nth(index).unwrap().namespace(),
                            PathObservationNamespace::Materialization(_)
                        ) {
                            materialized_sources.insert(index);
                        }
                    }
                    Some(self.target_path(artifact, &declared)?)
                }
            };
            if entry.path() != "MANIFEST" {
                entries.push((entry.path().to_owned(), target));
            }
        }
        entries.push((
            "MANIFEST".to_owned(),
            Some(self.target_path(&support.input_manifest, &declared)?),
        ));
        let workspace_name = support.runfiles.repository_prefix.to_string();
        crate::runtime::action_output_staging::validate_runfiles_layout(&workspace_name, &entries)
            .map_err(error)?;
        Ok(RunfilesLinks {
            workspace_name,
            entries,
            materialized_sources: materialized_sources.into_iter().collect(),
        })
    }
}
