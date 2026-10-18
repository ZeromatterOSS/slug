//! One original callback invocation followed by exact physical-input validation.
use std::io;

use slug_analysis_v2::ConfiguredBuildInfoActionView;
use slug_build_api_v2::BuildInfoEvaluationContext;
use slug_build_api_v2::BuildInfoTransformError;

use super::build_info_native_inputs::BuildInfoNativeInputs;
use super::build_info_native_inputs::BuildInfoNativeInputsKey;
use super::workspace_status_file_inputs::WorkspaceStatusFileInputs;
use super::workspace_status_publication::WorkspaceStatusPublicationRoot;
use super::workspace_status_publication::WorkspaceStatusPublicationTerminal;
use super::*;

#[path = "build_info_file_inputs/control.rs"]
mod control;

#[derive(Clone)]
pub(super) struct NativeBuildInfoFileInputsRoot {
    origin: Arc<WorkspaceStatusPublicationRoot>,
}
#[derive(Clone)]
pub(super) struct NativeBuildInfoFileInputsTerminal {
    pub(super) publication: WorkspaceStatusPublicationTerminal,
    original: Arc<BuildInfoNativeInputs>,
    origin: Arc<WorkspaceStatusPublicationRoot>,
    permit: Arc<Mutex<Option<ReadPermit>>>,
    pub(super) consumed: Option<Arc<ConsumedBuildInfo>>,
}
impl NativeBuildInfoFileInputsTerminal {
    pub(super) fn original(&self) -> &Arc<BuildInfoNativeInputs> {
        &self.original
    }
}
// The slot is shared by terminal clones; grants themselves cannot be cloned.
enum ReadPermit {
    Unevaluated(Arc<BuildInfoNativeInputs>),
    Prepared {
        inputs: Arc<BuildInfoNativeInputs>,
        header: Vec<u8>,
    },
    Published {
        inputs: Arc<BuildInfoNativeInputs>,
        files: Arc<WorkspaceStatusFileInputs>,
        header: Vec<u8>,
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
    fn take_permit(&self, terminal: &NativeBuildInfoFileInputsTerminal) -> io::Result<ReadPermit> {
        let permit = terminal
            .permit
            .lock()
            .map_err(|_| io::Error::other("physical BuildInfo read permit poisoned"))?
            .take()
            .ok_or_else(|| io::Error::other("physical BuildInfo read permit spent"))?;
        // Even a foreign owner or phase error spends this shared grant. The
        // guard is gone before validation, DICE completion, VM or filesystem IO.
        self.check_original(terminal)?;
        Ok(permit)
    }
    pub(super) fn publish_files(
        &self,
        terminal: &mut NativeBuildInfoFileInputsTerminal,
    ) -> io::Result<Arc<WorkspaceStatusFileInputs>> {
        let ReadPermit::Prepared { inputs, header } = self.take_permit(terminal)? else {
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
            .lock()
            .map_err(|_| io::Error::other("physical BuildInfo read permit poisoned"))? =
            Some(ReadPermit::Published {
                inputs,
                files: files.clone(),
                header,
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
            header,
        } = self.take_permit(terminal)?
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
        // This proves actual bytes equal the exact captured bytes already
        // passed to the original callback. Validation never calls it again.
        files.read(&inputs.status, view.status(), &mut scratch)?;
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
        match self.origin.compute(tx).await? {
            PreparationOutcome::Need(need) => Ok(PreparationOutcome::Need(need)),
            PreparationOutcome::Complete(publication) => {
                // Native fixed control is admitted at its actual owner, before
                // the allocation; the runtime's separate single invocation
                // ledger covers all input-scaled parsing/VM/rendering work.
                control::admit().map_err(computation)?;
                let original = publication.inputs.clone();
                // Issued before any terminal clone. A second complete can never
                // mint an independent callback or physical-publication grant.
                let permit = Arc::new(Mutex::new(Some(ReadPermit::Unevaluated(original.clone()))));
                Ok(PreparationOutcome::Complete(
                    NativeBuildInfoFileInputsTerminal {
                        original,
                        publication,
                        origin: self.origin.clone(),
                        permit,
                        consumed: None,
                    },
                ))
            }
        }
    }

    fn complete<'a>(
        &'a self,
        terminal: &'a mut Self::Terminal,
        context: NativeCommandCompletionContext<'a, '_>,
    ) -> impl std::future::Future<Output = Result<(), NativeDemandSessionError>> + 'a {
        async move {
            let ReadPermit::Unevaluated(inputs) =
                self.take_permit(terminal).map_err(computation)?
            else {
                return Err(computation("physical BuildInfo terminal already prepared"));
            };
            if !Arc::ptr_eq(&inputs, &terminal.original) || terminal.consumed.is_some() {
                return Err(computation("physical BuildInfo evaluation owner differs"));
            }
            let sink = context.callback_output.ok_or_else(|| {
                computation("native BuildInfo callback requires a current output sink")
            })?;
            let scope = context.command.runtime.source_scope.as_deref();
            // The single grant is already spent, with no mutex held across
            // inherited capture validation or the original evaluator below.
            self.origin
                .complete(&mut terminal.publication, context)
                .await?;
            let view = ConfiguredBuildInfoActionView::from_configured_action(
                inputs.template.action(),
                &inputs.status.capture.workspace,
            )
            .map_err(computation)?;
            let source = inputs
                .template
                .source()
                .result()
                .as_ref()
                .map_err(computation)?;
            let callback_context = RequestBuildInfoContext {
                scope,
                sink,
                mnemonic: view.action().mnemonic(),
            };
            let header = view
                .callback()
                .render(
                    inputs.status.bytes(view.status().kind()),
                    source.bytes(),
                    &callback_context,
                )
                .map_err(computation)?;
            super::build_info_header_publication::check_header_size(header.len())
                .map_err(computation)?;
            *terminal
                .permit
                .lock()
                .map_err(|_| computation("physical BuildInfo read permit poisoned"))? =
                Some(ReadPermit::Prepared { inputs, header });
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

/// Borrows the current native consumer only while its original callable runs.
struct RequestBuildInfoContext<'a> {
    scope: Option<&'a source_service::SourceServiceScope>,
    sink: &'a NativeCallbackOutput<'a>,
    mnemonic: &'a str,
}
impl BuildInfoEvaluationContext for RequestBuildInfoContext<'_> {
    fn is_cancelled(&self) -> bool {
        self.scope.is_some_and(|scope| scope.is_cancelled())
    }
    fn print(&self, text: &str) -> Result<(), BuildInfoTransformError> {
        (self.sink)(text)
    }
    fn mnemonic(&self) -> &str {
        self.mnemonic
    }
}
