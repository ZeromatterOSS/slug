//! A source reader owns one readable FD and the exact sealed root. No FD/file,
//! mutation or mount capability is exposed to its consumer.
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::os::unix::fs::FileExt;
use std::path::Path;

use sha2::Digest;
use sha2::Sha256;
use slug_bzlmod_v2::RepositoryWorkspaceNode;
use slug_workspace_v2::FileContentDigest;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::ResolvedPathContentOrigin;
use slug_workspace_v2::ResolvedPathState;

use super::*;

pub(in crate::runtime) struct RepositoryRootReader {
    // Handle and retained root/body owners drop before their credit.
    file: Option<File>,
    route: Option<RoutedRead>,
    root: Arc<RepositoryPhysicalRoot>,
    scope: Arc<SourceServiceScope>,
    entry: usize,
    framed: libc::stat,
    offset: u64,
    _credit: resources::ResourceCredit,
}
struct RoutedRead {
    source: Option<source::RepositorySourceReader>,
    frames: routed::CapturedRoute,
    sources: Arc<SourceLinkSet>,
}

impl std::fmt::Debug for RepositoryRootReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RepositoryRootReader")
            .field("entry", &self.entry)
            .field("offset", &self.offset)
            .finish_non_exhaustive()
    }
}

pub(super) fn open(
    root: &Arc<RepositoryPhysicalRoot>,
    relative: &str,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<RepositoryRootReader, PhysicalRootError> {
    root.check_origin(scope, active)?;
    let index = root
        .state
        .entries()
        .binary_search_by(|entry| entry.path().cmp(relative))
        .map_err(|_| PhysicalRootError::Snapshot)?;
    let entry = &root.state.entries()[index];
    let RepositoryWorkspaceNode::File(body) = entry.node() else {
        return Err(PhysicalRootError::Snapshot);
    };
    let credit = root
        .resources
        .reserve(std::mem::size_of::<RepositoryRootReader>(), 1, 0)?;
    let _scratch = root.resources.reserve(
        add(IO_SCRATCH, RepositoryWorkspaceBody::decode_scratch_bytes())?,
        3,
        0,
    )?;
    root.validate_backing()?;
    let io = RootIo::new(&root.file, io_limits(root.mode, &root.reservation))?;
    let before = io.lstat(Path::new(relative))?;
    if before.st_mode & libc::S_IFMT != libc::S_IFREG
        || before.st_nlink != 1
        || before.st_size < 0
        || before.st_size as u64 != body.len() as u64
        || (before.st_mode & 0o7777) as u16 != entry.mode()
    {
        return Err(PhysicalRootError::Snapshot);
    }
    let file = io.open_read_regular(Path::new(relative))?;
    let opened = nix::sys::stat::fstat(&file).map_err(io::Error::from)?;
    if !seal::unchanged(&before, &opened)
        || !seal::unchanged(&before, &io.lstat(Path::new(relative))?)
    {
        return Err(PhysicalRootError::Snapshot);
    }
    root.validate_backing()?;
    root.check_origin(scope, active)?;
    Ok(RepositoryRootReader {
        file: Some(file),
        route: None,
        root: root.clone(),
        scope: scope.clone(),
        entry: index,
        framed: before,
        offset: 0,
        _credit: credit,
    })
}

impl RepositoryRootReader {
    fn validate(&self) -> Result<(), PhysicalRootError> {
        self.scope.check()?;
        self.root.validate_backing()?;
        if let Some(route) = &self.route {
            route.frames.validate(&routed::RouteRoots::new(
                &self.root.file,
                &self.root.state,
                &route.sources,
                &self.root.resources,
                &self.scope,
                &|| true,
            ))?;
            if route.source.is_some() {
                return Ok(());
            }
        }
        let opened = nix::sys::stat::fstat(self.file.as_ref().ok_or(PhysicalRootError::Snapshot)?)
            .map_err(io::Error::from)?;
        let io = RootIo::new(
            &self.root.file,
            io_limits(self.root.mode, &self.root.reservation),
        )?;
        let named = io.lstat(Path::new(self.root.state.entries()[self.entry].path()))?;
        if !seal::unchanged(&self.framed, &opened) || !seal::unchanged(&self.framed, &named) {
            return Err(PhysicalRootError::Snapshot);
        }
        self.scope.check()?;
        Ok(())
    }
}

fn read_error(error: PhysicalRootError) -> io::Error {
    let kind = match &error {
        // Cancellation/deadline/resource refusals are sticky. Standard Read
        // helpers automatically retry Interrupted and would otherwise spin.
        PhysicalRootError::SourceService(_) | PhysicalRootError::Inactive => io::ErrorKind::Other,
        _ => io::ErrorKind::InvalidData,
    };
    io::Error::new(
        kind,
        format!("sealed repository source validation failed: {error:?}"),
    )
}

impl Read for RepositoryRootReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        let _scratch = self
            .root
            .resources
            .reserve(
                add(IO_SCRATCH, RepositoryWorkspaceBody::decode_scratch_bytes())
                    .map_err(read_error)?,
                3,
                0,
            )
            .map_err(read_error)?;
        self.validate().map_err(read_error)?;
        if self
            .route
            .as_ref()
            .is_some_and(|route| route.source.is_some())
        {
            let count = self
                .route
                .as_mut()
                .unwrap()
                .source
                .as_mut()
                .unwrap()
                .read(out)?;
            self.validate().map_err(read_error)?;
            return Ok(count);
        }
        let RepositoryWorkspaceNode::File(body) = self.root.state.entries()[self.entry].node()
        else {
            return Err(read_error(PhysicalRootError::Snapshot));
        };
        let available = (body.len() as u64).saturating_sub(self.offset);
        if available == 0 {
            let mut probe = [0u8; 1];
            if self
                .file
                .as_ref()
                .unwrap()
                .read_at(&mut probe, self.offset)?
                != 0
            {
                return Err(read_error(PhysicalRootError::Snapshot));
            }
            self.validate().map_err(read_error)?;
            return Ok(0);
        }
        let requested = out
            .len()
            .min(64 * 1024)
            .min(available.try_into().unwrap_or(usize::MAX));
        let count = self
            .file
            .as_ref()
            .unwrap()
            .read_at(&mut out[..requested], self.offset)?;
        let start =
            usize::try_from(self.offset).map_err(|_| read_error(PhysicalRootError::Snapshot))?;
        if count == 0 || !body.range_eq(start, &out[..count]) {
            return Err(read_error(PhysicalRootError::Snapshot));
        }
        self.validate().map_err(read_error)?;
        self.offset = self
            .offset
            .checked_add(count as u64)
            .ok_or_else(|| read_error(PhysicalRootError::Snapshot))?;
        Ok(count)
    }
}

impl RepositoryPhysicalRoot {
    /// Allocation identity is a fast path, never a foreign-state substitute.
    /// No SourceScope/IO check is added to existing metadata-only callers.
    pub(in crate::runtime) fn matches_state(
        &self,
        state: &Arc<slug_bzlmod_v2::RepositoryWorkspaceState>,
    ) -> Result<bool, PhysicalRootError> {
        if Arc::ptr_eq(&self.state, state) {
            return Ok(true);
        }
        let _working =
            self.resources
                .reserve(2 * RepositoryWorkspaceBody::decode_scratch_bytes(), 0, 0)?;
        Ok(self.state.as_ref() == state.as_ref())
    }
    /// Pure direct-member digest check with working storage paid by this root.
    /// It deliberately adds no SourceScope/deadline/FD/route activity check.
    pub(in crate::runtime) fn verify_direct_body_digest(
        &self,
        relative: &str,
        expected: FileContentDigest,
    ) -> Result<(), PhysicalRootError> {
        let _working =
            self.resources
                .reserve(RepositoryWorkspaceBody::decode_scratch_bytes(), 0, 0)?;
        let Some(RepositoryWorkspaceNode::File(body)) =
            self.state.entry(relative).map(|e| e.node())
        else {
            return Err(PhysicalRootError::Snapshot);
        };
        if body.len() as u64 != expected.size_bytes() {
            return Err(PhysicalRootError::Snapshot);
        }
        let mut hash = Sha256::new();
        body.for_each_chunk(&mut |chunk| hash.update(chunk));
        let actual: [u8; 32] = hash.finalize().into();
        if &actual != expected.sha256() {
            return Err(PhysicalRootError::Snapshot);
        }
        Ok(())
    }
    pub(in crate::runtime) fn verify_routed_source_metadata(
        &self,
        logical_relative: &str,
        origin: &ResolvedPathContentOrigin,
        real_path: &NormalizedAbsolutePath,
        digest: FileContentDigest,
    ) -> Result<(), PhysicalRootError> {
        self.verify_routed_source(
            logical_relative,
            origin,
            real_path,
            digest,
            &self.origin_scope,
        )
    }
    /// Pure sealed-membership verification. Logical ownership remains this
    /// General root even when the immutable terminal content is a Source.
    pub(in crate::runtime) fn verify_routed_source(
        &self,
        logical_relative: &str,
        origin: &ResolvedPathContentOrigin,
        real_path: &NormalizedAbsolutePath,
        digest: FileContentDigest,
        scope: &Arc<SourceServiceScope>,
    ) -> Result<(), PhysicalRootError> {
        self.check_origin(scope, &|| true)?;
        let _scratch = self.resources.reserve(
            add(
                add(IO_SCRATCH, RepositoryWorkspaceBody::decode_scratch_bytes())?,
                self.state
                    .source_bindings()
                    .len()
                    .checked_mul(4 * (8192 + 256))
                    .ok_or(PhysicalRootError::Resource)?,
            )?,
            0,
            0,
        )?;
        if !self.source_dependencies.covers_state(&self.state) {
            return Err(PhysicalRootError::Reservation);
        }
        let modeled = routed::model(
            &self.state,
            logical_relative,
            &self.resources,
            scope,
            &|| true,
        )?;
        let resolved = &modeled.resolved;
        if resolved.content_origin() != origin
            || resolved.real_path() != real_path
            || !matches!(resolved.state(),ResolvedPathState::Present(stat) if stat.kind()==PathNodeKind::RegularFile)
        {
            return Err(PhysicalRootError::Snapshot);
        }
        let expected = if origin.namespace() == self.anchor.namespace() {
            let relative = real_path
                .as_path()
                .strip_prefix(self.anchor.root().as_path())
                .ok()
                .and_then(|path| path.to_str())
                .ok_or(PhysicalRootError::Snapshot)?;
            let Some(RepositoryWorkspaceNode::File(body)) =
                self.state.entry(relative).map(|entry| entry.node())
            else {
                return Err(PhysicalRootError::Snapshot);
            };
            let mut hash = Sha256::new();
            body.try_for_each_chunk(&mut |chunk| {
                self.check_origin(scope, &|| true)?;
                hash.update(chunk);
                Ok::<_, PhysicalRootError>(())
            })?;
            FileContentDigest::new(hash.finalize().into(), body.len() as u64)
                .ok_or(PhysicalRootError::Snapshot)?
        } else {
            let owner = self
                .source_dependencies
                .owners()
                .iter()
                .filter_map(|owner| owner.source_root())
                .find(|root| root.anchor().namespace() == origin.namespace())
                .ok_or(PhysicalRootError::Reservation)?;
            owner.check_origin(scope, &|| true)?;
            if origin.anchor() != Some(owner.anchor()) {
                return Err(PhysicalRootError::Snapshot);
            }
            let relative = real_path
                .as_path()
                .strip_prefix(owner.anchor().root().as_path())
                .ok()
                .and_then(|path| path.to_str())
                .ok_or(PhysicalRootError::Snapshot)?;
            let Some(slug_bzlmod_v2::RepositorySourceSnapshotNode::File(expected)) =
                owner.state().entry(relative).map(|entry| entry.node())
            else {
                return Err(PhysicalRootError::Snapshot);
            };
            *expected
        };
        if digest != expected {
            return Err(PhysicalRootError::Snapshot);
        }
        self.check_origin(scope, &|| true)
    }
    pub(in crate::runtime) fn open_routed_member(
        self: &Arc<Self>,
        logical_relative: &str,
        origin: &ResolvedPathContentOrigin,
        real_path: &NormalizedAbsolutePath,
        digest: FileContentDigest,
        scope: &Arc<SourceServiceScope>,
    ) -> Result<RepositoryRootReader, PhysicalRootError> {
        self.verify_routed_source(logical_relative, origin, real_path, digest, scope)?;
        let sources = SourceLinkSet::new(
            self.source_dependencies.owners(),
            &self.resources,
            scope,
            &|| true,
        )?;
        self.validate_backing()?;
        let frames = routed::RouteRoots::new(
            &self.file,
            &self.state,
            &sources,
            &self.resources,
            scope,
            &|| true,
        )
        .resolve(logical_relative)?;
        if frames.resolved.content_origin() != origin || frames.resolved.real_path() != real_path {
            return Err(PhysicalRootError::Snapshot);
        }
        let mut reader = if origin.namespace() == self.anchor.namespace() {
            let relative = real_path
                .as_path()
                .strip_prefix(self.anchor.root().as_path())
                .ok()
                .and_then(|path| path.to_str())
                .ok_or(PhysicalRootError::Snapshot)?;
            open(self, relative, scope, &|| true)?
        } else {
            let credit = self.resources.reserve(
                std::mem::size_of::<RepositoryRootReader>() + std::mem::size_of::<RoutedRead>(),
                0,
                0,
            )?;
            let target = sources.source(origin.namespace())?;
            let relative = real_path
                .as_path()
                .strip_prefix(target.anchor().root().as_path())
                .ok()
                .and_then(|path| path.to_str())
                .ok_or(PhysicalRootError::Snapshot)?;
            let source = target.open_member(relative, scope, &|| true)?;
            let reader = RepositoryRootReader {
                file: None,
                route: Some(RoutedRead {
                    source: Some(source),
                    frames,
                    sources,
                }),
                root: self.clone(),
                scope: scope.clone(),
                entry: 0,
                framed: unsafe { std::mem::zeroed() },
                offset: 0,
                _credit: credit,
            };
            reader.validate()?;
            return Ok(reader);
        };
        reader.route = Some(RoutedRead {
            source: None,
            frames,
            sources,
        });
        reader.validate()?;
        Ok(reader)
    }
}
impl Seek for RepositoryRootReader {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let _scratch = self
            .root
            .resources
            .reserve(
                add(IO_SCRATCH, RepositoryWorkspaceBody::decode_scratch_bytes())
                    .map_err(read_error)?,
                3,
                0,
            )
            .map_err(read_error)?;
        self.validate().map_err(read_error)?;
        if self
            .route
            .as_ref()
            .is_some_and(|route| route.source.is_some())
        {
            let position = self
                .route
                .as_mut()
                .unwrap()
                .source
                .as_mut()
                .unwrap()
                .seek(from)?;
            self.validate().map_err(read_error)?;
            return Ok(position);
        }
        let length = self.framed.st_size as u64;
        let position = match from {
            SeekFrom::Start(position) => position as i128,
            SeekFrom::Current(delta) => self.offset as i128 + delta as i128,
            SeekFrom::End(delta) => length as i128 + delta as i128,
        };
        if position < 0 || position > i64::MAX as i128 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid sealed source seek",
            ));
        }
        self.offset = position as u64;
        Ok(self.offset)
    }
}
