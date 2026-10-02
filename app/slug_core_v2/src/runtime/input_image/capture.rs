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

struct CapturedImageOwner {
    certificate: ObservedInputImage,
    root: File,
    stage: Option<TempDir>,
    logical: Option<LogicalByteReservation>,
}

/// Empty private output storage, created before ordinary source facts are read.
/// Its allocation credit transfers to the completed owner. No writable path or
/// FD escapes before promotion.
pub(in crate::runtime) struct PreparedInputImageStage {
    stage: TempDir,
    reservation: ImageDescriptorReservation,
}
impl PreparedInputImageStage {
    pub(super) fn into_parts(self) -> (ImageDescriptorReservation, TempDir) {
        (self.reservation, self.stage)
    }
    #[cfg(test)]
    pub(in crate::runtime) fn test_path(&self) -> &Path {
        self.stage.path()
    }
}

pub(in crate::runtime) fn prepare_input_image_stage(
    resources: &WorkspaceImageResources,
) -> Result<PreparedInputImageStage, InputImageCaptureError> {
    #[cfg(target_os = "linux")]
    {
        let reservation = resources.descriptor_budget().try_reserve(4096 + 256)?;
        let stage = tempfile::Builder::new()
            .prefix("slug-input-image-")
            .tempdir()?;
        Ok(PreparedInputImageStage { stage, reservation })
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
    stage: PreparedInputImageStage,
) -> Result<CapturedInputImageLease, InputImageCaptureError> {
    capture_with_stage(authorized, resources, active, Some(stage))
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
    if prepared
        .as_ref()
        .is_some_and(|stage| !stage.reservation.same_budget(resources.descriptor_budget()))
    {
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
        Some(prepared) => (prepared.reservation, Some(prepared.stage)),
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
