//! A readable member retains only its exact digest data owner, original scope,
//! descriptor and finite credits. Partial Read/Seek is not a full SHA proof.
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::os::unix::fs::FileExt;

use super::*;
pub(in crate::runtime) struct RepositorySourceReader {
    file: File,
    root: Arc<RepositorySourceRoot>,
    relative: CompactString,
    frame: libc::stat,
    digest: FileContentDigest,
    offset: u64,
    _credit: ResourceCredit,
}
impl std::fmt::Debug for RepositorySourceReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RepositorySourceReader")
            .field("offset", &self.offset)
            .field("size", &self.digest.size_bytes())
            .finish_non_exhaustive()
    }
}
pub(super) fn open(
    root: &Arc<RepositorySourceRoot>,
    relative: &str,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<RepositorySourceReader, PhysicalRootError> {
    root.check_origin(scope, active)?;
    let entry = root
        .state
        .entry(relative)
        .ok_or(PhysicalRootError::Snapshot)?;
    let Node::File(digest) = entry.node() else {
        return Err(PhysicalRootError::Snapshot);
    };
    let credit = root.resources.reserve(
        add(
            std::mem::size_of::<RepositorySourceReader>(),
            relative.len() + 64,
        )?,
        1,
        0,
    )?;
    let _scratch = root.resources.reserve(IO_SCRATCH, 3, 0)?;
    root.validate_backing()?;
    let io = RootIo::new(&root.file, limits(root.reservation())?)?;
    let frame = io.lstat(Path::new(relative))?;
    if frame.st_mode & libc::S_IFMT != libc::S_IFREG
        || frame.st_nlink != 1
        || frame.st_size < 0
        || frame.st_size as u64 != digest.size_bytes()
        || (frame.st_mode & 0o7777) as u16 != entry.mode()
    {
        return Err(PhysicalRootError::Snapshot);
    }
    let file = io.open_read_regular(Path::new(relative))?;
    if !super::super::seal::unchanged(
        &frame,
        &nix::sys::stat::fstat(&file).map_err(io::Error::from)?,
    ) || !super::super::seal::unchanged(&frame, &io.lstat(Path::new(relative))?)
    {
        return Err(PhysicalRootError::Snapshot);
    }
    root.check_origin(scope, active)?;
    Ok(RepositorySourceReader {
        file,
        root: root.clone(),
        relative: CompactString::new(relative),
        frame,
        digest: *digest,
        offset: 0,
        _credit: credit,
    })
}
fn error(e: PhysicalRootError) -> io::Error {
    io::Error::other(format!("source member refused: {e:?}"))
}
impl RepositorySourceReader {
    fn validate(&self) -> Result<(), PhysicalRootError> {
        self.root.check_origin(&self.root.origin_scope, &|| true)?;
        self.root.validate_backing()?;
        let io = RootIo::new(&self.root.file, limits(self.root.reservation())?)?;
        if !super::super::seal::unchanged(
            &self.frame,
            &nix::sys::stat::fstat(&self.file).map_err(io::Error::from)?,
        ) || !super::super::seal::unchanged(
            &self.frame,
            &io.lstat(Path::new(self.relative.as_str()))?,
        ) {
            return Err(PhysicalRootError::Snapshot);
        }
        Ok(())
    }
}
impl Read for RepositorySourceReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        let _scratch = self
            .root
            .resources
            .reserve(IO_SCRATCH, 3, 0)
            .map_err(error)?;
        self.validate().map_err(error)?;
        let available = self.digest.size_bytes().saturating_sub(self.offset);
        if available == 0 {
            if self.file.read_at(&mut [0u8; 1], self.offset)? != 0 {
                return Err(error(PhysicalRootError::Snapshot));
            }
            self.validate().map_err(error)?;
            return Ok(0);
        }
        let requested = out.len().min(available.try_into().unwrap_or(usize::MAX));
        let count = self.file.read_at(&mut out[..requested], self.offset)?;
        if count == 0 {
            return Err(error(PhysicalRootError::Snapshot));
        }
        self.validate().map_err(error)?;
        self.offset += count as u64;
        Ok(count)
    }
}
impl Seek for RepositorySourceReader {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let _scratch = self
            .root
            .resources
            .reserve(IO_SCRATCH, 3, 0)
            .map_err(error)?;
        self.validate().map_err(error)?;
        let next = match from {
            SeekFrom::Start(n) => n as i128,
            SeekFrom::Current(n) => self.offset as i128 + n as i128,
            SeekFrom::End(n) => self.digest.size_bytes() as i128 + n as i128,
        };
        if next < 0 || next > i64::MAX as i128 {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }
        self.offset = next as u64;
        Ok(self.offset)
    }
}
