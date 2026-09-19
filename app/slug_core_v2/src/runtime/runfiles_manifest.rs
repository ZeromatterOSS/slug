//! Observed, metadata-only preparation of retained runfiles manifest inputs.

use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::RunfilesSupport;
use slug_build_api_v2::RunfilesSupportActionSpec;

use super::source_staging::checked_build_frontier;
use super::*;
use crate::runtime::ObservedSourceArtifactInput;
use crate::runtime::SourceArtifactInput;
use crate::runtime::SourceArtifactInputObservationKey;

#[path = "runfiles_manifest/paths.rs"]
mod paths;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct RunfilesManifestPreparationKey {
    build: BuildCommandRootKey,
    owner: ConfiguredTargetKey,
}

/// Retained inputs for pure manifest byte projection. Native acceptance validates
/// this source frontier; later execution must validate again before publication.
/// This value authorizes neither filesystem reads nor output effects.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct PreparedRunfilesManifests {
    evaluation: Arc<Result<BuildCommandEvaluation, BuildCommandError>>,
    workspace: NormalizedAbsolutePath,
    owner: usize,
    mapping_action: usize,
    sources: Arc<[Arc<ObservedSourceArtifactInput>]>,
    observations: PathObservationEpoch,
    certificate: SourceCertificate,
}

impl PreparedRunfilesManifests {
    pub fn evaluation(&self) -> &BuildCommandEvaluation {
        self.evaluation.as_ref().as_ref().unwrap()
    }

    fn mapping_spec(&self) -> &RunfilesSupportActionSpec {
        self.evaluation().action_closure.owners()[self.owner].actions()[self.mapping_action]
            .runfiles_support_spec()
            .unwrap()
    }

    pub fn support(&self) -> &RunfilesSupport {
        self.mapping_spec().support()
    }

    pub fn sources(&self) -> impl ExactSizeIterator<Item = &SourceArtifactInput> {
        self.sources
            .iter()
            .map(|source| source.result().as_ref().unwrap())
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }

    pub fn source_manifest_bytes(&self) -> Result<Vec<u8>, Arc<str>> {
        let declared = paths::declared_outputs(self.evaluation().analyses())?;
        let sources = self
            .sources()
            .map(|source| (source.label(), source))
            .collect::<SmallMap<_, _>>();
        self.support()
            .layout()
            .map_err(error)?
            .source_manifest_bytes(|artifact| match artifact {
                AnalysisArtifact::Source(label) => {
                    let source = sources.get(label).ok_or_else(|| {
                        error("runfiles source is absent from prepared observations")
                    })?;
                    paths::require_host_namespace(source.namespace())?;
                    paths::absolute_string(source.requested_path().as_path())
                }
                AnalysisArtifact::Derived { .. } => {
                    paths::generated_target(&self.workspace, &declared, artifact)
                }
            })
            .map_err(error)
    }

    pub fn repo_mapping_manifest_bytes(&self) -> Result<Vec<u8>, Arc<str>> {
        self.mapping_spec()
            .repo_mapping_manifest_bytes()
            .map_err(error)
    }
}

fn error(value: impl fmt::Display) -> Arc<str> {
    Arc::from(value.to_string())
}

impl RunfilesManifestPreparationKey {
    async fn prepare(
        &self,
        ctx: &mut DiceComputations<'_>,
    ) -> Result<PreparationOutcome<Arc<PreparedRunfilesManifests>>, Arc<str>> {
        let root = BuildCommandRootObservationKey::for_source_staging(self.build.clone())
            .ok_or_else(|| {
                error("runfiles manifest preparation requires an observed build root")
            })?;
        let terminal = match ctx.compute(&root).await.map_err(error)? {
            PreparationOutcome::Need(need) => return Ok(PreparationOutcome::Need(need)),
            PreparationOutcome::Complete(result) => result.map_err(error)?,
        };
        let mut observations =
            checked_build_frontier(Some(terminal.observations()), terminal.source_certificate())?;
        let evaluation = terminal.result;
        let build = evaluation.as_ref().as_ref().map_err(error)?;
        let owner = build
            .action_closure
            .owners()
            .iter()
            .position(|node| node.configured_target_key() == Some(&self.owner))
            .ok_or_else(|| error("runfiles manifest owner is absent from validated closure"))?;
        let node = &build.action_closure.owners()[owner];
        let mut mappings = node
            .actions()
            .iter()
            .enumerate()
            .filter_map(|(index, action)| {
                matches!(
                    action.runfiles_support_spec(),
                    Some(RunfilesSupportActionSpec::RepoMappingManifest { .. })
                )
                .then_some(index)
            });
        let mapping_action = mappings
            .next()
            .ok_or_else(|| error("selected owner has no runfiles repository mapping action"))?;
        if mappings.next().is_some() {
            return Err(error(
                "selected owner has ambiguous runfiles repository mapping actions",
            ));
        }
        let support = node.actions()[mapping_action]
            .runfiles_support_spec()
            .unwrap()
            .support();
        let source_action = node.actions().iter().any(|action| {
            matches!(
                action.runfiles_support_spec(),
                Some(RunfilesSupportActionSpec::SourceSymlinkManifest {
                    support: candidate,
                    ..
                }) if candidate == support
            )
        });
        if !source_action {
            return Err(error(
                "selected owner has no matching runfiles source manifest action",
            ));
        }
        let layout = support.layout().map_err(error)?;
        let declared = paths::declared_outputs(build.analyses())?;
        let mut artifacts = SmallSet::new();
        // Constituents include obscured/overridden targets and support manifests.
        // The tree itself is excluded by layout, but also needs exact ownership.
        for artifact in layout
            .constituents()
            .iter()
            .chain(std::iter::once(&support.tree))
        {
            match artifact {
                AnalysisArtifact::Source(_) => {
                    artifacts.insert(artifact.clone());
                }
                AnalysisArtifact::Derived { .. } => {
                    paths::generated_target(&self.build.workspace, &declared, artifact)?;
                }
            }
        }
        drop(layout);
        drop(declared);
        let mut sources = Vec::with_capacity(artifacts.len());
        for artifact in &artifacts {
            let key = SourceArtifactInputObservationKey::new(self.build.workspace.dupe(), artifact)
                .map_err(|value| error(format!("{value:?}")))?;
            match ctx.compute(&key).await.map_err(error)? {
                PreparationOutcome::Need(need) => return Ok(PreparationOutcome::Need(need)),
                PreparationOutcome::Complete(result) => {
                    let source = result.map_err(error)?;
                    let input = source
                        .result()
                        .as_ref()
                        .map_err(|value| error(format!("{value:?}")))?;
                    paths::require_host_namespace(input.namespace())?;
                    observations = union_build_observations(&observations, source.observations())
                        .map_err(error)?;
                    sources.push(source);
                }
            }
        }
        let certificate = SourceCertificate::from_epoch(observations.dupe()).map_err(error)?;
        let prepared = Arc::new(PreparedRunfilesManifests {
            evaluation,
            workspace: self.build.workspace.dupe(),
            owner,
            mapping_action,
            sources: sources.into(),
            observations,
            certificate,
        });
        // Validate encodability before acceptance; bytes remain call-scoped scratch.
        prepared.source_manifest_bytes()?;
        prepared.repo_mapping_manifest_bytes()?;
        Ok(PreparationOutcome::Complete(prepared))
    }
}

impl fmt::Display for RunfilesManifestPreparationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "runfiles-manifest-preparation:{}:{:?}",
            self.build, self.owner
        )
    }
}

#[async_trait]
impl Key for RunfilesManifestPreparationKey {
    type Value = PreparationOutcome<Result<Arc<PreparedRunfilesManifests>, Arc<str>>>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        ctx.store_evaluation_data(EventBatch::empty())
            .expect("one runfiles manifest preparation event batch");
        match self.prepare(ctx).await {
            Ok(outcome) => outcome.map(Ok),
            Err(error) => PreparationOutcome::Complete(Err(error)),
        }
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        Self::validity(x) && Self::validity(y) && x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        matches!(value, PreparationOutcome::Complete(Ok(_)))
    }
}

#[async_trait]
impl NativeCommandRoot for RunfilesManifestPreparationKey {
    type Terminal = Arc<PreparedRunfilesManifests>;
    fn initializes_request_revision(&self) -> bool {
        true
    }
    fn observations<'a>(&self, terminal: &'a Self::Terminal) -> Option<&'a PathObservationEpoch> {
        Some(&terminal.observations)
    }
    fn source_certificate<'a>(
        &self,
        terminal: &'a Self::Terminal,
    ) -> Option<&'a SourceCertificate> {
        Some(&terminal.certificate)
    }
    fn observed_selection_association(&self) -> ObservedSelectionAssociation {
        ObservedSelectionAssociation::SelectedDependencySuperset
    }
    fn event_reconciliation_policy(&self, _: &Self::Terminal) -> EventReconciliationPolicy {
        EventReconciliationPolicy::SourceCertifiedCurrentClosure
    }
    async fn compute(
        &self,
        tx: &mut dice::DiceTransaction,
    ) -> Result<PreparationOutcome<Self::Terminal>, NativeDemandSessionError> {
        match tx
            .compute(self)
            .await
            .map_err(|value| NativeDemandSessionError::Computation(anyhow::anyhow!("{value}")))?
        {
            PreparationOutcome::Need(need) => Ok(PreparationOutcome::Need(need)),
            PreparationOutcome::Complete(result) => result
                .map(PreparationOutcome::Complete)
                .map_err(|value| NativeDemandSessionError::Computation(anyhow::anyhow!("{value}"))),
        }
    }
}

impl WorkspaceRuntime {
    /// Prepare runfiles manifest metadata without executing actions or publishing outputs.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_runfiles_manifests_with_repository_environment(
        &self,
        targets: &[TargetPattern],
        owner: ConfiguredTargetKey,
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
        lockfile_mode: LockfileMode,
        registry_urls: &[String],
        repository_environment: slug_bzlmod_v2::RepositoryEnvironmentSnapshot,
        configuration_overlay: CommandConfigurationOverlay,
    ) -> Result<AcceptedCommand<Arc<PreparedRunfilesManifests>>, BuildCommandError> {
        let (build, request) = self.prepare_build_request(
            targets,
            command_policy,
            environment_policy,
            lockfile_mode,
            registry_urls,
            repository_environment,
            configuration_overlay,
        )?;
        self.drive_command(request, RunfilesManifestPreparationKey { build, owner })
            .map(|driven| driven.accepted)
            .map_err(BuildCommandError::infrastructure)
    }
}

#[cfg(test)]
#[path = "runfiles_manifest/tests.rs"]
mod tests;
