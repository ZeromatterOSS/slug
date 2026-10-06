//! Source-certified native publication over the original combined BuildInfo owner.
use std::io;

use slug_build_api_v2::WorkspaceStatusArtifactKind;

use super::build_info_native_inputs::BuildInfoNativeInputs;
use super::build_info_native_inputs::BuildInfoNativeInputsKey;
use super::workspace_status_content::WorkspaceStatusContent;
use super::*;
use crate::runtime::workspace_status_publication_file::PublicationOutcome;
use crate::runtime::workspace_status_publication_file::Staging;

#[derive(Clone)]
pub(super) struct WorkspaceStatusPublicationRoot(pub(super) BuildInfoNativeInputsKey);
#[derive(Clone)]
pub(super) struct WorkspaceStatusPublicationTerminal {
    pub(super) inputs: Arc<BuildInfoNativeInputs>,
    pub(super) pending: Option<Arc<Mutex<Staging>>>,
    pub(super) outcome: Option<PublicationOutcome>,
}

impl WorkspaceStatusPublicationRoot {
    pub(super) fn publish_with_files(
        &self,
        terminal: &mut WorkspaceStatusPublicationTerminal,
    ) -> io::Result<Arc<super::workspace_status_file_inputs::WorkspaceStatusFileInputs>> {
        if terminal.outcome.is_some() {
            return Err(io::Error::other("status publication already completed"));
        }
        let pending = terminal
            .pending
            .as_ref()
            .ok_or_else(|| io::Error::other("status publication has no stage"))?;
        let mut stage = pending
            .lock()
            .map_err(|_| io::Error::other("status stage lock poisoned"))?;
        let status = &terminal.inputs.status;
        let (outcome, files) = stage.publish_with_files(
            self.0.0.workspace().as_path(),
            &status.stable,
            &status.volatile,
        )?;
        terminal.outcome = Some(outcome);
        drop(stage);
        terminal.pending = None;
        Ok(Arc::new(
            super::workspace_status_file_inputs::WorkspaceStatusFileInputs::issued(
                status.clone(),
                files,
            ),
        ))
    }
}

pub(super) fn validate_publication_namespace(
    workspace: &NormalizedAbsolutePath,
    certificate: &SourceCertificate,
) -> io::Result<()> {
    let output = workspace.as_path().join("bazel-out");
    if certificate
        .observations()
        .observations()
        .keys()
        .any(|demand| demand.path().as_path().starts_with(&output))
    {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "status publication overlaps observed source namespace",
        ));
    }
    Ok(())
}
pub(super) fn validate_descriptors(
    workspace: &NormalizedAbsolutePath,
    status: &WorkspaceStatusContent,
) -> io::Result<()> {
    if &status.capture.workspace != workspace
        || status.artifacts.stable().root().workspace() != workspace
        || status.artifacts.volatile().root().workspace() != workspace
        || status.artifacts.stable().kind() != WorkspaceStatusArtifactKind::Stable
        || status.artifacts.volatile().kind() != WorkspaceStatusArtifactKind::Volatile
    {
        return Err(io::Error::other(
            "status publication descriptor association differs",
        ));
    }
    Ok(())
}
fn computation(error: impl std::fmt::Display) -> NativeDemandSessionError {
    NativeDemandSessionError::Computation(anyhow::anyhow!("{error}"))
}
#[async_trait]
impl NativeCommandRoot for WorkspaceStatusPublicationRoot {
    type Terminal = WorkspaceStatusPublicationTerminal;
    fn requires_workspace_status_inputs(&self) -> bool {
        self.0.requires_workspace_status_inputs()
    }
    fn initializes_request_revision(&self) -> bool {
        self.0.initializes_request_revision()
    }
    fn observations<'a>(&self, value: &'a Self::Terminal) -> Option<&'a PathObservationEpoch> {
        self.0.observations(&value.inputs)
    }
    fn source_certificate<'a>(&self, value: &'a Self::Terminal) -> Option<&'a SourceCertificate> {
        self.0.source_certificate(&value.inputs)
    }
    fn observed_selection_association(&self) -> ObservedSelectionAssociation {
        self.0.observed_selection_association()
    }
    fn event_reconciliation_policy(&self, value: &Self::Terminal) -> EventReconciliationPolicy {
        self.0.event_reconciliation_policy(&value.inputs)
    }
    async fn compute(
        &self,
        tx: &mut dice::DiceTransaction,
    ) -> Result<PreparationOutcome<Self::Terminal>, NativeDemandSessionError> {
        Ok(NativeCommandRoot::compute(&self.0, tx)
            .await?
            .map(|inputs| WorkspaceStatusPublicationTerminal {
                inputs,
                pending: None,
                outcome: None,
            }))
    }
    fn complete<'a>(
        &'a self,
        terminal: &'a mut Self::Terminal,
        context: NativeCommandCompletionContext<'a, '_>,
    ) -> impl std::future::Future<Output = Result<(), NativeDemandSessionError>> + 'a {
        async move {
            let workspace = self.0.0.workspace();
            if workspace.as_path() != context.command.runtime.workspace
                || terminal.pending.is_some()
                || terminal.outcome.is_some()
            {
                return Err(computation(
                    "status publication requires original unstaged native terminal",
                ));
            }
            // Original Arc/transaction association is checked before all filesystem IO.
            self.0.complete(&mut terminal.inputs, context).await?;
            validate_descriptors(workspace, &terminal.inputs.status).map_err(computation)?;
            validate_publication_namespace(workspace, terminal.inputs.template.certificate())
                .map_err(computation)?;
            let status = &terminal.inputs.status;
            let stage = Staging::new(workspace.as_path(), &status.stable, &status.volatile)
                .map_err(computation)?;
            terminal.pending = Some(Arc::new(Mutex::new(stage)));
            Ok(())
        }
    }
    fn publish(&self, terminal: &mut Self::Terminal) -> io::Result<()> {
        if terminal.outcome.is_some() {
            return Err(io::Error::other("status publication already completed"));
        }
        let pending = terminal
            .pending
            .as_ref()
            .ok_or_else(|| io::Error::other("status publication has no stage"))?;
        let mut stage = pending
            .lock()
            .map_err(|_| io::Error::other("status stage lock poisoned"))?;
        let status = &terminal.inputs.status;
        terminal.outcome = Some(stage.publish(
            self.0.0.workspace().as_path(),
            &status.stable,
            &status.volatile,
        )?);
        drop(stage);
        terminal.pending = None;
        Ok(())
    }
    fn publication_completed(&self, terminal: &Self::Terminal) -> bool {
        terminal.outcome.is_some()
    }
}
