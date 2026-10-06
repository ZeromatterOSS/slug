//! Tracked installed BuildInfo template content, without status or execution authority.

use slug_analysis_v2::ConfiguredAction;
use slug_analysis_v2::ConfiguredBuildInfoActionView;
use slug_analysis_v2::ObservedCatalogSourceFile;
use slug_analysis_v2::resolve_catalog_source_input_observed;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::BuildInfoTransformKind;

use super::source_staging::checked_build_frontier;
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct BuildInfoTemplatePreparationKey {
    build: BuildCommandRootKey,
    owner: ConfiguredTargetKey,
    action: usize,
}

/// DICE metadata retains the selected action and original immutable catalog owner.
/// A held handle also keeps the evaluation graph alive. No filesystem lease is needed.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct BuildInfoTemplateInputs {
    evaluation: Arc<Result<BuildCommandEvaluation, BuildCommandError>>,
    owner: usize,
    action: usize,
    source: ObservedCatalogSourceFile,
    observations: PathObservationEpoch,
    certificate: SourceCertificate,
}

impl BuildInfoTemplateInputs {
    pub(super) fn evaluation(&self) -> &BuildCommandEvaluation {
        self.evaluation.as_ref().as_ref().unwrap()
    }

    pub(super) fn action(&self) -> &ConfiguredAction {
        &self.evaluation().action_closure.owners()[self.owner].actions()[self.action]
    }

    pub(super) fn source(&self) -> &ObservedCatalogSourceFile {
        &self.source
    }

    pub(super) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }

    pub(super) fn certificate(&self) -> &SourceCertificate {
        &self.certificate
    }
}

fn error(value: impl fmt::Display) -> Arc<str> {
    Arc::from(value.to_string())
}

pub(super) fn checked_template_label<'a>(
    action: &'a ConfiguredAction,
    workspace: &NormalizedAbsolutePath,
    owner: &ConfiguredTargetKey,
) -> Result<&'a CanonicalLabel, Arc<str>> {
    let view =
        ConfiguredBuildInfoActionView::from_configured_action(action, workspace).map_err(error)?;
    if view.owner() != owner {
        return Err(error(
            "BuildInfo template action differs from selected owner",
        ));
    }
    let AnalysisArtifact::Source(label) = view.template() else {
        return Err(error(
            "BuildInfo template preparation requires an installed Source template",
        ));
    };
    let target = match view.callback().kind() {
        BuildInfoTransformKind::Info => "non_volatile_file.h.template",
        BuildInfoTransformKind::Version => "volatile_file.h.template",
    };
    if label.package().repo().as_str() != "bazel_tools"
        || label.package().package().as_str() != "tools/build_defs/build_info/templates"
        || label.target().as_str() != target
    {
        return Err(error(
            "BuildInfo template differs from its installed callback role",
        ));
    }
    Ok(label)
}

impl BuildInfoTemplatePreparationKey {
    pub(super) fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.build.workspace
    }

    pub(super) fn new(
        build: BuildCommandRootKey,
        owner: ConfiguredTargetKey,
        action: usize,
    ) -> Self {
        Self {
            build,
            owner,
            action,
        }
    }

    async fn prepare(
        &self,
        ctx: &mut DiceComputations<'_>,
    ) -> Result<PreparationOutcome<Arc<BuildInfoTemplateInputs>>, Arc<str>> {
        let root = BuildCommandRootObservationKey::for_source_staging(self.build.clone())
            .ok_or_else(|| {
                error("BuildInfo template preparation requires an observed build root")
            })?;
        let terminal = match ctx.compute(&root).await.map_err(error)? {
            PreparationOutcome::Need(need) => return Ok(PreparationOutcome::Need(need)),
            PreparationOutcome::Complete(result) => result.map_err(error)?,
        };
        let observations =
            checked_build_frontier(Some(terminal.observations()), terminal.source_certificate())?;
        let evaluation = terminal.result;
        let build = evaluation.as_ref().as_ref().map_err(error)?;
        let owner = build
            .action_closure
            .owners()
            .iter()
            .position(|node| node.configured_target_key() == Some(&self.owner))
            .ok_or_else(|| error("BuildInfo template owner is absent from validated closure"))?;
        let action = build.action_closure.owners()[owner]
            .actions()
            .get(self.action)
            .ok_or_else(|| {
                error("BuildInfo template action ordinal is absent from validated closure")
            })?;
        let label = checked_template_label(action, &self.build.workspace, &self.owner)?;
        let source =
            match resolve_catalog_source_input_observed(ctx, &self.build.workspace, label).await {
                PreparationOutcome::Need(need) => return Ok(PreparationOutcome::Need(need)),
                PreparationOutcome::Complete(result) => {
                    result.map_err(|value| error(format!("{value:?}")))?
                }
            };
        source
            .result()
            .as_ref()
            .map_err(error)?
            .as_ref()
            .as_ref()
            .map_err(error)?;
        let observations =
            union_build_observations(&observations, source.observations()).map_err(error)?;
        let certificate = SourceCertificate::from_epoch(observations.dupe()).map_err(error)?;
        Ok(PreparationOutcome::Complete(Arc::new(
            BuildInfoTemplateInputs {
                evaluation,
                owner,
                action: self.action,
                source,
                observations,
                certificate,
            },
        )))
    }
}

impl fmt::Display for BuildInfoTemplatePreparationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "build-info-template-preparation:{}:{:?}:{}",
            self.build, self.owner, self.action
        )
    }
}

#[async_trait]
impl Key for BuildInfoTemplatePreparationKey {
    type Value = PreparationOutcome<Result<Arc<BuildInfoTemplateInputs>, Arc<str>>>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        ctx.store_evaluation_data(EventBatch::empty())
            .expect("one BuildInfo template preparation event batch");
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
impl NativeCommandRoot for BuildInfoTemplatePreparationKey {
    type Terminal = Arc<BuildInfoTemplateInputs>;
    fn initializes_request_revision(&self) -> bool {
        true
    }
    fn observations<'a>(&self, terminal: &'a Self::Terminal) -> Option<&'a PathObservationEpoch> {
        Some(terminal.observations())
    }
    fn source_certificate<'a>(
        &self,
        terminal: &'a Self::Terminal,
    ) -> Option<&'a SourceCertificate> {
        Some(terminal.certificate())
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
        let outcome = tx
            .compute(self)
            .await
            .map_err(|value| NativeDemandSessionError::Computation(anyhow::anyhow!("{value}")))?;
        match outcome {
            PreparationOutcome::Need(need) => Ok(PreparationOutcome::Need(need)),
            PreparationOutcome::Complete(result) => result
                .map(PreparationOutcome::Complete)
                .map_err(|value| NativeDemandSessionError::Computation(anyhow::anyhow!("{value}"))),
        }
    }
}
