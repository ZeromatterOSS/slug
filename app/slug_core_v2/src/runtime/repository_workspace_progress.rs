//! Closed, consuming deterministic mutation of one inactive general workspace.
//! Trusted setup must exclude all other writers. There is no process-result
//! injection seam: archive capture uses its closed verified native owner;
//! Execute remains gated on the general native process owner. Every error
//! retires this entire generation, including partial writes.
use std::path::Path;

use slug_bzlmod_v2::RepositoryWorkspaceNode;
use slug_bzlmod_v2::RepositoryWorkspaceOperation;
use slug_bzlmod_v2::RepositoryWorkspaceRequest;
use slug_bzlmod_v2::RepositoryWorkspaceResult;
use slug_bzlmod_v2::RepositoryWorkspaceTranscript;

use super::snapshot::CreditedWorkspaceTranscript;
use super::*;
use crate::runtime::repository_filesystem_progress::deletion::DeleteWork;
use crate::runtime::repository_root_io::WriteMode;

pub(super) struct RepositoryWorkspaceProgress {
    builder: RepositoryPhysicalRootBuilder,
    transcript: CreditedWorkspaceTranscript,
    delete_work: DeleteWork,
    decoded: u64,
    scope: Arc<SourceServiceScope>,
    // Retire actual descriptors and transcript allocations before control credit.
    _control: resources::ResourceCredit,
}
impl RepositoryWorkspaceProgress {
    /// Begins only with the actual empty Archive root. A caller cannot import
    /// fabricated launch history or resume an unrelated physical generation.
    pub(super) fn new(
        builder: RepositoryPhysicalRootBuilder,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        check(scope, active)?;
        if builder.mode != RepositoryWorkspaceMode::Archive {
            return Err(PhysicalRootError::Mode);
        }
        let control = builder
            .resources
            .reserve(std::mem::size_of::<Self>(), 0, 0)?;
        let scratch = builder.resources.reserve(IO_SCRATCH, 3, 0)?;
        if backing_identity(&builder.file, builder.mode)? != builder.identity {
            return Err(PhysicalRootError::RootIdentity);
        }
        let io = RootIo::new(&builder.file, io_limits(builder.mode, &builder.reservation))?;
        let root_mode = (io.root_stat()?.st_mode & 0o7777) as u16;
        let bytes =
            RepositoryWorkspaceTranscript::initial_storage_bytes(&builder.reservation, root_mode)
                .map_err(PhysicalRootError::Protocol)?;
        let credit = builder.resources.reserve(
            add(bytes, std::mem::size_of::<CreditedWorkspaceTranscript>())?,
            0,
            0,
        )?;
        let transcript = RepositoryWorkspaceTranscript::new(builder.reservation.clone(), root_mode)
            .map_err(PhysicalRootError::Protocol)?;
        seal::verify(&io, transcript.current(), scope, active)?;
        if backing_identity(&builder.file, builder.mode)? != builder.identity {
            return Err(PhysicalRootError::RootIdentity);
        }
        drop(scratch);
        Ok(Self {
            builder,
            transcript: CreditedWorkspaceTranscript { transcript, credit },
            delete_work: DeleteWork::default(),
            decoded: 0,
            scope: scope.clone(),
            _control: control,
        })
    }

    pub(super) fn transcript(&self) -> &RepositoryWorkspaceTranscript {
        self.transcript.transcript()
    }

    fn check_scope(&self, supplied: &Arc<SourceServiceScope>) -> Result<(), PhysicalRootError> {
        crate::runtime::repository_io::source_service::check_identity(
            Some(&self.scope),
            Some(supplied),
        )
        .map_err(PhysicalRootError::SourceService)
    }

    /// Captures and decodes the declared archive before sampling the same actual
    /// root. Exact prefix replay performs no acquisition or decoded-byte charge.
    /// Existing explicit Host source acquisition policy remains separate from
    /// exclusively rooted workspace mutation.
    pub(super) fn apply_archive(
        mut self,
        request: RepositoryWorkspaceRequest,
        runtime: &tokio::runtime::Runtime,
        distdirs: &crate::runtime::ArchiveDistdirPolicy,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        self.check_scope(scope)?;
        check(scope, active)?;
        if self
            .transcript()
            .replay(&request)
            .map_err(PhysicalRootError::Protocol)?
            .is_some()
        {
            return Ok(self);
        }
        let capture = archive::VerifiedWorkspaceArchiveCapture::capture(
            &request,
            &self.builder.resources,
            runtime,
            distdirs,
            scope,
            active,
        )?;
        let result = capture.extract(&self.builder, &mut self.decoded, scope, active)?;
        self.complete_snapshot(request, result, scope, active)
    }

    fn complete_snapshot(
        mut self,
        request: RepositoryWorkspaceRequest,
        result: RepositoryWorkspaceResult,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        check(scope, active)?;
        let snapshot =
            snapshot::capture(&self.builder, self.transcript().current(), scope, active)?;
        let next = snapshot.complete(self.transcript(), request, result)?;
        self.transcript = next;
        Ok(self)
    }

    /// Incoming operation/argv/source storage is producer-credited already.
    /// Exact completed requests replay without rewinding the actual current tree.
    /// All refusal, cancellation and partial mutation paths consume this owner.
    pub(super) fn apply(
        mut self,
        request: RepositoryWorkspaceRequest,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        self.check_scope(scope)?;
        check(scope, active)?;
        if self
            .transcript()
            .replay(&request)
            .map_err(PhysicalRootError::Protocol)?
            .is_some()
        {
            return Ok(self);
        }
        let result = {
            let _scratch = self.builder.resources.reserve(IO_SCRATCH, 3, 0)?;
            if backing_identity(&self.builder.file, self.builder.mode)? != self.builder.identity {
                return Err(PhysicalRootError::RootIdentity);
            }
            let io = RootIo::new(
                &self.builder.file,
                io_limits(self.builder.mode, &self.builder.reservation),
            )?;
            let running = || active() && scope.check().is_ok();
            match request.operation() {
                RepositoryWorkspaceOperation::File { path, body, mode }
                | RepositoryWorkspaceOperation::Template {
                    path, body, mode, ..
                } => {
                    let path = Path::new(path.as_str());
                    if let Some(parent) = path.parent() {
                        io.ensure_directory(parent, 0o777)?;
                    }
                    io.write_file(
                        path,
                        body,
                        *mode as u32,
                        WriteMode::ReplaceRegular,
                        &running,
                    )?;
                    RepositoryWorkspaceResult::Unit
                }
                RepositoryWorkspaceOperation::Read(path) => {
                    // The full actual capture below authenticates these bytes;
                    // complete also requires the whole tree remain unchanged.
                    let Some(RepositoryWorkspaceNode::File(body)) = self
                        .transcript()
                        .current()
                        .entry(path)
                        .map(|entry| entry.node())
                    else {
                        return Err(PhysicalRootError::Snapshot);
                    };
                    RepositoryWorkspaceResult::Read(body.clone())
                }
                RepositoryWorkspaceOperation::Exists(path) => {
                    let exists = match io.lstat(Path::new(path.as_str())) {
                        Ok(_) => true,
                        Err(error)
                            if error.kind() == io::ErrorKind::NotFound
                                || error.raw_os_error() == Some(libc::ENOTDIR) =>
                        {
                            false
                        }
                        Err(error) => return Err(error.into()),
                    };
                    RepositoryWorkspaceResult::Exists(exists)
                }
                RepositoryWorkspaceOperation::Delete(path) => {
                    let removed =
                        crate::runtime::repository_filesystem_progress::deletion::delete_at(
                            &io,
                            path,
                            &mut self.delete_work,
                            &running,
                            Some(scope),
                        )
                        .map_err(PhysicalRootError::Mutation)?;
                    RepositoryWorkspaceResult::Delete(removed)
                }
                RepositoryWorkspaceOperation::Archive(_)
                | RepositoryWorkspaceOperation::Execute(_) => {
                    return Err(PhysicalRootError::UnsupportedOperation);
                }
            }
        };
        self.complete_snapshot(request, result, scope, active)
    }

    pub(super) fn seal(
        self,
        consumed_records: usize,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Arc<RepositoryPhysicalRoot>, PhysicalRootError> {
        self.check_scope(scope)?;
        let realization = self
            .transcript()
            .realize(consumed_records)
            .map_err(PhysicalRootError::Protocol)?;
        self.builder.seal(&realization, scope, active)
    }
}

#[cfg(test)]
#[path = "repository_workspace_progress/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "repository_workspace_progress/archive_tests.rs"]
mod archive_tests;

#[path = "repository_workspace_command.rs"]
pub(super) mod command;
