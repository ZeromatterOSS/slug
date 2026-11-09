//! Actual FD facts for the shared bounded resolver. Every retained frame is
//! prepaid and includes full native identity/timestamps, not millisecond facts
//! alone. Neither a modeled route nor display text opens a filesystem object.
use std::io::Read;

use slug_bzlmod_v2::RepositoryWorkspaceReadExpectation;
use slug_bzlmod_v2::RepositoryWorkspaceResult;
use slug_workspace_v2::AnchoredResolvedPathKey;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::ResolvedPath;
use slug_workspace_v2::ResolvedPathState;

use super::body_coverage::NativeBodyCandidates;
use super::body_coverage::NativeBodyLease;
use super::*;
use crate::runtime::dice::source_service::SourceBufferReservation;
use crate::runtime::dice::source_service::SourceCounter;

pub(super) struct RouteRoots<'a> {
    pub(super) file: &'a File,
    pub(super) state: &'a RepositoryWorkspaceState,
    pub(super) sources: &'a SourceLinkSet,
    pub(super) resources: &'a Arc<RepositoryRootResources>,
    pub(super) scope: &'a Arc<SourceServiceScope>,
    pub(super) active: &'a dyn Fn() -> bool,
    #[cfg(test)]
    observer: Option<&'a dyn Fn(PathObservationNamespace)>,
}
struct Frame {
    demand: PathObservationDemand,
    result: PathObservationResult,
    stat: Option<libc::stat>,
}
pub(super) struct CapturedRoute {
    pub(super) resolved: ResolvedPath,
    frames: Vec<Frame>,
    _credit: ResourceCredit,
    _working: WorkingCredit,
}
/// Pure modeled paths are still owned native work. The callback bounds facts
/// before synthetic results; working peak is paid before resolver allocation.
pub(super) struct ModeledRoute {
    pub(super) resolved: ResolvedPath,
    _credit: ResourceCredit,
    _working: WorkingCredit,
}
struct WorkingCredit {
    paid: usize,
    credit: ResourceCredit,
}
impl WorkingCredit {
    fn new(resources: &Arc<RepositoryRootResources>) -> Result<Self, PhysicalRootError> {
        Ok(Self {
            paid: 0,
            credit: resources.reserve(0, 0, 0)?,
        })
    }
    fn check(
        &mut self,
        bound: usize,
        scope: &SourceServiceScope,
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        check(scope, active)?;
        if bound > self.paid {
            self.credit
                .bytes
                .try_grow(bound - self.paid)
                .map_err(|_| PhysicalRootError::Resource)?;
            self.paid = bound;
        }
        Ok(())
    }
}
fn model_error(error: slug_bzlmod_v2::RepositoryWorkspaceError) -> PhysicalRootError {
    match error {
        slug_bzlmod_v2::RepositoryWorkspaceError::RetentionLimit => PhysicalRootError::Resource,
        other => PhysicalRootError::Protocol(other),
    }
}
pub(super) fn model(
    state: &RepositoryWorkspaceState,
    relative: &str,
    resources: &Arc<RepositoryRootResources>,
    scope: &SourceServiceScope,
    active: &dyn Fn() -> bool,
) -> Result<ModeledRoute, PhysicalRootError> {
    check(scope, active)?;
    if add(
        add(
            state
                .reservation()
                .display_root()
                .as_path()
                .as_os_str()
                .len(),
            usize::from(!relative.is_empty()),
        )?,
        relative.len(),
    )? > 4096
    {
        return Err(PhysicalRootError::Snapshot);
    }
    let context = state
        .resolution_anchor_storage_bytes()
        .map_err(PhysicalRootError::Protocol)?;
    let mut working = WorkingCredit::new(resources)?;
    let mut credit = resources.reserve(
        add(
            IO_SCRATCH,
            context.checked_mul(4).ok_or(PhysicalRootError::Resource)?,
        )?,
        0,
        0,
    )?;
    let mut count = 0usize;
    let mut paths = 0usize;
    let resolved = state
        .resolve_path_with_work(
            relative,
            |demand| {
                check(scope, active)?;
                count = add(count, 1)?;
                let bytes = demand
                    .path()
                    .retained_storage_bytes()
                    .ok_or(PhysicalRootError::Resource)?;
                paths = add(paths, bytes)?;
                if count > 131_072 || paths > 4 * 1024 * 1024 {
                    return Err(PhysicalRootError::Resource);
                }
                credit
                    .bytes
                    .try_grow(add(
                        bytes.checked_mul(8).ok_or(PhysicalRootError::Resource)?,
                        if matches!(demand.operation(), PathObservationOperation::ReadLink) {
                            16 * 1024
                        } else {
                            1024
                        },
                    )?)
                    .map_err(|_| PhysicalRootError::Resource)?;
                Ok(())
            },
            |bound| working.check(bound, scope, active),
        )?
        .map_err(model_error)?;
    check(scope, active)?;
    Ok(ModeledRoute {
        resolved,
        _credit: credit,
        _working: working,
    })
}
pub(super) struct CapturedRead {
    body: RepositoryWorkspaceBody,
    _body_lease: Option<NativeBodyLease>,
    _native: ResourceCredit,
    _source: SourceBufferReservation,
}
impl CapturedRead {
    pub(super) fn result(&self) -> RepositoryWorkspaceResult {
        RepositoryWorkspaceResult::Read(self.body.clone())
    }
}
fn missing(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::NotFound || error.raw_os_error() == Some(libc::ENOTDIR)
}
fn stat(io: &RootIo<'_>, path: &std::path::Path) -> Result<Option<libc::stat>, PhysicalRootError> {
    match io.lstat(path) {
        Ok(value) => Ok(Some(value)),
        Err(error) if missing(&error) => Ok(None),
        Err(error) => Err(error.into()),
    }
}
fn same(a: &Option<libc::stat>, b: &Option<libc::stat>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => seal::unchanged(a, b),
        (None, None) => true,
        _ => false,
    }
}
impl<'a> RouteRoots<'a> {
    pub(super) fn new(
        file: &'a File,
        state: &'a RepositoryWorkspaceState,
        sources: &'a SourceLinkSet,
        resources: &'a Arc<RepositoryRootResources>,
        scope: &'a Arc<SourceServiceScope>,
        active: &'a dyn Fn() -> bool,
    ) -> Self {
        Self {
            file,
            state,
            sources,
            resources,
            scope,
            active,
            #[cfg(test)]
            observer: None,
        }
    }
    #[cfg(test)]
    pub(super) fn observe_for_test(
        mut self,
        observer: &'a dyn Fn(PathObservationNamespace),
    ) -> Self {
        self.observer = Some(observer);
        self
    }
    pub(super) fn exists(&self, relative: &str) -> Result<bool, PhysicalRootError> {
        let expected = model(
            self.state,
            relative,
            self.resources,
            self.scope,
            self.active,
        )?;
        let expected = &expected.resolved;
        let route = self.resolve(relative)?;
        if route.resolved.real_path() != expected.real_path()
            || route.resolved.content_origin() != expected.content_origin()
            || std::mem::discriminant(&route.resolved.state())
                != std::mem::discriminant(&expected.state())
        {
            return Err(PhysicalRootError::Snapshot);
        }
        Ok(matches!(
            route.resolved.state(),
            ResolvedPathState::Present(_)
        ))
    }
    pub(super) fn read(&self, relative: &str) -> Result<CapturedRead, PhysicalRootError> {
        self.read_inner(relative, None)
    }
    pub(super) fn read_with_body_candidates(
        &self,
        relative: &str,
        candidates: &mut NativeBodyCandidates,
    ) -> Result<CapturedRead, PhysicalRootError> {
        candidates.ledger().check_origin(
            self.resources,
            self.scope,
            self.state.reservation(),
            self.active,
        )?;
        self.read_inner(relative, Some(candidates))
    }
    fn read_inner(
        &self,
        relative: &str,
        mut candidates: Option<&mut NativeBodyCandidates>,
    ) -> Result<CapturedRead, PhysicalRootError> {
        check(self.scope, self.active)?;
        let model = model(
            self.state,
            relative,
            self.resources,
            self.scope,
            self.active,
        )?;
        let source_capture = model.resolved.content_origin().namespace()
            != PathObservationNamespace::Materialization(self.state.reservation().instance());
        if source_capture {
            self.scope.charge(SourceCounter::SourceAttempts, 1)?;
        }
        let expected = self
            .state
            .read_expectation_for_resolved(&model.resolved)
            .map_err(PhysicalRootError::Protocol)?;
        let body_bytes = match expected {
            RepositoryWorkspaceReadExpectation::Source(digest) => {
                usize::try_from(digest.size_bytes()).map_err(|_| PhysicalRootError::Resource)?
            }
            _ => 0,
        };
        let native = self.resources.reserve(
            add(
                body_bytes
                    .checked_mul(if candidates.is_some() { 1 } else { 2 })
                    .ok_or(PhysicalRootError::Resource)?,
                add(IO_SCRATCH, RepositoryWorkspaceBody::decode_scratch_bytes())?,
            )?,
            3,
            0,
        )?;
        let source = self.scope.reserve_buffer(add(
            body_bytes
                .checked_mul(2)
                .ok_or(PhysicalRootError::Resource)?,
            64 * 1024,
        )?)?;
        let route = self.resolve(relative)?;
        if route.resolved.real_path() != model.resolved.real_path()
            || route.resolved.content_origin() != model.resolved.content_origin()
            || !matches!(route.resolved.state(),ResolvedPathState::Present(stat) if stat.kind()==PathNodeKind::RegularFile)
        {
            return Err(PhysicalRootError::Snapshot);
        }
        let mut body_lease = None;
        let body = match expected {
            RepositoryWorkspaceReadExpectation::Owned(body) => {
                let relative = route
                    .resolved
                    .real_path()
                    .as_path()
                    .strip_prefix(self.state.reservation().display_root().as_path())
                    .map_err(|_| PhysicalRootError::Snapshot)?;
                let io = RootIo::new(
                    self.file,
                    io_limits(self.state.mode(), self.state.reservation()),
                )?;
                let mut offset = 0usize;
                let captured = io.read_file(
                    relative,
                    &|| (self.active)() && self.scope.check().is_ok(),
                    |chunk| {
                        check(self.scope, self.active)
                            .map_err(|_| io::Error::other("inactive source read"))?;
                        let end = offset
                            .checked_add(chunk.len())
                            .filter(|end| *end <= body.len())
                            .ok_or_else(|| io::Error::other("source read size changed"))?;
                        if !body.range_eq(offset, chunk) {
                            return Err(io::Error::other("source read bytes changed"));
                        }
                        offset = end;
                        Ok(())
                    },
                );
                check(self.scope, self.active)?;
                captured?;
                if offset != body.len() {
                    return Err(PhysicalRootError::Snapshot);
                }
                if let Some(candidates) = candidates.as_deref_mut() {
                    body_lease = Some(candidates.retain(body, self.active)?);
                }
                body.clone()
            }
            RepositoryWorkspaceReadExpectation::Source(digest) => {
                let target = self
                    .sources
                    .source(route.resolved.content_origin().namespace())?;
                let relative = route
                    .resolved
                    .real_path()
                    .as_path()
                    .strip_prefix(target.anchor().root().as_path())
                    .ok()
                    .and_then(|path| path.to_str())
                    .ok_or(PhysicalRootError::Snapshot)?;
                let mut reader = target.open_member(relative, self.scope, self.active)?;
                let size = usize::try_from(digest.size_bytes())
                    .map_err(|_| PhysicalRootError::Snapshot)?;
                if size > 2 * 1024 * 1024 {
                    return Err(PhysicalRootError::Snapshot);
                }
                // One actual Vec scratch owner plus separately prepaid Arc.
                // Source live-buffer accounting remains the original 2x bound.
                let pending = match candidates.as_deref_mut() {
                    Some(candidates) => Some(candidates.reserve_new(size, self.active)?),
                    None => None,
                };
                let mut bytes = Vec::new();
                bytes
                    .try_reserve_exact(size)
                    .map_err(|_| PhysicalRootError::Resource)?;
                if bytes.capacity() > size {
                    return Err(PhysicalRootError::Resource);
                }
                bytes.resize(size, 0);
                for chunk in bytes.chunks_mut(64 * 1024) {
                    let mut offset = 0;
                    while offset < chunk.len() {
                        check(self.scope, self.active)?;
                        let count = reader.read(&mut chunk[offset..])?;
                        if count == 0 {
                            return Err(PhysicalRootError::Snapshot);
                        }
                        self.scope
                            .charge(SourceCounter::DecodedBytes, count as u64)?;
                        offset += count;
                    }
                }
                let mut probe = [0];
                if reader.read(&mut probe)? != 0 {
                    return Err(PhysicalRootError::Snapshot);
                }
                expected
                    .validate(&bytes)
                    .map_err(PhysicalRootError::Protocol)?;
                let body =
                    RepositoryWorkspaceBody::from(Arc::<[u8]>::from(bytes.into_boxed_slice()));
                if let (Some(candidates), Some(pending)) = (candidates.as_deref_mut(), pending) {
                    body_lease = Some(candidates.finish(pending, body, self.active)?);
                    body_lease.as_ref().unwrap().body().clone()
                } else {
                    body
                }
            }
        };
        route.validate(self)?;
        check(self.scope, self.active)?;
        Ok(CapturedRead {
            body,
            _body_lease: body_lease,
            _native: native,
            _source: source,
        })
    }
    fn fact(
        &self,
        demand: &PathObservationDemand,
    ) -> Result<(PathObservationResult, Option<libc::stat>), PhysicalRootError> {
        check(self.scope, self.active)?;
        #[cfg(test)]
        if let Some(observer) = self.observer {
            observer(demand.namespace());
        }
        if !matches!(
            demand.operation(),
            PathObservationOperation::Lstat | PathObservationOperation::ReadLink
        ) {
            return Err(PhysicalRootError::Snapshot);
        }
        let _scratch = self.resources.reserve(
            add(
                IO_SCRATCH,
                self.state
                    .resolution_anchor_storage_bytes()
                    .map_err(PhysicalRootError::Protocol)?
                    .checked_mul(4)
                    .ok_or(PhysicalRootError::Resource)?,
            )?,
            3,
            0,
        )?;
        let anchor = self
            .state
            .resolution_anchor()
            .map_err(PhysicalRootError::Protocol)?;
        let (file, limits, bound, source_state) = if demand.namespace() == anchor.namespace() {
            (
                self.file,
                io_limits(self.state.mode(), self.state.reservation()),
                anchor,
                None,
            )
        } else {
            let source = self.sources.source(demand.namespace())?;
            source.check_origin(self.scope, self.active)?;
            source.validate_backing()?;
            if !anchor.source_anchors().iter().any(|terminal| {
                terminal.namespace() == source.anchor().namespace()
                    && terminal.root() == source.anchor().root()
            }) {
                return Err(PhysicalRootError::Reservation);
            }
            (
                &source.file,
                source::limits(source.reservation())?,
                source.anchor().clone(),
                Some(source.state()),
            )
        };
        let relative = demand
            .path()
            .as_path()
            .strip_prefix(bound.root().as_path())
            .map_err(|_| PhysicalRootError::Reservation)?;
        let io = RootIo::new(file, limits)?;
        let before = stat(&io, relative)?;
        let relative_text = relative.to_str().ok_or(PhysicalRootError::Snapshot)?;
        let expected = if let Some(state) = source_state {
            state.entry(relative_text).map(|entry| match entry.node() {
                slug_bzlmod_v2::RepositorySourceSnapshotNode::Directory => {
                    (libc::S_IFDIR, 0, entry.mode(), None)
                }
                slug_bzlmod_v2::RepositorySourceSnapshotNode::File(digest) => {
                    (libc::S_IFREG, digest.size_bytes(), entry.mode(), None)
                }
                slug_bzlmod_v2::RepositorySourceSnapshotNode::Link(target) => (
                    libc::S_IFLNK,
                    target.len() as u64,
                    entry.mode(),
                    Some(target.as_str()),
                ),
            })
        } else {
            self.state
                .entry(relative_text)
                .map(|entry| match entry.node() {
                    slug_bzlmod_v2::RepositoryWorkspaceNode::Directory => {
                        (libc::S_IFDIR, 0, entry.mode(), None)
                    }
                    slug_bzlmod_v2::RepositoryWorkspaceNode::File(body) => {
                        (libc::S_IFREG, body.len() as u64, entry.mode(), None)
                    }
                    slug_bzlmod_v2::RepositoryWorkspaceNode::Link(target) => (
                        libc::S_IFLNK,
                        target.len() as u64,
                        entry.mode(),
                        Some(target.as_str()),
                    ),
                })
        };
        match (expected, &before) {
            (None, None) => (),
            (Some((kind, size, mode, _)), Some(actual))
                if actual.st_mode & libc::S_IFMT == kind
                    && actual.st_mode & 0o7777 == mode as u32
                    && (kind == libc::S_IFDIR
                        || actual.st_size >= 0 && actual.st_size as u64 == size)
                    && (kind != libc::S_IFREG || actual.st_nlink == 1) =>
            {
                ()
            }
            _ => return Err(PhysicalRootError::Snapshot),
        }
        let epoch = super::super::path_observation::rooted::observe_rooted(
            &io,
            &bound,
            std::slice::from_ref(demand),
            self.scope,
            self.active,
        )
        .map_err(PhysicalRootError::Observation)?;
        let result = epoch
            .get(demand)
            .ok_or(PhysicalRootError::Snapshot)?
            .as_ref()
            .clone();
        if let PathObservationResult::ReadLink(slug_workspace_v2::PathOperationResult::Present(
            actual,
        )) = &result
        {
            if expected
                .and_then(|(_, _, _, target)| target)
                .is_none_or(|target| actual.as_os_str() != std::ffi::OsStr::new(target))
            {
                return Err(PhysicalRootError::Snapshot);
            }
        }
        if !same(&before, &stat(&io, relative)?) {
            return Err(PhysicalRootError::Snapshot);
        }
        check(self.scope, self.active)?;
        Ok((result, before))
    }
    pub(super) fn resolve(&self, relative: &str) -> Result<CapturedRoute, PhysicalRootError> {
        if add(
            add(
                self.state
                    .reservation()
                    .display_root()
                    .as_path()
                    .as_os_str()
                    .len(),
                usize::from(!relative.is_empty()),
            )?,
            relative.len(),
        )? > 4096
        {
            return Err(PhysicalRootError::Snapshot);
        }
        if !self.sources.dependencies().covers_state(self.state) {
            return Err(PhysicalRootError::Reservation);
        }
        let context = self
            .state
            .resolution_anchor_storage_bytes()
            .map_err(PhysicalRootError::Protocol)?;
        let mut working = WorkingCredit::new(self.resources)?;
        let mut credit = self.resources.reserve(
            add(
                IO_SCRATCH,
                context.checked_mul(4).ok_or(PhysicalRootError::Resource)?,
            )?,
            0,
            0,
        )?;
        // Anchor context and shared resolver working arrays are prepaid before
        // constructing any paths or asking for native facts.
        let logical = NormalizedAbsolutePath::new(
            self.state
                .reservation()
                .display_root()
                .as_path()
                .join(relative),
        )
        .map_err(|_| PhysicalRootError::Snapshot)?;
        let anchor = self
            .state
            .resolution_anchor()
            .map_err(PhysicalRootError::Protocol)?;
        let key = AnchoredResolvedPathKey::new(anchor, logical.as_path())
            .map_err(|_| PhysicalRootError::Snapshot)?;
        let mut frames = Vec::new();
        let mut paths = 0usize;
        let resolved = key
            .resolve_sync_with_work(
                |demand| {
                    check(self.scope, self.active)?;
                    paths = add(
                        paths,
                        demand
                            .path()
                            .retained_storage_bytes()
                            .ok_or(PhysicalRootError::Resource)?,
                    )?;
                    if frames.len() == 131_072 || paths > 4 * 1024 * 1024 {
                        return Err(PhysicalRootError::Resource);
                    }
                    // Includes resolver-owned route copies, result allocation overlap,
                    // vector growth, and a possible retained literal link target.
                    let path = demand
                        .path()
                        .retained_storage_bytes()
                        .ok_or(PhysicalRootError::Resource)?;
                    let extra = add(
                        path.checked_mul(8).ok_or(PhysicalRootError::Resource)?,
                        if matches!(demand.operation(), PathObservationOperation::ReadLink) {
                            16 * 1024
                        } else {
                            1024
                        },
                    )?;
                    credit
                        .bytes
                        .try_grow(extra)
                        .map_err(|_| PhysicalRootError::Resource)?;
                    if frames.len() == frames.capacity() {
                        let amount = frames.capacity().max(8).min(131_072 - frames.len());
                        credit
                            .bytes
                            .try_grow(
                                amount
                                    .checked_mul(2 * std::mem::size_of::<Frame>())
                                    .ok_or(PhysicalRootError::Resource)?,
                            )
                            .map_err(|_| PhysicalRootError::Resource)?;
                        frames
                            .try_reserve_exact(amount)
                            .map_err(|_| PhysicalRootError::Resource)?;
                    }
                    let (result, stat) = self.fact(demand)?;
                    frames.push(Frame {
                        demand: demand.clone(),
                        result: result.clone(),
                        stat,
                    });
                    Ok(result)
                },
                |bound| working.check(bound, self.scope, self.active),
            )?
            .map_err(|error| match error {
                slug_workspace_v2::AnchoredPathResolutionError::WorkingStorageRefused => {
                    PhysicalRootError::Resource
                }
                _ => PhysicalRootError::Snapshot,
            })?;
        let route = CapturedRoute {
            resolved,
            frames,
            _credit: credit,
            _working: working,
        };
        route.validate(self)?;
        Ok(route)
    }
}
impl CapturedRoute {
    pub(super) fn validate(&self, roots: &RouteRoots<'_>) -> Result<(), PhysicalRootError> {
        for frame in &self.frames {
            let (result, stat) = roots.fact(&frame.demand)?;
            if result != frame.result || !same(&stat, &frame.stat) {
                return Err(PhysicalRootError::Snapshot);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use slug_bzlmod_v2::GeneratedRepositoryArchiveEffect;
    use slug_bzlmod_v2::GeneratedRepositoryArchiveFormat;
    use slug_bzlmod_v2::RepositoryWorkspaceEntry as Entry;
    use slug_bzlmod_v2::RepositoryWorkspaceNode as Node;
    use slug_bzlmod_v2::RepositoryWorkspaceOperation as Op;
    use slug_bzlmod_v2::RepositoryWorkspaceTranscript;

    use super::*;

    fn chain() -> Arc<RepositoryWorkspaceState> {
        // Pure immutable metadata only, not an archive/native completion proof.
        let reservation = super::super::native_tests::reservation(
            std::path::Path::new("/modeled-route/root"),
            3319,
        );
        let mut entries = vec![Entry::new("".into(), 0o755, Node::Directory).unwrap()];
        let mut parent = String::new();
        for _ in 0..1400 {
            if !parent.is_empty() {
                parent.push('/');
            }
            parent.push('a');
            entries.push(Entry::new(parent.as_str().into(), 0o755, Node::Directory).unwrap());
        }
        for index in 0..39 {
            let name = if index == 0 {
                "link".to_owned()
            } else {
                format!("{parent}/link{index:02}")
            };
            let target = if index == 38 {
                format!("{parent}/body")
            } else {
                format!("{parent}/link{:02}", index + 1)
            };
            entries.push(
                Entry::new(
                    name.into(),
                    0o777,
                    Node::Link(
                        format!(
                            "{}/{target}",
                            reservation.display_root().as_path().display()
                        )
                        .into(),
                    ),
                )
                .unwrap(),
            );
        }
        entries.push(
            Entry::new(
                format!("{parent}/body").into(),
                0o644,
                Node::File(Arc::<[u8]>::from([]).into()),
            )
            .unwrap(),
        );
        entries.sort_by(|a, b| a.path().cmp(b.path()));
        let transcript = RepositoryWorkspaceTranscript::new(reservation, 0o755).unwrap();
        let operation = Op::Archive(
            GeneratedRepositoryArchiveEffect::new(
                Arc::from(["https://example.invalid/model.tar.gz".into()]),
                Some([0; 32]),
                "".into(),
                GeneratedRepositoryArchiveFormat::TarGz,
                "modeled".into(),
                "".into(),
            )
            .unwrap(),
        );
        transcript
            .complete(
                transcript.request(operation).unwrap(),
                RepositoryWorkspaceResult::Archive([0; 32]),
                RepositoryWorkspaceMode::Archive,
                entries,
            )
            .unwrap()
            .current()
            .clone()
    }
    #[test]
    fn modeled_routes_bound_path_work_refund_and_check_scope_before_native_io() {
        let state = chain();
        let resources = RepositoryRootResources::new(
            RepositoryRootResourceLimits::new(128 * 1024 * 1024, 16, 4 * ARCHIVE_BYTES).unwrap(),
        )
        .unwrap();
        let scope = super::super::tests::scope();
        let baseline = resources.snapshot().retained_bytes;
        assert!(matches!(
            model(&state, "link", &resources, &scope, &|| true),
            Err(PhysicalRootError::Resource)
        ));
        assert_eq!(resources.snapshot().native_fds, 0);
        assert_eq!(resources.snapshot().retained_bytes, baseline);
        let polls = std::cell::Cell::new(0);
        let active = || {
            let n = polls.get();
            polls.set(n + 1);
            n < 3
        };
        assert!(matches!(
            model(&state, "link", &resources, &scope, &active),
            Err(PhysicalRootError::Inactive)
        ));
        assert!(polls.get() > 1);
        assert_eq!(resources.snapshot().native_fds, 0);
        assert_eq!(resources.snapshot().retained_bytes, baseline);
        let tiny = RepositoryRootResources::new(
            RepositoryRootResourceLimits::new(64 * 1024, 1, 1).unwrap(),
        )
        .unwrap();
        let before = tiny.snapshot();
        assert!(matches!(
            model(&state, "link", &tiny, &scope, &|| true),
            Err(PhysicalRootError::Resource)
        ));
        assert_eq!(tiny.snapshot().native_fds, 0);
        assert_eq!(tiny.snapshot().retained_bytes, before.retained_bytes);
        scope.cancel();
        assert!(matches!(
            model(&state, "link", &resources, &scope, &|| true),
            Err(PhysicalRootError::SourceService(
                SourceServiceError::Cancelled
            ))
        ));
        assert_eq!(resources.snapshot().native_fds, 0);
        assert_eq!(resources.snapshot().retained_bytes, baseline);
        let work = std::time::Instant::now() + std::time::Duration::from_millis(1);
        let expired = SourceServiceScope::new_for_test(
            state.reservation().workspace().clone(),
            work,
            work + std::time::Duration::from_secs(1),
            crate::runtime::dice::source_service::SourceServiceLimits::default(),
        )
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(matches!(
            model(&state, "link", &resources, &expired, &|| true),
            Err(PhysicalRootError::SourceService(
                SourceServiceError::WorkDeadline
            ))
        ));
        assert_eq!(resources.snapshot().native_fds, 0);
        assert_eq!(resources.snapshot().retained_bytes, baseline);
    }
}
