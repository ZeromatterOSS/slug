//! Closed actual archive acquisition→rooted extraction handoff. The existing
//! admitted Host/HTTP source policy is unchanged; source acquisition is not newly
//! confined. Workspace mutation uses only the selected RootIo descriptor route.
//! Trusted capture creation excludes other writers until capture retirement.

use std::fs::Metadata;
use std::os::unix::fs::FileExt;
use std::os::unix::fs::MetadataExt;

use sha2::Digest;
use sha2::Sha256;
use slug_bzlmod_v2::GeneratedRepositoryArchiveEffect;
use slug_bzlmod_v2::RepositoryWorkspaceOperation;
use slug_bzlmod_v2::RepositoryWorkspaceRequest;
use slug_bzlmod_v2::RepositoryWorkspaceResult;

use super::*;
use crate::runtime::ArchiveDistdirPolicy;
use crate::runtime::repository_archive_http::capture_workspace_generated_archive;

const CAPTURE_BYTES: u64 = 128 * 1024 * 1024;
const CHUNK_BYTES: usize = 64 * 1024;
const MAX_URLS: usize = 8;
const MAX_URL_BYTES: usize = 2048;
// Producer-selected tempfile path storage and actual SHA/stat/syscall scratch.
// Source-scope transport buffers/codec banks remain separately admitted.
const CAPTURE_SCRATCH: usize = CHUNK_BYTES + 2 * 4096 + 4 * std::mem::size_of::<Metadata>() + 1024;
// The existing decoder retains <=1024 names of <=256 bytes and its bounded
// duplicate-destination set. Prepay old/new table/string peaks conservatively;
// no new map/hasher or archive policy is introduced by this adapter.
const DECODE_CONTROL: usize = 4 * 1024 * (256 + std::mem::size_of::<String>() + 32) + 4096;

/// A capability minted only by the actual full-FD verifying producer below.
/// No path/File/writer/unchecked digest constructor escapes this leaf. The
/// capture closes before its FD/backing/control credits can retire.
pub(super) struct VerifiedWorkspaceArchiveCapture<'request> {
    capture: Option<tempfile::NamedTempFile>,
    request: &'request RepositoryWorkspaceRequest,
    frame: Option<Metadata>,
    sha256: [u8; 32],
    resources: Arc<RepositoryRootResources>,
    scope: Arc<SourceServiceScope>,
    _credit: resources::ResourceCredit,
    storage_credit: Option<resources::ResourceCredit>,
}
impl std::fmt::Debug for VerifiedWorkspaceArchiveCapture<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerifiedWorkspaceArchiveCapture")
            .field("index", &self.request.index())
            .field("bytes", &self.frame.as_ref().map(Metadata::len))
            .finish_non_exhaustive()
    }
}

impl<'request> VerifiedWorkspaceArchiveCapture<'request> {
    /// Incoming request owners are producer-credited and stay borrowed. The
    /// Archive intent—not a tool/source/caller profile—selects capture semantics.
    pub(super) fn capture(
        request: &'request RepositoryWorkspaceRequest,
        resources: &Arc<RepositoryRootResources>,
        runtime: &tokio::runtime::Runtime,
        distdirs: &ArchiveDistdirPolicy,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        check(scope, active)?;
        let RepositoryWorkspaceOperation::Archive(archive) = request.operation() else {
            return Err(PhysicalRootError::UnsupportedOperation);
        };
        if archive.urls().is_empty() || archive.urls().len() > MAX_URLS {
            return Err(PhysicalRootError::Resource);
        }
        let mut url_storage = archive
            .urls()
            .len()
            .checked_mul(2 * std::mem::size_of::<String>())
            .ok_or(PhysicalRootError::Resource)?;
        for url in archive.urls() {
            if url.len() > MAX_URL_BYTES {
                return Err(PhysicalRootError::Resource);
            }
            url_storage = add(
                url_storage,
                add(
                    url.len()
                        .checked_mul(2)
                        .ok_or(PhysicalRootError::Resource)?,
                    64,
                )?,
            )?;
        }
        // Eight conservative acquisition slots include capture/source/control
        // peaks. The128MiB compressed allowance is backing storage, not RAM.
        let credit = resources.reserve(
            add(
                std::mem::size_of::<Self>(),
                add(CAPTURE_SCRATCH, url_storage)?,
            )?,
            8,
            0,
        )?;
        let storage_credit = resources.reserve(0, 0, CAPTURE_BYTES)?;
        let mut urls = Vec::new();
        urls.try_reserve_exact(archive.urls().len())
            .map_err(|_| PhysicalRootError::Resource)?;
        if urls.capacity() > archive.urls().len() {
            return Err(PhysicalRootError::Resource);
        }
        for url in archive.urls() {
            let mut owned = String::new();
            owned
                .try_reserve_exact(url.len())
                .map_err(|_| PhysicalRootError::Resource)?;
            if owned.capacity() > url.len().saturating_mul(2).saturating_add(64) {
                return Err(PhysicalRootError::Resource);
            }
            owned.push_str(url);
            urls.push(owned);
        }
        let captured = capture_workspace_generated_archive(
            &urls,
            archive.expected_sha256(),
            runtime,
            active,
            scope,
            distdirs,
        );
        // Retire copied URL storage before the later owner's failure cleanup
        // can return the control credit that paid for these allocations.
        drop(urls);
        let capture = match captured {
            Ok(capture) => capture,
            Err(error) => {
                check(scope, active)?;
                return Err(PhysicalRootError::Mutation(error));
            }
        };
        // Arm owner-first cleanup before every post-acquisition fallible check.
        let mut owner = Self {
            capture: Some(capture),
            request,
            frame: None,
            sha256: [0; 32],
            resources: resources.clone(),
            scope: scope.clone(),
            _credit: credit,
            storage_credit: Some(storage_credit),
        };
        let path = owner.capture.as_ref().expect("new capture").path();
        if path.as_os_str().len() > 4096 || path.as_os_str().as_encoded_bytes().contains(&0) {
            return Err(PhysicalRootError::Resource);
        }
        let file = owner.capture.as_ref().unwrap().as_file();
        let frame = file.metadata()?;
        let sha256 = verify_capture(file, &frame, archive.expected_sha256(), scope, active)?;
        owner.frame = Some(frame);
        owner.sha256 = sha256;
        check(scope, active)?;
        Ok(owner)
    }

    pub(super) fn extract(
        mut self,
        builder: &RepositoryPhysicalRootBuilder,
        aggregate_decoded: &mut u64,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<RepositoryWorkspaceResult, PhysicalRootError> {
        self.check_scope(scope, active)?;
        if self.request.reservation().as_ref() != builder.reservation.as_ref() {
            return Err(PhysicalRootError::Reservation);
        }
        if !Arc::ptr_eq(&self.resources, &builder.resources) {
            return Err(PhysicalRootError::Resource);
        }
        let _decode = builder
            .resources
            .reserve(add(IO_SCRATCH, DECODE_CONTROL)?, 4, 0)?;
        if backing_identity(&builder.file, builder.mode)? != builder.identity {
            return Err(PhysicalRootError::RootIdentity);
        }
        let io = RootIo::new(&builder.file, io_limits(builder.mode, &builder.reservation))?;
        let result = self.extract_at(&io, aggregate_decoded, scope, active)?;
        if backing_identity(&builder.file, builder.mode)? != builder.identity {
            return Err(PhysicalRootError::RootIdentity);
        }
        self.check_scope(scope, active)?;
        self.close_capture()?;
        check(scope, active)?;
        Ok(result)
    }

    fn archive(&self) -> &GeneratedRepositoryArchiveEffect {
        match self.request.operation() {
            RepositoryWorkspaceOperation::Archive(archive) => archive,
            _ => unreachable!("only Archive producer constructs this owner"),
        }
    }
    fn check_scope(
        &self,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        if !Arc::ptr_eq(&self.scope, scope) {
            return Err(PhysicalRootError::SourceService(
                SourceServiceError::ForeignScope,
            ));
        }
        check(scope, active)
    }
    fn extract_at(
        &mut self,
        root: &RootIo<'_>,
        aggregate_decoded: &mut u64,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<RepositoryWorkspaceResult, PhysicalRootError> {
        self.check_scope(scope, active)?;
        let file = self
            .capture
            .as_ref()
            .expect("live verified capture")
            .as_file();
        let before = self.frame.as_ref().expect("only verified captures extract");
        if !unchanged(before, &file.metadata()?) {
            return Err(PhysicalRootError::Snapshot);
        }
        let running = || active() && scope.check().is_ok();
        crate::runtime::repository_generated_archive::extract_generated_archive_at(
            file,
            root,
            self.archive().strip_prefix(),
            self.archive().format(),
            &running,
            aggregate_decoded,
            Some(scope),
        )
        .map_err(PhysicalRootError::Mutation)?;
        self.check_scope(scope, active)?;
        if !unchanged(before, &file.metadata()?) {
            return Err(PhysicalRootError::Snapshot);
        }
        Ok(RepositoryWorkspaceResult::Archive(self.sha256))
    }
    fn close_capture(&mut self) -> Result<(), PhysicalRootError> {
        if let Some(capture) = self.capture.take() {
            if let Err(error) = capture.close() {
                // Failed unlink may leave backing storage behind. Never recycle
                // its allowance; this matches existing source-owner cleanup.
                if let Some(credit) = self.storage_credit.take() {
                    std::mem::forget(credit);
                }
                return Err(PhysicalRootError::Io(error));
            }
        }
        Ok(())
    }
}
impl Drop for VerifiedWorkspaceArchiveCapture<'_> {
    fn drop(&mut self) {
        let _ = self.close_capture();
    }
}

fn unchanged(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.mode() == b.mode()
        && a.nlink() == b.nlink()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
fn invalid(message: &'static str) -> PhysicalRootError {
    PhysicalRootError::Mutation(
        crate::runtime::repository_io::ArchiveMaterializationError::materialization(message),
    )
}
fn verify_capture(
    file: &File,
    frame: &Metadata,
    expected: Option<[u8; 32]>,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<[u8; 32], PhysicalRootError> {
    check(scope, active)?;
    if !frame.is_file() || frame.nlink() != 1 || frame.len() > CAPTURE_BYTES {
        return Err(invalid(
            "generated archive capture exceeds its regular single-link bound",
        ));
    }
    let mut buffer = [0u8; CHUNK_BYTES];
    let mut offset = 0u64;
    let mut hash = Sha256::new();
    loop {
        check(scope, active)?;
        let remaining = frame.len().saturating_sub(offset);
        let requested = if remaining == 0 {
            1
        } else {
            remaining.min(CHUNK_BYTES as u64) as usize
        };
        let count = match file.read_at(&mut buffer[..requested], offset) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        check(scope, active)?;
        if count == 0 {
            if offset != frame.len() {
                return Err(invalid("generated archive capture was truncated"));
            }
            break;
        }
        if count as u64 > remaining {
            return Err(invalid("generated archive capture grew"));
        }
        offset = offset
            .checked_add(count as u64)
            .ok_or(PhysicalRootError::Resource)?;
        hash.update(&buffer[..count]);
        #[cfg(test)]
        crate::runtime::repository_archive_checksum::work_for_test::record_verified(count);
    }
    if !unchanged(frame, &file.metadata()?) {
        return Err(PhysicalRootError::Snapshot);
    }
    let actual: [u8; 32] = hash.finalize().into();
    if expected.is_some_and(|expected| actual != expected) {
        return Err(invalid("generated archive capture SHA-256 mismatch"));
    }
    check(scope, active)?;
    Ok(actual)
}

#[cfg(test)]
#[path = "repository_workspace_archive/tests.rs"]
mod tests;
