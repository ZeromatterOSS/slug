//! Native, scoped image I/O. A selector is never filesystem authority.

use std::fmt;
use std::fs::File;
use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use dupe::Dupe;
use slug_workspace_v2::ImageDescriptorBudget;
use slug_workspace_v2::ImageDescriptorBudgetError;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationResult;
use tempfile::TempDir;

mod capture;
#[cfg(target_os = "linux")]
pub(super) mod elf_metadata;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(all(test, target_os = "linux"))]
mod tests;
pub(super) use capture::AuthorizedObservedInputImage;
pub(super) use capture::CapturedInputImageLease;
pub(super) use capture::capture_input_image;
pub(super) use capture::capture_prepared_input_image;
pub(super) use capture::prepare_input_image_stage;
pub(super) use capture::verify_frontier;

#[derive(Debug)]
pub(super) enum InputImageCaptureError {
    Unauthorized,
    Cancelled,
    StaleSource,
    UnsupportedPlatform,
    LimitExceeded,
    DescriptorBudget(ImageDescriptorBudgetError),
    LogicalByteBudget,
    Io(io::Error),
}

impl fmt::Display for InputImageCaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "input image I/O: {error}"),
            _ => write!(f, "input image capture: {self:?}"),
        }
    }
}
impl std::error::Error for InputImageCaptureError {}
impl From<io::Error> for InputImageCaptureError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<ImageDescriptorBudgetError> for InputImageCaptureError {
    fn from(value: ImageDescriptorBudgetError) -> Self {
        Self::DescriptorBudget(value)
    }
}

struct LogicalByteBudget {
    limit: u64,
    used: AtomicU64,
}

/// Workspace-owned services. Limits affect readiness, never image identity.
#[derive(Clone, Dupe)]
pub(super) struct WorkspaceImageResources {
    descriptor: ImageDescriptorBudget,
    logical: Arc<LogicalByteBudget>,
}

impl WorkspaceImageResources {
    pub(super) fn new(descriptor: ImageDescriptorBudget, logical_byte_limit: u64) -> Self {
        Self {
            descriptor,
            logical: Arc::new(LogicalByteBudget {
                limit: logical_byte_limit,
                used: AtomicU64::new(0),
            }),
        }
    }
    pub(super) fn descriptor_budget(&self) -> &ImageDescriptorBudget {
        &self.descriptor
    }
    fn reserve_logical(
        &self,
        bytes: u64,
    ) -> Result<LogicalByteReservation, InputImageCaptureError> {
        let mut used = self.logical.used.load(Ordering::Acquire);
        loop {
            let next = used
                .checked_add(bytes)
                .filter(|next| *next <= self.logical.limit)
                .ok_or(InputImageCaptureError::LogicalByteBudget)?;
            match self.logical.used.compare_exchange_weak(
                used,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Ok(LogicalByteReservation {
                        budget: self.logical.clone(),
                        bytes,
                    });
                }
                Err(actual) => used = actual,
            }
        }
    }
    #[cfg(test)]
    pub(in crate::runtime) fn logical_bytes_used(&self) -> u64 {
        self.logical.used.load(Ordering::Acquire)
    }
}

struct LogicalByteReservation {
    budget: Arc<LogicalByteBudget>,
    bytes: u64,
}
impl Drop for LogicalByteReservation {
    fn drop(&mut self) {
        self.budget.used.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

/// Only the materializer's validated factory may construct a source. The FD
/// anchors every operation; the pin keeps a selected immutable source alive.
pub(super) struct ScopedImageSource {
    namespace: PathObservationNamespace,
    anchor: NormalizedAbsolutePath,
    root: File,
    _pin: Option<Arc<TempDir>>,
}

impl ScopedImageSource {
    pub(super) fn new(
        namespace: PathObservationNamespace,
        anchor: NormalizedAbsolutePath,
        root: File,
        pin: Option<Arc<TempDir>>,
    ) -> Result<Self, InputImageCaptureError> {
        #[cfg(target_os = "linux")]
        {
            let opened = linux::fd_stat(&root)?;
            let named = linux::fd_stat(&open_image_anchor(&anchor)?)?;
            if opened.st_mode & libc::S_IFMT != libc::S_IFDIR
                || opened.st_dev != named.st_dev
                || opened.st_ino != named.st_ino
            {
                return Err(InputImageCaptureError::StaleSource);
            }
            Ok(Self {
                namespace,
                anchor,
                root,
                _pin: pin,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (namespace, anchor, root, pin);
            Err(InputImageCaptureError::UnsupportedPlatform)
        }
    }

    pub(super) fn observe(
        &self,
        demand: &PathObservationDemand,
        budget: &ImageDescriptorBudget,
        active: &dyn Fn() -> bool,
    ) -> Result<PathObservationResult, InputImageCaptureError> {
        if !active() {
            return Err(InputImageCaptureError::Cancelled);
        }
        let slug_workspace_v2::PathObservationOperation::InputImage(request) = demand.operation()
        else {
            return Err(InputImageCaptureError::Unauthorized);
        };
        // ELF metadata requires current certified-file membership, not the
        // generic scoped source capability. Reject before anchor I/O.
        if matches!(
            request.operation(),
            slug_workspace_v2::ImageObservationOperation::BoundedElfMetadata { .. }
        ) {
            return Err(InputImageCaptureError::Unauthorized);
        }
        if demand.namespace() != self.namespace
            || request.scope_anchor_root() != &self.anchor
            || !request.same_budget(budget)
            || !demand.path().as_path().starts_with(self.anchor.as_path())
            || !demand
                .image_path()
                .is_some_and(|path| path.same_budget(budget))
        {
            return Err(InputImageCaptureError::Unauthorized);
        }
        #[cfg(target_os = "linux")]
        {
            self.check_current_anchor(active)?;
            let result = linux::observe(self, demand, request, budget, active)?;
            self.check_current_anchor(active)?;
            Ok(result)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (request, budget);
            Err(InputImageCaptureError::UnsupportedPlatform)
        }
    }

    fn check_current_anchor(
        &self,
        active: &dyn Fn() -> bool,
    ) -> Result<(), InputImageCaptureError> {
        if !active() {
            return Err(InputImageCaptureError::Cancelled);
        }
        #[cfg(target_os = "linux")]
        {
            let opened = linux::fd_stat(&self.root)?;
            let named = linux::fd_stat(&open_image_anchor(&self.anchor)?)?;
            if opened.st_dev != named.st_dev || opened.st_ino != named.st_ino {
                return Err(InputImageCaptureError::StaleSource);
            }
            Ok(())
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(InputImageCaptureError::UnsupportedPlatform)
        }
    }
}

/// The factory first validates the semantic scope, then opens every ancestor
/// without following links. An anchor spelling alone cannot grant authority.
pub(super) fn open_image_anchor(
    anchor: &NormalizedAbsolutePath,
) -> Result<File, InputImageCaptureError> {
    #[cfg(target_os = "linux")]
    {
        Ok(linux::open_anchor(anchor.as_path())?)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = anchor;
        Err(InputImageCaptureError::UnsupportedPlatform)
    }
}
