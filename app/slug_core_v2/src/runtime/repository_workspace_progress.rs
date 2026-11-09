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

use super::body_coverage::NativeBodyCandidates;
use super::body_coverage::NativeBodyLedger;
use super::snapshot::CreditedWorkspaceTranscript;
use super::*;
use crate::runtime::repository_filesystem_progress::deletion::DeleteWork;
use crate::runtime::repository_root_io::WriteMode;

struct IncomingOperation {
    request: RepositoryWorkspaceRequest,
    candidates: NativeBodyCandidates,
}
impl IncomingOperation {
    fn cover(&mut self, active: &dyn Fn() -> bool) -> Result<(), PhysicalRootError> {
        self.request.try_visit_body_allocations(&mut |body| {
            self.candidates.retain(body, active).map(|_| ())
        })
    }
}

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
        let body_ledger =
            NativeBodyLedger::new(&builder.resources, scope, &builder.reservation, active)?;
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
                body_ledger: Some(body_ledger),
                credit,
            },
            delete_work: DeleteWork::default(),
            decoded: 0,
            scope: scope.clone(),
            link_sources: None,
            _control: control,
        })
    }

    pub(super) fn body_ledger(&self) -> &Arc<NativeBodyLedger> {
        self.transcript
            .body_ledger
            .as_ref()
            .expect("native progress has body coverage")
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
        self,
        request: RepositoryWorkspaceRequest,
        runtime: &tokio::runtime::Runtime,
        distdirs: &crate::runtime::ArchiveDistdirPolicy,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        self.check_scope(scope)?;
        check(scope, active)?;
        // Keep the raw request ahead of candidate destruction even if staging
        // refuses before capture or mutation.
        let candidates = self.body_ledger().candidates()?;
        self.apply_archive_with_candidates(request, candidates, runtime, distdirs, scope, active)
    }
    pub(super) fn apply_archive_with_candidates(
        mut self,
        request: RepositoryWorkspaceRequest,
        candidates: NativeBodyCandidates,
        runtime: &tokio::runtime::Runtime,
        distdirs: &crate::runtime::ArchiveDistdirPolicy,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        let mut incoming = IncomingOperation {
            request,
            candidates,
        };
        self.check_scope(scope)?;
        check(scope, active)?;
        incoming.candidates.ledger().check_origin(
            &self.builder.resources,
            scope,
            &self.builder.reservation,
            active,
        )?;
        incoming.cover(active)?;
        if self.link_sources.is_none() {
            let sources = SourceLinkSet::empty(&self.builder.resources, scope, active)?;
            self.admit_link_sources(sources)?;
        }
        // Admit after the existing scope/origin/declared-source checks, but
        // before replay can compare independent operation/Read-result bodies.
        let _semantic_working = self.builder.resources.reserve(
            2 * RepositoryWorkspaceBody::decode_scratch_bytes(),
            0,
            0,
        )?;
        if self
            .transcript()
            .replay(&incoming.request)
            .map_err(PhysicalRootError::Protocol)?
            .is_some()
        {
            return Ok(self);
        }
        let capture = archive::VerifiedWorkspaceArchiveCapture::capture(
            &incoming.request,
            &self.builder.resources,
            runtime,
            distdirs,
            scope,
            active,
        )?;
        let result = capture.extract(&self.builder, &mut self.decoded, scope, active)?;
        self.complete_snapshot(incoming, result, scope, active)
    }

    fn complete_snapshot(
        mut self,
        incoming: IncomingOperation,
        result: RepositoryWorkspaceResult,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        // Result (which can own a newly captured body) precedes incoming
        // candidates, including capture refusal before completion is entered.
        struct CompletionInput {
            result: RepositoryWorkspaceResult,
            incoming: IncomingOperation,
        }
        let mut input = CompletionInput { result, incoming };
        check(scope, active)?;
        let sources = match &self.link_sources {
            Some(sources) => sources.clone(),
            None => SourceLinkSet::empty(&self.builder.resources, scope, active)?,
        };
        let snapshot = snapshot::capture_with_candidates(
            &self.builder,
            self.transcript().current(),
            sources,
            &mut input.incoming.candidates,
            scope,
            active,
        )?;
        let next = snapshot
            .with_body_candidates(input.incoming.candidates)
            .complete_active(
                self.transcript(),
                input.incoming.request,
                input.result,
                self.transcript.dependencies.clone(),
                active,
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
        let mut incoming = IncomingOperation {
            request: request.request().clone(),
            candidates: self.body_ledger().candidates()?,
        };
        if let Some(bodies) = request.body_candidates() {
            incoming.candidates.absorb(bodies, active)?;
        }
        incoming.cover(active)?;
        self.complete_snapshot(incoming, result, scope, active)
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
        self,
        request: RepositoryWorkspaceRequest,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        self.check_scope(scope)?;
        check(scope, active)?;
        let candidates = self.body_ledger().candidates()?;
        self.apply_with_candidates(request, candidates, scope, active)
    }
    pub(super) fn apply_with_candidates(
        mut self,
        request: RepositoryWorkspaceRequest,
        candidates: NativeBodyCandidates,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        let mut incoming = IncomingOperation {
            request,
            candidates,
        };
        self.check_scope(scope)?;
        check(scope, active)?;
        incoming.candidates.ledger().check_origin(
            &self.builder.resources,
            scope,
            &self.builder.reservation,
            active,
        )?;
        incoming.cover(active)?;
        if self.link_sources.is_none() {
            let sources = SourceLinkSet::empty(&self.builder.resources, scope, active)?;
            self.admit_link_sources(sources)?;
        }
        // Admit after the existing scope/origin/declared-source checks, but
        // before replay can compare independent operation/Read-result bodies.
        let _semantic_working = self.builder.resources.reserve(
            2 * RepositoryWorkspaceBody::decode_scratch_bytes(),
            0,
            0,
        )?;
        if self
            .transcript()
            .replay(&incoming.request)
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
            match incoming.request.operation() {
                RepositoryWorkspaceOperation::File { path, body, mode }
                | RepositoryWorkspaceOperation::Template {
                    path, body, mode, ..
                } => {
                    let path = Path::new(path.as_str());
                    if let Some(parent) = path.parent() {
                        io.ensure_directory(parent, 0o777)?;
                    }
                    let _decode = self.builder.resources.reserve(
                        RepositoryWorkspaceBody::decode_scratch_bytes(),
                        0,
                        0,
                    )?;
                    io.write_file_chunks(
                        path,
                        body.len() as u64,
                        *mode as u32,
                        WriteMode::ReplaceRegular,
                        &running,
                        |visit| body.try_for_each_chunk(&mut |chunk| visit(chunk)),
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
                    .read_with_body_candidates(path, &mut incoming.candidates)?;
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
        let completed = self.complete_snapshot(incoming, result, scope, active);
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
        let mut comparison = ViewComparison::new(&self.builder.resources, scope, active)?;
        if !comparison.transcript_equal(self.transcript(), realization.transcript())? {
            return Err(PhysicalRootError::Protocol(
                slug_bzlmod_v2::RepositoryWorkspaceError::ForeignPredecessor,
            ));
        }
        self.check_scope(scope)?;
        let _realization_credit = self.realization_credit()?;
        // Preserve the closed SHA while selecting the native allocation graph.
        // Keep decoder working credit through canonicalized_for's full Eq.
        let canonical = realization
            .canonicalized_for(self.transcript())
            .map_err(PhysicalRootError::Protocol)?;
        drop(comparison);
        self.seal_canonical(canonical, _realization_credit, scope, active)
    }

    pub(super) fn seal(
        self,
        consumed_records: usize,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Arc<RepositoryPhysicalRoot>, PhysicalRootError> {
        self.check_scope(scope)?;
        let _realization_credit = self.realization_credit()?;
        // Preserve realize's body-free error before admitting decode work.
        if consumed_records != self.transcript().records().len() {
            return Err(PhysicalRootError::Protocol(
                slug_bzlmod_v2::RepositoryWorkspaceError::UnusedRecords,
            ));
        }
        // Direct Progress callers have no enclosing Command working credit.
        let decode = self.builder.resources.reserve(
            RepositoryWorkspaceBody::decode_scratch_bytes(),
            0,
            0,
        )?;
        let realization = self
            .transcript()
            .realize(consumed_records)
            .map_err(PhysicalRootError::Protocol)?;
        drop(decode);
        self.seal_canonical(realization, _realization_credit, scope, active)
    }

    fn realization_credit(&self) -> Result<ResourceCredit, PhysicalRootError> {
        // The old complete transcript guard remains armed throughout this
        // borrowed clone. Prepay only its additional realization/cache storage.
        let full = self
            .transcript()
            .realization_body_covered_storage_partition()
            .map_err(PhysicalRootError::Protocol)?;
        let existing = self
            .transcript()
            .body_covered_storage_partition()
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
        self.builder.resources.reserve(
            full.checked_sub(existing)
                .ok_or(PhysicalRootError::Resource)?,
            0,
            0,
        )
    }

    fn seal_canonical(
        self,
        realization: RepositoryWorkspaceRealization,
        realization_credit: ResourceCredit,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Arc<RepositoryPhysicalRoot>, PhysicalRootError> {
        struct SealInput {
            // The realization borrows full historical allocations through its
            // transcript. Retire it before the progress graph and its credits.
            realization: RepositoryWorkspaceRealization,
            progress: RepositoryWorkspaceProgress,
            _credit: ResourceCredit,
        }
        let input = SealInput {
            realization,
            progress: self,
            _credit: realization_credit,
        };
        let deps = match &input.progress.transcript.dependencies {
            Some(deps) => deps.clone(),
            None => NativeReadViewDependencies::empty(&input.progress.builder.resources, 0)?,
        };
        let final_bodies = input.progress.body_ledger().final_set(
            |visit| {
                input
                    .progress
                    .transcript()
                    .current()
                    .try_visit_body_allocations(&mut |body| visit(body))
            },
            active,
        )?;
        input.progress.builder.seal_with_body_coverage(
            &input.realization,
            deps,
            final_bodies,
            scope,
            active,
        )
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
