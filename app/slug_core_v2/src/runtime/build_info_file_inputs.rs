//! Current native publication reads actual status bytes into the checked renderer.
use std::io;

use slug_analysis_v2::ConfiguredBuildInfoActionView;

use super::build_info_native_inputs::BuildInfoNativeInputs;
use super::build_info_native_inputs::BuildInfoNativeInputsKey;
use super::workspace_status_file_inputs::WorkspaceStatusFileInputs;
use super::workspace_status_publication::WorkspaceStatusPublicationRoot;
use super::workspace_status_publication::WorkspaceStatusPublicationTerminal;
use super::*;

#[derive(Clone)]
pub(super) struct NativeBuildInfoFileInputsRoot {
    origin: Arc<WorkspaceStatusPublicationRoot>,
}
#[derive(Clone)]
pub(super) struct NativeBuildInfoFileInputsTerminal {
    pub(super) publication: WorkspaceStatusPublicationTerminal,
    original: Arc<BuildInfoNativeInputs>,
    origin: Arc<WorkspaceStatusPublicationRoot>,
    permit: Option<Arc<Mutex<Option<ReadPermit>>>>,
    pub(super) consumed: Option<Arc<ConsumedBuildInfo>>,
}
impl NativeBuildInfoFileInputsTerminal {
    pub(super) fn original(&self) -> &Arc<BuildInfoNativeInputs> {
        &self.original
    }
}
// The slot is shared by terminal clones; grants themselves cannot be cloned.
enum ReadPermit {
    Prepared(Arc<BuildInfoNativeInputs>),
    Published {
        inputs: Arc<BuildInfoNativeInputs>,
        files: Arc<WorkspaceStatusFileInputs>,
    },
}
#[derive(Debug)]
pub(super) struct ConsumedBuildInfo {
    pub(super) inputs: Arc<BuildInfoNativeInputs>,
    pub(super) files: Arc<WorkspaceStatusFileInputs>,
    pub(super) header: Vec<u8>,
}
impl NativeBuildInfoFileInputsRoot {
    pub(super) fn new(key: BuildInfoNativeInputsKey) -> Self {
        Self {
            origin: Arc::new(WorkspaceStatusPublicationRoot(key)),
        }
    }
    pub(super) fn check_original(
        &self,
        terminal: &NativeBuildInfoFileInputsTerminal,
    ) -> io::Result<()> {
        if !Arc::ptr_eq(&self.origin, &terminal.origin)
            || !Arc::ptr_eq(&terminal.original, &terminal.publication.inputs)
        {
            return Err(io::Error::other(
                "physical BuildInfo original issuance differs",
            ));
        }
        Ok(())
    }
    fn take_permit(
        &self,
        terminal: &NativeBuildInfoFileInputsTerminal,
        published: bool,
    ) -> io::Result<ReadPermit> {
        let permit = terminal
            .permit
            .as_ref()
            .ok_or_else(|| io::Error::other("physical BuildInfo has no read permit"))?
            .lock()
            .map_err(|_| io::Error::other("physical BuildInfo read permit poisoned"))?
            .take()
            .ok_or_else(|| io::Error::other("physical BuildInfo read permit spent"))?;
        // Every attempted use spends the shared grant, including binding/phase errors.
        // The lock is released before checks and synchronous filesystem work.
        self.check_original(terminal)?;
        if !matches!(
            (&permit, published),
            (ReadPermit::Prepared(_), false) | (ReadPermit::Published { .. }, true)
        ) {
            return Err(io::Error::other(
                "physical BuildInfo read permit phase differs",
            ));
        }
        Ok(permit)
    }
    pub(super) fn publish_files(
        &self,
        terminal: &mut NativeBuildInfoFileInputsTerminal,
    ) -> io::Result<Arc<WorkspaceStatusFileInputs>> {
        let ReadPermit::Prepared(inputs) = self.take_permit(terminal, false)? else {
            return Err(io::Error::other(
                "physical BuildInfo publication permit spent",
            ));
        };
        if !Arc::ptr_eq(&inputs, &terminal.original) {
            return Err(io::Error::other("physical BuildInfo permit owner differs"));
        }
        let files = self.origin.publish_with_files(&mut terminal.publication)?;
        *terminal
            .permit
            .as_ref()
            .unwrap()
            .lock()
            .map_err(|_| io::Error::other("physical BuildInfo read permit poisoned"))? =
            Some(ReadPermit::Published {
                inputs,
                files: files.clone(),
            });
        Ok(files)
    }
    pub(super) fn consume_files(
        &self,
        terminal: &mut NativeBuildInfoFileInputsTerminal,
        files: Arc<WorkspaceStatusFileInputs>,
    ) -> io::Result<()> {
        let ReadPermit::Published {
            inputs,
            files: issued,
        } = self.take_permit(terminal, true)?
        else {
            return Err(io::Error::other(
                "physical BuildInfo files have not published",
            ));
        };
        if !Arc::ptr_eq(&inputs, &terminal.original) || !Arc::ptr_eq(&issued, &files) {
            return Err(io::Error::other("physical BuildInfo receipt owner differs"));
        }
        let view = ConfiguredBuildInfoActionView::from_configured_action(
            inputs.template.action(),
            self.origin.0.0.workspace(),
        )
        .map_err(|e| io::Error::other(e.to_string()))?;
        let mut scratch = [0; 8236];
        let actual = files.read(&inputs.status, view.status(), &mut scratch)?;
        let source = inputs
            .template
            .source()
            .result()
            .as_ref()
            .map_err(|e| io::Error::other(e.to_string()))?
            .as_ref()
            .as_ref()
            .map_err(|e| io::Error::other(e.to_string()))?;
        let header = view
            .callback()
            .render_supplied_bytes(actual, source.bytes())
            .map_err(|e| io::Error::other(e.to_string()))?;
        terminal.consumed = Some(Arc::new(ConsumedBuildInfo {
            inputs,
            files,
            header,
        }));
        Ok(())
    }
}
fn computation(error: impl std::fmt::Display) -> NativeDemandSessionError {
    NativeDemandSessionError::Computation(anyhow::anyhow!("{error}"))
}
#[async_trait]
impl NativeCommandRoot for NativeBuildInfoFileInputsRoot {
    type Terminal = NativeBuildInfoFileInputsTerminal;
    fn requires_workspace_status_inputs(&self) -> bool {
        self.origin.requires_workspace_status_inputs()
    }
    fn initializes_request_revision(&self) -> bool {
        self.origin.initializes_request_revision()
    }
    fn observations<'a>(&self, value: &'a Self::Terminal) -> Option<&'a PathObservationEpoch> {
        self.origin.0.observations(&value.original)
    }
    fn source_certificate<'a>(&self, value: &'a Self::Terminal) -> Option<&'a SourceCertificate> {
        self.origin.0.source_certificate(&value.original)
    }
    fn observed_selection_association(&self) -> ObservedSelectionAssociation {
        self.origin.observed_selection_association()
    }
    fn event_reconciliation_policy(&self, value: &Self::Terminal) -> EventReconciliationPolicy {
        self.origin.0.event_reconciliation_policy(&value.original)
    }
    async fn compute(
        &self,
        tx: &mut dice::DiceTransaction,
    ) -> Result<PreparationOutcome<Self::Terminal>, NativeDemandSessionError> {
        Ok(self
            .origin
            .compute(tx)
            .await?
            .map(|publication| NativeBuildInfoFileInputsTerminal {
                original: publication.inputs.clone(),
                publication,
                origin: self.origin.clone(),
                permit: None,
                consumed: None,
            }))
    }
    fn complete<'a>(
        &'a self,
        terminal: &'a mut Self::Terminal,
        context: NativeCommandCompletionContext<'a, '_>,
    ) -> impl std::future::Future<Output = Result<(), NativeDemandSessionError>> + 'a {
        async move {
            self.check_original(terminal).map_err(computation)?;
            if terminal.permit.is_some() || terminal.consumed.is_some() {
                return Err(computation("physical BuildInfo terminal already prepared"));
            }
            // No permit lock exists across the inherited capture/DICE completion.
            self.origin
                .complete(&mut terminal.publication, context)
                .await?;
            terminal.permit = Some(Arc::new(Mutex::new(Some(ReadPermit::Prepared(
                terminal.original.clone(),
            )))));
            Ok(())
        }
    }
    fn publish(&self, terminal: &mut Self::Terminal) -> io::Result<()> {
        let files = self.publish_files(terminal)?;
        self.consume_files(terminal, files)
    }
    fn publication_completed(&self, terminal: &Self::Terminal) -> bool {
        self.origin.publication_completed(&terminal.publication)
    }
}
