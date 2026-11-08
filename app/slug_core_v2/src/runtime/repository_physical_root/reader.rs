//! A source reader owns one readable FD and the exact sealed root. No FD/file,
//! mutation or mount capability is exposed to its consumer.
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::os::unix::fs::FileExt;
use std::path::Path;

use slug_bzlmod_v2::RepositoryWorkspaceNode;

use super::*;

pub(in crate::runtime) struct RepositoryRootReader {
    // Handle and retained root/body owners drop before their credit.
    file: File,
    root: Arc<RepositoryPhysicalRoot>,
    scope: Arc<SourceServiceScope>,
    entry: usize,
    framed: libc::stat,
    offset: u64,
    _credit: resources::ResourceCredit,
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
    let _scratch = root.resources.reserve(IO_SCRATCH, 3, 0)?;
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
        file,
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
        let opened = nix::sys::stat::fstat(&self.file).map_err(io::Error::from)?;
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
            .reserve(IO_SCRATCH, 3, 0)
            .map_err(read_error)?;
        self.validate().map_err(read_error)?;
        let RepositoryWorkspaceNode::File(body) = self.root.state.entries()[self.entry].node()
        else {
            return Err(read_error(PhysicalRootError::Snapshot));
        };
        let available = (body.len() as u64).saturating_sub(self.offset);
        if available == 0 {
            let mut probe = [0u8; 1];
            if self.file.read_at(&mut probe, self.offset)? != 0 {
                return Err(read_error(PhysicalRootError::Snapshot));
            }
            self.validate().map_err(read_error)?;
            return Ok(0);
        }
        let requested = out.len().min(available.try_into().unwrap_or(usize::MAX));
        let count = self.file.read_at(&mut out[..requested], self.offset)?;
        let start =
            usize::try_from(self.offset).map_err(|_| read_error(PhysicalRootError::Snapshot))?;
        if count == 0 || out[..count] != body[start..start + count] {
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
impl Seek for RepositoryRootReader {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let _scratch = self
            .root
            .resources
            .reserve(IO_SCRATCH, 3, 0)
            .map_err(read_error)?;
        self.validate().map_err(read_error)?;
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
