//! A completed normal-compute certificate is copied through scoped authority.

use std::fs::File;
#[cfg(test)]
use std::path::Path;

use dupe::Dupe;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ImageDescriptorReservation;
use slug_workspace_v2::ObservedInputImage;
use slug_workspace_v2::PathObservationResult;
use tempfile::TempDir;

use super::InputImageCaptureError;
use super::LogicalByteReservation;
use super::ScopedImageSource;
use super::WorkspaceImageResources;

/// Only the normal NativeDemand bridge constructs this after current reachable
/// closure and selected-source validation. It is deliberately not a public API.
pub(in crate::runtime) struct AuthorizedObservedInputImage {
    pub(super) source: ScopedImageSource,
    pub(super) certificate: ObservedInputImage,
}
impl AuthorizedObservedInputImage {
    pub(in crate::runtime) fn new(
        source: ScopedImageSource,
        certificate: ObservedInputImage,
    ) -> Self {
        Self {
            source,
            certificate,
        }
    }
}

pub(super) struct CapturedImageOwner {
    certificate: ObservedInputImage,
    root: File,
    stage: Option<TempDir>,
    logical: Option<LogicalByteReservation>,
}

/// Empty private output storage, created before ordinary source facts are read.
/// Its allocation credit transfers to the completed owner. No writable path or
/// FD escapes before promotion.
pub(in crate::runtime) struct PreparedInputImageStage {
    // Stage/partial owner must clean up before its descriptor credit is released.
    pub(super) state: PreparedStageState,
    pub(super) reservation: Option<ImageDescriptorReservation>,
}
pub(super) enum PreparedStageState {
    Ready(TempDir),
    PartialDirectory(CapturedImageOwner),
    PartialFile(super::file_capture::FileOwner),
    // Only reached when ownership has transferred into a verified normal lease.
    // A consumed or partial stage is never a retry/read capability.
    Consumed,
}
impl PreparedInputImageStage {
    pub(super) fn into_parts(self) -> (ImageDescriptorReservation, TempDir) {
        match self.state {
            PreparedStageState::Ready(stage) => (self.reservation.expect("ready credit"), stage),
            _ => unreachable!("legacy capture accepts only a fresh private stage"),
        }
    }
    #[cfg(test)]
    pub(in crate::runtime) fn test_is_partial(&self) -> bool {
        matches!(
            self.state,
            PreparedStageState::PartialDirectory(_) | PreparedStageState::PartialFile(_)
        )
    }
    #[cfg(test)]
    pub(in crate::runtime) fn test_path(&self) -> &Path {
        match &self.state {
            PreparedStageState::Ready(stage) => stage.path(),
            PreparedStageState::PartialDirectory(owner) => owner.stage.as_ref().unwrap().path(),
            PreparedStageState::PartialFile(owner) => owner.stage_path(),
            PreparedStageState::Consumed => panic!("completed stage has transferred its owner"),
        }
    }
}

pub(in crate::runtime) fn prepare_input_image_stage(
    resources: &WorkspaceImageResources,
) -> Result<PreparedInputImageStage, InputImageCaptureError> {
    #[cfg(target_os = "linux")]
    {
        let reservation = resources
            .descriptor_budget()
            .try_reserve(4096 + 256 + std::mem::size_of::<PreparedInputImageStage>())?;
        let stage = tempfile::Builder::new()
            .prefix("slug-input-image-")
            .tempdir()?;
        Ok(PreparedInputImageStage {
            state: PreparedStageState::Ready(stage),
            reservation: Some(reservation),
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = resources;
        Err(InputImageCaptureError::UnsupportedPlatform)
    }
}

impl Drop for CapturedImageOwner {
    fn drop(&mut self) {
        // Exact source permissions may include mode 000. Restore traversal on
        // private owned directories before TempDir recursively removes them.
        #[cfg(target_os = "linux")]
        super::linux::restore_directories(&self.root, &self.certificate);
        if self
            .stage
            .take()
            .is_some_and(|stage| stage.close().is_err())
        {
            // A failed removal must not grant fresh disk credits for bytes
            // still on disk. Fail closed for this workspace resource service.
            if let Some(logical) = self.logical.take() {
                std::mem::forget(logical);
            }
        }
    }
}

/// No writable FD/path escapes this capability. Clones share the stage and
/// credits; DICE-history owners and retained handles extend the same lifetime.
#[derive(Clone, Dupe)]
pub(in crate::runtime) struct CapturedInputImageLease(BudgetedImageBlock<CapturedImageOwner>);

impl std::fmt::Debug for CapturedInputImageLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CapturedInputImageLease")
            .field("entries", &self.0.get().certificate.entries().len())
            .finish_non_exhaustive()
    }
}

impl CapturedInputImageLease {
    pub(in crate::runtime) fn with_read_root<R>(
        &self,
        visitor: impl FnOnce(&File, &ObservedInputImage) -> R,
    ) -> R {
        let owner = self.0.get();
        visitor(&owner.root, &owner.certificate)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn test_path(&self) -> &Path {
        self.0
            .get()
            .stage
            .as_ref()
            .expect("live image stage")
            .path()
    }
    #[cfg(test)]
    pub(in crate::runtime) fn test_certificate(&self) -> &ObservedInputImage {
        &self.0.get().certificate
    }
}

pub(in crate::runtime) fn verify_frontier(
    source: &ScopedImageSource,
    certificate: &ObservedInputImage,
    resources: &WorkspaceImageResources,
    active: &dyn Fn() -> bool,
) -> Result<(), InputImageCaptureError> {
    source.check_current_anchor(active)?;
    for (demand, expected) in certificate.observations().observations() {
        let PathObservationResult::InputImage(_) = expected.as_ref() else {
            return Err(InputImageCaptureError::Unauthorized);
        };
        if source.observe(demand, resources.descriptor_budget(), active)? != **expected {
            return Err(InputImageCaptureError::StaleSource);
        }
    }
    source.check_current_anchor(active)
}

pub(in crate::runtime) fn capture_input_image(
    authorized: AuthorizedObservedInputImage,
    resources: &WorkspaceImageResources,
    active: &dyn Fn() -> bool,
) -> Result<CapturedInputImageLease, InputImageCaptureError> {
    capture_with_stage(authorized, resources, active, None)
}

pub(in crate::runtime) fn capture_prepared_input_image(
    authorized: AuthorizedObservedInputImage,
    resources: &WorkspaceImageResources,
    active: &dyn Fn() -> bool,
    mut stage: PreparedInputImageStage,
) -> Result<CapturedInputImageLease, InputImageCaptureError> {
    capture_prepared_input_image_retaining(authorized, resources, active, &mut stage)
}

/// The caller keeps this cell through command restoration on every refusal.
/// Only a Ready cell can begin capture; Partial never grants read/retry authority.
pub(in crate::runtime) fn capture_prepared_input_image_retaining(
    authorized: AuthorizedObservedInputImage,
    resources: &WorkspaceImageResources,
    active: &dyn Fn() -> bool,
    prepared: &mut PreparedInputImageStage,
) -> Result<CapturedInputImageLease, InputImageCaptureError> {
    if !matches!(prepared.state, PreparedStageState::Ready(_))
        || !prepared
            .reservation
            .as_ref()
            .is_some_and(|r| r.same_budget(resources.descriptor_budget()))
    {
        return Err(InputImageCaptureError::Unauthorized);
    }
    let AuthorizedObservedInputImage {
        source,
        certificate,
    } = authorized;
    if certificate.is_regular_file_root()
        || source.namespace != certificate.namespace()
        || &source.anchor != certificate.anchor_root()
        || !certificate
            .source_root()
            .as_path()
            .starts_with(source.anchor.as_path())
        || !certificate.same_budget(resources.descriptor_budget())
    {
        return Err(InputImageCaptureError::Unauthorized);
    }
    verify_frontier(&source, &certificate, resources, active)?;
    let logical = resources.reserve_logical(certificate.total_file_bytes())?;
    #[cfg(target_os = "linux")]
    {
        let root = match &prepared.state {
            PreparedStageState::Ready(stage) => super::linux::open_anchor(stage.path())?,
            _ => unreachable!(),
        };
        let PreparedStageState::Ready(stage) =
            std::mem::replace(&mut prepared.state, PreparedStageState::Consumed)
        else {
            unreachable!()
        };
        // Install the partial owner before the first copy or subsequent refusal.
        prepared.state = PreparedStageState::PartialDirectory(CapturedImageOwner {
            certificate,
            root,
            stage: Some(stage),
            logical: Some(logical),
        });
        let PreparedStageState::PartialDirectory(owner) = &prepared.state else {
            unreachable!()
        };
        super::linux::copy_image(&source, &owner.root, &owner.certificate, active)?;
        verify_frontier(&source, &owner.certificate, resources, active)?;
        super::linux::finish_directory_modes(&owner.root, &owner.certificate, active)?;
        source.check_current_anchor(active)?;
        let PreparedStageState::PartialDirectory(owner) =
            std::mem::replace(&mut prepared.state, PreparedStageState::Consumed)
        else {
            unreachable!()
        };
        match prepared
            .reservation
            .take()
            .unwrap()
            .freeze_preserving(owner)
        {
            Ok(block) => Ok(CapturedInputImageLease(block)),
            Err((error, owner, reservation)) => {
                prepared.state = PreparedStageState::PartialDirectory(owner);
                prepared.reservation = Some(reservation);
                Err(error.into())
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = logical;
        Err(InputImageCaptureError::UnsupportedPlatform)
    }
}

fn capture_with_stage(
    authorized: AuthorizedObservedInputImage,
    resources: &WorkspaceImageResources,
    active: &dyn Fn() -> bool,
    prepared: Option<PreparedInputImageStage>,
) -> Result<CapturedInputImageLease, InputImageCaptureError> {
    if authorized.certificate.is_regular_file_root() {
        return Err(InputImageCaptureError::Unauthorized);
    }
    if prepared.as_ref().is_some_and(|stage| {
        !stage
            .reservation
            .as_ref()
            .is_some_and(|r| r.same_budget(resources.descriptor_budget()))
    }) {
        return Err(InputImageCaptureError::Unauthorized);
    }
    let AuthorizedObservedInputImage {
        source,
        certificate,
    } = authorized;
    if source.namespace != certificate.namespace()
        || &source.anchor != certificate.anchor_root()
        || !certificate
            .source_root()
            .as_path()
            .starts_with(source.anchor.as_path())
        || !certificate.same_budget(resources.descriptor_budget())
    {
        return Err(InputImageCaptureError::Unauthorized);
    }
    verify_frontier(&source, &certificate, resources, active)?;
    // Prepared empty storage already owns its descriptor credit. Legacy capture
    // reserves before creating storage; both reserve payload before first copy.
    // Bind credit first so an early error drops stage buffers before the credit.
    let (reservation, stage) = match prepared {
        Some(prepared) => {
            let (credit, stage) = prepared.into_parts();
            (credit, Some(stage))
        }
        None => (resources.descriptor_budget().try_reserve(4096 + 256)?, None),
    };
    let logical = resources.reserve_logical(certificate.total_file_bytes())?;
    #[cfg(target_os = "linux")]
    {
        let stage = match stage {
            Some(stage) => stage,
            None => tempfile::Builder::new()
                .prefix("slug-input-image-")
                .tempdir()?,
        };
        let root = super::linux::open_anchor(stage.path())?;
        let owner = CapturedImageOwner {
            certificate,
            root,
            stage: Some(stage),
            logical: Some(logical),
        };
        super::linux::copy_image(&source, &owner.root, &owner.certificate, active)?;
        verify_frontier(&source, &owner.certificate, resources, active)?;
        super::linux::finish_directory_modes(&owner.root, &owner.certificate, active)?;
        source.check_current_anchor(active)?;
        Ok(CapturedInputImageLease(reservation.freeze(owner)?))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (stage, reservation, logical);
        Err(InputImageCaptureError::UnsupportedPlatform)
    }
}
