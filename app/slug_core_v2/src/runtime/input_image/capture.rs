//! A completed normal-compute certificate is copied through scoped authority.

use std::fs::File;
#[cfg(test)]
use std::path::Path;

use dupe::Dupe;
use slug_workspace_v2::BudgetedImageBlock;
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
    source: ScopedImageSource,
    certificate: ObservedInputImage,
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
    // Reserve descriptor/header and logical payload before creating stage or
    // copying a byte. The opaque certificate already retains its own credits.
    let reservation = resources.descriptor_budget().try_reserve(4096 + 256)?;
    let logical = resources.reserve_logical(certificate.total_file_bytes())?;
    #[cfg(target_os = "linux")]
    {
        let stage = tempfile::Builder::new()
            .prefix("slug-input-image-")
            .tempdir()?;
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
        let _ = (reservation, logical);
        Err(InputImageCaptureError::UnsupportedPlatform)
    }
}
