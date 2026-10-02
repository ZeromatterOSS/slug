//! Authenticated ELF wire facts through an already declared image member.
use slug_workspace_v2::ElfMetadataOutcome;
use slug_workspace_v2::ElfMetadataPolicy;
use slug_workspace_v2::ElfSourceDigestBinding;
use slug_workspace_v2::ImageChargedPath;
use slug_workspace_v2::ImageObservationData;
use slug_workspace_v2::ImageObservationOperation;
use slug_workspace_v2::ImageObservationResult;
use slug_workspace_v2::InputImageEntryKind;
use slug_workspace_v2::ObservedInputImage;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use slug_workspace_v2::VerifiedElfMetadata;

use super::InputImageCaptureError;
use super::ScopedImageSource;
use super::WorkspaceImageResources;

#[cfg(target_os = "linux")]
mod parser;
#[cfg(target_os = "linux")]
mod proof;
#[cfg(all(test, target_os = "linux"))]
mod tests;

/// The normal bridge mints this only after current image compute/selection.
/// No caller path, expected hash or previously captured stage is authority.
pub(in crate::runtime) struct AuthorizedElfObservation {
    source: ScopedImageSource,
    certificate: ObservedInputImage,
    file: ImageChargedPath,
    policy: ElfMetadataPolicy,
    digest: ElfSourceDigestBinding,
    mode: u32,
}

impl AuthorizedElfObservation {
    pub(in crate::runtime) fn new(
        source: ScopedImageSource,
        certificate: ObservedInputImage,
        file: ImageChargedPath,
        policy: ElfMetadataPolicy,
        resources: &WorkspaceImageResources,
    ) -> Result<Self, InputImageCaptureError> {
        if source.namespace != certificate.namespace()
            || &source.anchor != certificate.anchor_root()
            || !certificate
                .source_root()
                .as_path()
                .starts_with(source.anchor.as_path())
            || !file
                .as_path()
                .starts_with(certificate.source_root().as_path())
            || !certificate.same_budget(resources.descriptor_budget())
            || !file.same_budget(resources.descriptor_budget())
        {
            return Err(InputImageCaptureError::Unauthorized);
        }
        let index = certificate
            .entries()
            .binary_search_by(|entry| entry.path().as_path().cmp(file.as_path()))
            .map_err(|_| InputImageCaptureError::Unauthorized)?;
        let entry = &certificate.entries()[index];
        let InputImageEntryKind::RegularFile(digest) = entry.kind() else {
            return Err(InputImageCaptureError::Unauthorized);
        };
        let mode = entry.mode();
        if digest.size_bytes() > certificate.policy().limits().max_file_bytes {
            return Err(InputImageCaptureError::Unauthorized);
        }
        Ok(Self {
            source,
            certificate,
            file,
            policy,
            digest: ElfSourceDigestBinding::from_digest(digest),
            mode,
        })
    }
}

pub(in crate::runtime) fn observe_elf_metadata(
    authorized: &AuthorizedElfObservation,
    demand: &PathObservationDemand,
    resources: &WorkspaceImageResources,
    active: &dyn Fn() -> bool,
) -> Result<PathObservationResult, InputImageCaptureError> {
    if !active() {
        return Err(InputImageCaptureError::Cancelled);
    }
    let PathObservationOperation::InputImage(request) = demand.operation() else {
        return Err(InputImageCaptureError::Unauthorized);
    };
    let ImageObservationOperation::BoundedElfMetadata {
        expected_digest,
        policy,
    } = request.operation()
    else {
        return Err(InputImageCaptureError::Unauthorized);
    };
    if demand.namespace() != authorized.source.namespace
        || demand.path().as_path() != authorized.file.as_path()
        || request.scope_anchor_root() != &authorized.source.anchor
        || expected_digest != authorized.digest
        || policy != authorized.policy
        || !request.same_budget(resources.descriptor_budget())
        || !demand
            .image_path()
            .is_some_and(|path| path.same_budget(resources.descriptor_budget()))
        || !authorized
            .certificate
            .same_budget(resources.descriptor_budget())
    {
        return Err(InputImageCaptureError::Unauthorized);
    }
    #[cfg(target_os = "linux")]
    {
        super::capture::verify_frontier(
            &authorized.source,
            &authorized.certificate,
            resources,
            active,
        )?;
        let relative = authorized
            .file
            .as_path()
            .strip_prefix(authorized.source.anchor.as_path())
            .map_err(|_| InputImageCaptureError::Unauthorized)?;
        let before = super::linux::fs::lstat(&authorized.source.root, relative)?;
        let file = super::linux::fs::regular(&authorized.source.root, relative)?;
        let opened = super::linux::fd_stat(&file)?;
        if !same_stat(&before, &opened)
            || opened.st_size < 0
            || opened.st_size as u64 != expected_digest.size_bytes()
            || opened.st_mode & 0o7777 != authorized.mode
        {
            return Err(InputImageCaptureError::StaleSource);
        }
        let mut credit = resources.descriptor_budget().try_reserve(512)?;
        let mut reader = proof::ProofReader::new(
            &file,
            expected_digest.size_bytes(),
            policy,
            resources.descriptor_budget(),
            active,
        )?;
        let tentative = parser::parse(&mut reader, policy, &mut credit)?;
        reader.verify(expected_digest)?;
        if matches!(&tentative, ElfMetadataOutcome::Rejected(_)) {
            // Partial decoded facts have already dropped. History retains only
            // the authenticated small enum, not their abandoned capacity peak.
            drop(credit);
            credit = resources.descriptor_budget().try_reserve(512)?;
        }
        let after = super::linux::fd_stat(&file)?;
        let named = super::linux::fs::lstat(&authorized.source.root, relative)?;
        if !same_stat(&before, &after) || !same_stat(&after, &named) {
            return Err(InputImageCaptureError::StaleSource);
        }
        super::capture::verify_frontier(
            &authorized.source,
            &authorized.certificate,
            resources,
            active,
        )?;
        if !active() {
            return Err(InputImageCaptureError::Cancelled);
        }
        let verified = VerifiedElfMetadata::new(expected_digest, policy, tentative, credit)?;
        let result_credit = resources.descriptor_budget().try_reserve(256)?;
        let result = ImageObservationResult::new(
            demand.image_path().expect("validated charged path").clone(),
            request,
            ImageObservationData::ElfMetadata(PathOperationResult::Present(verified)),
            result_credit,
        )?;
        if !active() {
            return Err(InputImageCaptureError::Cancelled);
        }
        Ok(PathObservationResult::InputImage(result))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (expected_digest, policy);
        Err(InputImageCaptureError::UnsupportedPlatform)
    }
}

#[cfg(target_os = "linux")]
fn same_stat(left: &libc::stat, right: &libc::stat) -> bool {
    left.st_dev == right.st_dev
        && left.st_ino == right.st_ino
        && left.st_mode == right.st_mode
        && left.st_size == right.st_size
        && left.st_mtime == right.st_mtime
        && left.st_mtime_nsec == right.st_mtime_nsec
        && left.st_ctime == right.st_ctime
        && left.st_ctime_nsec == right.st_ctime_nsec
}
