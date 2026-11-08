//! Closed, consuming deterministic mutation of one inactive general workspace.
//! Trusted setup must exclude all other writers. There is no process-result
//! injection seam: archive capture uses its closed verified native owner;
//! Execute enters only through the separate whole-command process owner. Every error
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
    link_sources: Option<Arc<SourceLinkSet>>,
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
        builder.check_origin(scope, active)?;
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
            transcript: CreditedWorkspaceTranscript {
                transcript,
                dependencies: None,
                credit,
            },
            delete_work: DeleteWork::default(),
            decoded: 0,
            scope: scope.clone(),
            link_sources: None,
            _control: control,
        })
    }

    pub(super) fn builder(&self) -> &RepositoryPhysicalRootBuilder {
        &self.builder
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
        if self.link_sources.is_none() {
            let sources = SourceLinkSet::empty(&self.builder.resources, scope, active)?;
            self.admit_link_sources(sources)?;
        }
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
        let sources = match &self.link_sources {
            Some(sources) => sources.clone(),
            None => SourceLinkSet::empty(&self.builder.resources, scope, active)?,
        };
        let snapshot = snapshot::capture_with_sources(
            &self.builder,
            self.transcript().current(),
            sources,
            scope,
            active,
        )?;
        let next = snapshot.complete_with_dependencies(
            self.transcript(),
            request,
            result,
            self.transcript.dependencies.clone(),
        )?;
        self.transcript = next;
        self.link_sources = None;
        Ok(self)
    }

    pub(super) fn complete_snapshot_covered(
        mut self,
        request: Arc<CoveredWorkspaceRequest>,
        result: RepositoryWorkspaceResult,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        let dependencies = if let Some(prior) = &self.transcript.dependencies {
            prior.merge(request.dependencies().owners(), &self.builder.resources)?
        } else {
            request.dependencies().clone()
        };
        if !dependencies.covers_transcript(self.transcript()) {
            return Err(PhysicalRootError::Reservation);
        }
        self.transcript.dependencies = Some(dependencies);
        let views = match request.operation() {
            RepositoryWorkspaceOperation::Execute(execute) => execute.read_views().views(),
            _ => return Err(PhysicalRootError::UnsupportedOperation),
        };
        self.link_sources = Some(SourceLinkSet::from_views(
            request.dependencies(),
            views,
            &self.builder.resources,
            scope,
            active,
        )?);
        self.complete_snapshot(request.request().clone(), result, scope, active)
    }
    pub(super) fn admit_link_sources(
        &mut self,
        sources: Arc<SourceLinkSet>,
    ) -> Result<(), PhysicalRootError> {
        if !sources
            .dependencies()
            .covers_state(self.transcript().current())
        {
            return Err(PhysicalRootError::Reservation);
        }
        let deps = match &self.transcript.dependencies {
            Some(old) => old.merge(sources.dependencies().owners(), &self.builder.resources)?,
            None => sources.dependencies().clone(),
        };
        self.transcript.dependencies = Some(deps);
        self.link_sources = Some(sources);
        Ok(())
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
        if self.link_sources.is_none() {
            let sources = SourceLinkSet::empty(&self.builder.resources, scope, active)?;
            self.admit_link_sources(sources)?;
        }
        if self
            .transcript()
            .replay(&request)
            .map_err(PhysicalRootError::Protocol)?
            .is_some()
        {
            return Ok(self);
        }
        let mut captured_read = None;
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
                    let sources = self
                        .link_sources
                        .as_ref()
                        .ok_or(PhysicalRootError::Reservation)?;
                    let read = routed::RouteRoots::new(
                        &self.builder.file,
                        self.transcript().current(),
                        sources,
                        &self.builder.resources,
                        scope,
                        active,
                    )
                    .read(path)?;
                    let result = read.result();
                    captured_read = Some(read);
                    result
                }
                RepositoryWorkspaceOperation::Exists(path) => {
                    let sources = self
                        .link_sources
                        .as_ref()
                        .ok_or(PhysicalRootError::Reservation)?;
                    let exists = routed::RouteRoots::new(
                        &self.builder.file,
                        self.transcript().current(),
                        sources,
                        &self.builder.resources,
                        scope,
                        active,
                    )
                    .exists(path)?;
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
                RepositoryWorkspaceOperation::Symlink { path, target, .. } => {
                    let sources = self
                        .link_sources
                        .as_ref()
                        .ok_or(PhysicalRootError::Reservation)?;
                    let proof =
                        sources.validate_absolute(&self.builder.reservation, path, target)?;
                    let path = Path::new(path.as_str());
                    if let Some(parent) = path.parent() {
                        io.ensure_directory(parent, 0o777)?;
                    }
                    io.symlink_validated_absolute(path, &proof)?;
                    RepositoryWorkspaceResult::Unit
                }
                RepositoryWorkspaceOperation::Archive(_)
                | RepositoryWorkspaceOperation::Execute(_) => {
                    return Err(PhysicalRootError::UnsupportedOperation);
                }
            }
        };
        let completed = self.complete_snapshot(request, result, scope, active);
        drop(captured_read);
        completed
    }

    pub(super) fn seal_realization(
        self,
        realization: &RepositoryWorkspaceRealization,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Arc<RepositoryPhysicalRoot>, PhysicalRootError> {
        self.check_scope(scope)?;
        if !ViewComparison::new(&self.builder.resources, scope, active)?
            .transcript_equal(self.transcript(), realization.transcript())?
        {
            return Err(PhysicalRootError::Protocol(
                slug_bzlmod_v2::RepositoryWorkspaceError::ForeignPredecessor,
            ));
        }
        // Seal the native canonical transcript after exact foreign comparison;
        // equal foreign Source allocations must not replace our covered data.
        self.seal(realization.transcript().records().len(), scope, active)
    }

    pub(super) fn seal(
        self,
        consumed_records: usize,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Arc<RepositoryPhysicalRoot>, PhysicalRootError> {
        self.check_scope(scope)?;
        // The old complete transcript guard remains armed throughout this
        // borrowed clone. Prepay only its additional realization/cache storage.
        let full = self
            .transcript()
            .realization_storage_partition()
            .map_err(PhysicalRootError::Protocol)?;
        let existing = self
            .transcript()
            .storage_partition()
            .map_err(PhysicalRootError::Protocol)?;
        let (full, existing) = if let Some(deps) = &self.transcript.dependencies {
            if !deps.covers_transcript(self.transcript()) {
                return Err(PhysicalRootError::Reservation);
            }
            (full.owned_bytes(), existing.owned_bytes())
        } else {
            (
                full.total_bytes().map_err(PhysicalRootError::Protocol)?,
                existing
                    .total_bytes()
                    .map_err(PhysicalRootError::Protocol)?,
            )
        };
        let _realization_credit = self.builder.resources.reserve(
            full.checked_sub(existing)
                .ok_or(PhysicalRootError::Resource)?,
            0,
            0,
        )?;
        let realization = self
            .transcript()
            .realize(consumed_records)
            .map_err(PhysicalRootError::Protocol)?;
        let deps = match &self.transcript.dependencies {
            Some(deps) => deps.clone(),
            None => NativeReadViewDependencies::empty(&self.builder.resources, 0)?,
        };
        self.builder
            .seal_with_dependencies(&realization, deps, scope, active)
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
