//! Certified immutable native status/template metadata; evaluation is attempt-owned.
use slug_analysis_v2::ConfiguredBuildInfoActionView;

use super::super::workspace_status_inputs::*;
use super::build_info_template_inputs::BuildInfoTemplateInputs;
use super::build_info_template_inputs::BuildInfoTemplatePreparationKey;
use super::workspace_status_content::WorkspaceStatusContent;
use super::workspace_status_content::WorkspaceStatusContentKey;
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct BuildInfoNativeInputsKey(pub(super) BuildInfoTemplatePreparationKey);

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct BuildInfoNativeInputs {
    pub(super) template: Arc<BuildInfoTemplateInputs>,
    pub(super) status: Arc<WorkspaceStatusContent>,
}

fn error(value: impl fmt::Display) -> Arc<str> {
    Arc::from(value.to_string())
}

impl fmt::Display for BuildInfoNativeInputsKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "build-info-native-inputs:{}", self.0)
    }
}

#[async_trait]
impl Key for BuildInfoNativeInputsKey {
    type Value = PreparationOutcome<Result<Arc<BuildInfoNativeInputs>, Arc<str>>>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        ctx.store_evaluation_data(EventBatch::empty())
            .expect("one native BuildInfo batch");
        let result = async {
            let status = ctx
                .compute(&WorkspaceStatusContentKey(self.0.workspace().dupe()))
                .await
                .map_err(error)?
                .map_err(error)?;
            let template = match ctx.compute(&self.0).await.map_err(error)? {
                PreparationOutcome::Need(need) => return Ok(PreparationOutcome::Need(need)),
                PreparationOutcome::Complete(result) => result?,
            };
            let view = ConfiguredBuildInfoActionView::from_configured_action(
                template.action(),
                self.0.workspace(),
            )
            .map_err(error)?;
            let expected_status = match view.status().kind() {
                slug_build_api_v2::WorkspaceStatusArtifactKind::Stable => status.artifacts.stable(),
                slug_build_api_v2::WorkspaceStatusArtifactKind::Volatile => {
                    status.artifacts.volatile()
                }
            };
            if view.status() != expected_status {
                return Err(error("native BuildInfo status descriptor differs"));
            }
            Ok(PreparationOutcome::Complete(Arc::new(
                BuildInfoNativeInputs { template, status },
            )))
        }
        .await;
        match result {
            Ok(outcome) => outcome.map(Ok),
            Err(error) => PreparationOutcome::Complete(Err(error)),
        }
    }
    fn equality(left: &Self::Value, right: &Self::Value) -> bool {
        Self::validity(left) && Self::validity(right) && left.complete_eq(right)
    }
    fn validity(value: &Self::Value) -> bool {
        matches!(value, PreparationOutcome::Complete(Ok(_)))
    }
}

pub(super) async fn validate_capture(
    capture: &Arc<WorkspaceStatusCapture>,
    context: &NativeCommandCompletionContext<'_, '_>,
) -> Result<(), NativeDemandSessionError> {
    let command_capture = context.command.workspace_status.as_ref().ok_or_else(|| {
        NativeDemandSessionError::Computation(anyhow::anyhow!(StatusError::Unauthorized))
    })?;
    if !Arc::ptr_eq(capture, command_capture) {
        return Err(NativeDemandSessionError::Computation(anyhow::anyhow!(
            StatusError::ForeignCapture
        )));
    }
    let mut transaction = context.transaction.clone();
    let value = transaction
        .compute(&WorkspaceStatusCaptureKey(capture.workspace.dupe()))
        .await
        .map_err(|value| NativeDemandSessionError::Computation(anyhow::anyhow!(value)))?;
    if !matches!(value, WorkspaceStatusCaptureValue::Present(ref original) if Arc::ptr_eq(original, capture))
    {
        return Err(NativeDemandSessionError::Computation(anyhow::anyhow!(
            StatusError::ForeignCapture
        )));
    }
    Ok(())
}

#[async_trait]
impl NativeCommandRoot for BuildInfoNativeInputsKey {
    type Terminal = Arc<BuildInfoNativeInputs>;
    fn requires_workspace_status_inputs(&self) -> bool {
        true
    }
    fn initializes_request_revision(&self) -> bool {
        true
    }
    fn observations<'a>(&self, value: &'a Self::Terminal) -> Option<&'a PathObservationEpoch> {
        Some(value.template.observations())
    }
    fn source_certificate<'a>(&self, value: &'a Self::Terminal) -> Option<&'a SourceCertificate> {
        Some(value.template.certificate())
    }
    fn observed_selection_association(&self) -> ObservedSelectionAssociation {
        ObservedSelectionAssociation::SelectedDependencySuperset
    }
    fn event_reconciliation_policy(&self, _: &Self::Terminal) -> EventReconciliationPolicy {
        EventReconciliationPolicy::SourceCertifiedCurrentClosure
    }
    fn complete<'a>(
        &'a self,
        value: &'a mut Self::Terminal,
        context: NativeCommandCompletionContext<'a, '_>,
    ) -> impl std::future::Future<Output = Result<(), NativeDemandSessionError>> + 'a {
        async move { validate_capture(&value.status.capture, &context).await }
    }
    async fn compute(
        &self,
        tx: &mut dice::DiceTransaction,
    ) -> Result<PreparationOutcome<Self::Terminal>, NativeDemandSessionError> {
        match tx
            .compute(self)
            .await
            .map_err(|value| NativeDemandSessionError::Computation(anyhow::anyhow!(value)))?
        {
            PreparationOutcome::Need(need) => Ok(PreparationOutcome::Need(need)),
            PreparationOutcome::Complete(result) => result
                .map(PreparationOutcome::Complete)
                .map_err(|value| NativeDemandSessionError::Computation(anyhow::anyhow!("{value}"))),
        }
    }
}
