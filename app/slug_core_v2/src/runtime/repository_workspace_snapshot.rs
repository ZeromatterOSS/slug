//! Actual complete-tree capture for closed native progress. This child of the
//! physical-root owner borrows its authenticated FD; no native authority or
//! resource-bearing value enters DICE. All external writers must be quiescent.

use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use compact_str::CompactString;
use slug_bzlmod_v2::RepositoryWorkspaceEntry;
use slug_bzlmod_v2::RepositoryWorkspaceNode;
use slug_bzlmod_v2::RepositoryWorkspaceRequest;
use slug_bzlmod_v2::RepositoryWorkspaceResult;
use slug_bzlmod_v2::RepositoryWorkspaceTranscript;

use super::body_coverage::NativeBodyAdmission;
use super::body_coverage::NativeBodyCandidates;
use super::body_coverage::NativeBodyLedger;
use super::*;

#[path = "repository_workspace_snapshot/body_recovery.rs"]
mod body_recovery;

/// Entries/bodies precede their credit on destruction. No uncredited owned
/// entries accessor exists; protocol completion prepays its successor first.
#[derive(Debug)]
pub(super) struct CapturedWorkspaceSnapshot<'prior> {
    entries: Vec<RepositoryWorkspaceEntry>,
    mode: RepositoryWorkspaceMode,
    prior: &'prior RepositoryWorkspaceState,
    resources: Arc<RepositoryRootResources>,
    scope: Arc<SourceServiceScope>,
    body_candidates: Option<NativeBodyCandidates>,
    credit: resources::ResourceCredit,
}

/// Native progress retains this owner; only its immutable transcript projects
/// into DICE. Parent progress may transfer these fields together after admission.
#[derive(Debug)]
pub(super) struct CreditedWorkspaceTranscript {
    pub(super) transcript: RepositoryWorkspaceTranscript,
    pub(super) dependencies: Option<Arc<NativeReadViewDependencies>>,
    pub(super) body_ledger: Option<Arc<NativeBodyLedger>>,
    pub(super) credit: resources::ResourceCredit,
}
impl CreditedWorkspaceTranscript {
    pub(super) fn transcript(&self) -> &RepositoryWorkspaceTranscript {
        &self.transcript
    }
}
impl CapturedWorkspaceSnapshot<'_> {
    pub(super) fn entries(&self) -> &[RepositoryWorkspaceEntry] {
        &self.entries
    }
    pub(super) fn mode(&self) -> RepositoryWorkspaceMode {
        self.mode
    }

    pub(super) fn complete(
        self,
        transcript: &RepositoryWorkspaceTranscript,
        request: RepositoryWorkspaceRequest,
        result: RepositoryWorkspaceResult,
    ) -> Result<CreditedWorkspaceTranscript, PhysicalRootError> {
        self.complete_with_dependencies(transcript, request, result, None)
    }
    pub(super) fn complete_with_dependencies(
        self,
        transcript: &RepositoryWorkspaceTranscript,
        request: RepositoryWorkspaceRequest,
        result: RepositoryWorkspaceResult,
        dependencies: Option<Arc<NativeReadViewDependencies>>,
    ) -> Result<CreditedWorkspaceTranscript, PhysicalRootError> {
        self.complete_active(transcript, request, result, dependencies, &|| true)
    }
    pub(super) fn with_body_candidates(mut self, candidates: NativeBodyCandidates) -> Self {
        self.body_candidates = Some(candidates);
        self
    }
    pub(super) fn complete_active(
        self,
        transcript: &RepositoryWorkspaceTranscript,
        request: RepositoryWorkspaceRequest,
        result: RepositoryWorkspaceResult,
        dependencies: Option<Arc<NativeReadViewDependencies>>,
        active: &dyn Fn() -> bool,
    ) -> Result<CreditedWorkspaceTranscript, PhysicalRootError> {
        // Raw incoming/entry owners retire before candidate credits on EVERY
        // early return. Consumed producer arguments retire before this guard.
        struct Input<'a> {
            request: Option<RepositoryWorkspaceRequest>,
            result: Option<RepositoryWorkspaceResult>,
            snapshot: CapturedWorkspaceSnapshot<'a>,
        }
        let mut input = Input {
            request: Some(request),
            result: Some(result),
            snapshot: self,
        };
        let request = input.request.as_ref().unwrap();
        let result = input.result.as_ref().unwrap();
        let snapshot = &mut input.snapshot;
        if transcript.reservation().as_ref() != snapshot.prior.reservation().as_ref()
            || request.reservation().as_ref() != snapshot.prior.reservation().as_ref()
        {
            return Err(PhysicalRootError::Reservation);
        }
        if transcript.current().as_ref() != snapshot.prior
            || request.predecessor().as_ref() != snapshot.prior
        {
            return Err(PhysicalRootError::Protocol(
                slug_bzlmod_v2::RepositoryWorkspaceError::ForeignPredecessor,
            ));
        }
        let body_ledger = if let Some(candidates) = snapshot.body_candidates.as_mut() {
            candidates.ledger().check_request_origin(
                &snapshot.resources,
                &snapshot.scope,
                transcript.reservation(),
                active,
            )?;
            transcript.try_visit_completion_body_allocations(
                request,
                result,
                &snapshot.entries,
                &mut |body| candidates.retain(body, active).map(|_| ()),
            )?;
            Some(candidates.ledger().clone())
        } else {
            None
        };
        let storage = if body_ledger.is_some() {
            transcript.completion_body_covered_storage_partition(
                request,
                result,
                snapshot.mode,
                &snapshot.entries,
            )
        } else {
            transcript.completion_storage_partition(
                request,
                result,
                snapshot.mode,
                &snapshot.entries,
            )
        }
        .map_err(PhysicalRootError::Protocol)?;
        let forecast = match &dependencies {
            Some(deps) => {
                if !deps.covers_transcript(transcript) || !deps.covers_request(request) {
                    return Err(PhysicalRootError::Reservation);
                }
                storage.owned_bytes()
            }
            None => storage.total_bytes().map_err(PhysicalRootError::Protocol)?,
        };
        let permanent = snapshot.resources.reserve(
            add(forecast, std::mem::size_of::<CreditedWorkspaceTranscript>())?,
            0,
            0,
        )?;
        let entries = std::mem::take(&mut input.snapshot.entries);
        let completed = transcript
            .complete(
                input.request.take().unwrap(),
                input.result.take().unwrap(),
                input.snapshot.mode,
                entries,
            )
            .map_err(PhysicalRootError::Protocol)?;
        if let Some(ledger) = &body_ledger {
            // Completed transcript is a local declared AFTER the still-armed
            // input guard, so refusal drops all canonical raw owners first.
            ledger.commit_canonical(
                input.snapshot.body_candidates.as_mut().unwrap(),
                |visit| completed.try_visit_body_allocations(&mut |body| visit(body)),
                active,
            )?;
        }
        Ok(CreditedWorkspaceTranscript {
            transcript: completed,
            dependencies,
            body_ledger,
            credit: permanent,
        })
    }
}

#[derive(Clone, Copy)]
struct Limits {
    entries: usize,
    names: usize,
    bodies: u64,
    depth: usize,
}
impl Limits {
    fn for_mode(mode: RepositoryWorkspaceMode) -> Self {
        match mode {
            RepositoryWorkspaceMode::Archive => Self {
                entries: 131_072,
                names: 4 * 1024 * 1024,
                bodies: 1024 * 1024 * 1024,
                depth: 2048,
            },
            RepositoryWorkspaceMode::Process => Self {
                entries: 8192,
                names: 1024 * 1024,
                bodies: 64 * 1024 * 1024,
                depth: 64,
            },
        }
    }
}

enum Node {
    Directory,
    File(Option<Arc<[u8]>>),
    Link(Option<CompactString>),
}
struct Row {
    path: CompactString,
    stat: libc::stat,
    node: Node,
}

pub(super) fn capture<'prior>(
    builder: &RepositoryPhysicalRootBuilder,
    prior: &'prior RepositoryWorkspaceState,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<CapturedWorkspaceSnapshot<'prior>, PhysicalRootError> {
    let sources = SourceLinkSet::empty(&builder.resources, scope, active)?;
    capture_with_sources(builder, prior, sources, scope, active)
}
pub(super) fn capture_with_sources<'prior>(
    builder: &RepositoryPhysicalRootBuilder,
    prior: &'prior RepositoryWorkspaceState,
    sources: Arc<SourceLinkSet>,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<CapturedWorkspaceSnapshot<'prior>, PhysicalRootError> {
    capture_builder(builder, prior, sources, None, scope, active)
}
pub(super) fn capture_with_candidates<'prior>(
    builder: &RepositoryPhysicalRootBuilder,
    prior: &'prior RepositoryWorkspaceState,
    sources: Arc<SourceLinkSet>,
    candidates: &mut NativeBodyCandidates,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<CapturedWorkspaceSnapshot<'prior>, PhysicalRootError> {
    candidates
        .ledger()
        .check_origin(&builder.resources, scope, &builder.reservation, active)?;
    capture_builder(builder, prior, sources, Some(candidates), scope, active)
}
fn capture_builder<'prior>(
    builder: &RepositoryPhysicalRootBuilder,
    prior: &'prior RepositoryWorkspaceState,
    sources: Arc<SourceLinkSet>,
    candidates: Option<&mut NativeBodyCandidates>,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<CapturedWorkspaceSnapshot<'prior>, PhysicalRootError> {
    check(scope, active)?;
    if prior.reservation().as_ref() != builder.reservation.as_ref() {
        return Err(PhysicalRootError::Reservation);
    }
    if prior.mode() == RepositoryWorkspaceMode::Process
        && builder.mode != RepositoryWorkspaceMode::Process
    {
        return Err(PhysicalRootError::Mode);
    }
    let _scratch = builder.resources.reserve(IO_SCRATCH, 3, 0)?;
    if backing_identity(&builder.file, builder.mode)? != builder.identity {
        return Err(PhysicalRootError::RootIdentity);
    }
    let io = RootIo::new(&builder.file, io_limits(builder.mode, &builder.reservation))?;
    let captured = capture_at_with_sources(
        &io,
        &builder.reservation,
        builder.mode,
        prior,
        &builder.resources,
        scope,
        active,
        Limits::for_mode(builder.mode),
        Some(&sources),
        candidates,
    )?;
    if backing_identity(&builder.file, builder.mode)? != builder.identity {
        return Err(PhysicalRootError::RootIdentity);
    }
    check(scope, active)?;
    Ok(captured)
}

// RootIO-only seam for focused sampler tests. It is never a physical factory or
// production Host fallback. Caller has already reserved the native scratch/FDs.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn capture_at<'prior>(
    io: &RootIo<'_>,
    reservation: &RepositoryWorkspaceReservation,
    mode: RepositoryWorkspaceMode,
    prior: &'prior RepositoryWorkspaceState,
    resources: &Arc<RepositoryRootResources>,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
    limits: Limits,
) -> Result<CapturedWorkspaceSnapshot<'prior>, PhysicalRootError> {
    capture_at_with_sources(
        io,
        reservation,
        mode,
        prior,
        resources,
        scope,
        active,
        limits,
        None,
        None,
    )
}
#[allow(clippy::too_many_arguments)]
fn capture_at_with_sources<'prior>(
    io: &RootIo<'_>,
    reservation: &RepositoryWorkspaceReservation,
    mode: RepositoryWorkspaceMode,
    prior: &'prior RepositoryWorkspaceState,
    resources: &Arc<RepositoryRootResources>,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
    limits: Limits,
    sources: Option<&SourceLinkSet>,
    mut candidates: Option<&mut NativeBodyCandidates>,
) -> Result<CapturedWorkspaceSnapshot<'prior>, PhysicalRootError> {
    check(scope, active)?;
    if prior.reservation().as_ref() != reservation {
        return Err(PhysicalRootError::Reservation);
    }
    let mut capture = CapturedWorkspaceSnapshot {
        entries: Vec::new(),
        mode,
        prior,
        resources: resources.clone(),
        scope: scope.clone(),
        body_candidates: None,
        credit: resources.reserve(
            std::mem::size_of::<CapturedWorkspaceSnapshot>() + 1024,
            0,
            0,
        )?,
    };
    // Rows die before capture credit on every failure. Enumeration callbacks
    // retain only bounded metadata; body IO starts after their DIR has closed.
    let mut rows = Vec::new();
    let root = io.root_stat()?;
    grow::<Row>(&mut capture.credit, &mut rows, limits.entries)?;
    rows.push(Row {
        path: "".into(),
        stat: root,
        node: node(&root)?,
    });
    let mut names = 0usize;
    let mut bodies = 0u64;
    let mut cursor = 0;
    let mut path_scratch = [0u8; 4096];
    let mut link_scratch = [0u8; 4097];
    while cursor < rows.len() {
        check(scope, active)?;
        let index = cursor;
        cursor += 1;
        let length = rows[index].path.len();
        path_scratch[..length].copy_from_slice(rows[index].path.as_bytes());
        let text = std::str::from_utf8(&path_scratch[..length])
            .expect("only validated Unicode paths retained");
        let path = Path::new(text);
        let framed = rows[index].stat;
        if !seal::unchanged(&framed, &io.lstat(path)?) {
            return Err(PhysicalRootError::Snapshot);
        }
        match &rows[index].node {
            Node::Directory => {
                let running = || active() && scope.check().is_ok();
                let mut visitor_error = None;
                let mut child_scratch = [0u8; 4096];
                let listed = io.read_directory(path, &running, |name, stat| {
                    let result = (|| {
                        check(scope, active)?;
                        let slash = usize::from(!text.is_empty());
                        let child_len = add(add(text.len(), slash)?, name.as_bytes().len())?;
                        let absolute = add(
                            reservation.display_root().as_path().as_os_str().len(),
                            add(1, child_len)?,
                        )?;
                        if rows.len() == limits.entries || child_len > 4096 || absolute > 4096 {
                            return Err(PhysicalRootError::Snapshot);
                        }
                        names = add(names, child_len)?;
                        if names > limits.names {
                            return Err(PhysicalRootError::Snapshot);
                        }
                        child_scratch[..text.len()].copy_from_slice(text.as_bytes());
                        if slash != 0 {
                            child_scratch[text.len()] = b'/';
                        }
                        child_scratch[text.len() + slash..child_len]
                            .copy_from_slice(name.as_bytes());
                        let child = std::str::from_utf8(&child_scratch[..child_len])
                            .map_err(|_| PhysicalRootError::Snapshot)?;
                        if child.split('/').count() > limits.depth {
                            return Err(PhysicalRootError::Snapshot);
                        }
                        let node = node(stat)?;
                        grow::<Row>(&mut capture.credit, &mut rows, limits.entries)?;
                        // CompactString's inline capacity is conservative here;
                        // heap allocation is exact/bounded and checked afterward.
                        capture
                            .credit
                            .bytes
                            .try_grow(add(
                                child_len
                                    .checked_mul(2)
                                    .ok_or(PhysicalRootError::Resource)?,
                                64,
                            )?)
                            .map_err(|_| PhysicalRootError::Resource)?;
                        let path = CompactString::from(child);
                        if path.capacity() > child_len.saturating_mul(2).saturating_add(64) {
                            return Err(PhysicalRootError::Resource);
                        }
                        rows.push(Row {
                            path,
                            stat: *stat,
                            node,
                        });
                        Ok(())
                    })();
                    match result {
                        Ok(()) => Ok(()),
                        Err(error) => {
                            visitor_error = Some(error);
                            Err(io::Error::other("workspace capture directory refused"))
                        }
                    }
                });
                if let Some(error) = visitor_error {
                    return Err(error);
                }
                check(scope, active)?;
                listed?;
            }
            Node::File(_) => {
                let size =
                    u64::try_from(framed.st_size).map_err(|_| PhysicalRootError::Snapshot)?;
                bodies = bodies
                    .checked_add(size)
                    .filter(|sum| *sum <= limits.bodies)
                    .ok_or(PhysicalRootError::Snapshot)?;
                let body = capture_body_covered(
                    io,
                    path,
                    &framed,
                    prior,
                    &mut capture.credit,
                    candidates.as_deref_mut(),
                    scope,
                    active,
                )?;
                rows[index].node = Node::File(Some(body));
            }
            Node::Link(_) => {
                let target = io.read_link(path, &mut link_scratch)?;
                check(scope, active)?;
                let target =
                    std::str::from_utf8(target).map_err(|_| PhysicalRootError::Snapshot)?;
                match sources {
                    Some(sources) => {
                        sources.classify(reservation, text, target)?;
                    }
                    None => valid_link(reservation, text, target)?,
                }
                names = add(names, target.len())?;
                if names > limits.names {
                    return Err(PhysicalRootError::Snapshot);
                }
                capture
                    .credit
                    .bytes
                    .try_grow(add(
                        target
                            .len()
                            .checked_mul(2)
                            .ok_or(PhysicalRootError::Resource)?,
                        64,
                    )?)
                    .map_err(|_| PhysicalRootError::Resource)?;
                let retained = CompactString::from(target);
                if retained.capacity() > target.len().saturating_mul(2).saturating_add(64) {
                    return Err(PhysicalRootError::Resource);
                }
                rows[index].node = Node::Link(Some(retained));
            }
        }
        if !seal::unchanged(&framed, &io.lstat(path)?) {
            return Err(PhysicalRootError::Snapshot);
        }
    }
    // Recheck every discovered name after capture. Quiescence is still the
    // authority precondition; finite checks never prove child/writer death.
    for row in &rows {
        check(scope, active)?;
        if !seal::unchanged(&row.stat, &io.lstat(Path::new(row.path.as_str()))?) {
            return Err(PhysicalRootError::Snapshot);
        }
    }
    let bytes = rows
        .len()
        .checked_mul(std::mem::size_of::<RepositoryWorkspaceEntry>())
        .and_then(|n| n.checked_add(64))
        .ok_or(PhysicalRootError::Resource)?;
    capture
        .credit
        .bytes
        .try_grow(bytes)
        .map_err(|_| PhysicalRootError::Resource)?;
    capture
        .entries
        .try_reserve_exact(rows.len())
        .map_err(|_| PhysicalRootError::Resource)?;
    if capture.entries.capacity() > rows.len() {
        return Err(PhysicalRootError::Resource);
    }
    for row in rows {
        let node = match row.node {
            Node::Directory => RepositoryWorkspaceNode::Directory,
            Node::File(Some(body)) => RepositoryWorkspaceNode::File(body),
            Node::Link(Some(target)) => RepositoryWorkspaceNode::Link(target),
            _ => return Err(PhysicalRootError::Snapshot),
        };
        capture.entries.push(
            RepositoryWorkspaceEntry::new(row.path, (row.stat.st_mode & 0o7777) as u16, node)
                .map_err(PhysicalRootError::Protocol)?,
        );
    }
    capture
        .entries
        .sort_unstable_by(|a, b| a.path().cmp(b.path()));
    check(scope, active)?;
    Ok(capture)
}

fn grow<T>(
    credit: &mut resources::ResourceCredit,
    values: &mut Vec<T>,
    limit: usize,
) -> Result<(), PhysicalRootError> {
    if values.len() == values.capacity() {
        let capacity = values
            .capacity()
            .checked_mul(2)
            .ok_or(PhysicalRootError::Resource)?
            .max(4)
            .min(limit);
        if capacity <= values.len() {
            return Err(PhysicalRootError::Snapshot);
        }
        credit
            .bytes
            .try_grow(
                capacity
                    .checked_mul(std::mem::size_of::<T>())
                    .and_then(|n| n.checked_add(64))
                    .ok_or(PhysicalRootError::Resource)?,
            )
            .map_err(|_| PhysicalRootError::Resource)?;
        values
            .try_reserve_exact(capacity - values.len())
            .map_err(|_| PhysicalRootError::Resource)?;
        if values.capacity() > capacity {
            return Err(PhysicalRootError::Resource);
        }
    }
    Ok(())
}
fn node(stat: &libc::stat) -> Result<Node, PhysicalRootError> {
    match stat.st_mode & libc::S_IFMT {
        libc::S_IFDIR => Ok(Node::Directory),
        libc::S_IFREG if stat.st_nlink == 1 => Ok(Node::File(None)),
        libc::S_IFLNK => Ok(Node::Link(None)),
        _ => Err(PhysicalRootError::Snapshot),
    }
}
fn body_bytes(length: usize) -> Result<usize, PhysicalRootError> {
    let layout =
        std::alloc::Layout::array::<u8>(length).map_err(|_| PhysicalRootError::Resource)?;
    let (layout, _) = std::alloc::Layout::new::<[usize; 2]>()
        .extend(layout)
        .map_err(|_| PhysicalRootError::Resource)?;
    Ok(layout.pad_to_align().size())
}
fn capture_body(
    io: &RootIo<'_>,
    path: &Path,
    framed: &libc::stat,
    prior: &RepositoryWorkspaceState,
    credit: &mut resources::ResourceCredit,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<Arc<[u8]>, PhysicalRootError> {
    capture_body_covered(io, path, framed, prior, credit, None, scope, active)
}
fn capture_body_covered(
    io: &RootIo<'_>,
    path: &Path,
    framed: &libc::stat,
    prior: &RepositoryWorkspaceState,
    credit: &mut resources::ResourceCredit,
    mut candidates: Option<&mut NativeBodyCandidates>,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<Arc<[u8]>, PhysicalRootError> {
    let length = usize::try_from(framed.st_size).map_err(|_| PhysicalRootError::Snapshot)?;
    // Pure sampler retains its original full, conservative body forecast.
    if candidates.is_none() {
        credit
            .bytes
            .try_grow(body_bytes(length)?)
            .map_err(|_| PhysicalRootError::Resource)?;
    }
    let running = || active() && scope.check().is_ok();
    if let Some(RepositoryWorkspaceNode::File(old)) = prior
        .entry(path.to_str().ok_or(PhysicalRootError::Snapshot)?)
        .map(|e| e.node())
    {
        if old.len() == length {
            if body_recovery::observed_equal(io, path, framed, old, scope, active)? {
                if let Some(candidates) = candidates.as_deref_mut() {
                    candidates.retain(old, active)?;
                }
                return Ok(old.clone());
            }
        }
    }
    let pending = match candidates.as_deref_mut() {
        Some(candidates) => match candidates.reserve_new_for_capture(length, active)? {
            NativeBodyAdmission::New(pending) => Some(pending),
            NativeBodyAdmission::Refused(refusal) => {
                return body_recovery::capture_existing(
                    io, path, framed, candidates, refusal, scope, active,
                );
            }
        },
        None => None,
    };
    let mut body = Arc::<[u8]>::new_uninit_slice(length);
    let mut offset = 0usize;
    let captured = io.read_file(path, &running, |chunk| {
        let end = offset
            .checked_add(chunk.len())
            .ok_or_else(|| io::Error::other("workspace capture overflow"))?;
        if end > length {
            return Err(io::Error::other("workspace capture grew"));
        }
        let slots = Arc::get_mut(&mut body).expect("unpublished snapshot body is unique");
        for (slot, byte) in slots[offset..end].iter_mut().zip(chunk) {
            slot.write(*byte);
        }
        offset = end;
        Ok(())
    });
    check(scope, active)?;
    let captured = captured?;
    if offset != length || !seal::unchanged(framed, &captured.metadata) {
        return Err(PhysicalRootError::Snapshot);
    }
    // SAFETY: every exact admitted slot was initialized once; actual EOF and
    // FD/name identity were checked by RootIo before this view can escape.
    let body = unsafe { body.assume_init() };
    if let (Some(candidates), Some(pending)) = (candidates, pending) {
        Ok(candidates.finish(pending, body, active)?.body().clone())
    } else {
        Ok(body)
    }
}
fn valid_link(
    reservation: &RepositoryWorkspaceReservation,
    path: &str,
    target: &str,
) -> Result<(), PhysicalRootError> {
    if target.is_empty() || target.len() > 4096 || target.contains(['\0', '\\']) {
        return Err(PhysicalRootError::Snapshot);
    }
    let root = reservation
        .display_root()
        .as_path()
        .to_str()
        .ok_or(PhysicalRootError::Snapshot)?;
    let relative = if target.starts_with('/') {
        let tail = target
            .strip_prefix(root)
            .ok_or(PhysicalRootError::Snapshot)?;
        if tail.is_empty() {
            ""
        } else {
            tail.strip_prefix('/').ok_or(PhysicalRootError::Snapshot)?
        }
    } else {
        target
    };
    let mut depth = if target.starts_with('/') {
        0
    } else {
        path.rsplit_once('/')
            .map_or(0, |(parent, _)| parent.split('/').count())
    };
    for part in relative.split('/') {
        match part {
            "" | "." => {}
            ".." => depth = depth.checked_sub(1).ok_or(PhysicalRootError::Snapshot)?,
            _ => depth = depth.checked_add(1).ok_or(PhysicalRootError::Snapshot)?,
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "repository_workspace_snapshot/tests.rs"]
mod tests;
