//! Exact regular-root certificates use private directory storage, never a tree.
use std::fs::File;
use std::path::Path;

use dupe::Dupe;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::InputImageEntryKind;
use slug_workspace_v2::ObservedInputImage;
use tempfile::TempDir;

use super::*;

pub(super) struct FileOwner {
    certificate: ObservedInputImage,
    root: File,
    stage: Option<TempDir>,
    logical: Option<LogicalByteReservation>,
}
impl FileOwner {
    #[cfg(test)]
    pub(super) fn stage_path(&self) -> &Path {
        self.stage.as_ref().unwrap().path()
    }
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
    mut stage: PreparedInputImageStage,
) -> Result<CapturedInputFileLease, InputImageCaptureError> {
    capture_prepared_input_file_retaining(authorized, resources, active, &mut stage)
}

pub(in crate::runtime) fn capture_prepared_input_file_retaining(
    authorized: AuthorizedObservedInputImage,
    resources: &WorkspaceImageResources,
    active: &dyn Fn() -> bool,
    prepared: &mut PreparedInputImageStage,
) -> Result<CapturedInputFileLease, InputImageCaptureError> {
    use super::capture::PreparedStageState;
    let AuthorizedObservedInputImage {
        source,
        certificate,
    } = authorized;
    if !matches!(prepared.state, PreparedStageState::Ready(_))
        || !prepared
            .reservation
            .as_ref()
            .is_some_and(|r| r.same_budget(resources.descriptor_budget()))
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
    prepared
        .reservation
        .as_mut()
        .unwrap()
        .try_grow(std::mem::size_of::<FileOwner>() + 256)?;
    super::capture::verify_frontier(&source, &certificate, resources, active)?;
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
        prepared.state = PreparedStageState::PartialFile(FileOwner {
            certificate,
            root,
            stage: Some(stage),
            logical: Some(logical),
        });
        let owner = match &prepared.state {
            PreparedStageState::PartialFile(owner) => owner,
            _ => unreachable!(),
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
        let PreparedStageState::PartialFile(owner) =
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
            Ok(block) => Ok(CapturedInputFileLease(block)),
            Err((error, owner, reservation)) => {
                prepared.state = PreparedStageState::PartialFile(owner);
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
