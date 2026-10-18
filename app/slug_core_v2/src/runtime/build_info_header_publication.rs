//! Private physical-input-derived scalar header publication under native finalization.
use std::io;
use std::io::Write;

use slug_analysis_v2::ConfiguredBuildInfoActionView;
use starlark_map::small_set::SmallSet;

use super::build_info_file_inputs::ConsumedBuildInfo;
use super::build_info_file_inputs::NativeBuildInfoFileInputsRoot;
use super::build_info_file_inputs::NativeBuildInfoFileInputsTerminal;
use super::build_info_native_inputs::BuildInfoNativeInputs;
use super::build_info_native_inputs::BuildInfoNativeInputsKey;
use super::*;
use crate::runtime::action_output_staging::ActionOutputStaging;
use crate::runtime::action_output_staging::PublishedActionOutputs;
use crate::runtime::configured_output::ConfiguredOutputOwner;

pub(super) const MAX_HEADER_BYTES: usize = slug_build_api_v2::BUILD_INFO_MAX_OUTPUT_BYTES;
pub(super) fn check_header_size(length: usize) -> io::Result<()> {
    if length > MAX_HEADER_BYTES {
        Err(io::Error::other(
            "native BuildInfo header byte budget exceeded",
        ))
    } else {
        Ok(())
    }
}
#[derive(Clone)]
pub(super) struct NativeBuildInfoHeaderPublicationRoot {
    origin: Arc<NativeBuildInfoFileInputsRoot>,
}
#[derive(Clone)]
pub(super) struct NativeBuildInfoHeaderPublicationTerminal {
    pub(super) physical: NativeBuildInfoFileInputsTerminal,
    original: Arc<BuildInfoNativeInputs>,
    origin: Arc<NativeBuildInfoFileInputsRoot>,
    permit: Option<Arc<Mutex<Option<HeaderPermit>>>>,
    pub(super) pending: Option<Arc<Mutex<Option<ActionOutputStaging>>>>,
    pub(super) published: Option<Arc<PublishedBuildInfoHeader>>,
}
#[derive(Debug)]
pub(super) struct PublishedBuildInfoHeader {
    pub(super) consumed: Arc<ConsumedBuildInfo>,
    pub(super) outputs: PublishedActionOutputs,
}
enum HeaderPermit {
    Prepared {
        inputs: Arc<BuildInfoNativeInputs>,
        owner: Arc<ConfiguredOutputOwner>,
    },
    Staged {
        inputs: Arc<BuildInfoNativeInputs>,
        consumed: Arc<ConsumedBuildInfo>,
        stage: StageLease,
    },
}
// A retained scratch clone never retains the stage after its owning grant retires.
struct StageLease(Arc<Mutex<Option<ActionOutputStaging>>>);
impl Drop for StageLease {
    fn drop(&mut self) {
        let stage = self.0.lock().unwrap_or_else(|e| e.into_inner()).take();
        drop(stage);
    }
}
impl NativeBuildInfoHeaderPublicationRoot {
    pub(super) fn new(key: BuildInfoNativeInputsKey) -> Self {
        Self {
            origin: Arc::new(NativeBuildInfoFileInputsRoot::new(key)),
        }
    }
    fn guard(&self, terminal: &NativeBuildInfoHeaderPublicationTerminal) -> io::Result<()> {
        if !Arc::ptr_eq(&self.origin, &terminal.origin)
            || !Arc::ptr_eq(&terminal.original, &terminal.physical.publication.inputs)
        {
            return Err(io::Error::other("header original issuance differs"));
        }
        self.origin.check_original(&terminal.physical)
    }
    fn take(
        &self,
        terminal: &NativeBuildInfoHeaderPublicationTerminal,
    ) -> io::Result<HeaderPermit> {
        let permit = terminal
            .permit
            .as_ref()
            .ok_or_else(|| io::Error::other("header has no permit"))?
            .lock()
            .map_err(|_| io::Error::other("header permit poisoned"))?
            .take()
            .ok_or_else(|| io::Error::other("header permit spent"))?;
        self.guard(terminal)?;
        Ok(permit)
    }
    pub(super) fn stage_header(
        &self,
        terminal: &mut NativeBuildInfoHeaderPublicationTerminal,
    ) -> io::Result<()> {
        let HeaderPermit::Prepared { inputs, owner } = self.take(terminal)? else {
            return Err(io::Error::other("header stage permit phase differs"));
        };
        if !Arc::ptr_eq(&inputs, &terminal.original)
            || terminal.pending.is_some()
            || terminal.published.is_some()
        {
            return Err(io::Error::other("header prepared owner differs"));
        }
        self.origin.publish(&mut terminal.physical)?;
        let consumed = terminal
            .physical
            .consumed
            .as_ref()
            .ok_or_else(|| io::Error::other("header physical consumption missing"))?
            .clone();
        if !Arc::ptr_eq(&consumed.inputs, &inputs) {
            return Err(io::Error::other("header physical owner differs"));
        }
        check_header_size(consumed.header.len())?;
        let view = ConfiguredBuildInfoActionView::from_configured_action(
            inputs.template.action(),
            &inputs.status.capture.workspace,
        )
        .map_err(io::Error::other)?;
        let reserved: SmallSet<&str> = view.output().root_relative_path().split('/').collect();
        let mut staging = owner.stage_output_subset(
            view.action(),
            std::slice::from_ref(view.output()),
            &reserved,
        )?;
        drop(reserved);
        {
            let mut file = staging.create_file(0, "")?;
            file.write_all(&consumed.header)?;
        }
        staging.seal_nonexecutable(&inputs.status.capture.workspace, &consumed.header)?;
        let pending = Arc::new(Mutex::new(Some(staging)));
        *terminal
            .permit
            .as_ref()
            .unwrap()
            .lock()
            .map_err(|_| io::Error::other("header permit poisoned"))? =
            Some(HeaderPermit::Staged {
                inputs,
                consumed,
                stage: StageLease(pending.clone()),
            });
        terminal.pending = Some(pending);
        Ok(())
    }
    pub(super) fn publish_header(
        &self,
        terminal: &mut NativeBuildInfoHeaderPublicationTerminal,
    ) -> io::Result<()> {
        let HeaderPermit::Staged {
            inputs,
            consumed,
            stage,
        } = self.take(terminal)?
        else {
            return Err(io::Error::other("header publication permit phase differs"));
        };
        if !Arc::ptr_eq(&inputs, &terminal.original)
            || !matches!(&terminal.physical.consumed, Some(value) if Arc::ptr_eq(value, &consumed))
            || !matches!(&terminal.pending, Some(value) if Arc::ptr_eq(value, &stage.0))
            || terminal.published.is_some()
        {
            return Err(io::Error::other("header staged owner differs"));
        }
        let mut staging = stage
            .0
            .lock()
            .map_err(|_| io::Error::other("header stage poisoned"))?
            .take()
            .ok_or_else(|| io::Error::other("header stage retired"))?;
        let view = ConfiguredBuildInfoActionView::from_configured_action(
            inputs.template.action(),
            &inputs.status.capture.workspace,
        )
        .map_err(io::Error::other)?;
        let mut scratch = [0; 8236];
        consumed
            .files
            .read(&inputs.status, view.status(), &mut scratch)?;
        let outputs = staging.publish_nonexecutable(&consumed.header)?;
        drop(staging);
        terminal.pending = None;
        terminal.published = Some(Arc::new(PublishedBuildInfoHeader { consumed, outputs }));
        Ok(())
    }
}
fn computation(value: impl std::fmt::Display) -> NativeDemandSessionError {
    NativeDemandSessionError::Computation(anyhow::anyhow!("{value}"))
}
#[async_trait]
impl NativeCommandRoot for NativeBuildInfoHeaderPublicationRoot {
    type Terminal = NativeBuildInfoHeaderPublicationTerminal;
    fn requires_workspace_status_inputs(&self) -> bool {
        self.origin.requires_workspace_status_inputs()
    }
    fn initializes_request_revision(&self) -> bool {
        self.origin.initializes_request_revision()
    }
    fn observations<'a>(&self, value: &'a Self::Terminal) -> Option<&'a PathObservationEpoch> {
        Some(value.original.template.observations())
    }
    fn source_certificate<'a>(&self, value: &'a Self::Terminal) -> Option<&'a SourceCertificate> {
        Some(value.original.template.certificate())
    }
    fn observed_selection_association(&self) -> ObservedSelectionAssociation {
        self.origin.observed_selection_association()
    }
    fn event_reconciliation_policy(&self, _: &Self::Terminal) -> EventReconciliationPolicy {
        EventReconciliationPolicy::SourceCertifiedCurrentClosure
    }
    async fn compute(
        &self,
        tx: &mut dice::DiceTransaction,
    ) -> Result<PreparationOutcome<Self::Terminal>, NativeDemandSessionError> {
        Ok(self.origin.compute(tx).await?.map(|physical| {
            NativeBuildInfoHeaderPublicationTerminal {
                original: physical.original().clone(),
                physical,
                origin: self.origin.clone(),
                permit: None,
                pending: None,
                published: None,
            }
        }))
    }
    fn complete<'a>(
        &'a self,
        terminal: &'a mut Self::Terminal,
        context: NativeCommandCompletionContext<'a, '_>,
    ) -> impl std::future::Future<Output = Result<(), NativeDemandSessionError>> + 'a {
        async move {
            self.guard(terminal).map_err(computation)?;
            if terminal.permit.is_some()
                || terminal.pending.is_some()
                || terminal.published.is_some()
            {
                return Err(computation("header terminal already prepared"));
            }
            let owner = context.command.runtime.configured_output.clone();
            self.origin
                .complete(&mut terminal.physical, context)
                .await?;
            terminal.permit = Some(Arc::new(Mutex::new(Some(HeaderPermit::Prepared {
                inputs: terminal.original.clone(),
                owner,
            }))));
            Ok(())
        }
    }
    fn publish(&self, terminal: &mut Self::Terminal) -> io::Result<()> {
        self.stage_header(terminal)?;
        self.publish_header(terminal)
    }
    fn publication_completed(&self, terminal: &Self::Terminal) -> bool {
        self.origin.publication_completed(&terminal.physical) || terminal.published.is_some()
    }
}
