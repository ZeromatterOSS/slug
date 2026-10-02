//! Exact regular-root certificates use private directory storage, never a tree.
use std::fs::File;
use std::path::Path;

use dupe::Dupe;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::InputImageEntryKind;
use slug_workspace_v2::ObservedInputImage;
use tempfile::TempDir;

use super::*;

struct FileOwner {
    certificate: ObservedInputImage,
    root: File,
    stage: Option<TempDir>,
    logical: Option<LogicalByteReservation>,
}
impl Drop for FileOwner {
    fn drop(&mut self) {
        // Private storage stays traversable even when the certified file is000.
        if self
            .stage
            .take()
            .is_some_and(|stage| stage.close().is_err())
        {
            if let Some(logical) = self.logical.take() {
                std::mem::forget(logical);
            }
        }
    }
}
#[derive(Clone, Dupe)]
pub(in crate::runtime) struct CapturedInputFileLease(BudgetedImageBlock<FileOwner>);
impl CapturedInputFileLease {
    pub(in crate::runtime) fn with_read_leaf<R>(
        &self,
        visitor: impl FnOnce(&File, &Path, &ObservedInputImage) -> R,
    ) -> R {
        let owner = self.0.get();
        visitor(&owner.root, Path::new("leaf"), &owner.certificate)
    }
    #[cfg(test)]
    pub(in crate::runtime) fn test_path(&self) -> std::path::PathBuf {
        self.0.get().stage.as_ref().unwrap().path().join("leaf")
    }
}
pub(in crate::runtime) fn capture_prepared_input_file(
    authorized: AuthorizedObservedInputImage,
    resources: &WorkspaceImageResources,
    active: &dyn Fn() -> bool,
    stage: PreparedInputImageStage,
) -> Result<CapturedInputFileLease, InputImageCaptureError> {
    // Local drop order keeps storage cleanup ahead of descriptor credit release
    // on every early refusal before the completed FileOwner exists.
    let (mut reservation, stage) = stage.into_parts();
    let AuthorizedObservedInputImage {
        source,
        certificate,
    } = authorized;
    if !reservation.same_budget(resources.descriptor_budget())
        || !certificate.same_budget(resources.descriptor_budget())
        || !certificate.is_regular_file_root()
        || certificate.entries().len() != 1
        || certificate.entries()[0].path().as_path() != certificate.source_root().as_path()
        || !matches!(
            certificate.entries()[0].kind(),
            InputImageEntryKind::RegularFile(_)
        )
        || source.namespace != certificate.namespace()
        || &source.anchor != certificate.anchor_root()
        || !certificate
            .source_root()
            .as_path()
            .starts_with(source.anchor.as_path())
    {
        return Err(InputImageCaptureError::Unauthorized);
    }
    reservation.try_grow(std::mem::size_of::<FileOwner>() + 256)?;
    super::capture::verify_frontier(&source, &certificate, resources, active)?;
    let logical = resources.reserve_logical(certificate.total_file_bytes())?;
    #[cfg(target_os = "linux")]
    {
        let root = super::linux::open_anchor(stage.path())?;
        let owner = FileOwner {
            certificate,
            root,
            stage: Some(stage),
            logical: Some(logical),
        };
        let _scratch = resources
            .descriptor_budget()
            .try_reserve(64 * 1024 + 8192)?;
        super::linux::file_copy::copy_file(&source, &owner.root, &owner.certificate, active)?;
        super::capture::verify_frontier(&source, &owner.certificate, resources, active)?;
        source.check_current_anchor(active)?;
        if !active() {
            return Err(InputImageCaptureError::Cancelled);
        }
        Ok(CapturedInputFileLease(reservation.freeze(owner)?))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (stage, logical);
        Err(InputImageCaptureError::UnsupportedPlatform)
    }
}
