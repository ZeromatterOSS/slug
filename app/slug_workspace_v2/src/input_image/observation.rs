//! Scoped image observations: exact requests and independently credited results.

use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;

use super::ElfMetadataPolicy;
use super::ElfSourceDigestBinding;
use super::VerifiedElfMetadata;
use super::budget::BudgetedImageBlock;
use super::budget::ImageChargedPath;
use super::budget::ImageDescriptorBudget;
use super::budget::ImageDescriptorBudgetError;
use super::budget::ImageDescriptorReservation;
use crate::FileContentDigest;
use crate::NormalizedAbsolutePath;
use crate::PathDirectoryEntries;
use crate::PathLstat;
use crate::PathOperationResult;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative, Dupe
)]
pub struct DirectoryObservationLimits {
    pub max_entries: usize,
    pub max_name_bytes_total: usize,
    pub max_component_bytes: usize,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative, Dupe
)]
pub enum ImageObservationOperation {
    BoundedElfMetadata {
        expected_digest: ElfSourceDigestBinding,
        policy: ElfMetadataPolicy,
    },
    Lstat,
    BoundedDigest {
        max_bytes: u64,
    },
    BoundedDirectory(DirectoryObservationLimits),
    BoundedReadLink {
        max_target_bytes: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative, Dupe)]
pub struct ImageObservationRequest {
    scope_anchor_root: ImageChargedPath,
    operation: ImageObservationOperation,
}

impl ImageObservationRequest {
    pub fn new(scope_anchor_root: ImageChargedPath, operation: ImageObservationOperation) -> Self {
        Self {
            scope_anchor_root,
            operation,
        }
    }
    pub fn scope_anchor_root(&self) -> &NormalizedAbsolutePath {
        self.scope_anchor_root.normalized_ref()
    }
    pub fn operation(&self) -> ImageObservationOperation {
        self.operation
    }
    pub fn same_budget(&self, budget: &ImageDescriptorBudget) -> bool {
        self.scope_anchor_root.same_budget(budget)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum ImageObservationData {
    ElfMetadata(PathOperationResult<VerifiedElfMetadata>),
    Lstat(PathOperationResult<PathLstat>),
    Digest(PathOperationResult<FileContentDigest>),
    Directory(PathOperationResult<PathDirectoryEntries>),
    ReadLink(PathOperationResult<Arc<PathBuf>>),
    LimitExceeded,
}

#[derive(Debug, PartialEq, Eq, Allocative)]
struct ImageObservationResultInner {
    issued_path: ImageChargedPath,
    request: ImageObservationRequest,
    data: ImageObservationData,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ImageObservationResult(BudgetedImageBlock<ImageObservationResultInner>);

impl ImageObservationResult {
    pub fn new(
        issued_path: ImageChargedPath,
        request: ImageObservationRequest,
        data: ImageObservationData,
        reservation: ImageDescriptorReservation,
    ) -> Result<Self, ImageDescriptorBudgetError> {
        if !issued_path.same_budget(reservation.budget())
            || !request.same_budget(reservation.budget())
        {
            return Err(ImageDescriptorBudgetError::ForeignBudget);
        }
        let matches = matches!(
            (request.operation(), &data),
            (
                ImageObservationOperation::Lstat,
                ImageObservationData::Lstat(_)
            ) | (
                ImageObservationOperation::BoundedDigest { .. },
                ImageObservationData::Digest(_)
            ) | (
                ImageObservationOperation::BoundedDirectory(_),
                ImageObservationData::Directory(_)
            ) | (
                ImageObservationOperation::BoundedReadLink { .. },
                ImageObservationData::ReadLink(_)
            ) | (
                ImageObservationOperation::BoundedElfMetadata { .. },
                ImageObservationData::ElfMetadata(_)
            ) | (_, ImageObservationData::LimitExceeded)
        );
        if !matches
            || !issued_path
                .as_path()
                .starts_with(request.scope_anchor_root().as_path())
        {
            return Err(ImageDescriptorBudgetError::ForeignBudget);
        }
        let within_bounds = match (request.operation(), &data) {
            (
                ImageObservationOperation::BoundedElfMetadata {
                    expected_digest,
                    policy,
                },
                ImageObservationData::ElfMetadata(PathOperationResult::Present(value)),
            ) => {
                if !value.same_budget(reservation.budget()) {
                    return Err(ImageDescriptorBudgetError::ForeignBudget);
                }
                value.digest() == expected_digest && value.policy() == policy
            }
            (
                ImageObservationOperation::BoundedElfMetadata { .. },
                ImageObservationData::LimitExceeded,
            ) => false,
            (
                ImageObservationOperation::BoundedDigest { max_bytes },
                ImageObservationData::Digest(PathOperationResult::Present(digest)),
            ) => digest.size_bytes() <= max_bytes,
            (
                ImageObservationOperation::BoundedReadLink { max_target_bytes },
                ImageObservationData::ReadLink(PathOperationResult::Present(target)),
            ) => target.as_os_str().len() <= max_target_bytes,
            (
                ImageObservationOperation::BoundedDirectory(limits),
                ImageObservationData::Directory(PathOperationResult::Present(rows)),
            ) => {
                let mut bytes = 0usize;
                rows.entries().len() <= limits.max_entries
                    && rows.entries().iter().all(|row| {
                        let size = row.name().as_os_str().len();
                        if size > limits.max_component_bytes {
                            return false;
                        }
                        match bytes.checked_add(size) {
                            Some(total) => {
                                bytes = total;
                                total <= limits.max_name_bytes_total
                            }
                            None => false,
                        }
                    })
            }
            _ => true,
        };
        if !within_bounds {
            return Err(ImageDescriptorBudgetError::ChargeOverflow);
        }
        Ok(Self(reservation.freeze(ImageObservationResultInner {
            issued_path,
            request,
            data,
        })?))
    }
    pub fn request(&self) -> &ImageObservationRequest {
        &self.0.get().request
    }
    pub fn issued_path(&self) -> &ImageChargedPath {
        &self.0.get().issued_path
    }
    pub fn data(&self) -> &ImageObservationData {
        &self.0.get().data
    }
    pub fn same_budget(&self, budget: &ImageDescriptorBudget) -> bool {
        self.issued_path().same_budget(budget)
            && self.request().same_budget(budget)
            && self.0.same_budget(budget)
            && match self.data() {
                ImageObservationData::ElfMetadata(PathOperationResult::Present(value)) => {
                    value.same_budget(budget)
                }
                _ => true,
            }
    }
}
