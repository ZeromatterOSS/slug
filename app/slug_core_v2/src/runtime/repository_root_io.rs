//! Private descriptor-relative repository IO. This borrows an already selected
//! root; it never discovers roots, mints source authority or owns mount leases.
//!
//! Mutation requires the caller's exclusive, quiescent workspace capability;
//! reads require that capability or a sealed generation. Component no-follow
//! walks are not a substitute for excluding concurrent rename/mount writers.
//! The owning adapter must reserve descriptor/scratch credits before calling:
//! at most three temporary descriptors and one 64 KiB read/write scratch buffer
//! (or one 8 KiB directory buffer) are live per operation. Directory callbacks borrow a bounded native name;
//! any retained capture and command-wide work/body accounting belong to callers.

use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use sha2::Digest;
use sha2::Sha256;

mod linux;

/// Local operation bounds, not physical quota or a command-wide memory receipt.
/// The path allowance is relative: the owner must first subtract display-prefix
/// bytes from its admitted absolute-path limit. No API normalizes input paths.
#[derive(Clone, Copy, Debug)]
pub(super) struct RootIoLimits {
    pub max_path_bytes: usize,
    pub max_name_bytes: usize,
    pub max_depth: usize,
    pub max_entries: usize,
    pub max_directory_name_bytes: usize,
    pub max_file_bytes: u64,
    pub max_link_bytes: usize,
}

impl RootIoLimits {
    fn validate(self) -> io::Result<Self> {
        // Archive-only complete trees have an absolute-path bound, not the
        // separate deletion walk's 256-level cap. Process owners select 64.
        // At most 2048 normal one-byte components can fit a 4096-byte path.
        if self.max_path_bytes > 4096
            || self.max_name_bytes > 255
            || self.max_depth > 2048
            || self.max_entries > 131_072
            || self.max_directory_name_bytes > 4 * 1024 * 1024
            || self.max_file_bytes > 1024 * 1024 * 1024
            || self.max_link_bytes > 4096
        {
            return Err(invalid("rooted IO bounds exceed the admitted envelope"));
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum WriteMode {
    CreateNew,
    /// Truncate an existing single-link regular file in place, or create it.
    /// No extra temporary inode or copy-on-replace allocation is introduced.
    ReplaceRegular,
}

pub(super) struct FileCapture {
    pub metadata: libc::stat,
    pub digest: [u8; 32],
}

/// A borrowed IO adapter, never a retained capability in a semantic value.
pub(super) struct RootIo<'root> {
    root: &'root File,
    initial_root: libc::stat,
    limits: RootIoLimits,
}

impl<'root> RootIo<'root> {
    pub fn new(root: &'root File, limits: RootIoLimits) -> io::Result<Self> {
        let limits = limits.validate()?;
        let initial_root = linux::stat(root)?;
        if initial_root.st_mode & libc::S_IFMT != libc::S_IFDIR {
            return Err(invalid("repository root descriptor is not a directory"));
        }
        Ok(Self {
            root,
            initial_root,
            limits,
        })
    }

    pub fn root_stat(&self) -> io::Result<libc::stat> {
        let current = linux::stat(self.root)?;
        if current.st_dev != self.initial_root.st_dev
            || current.st_ino != self.initial_root.st_ino
            || current.st_mode & libc::S_IFMT != libc::S_IFDIR
        {
            return Err(stale());
        }
        Ok(current)
    }

    /// Admit declared logical payload before opening/truncating a destination.
    /// Streaming adapters must also enforce this amount while copying bytes.
    pub fn admit_file_size(&self, size: u64) -> io::Result<()> {
        if size > self.limits.max_file_bytes {
            return Err(limit());
        }
        Ok(())
    }

    fn validate_path<'path>(
        &self,
        relative: &'path Path,
        root_allowed: bool,
    ) -> io::Result<&'path [u8]> {
        let bytes = relative.as_os_str().as_bytes();
        if bytes.is_empty() && root_allowed {
            return Ok(bytes);
        }
        if bytes.is_empty() || bytes.len() > self.limits.max_path_bytes || bytes.contains(&0) {
            return Err(invalid("invalid rooted relative path"));
        }
        let mut depth = 0usize;
        for name in bytes.split(|byte| *byte == b'/') {
            if name.is_empty()
                || name == b"."
                || name == b".."
                || name.len() > self.limits.max_name_bytes
            {
                return Err(invalid("rooted path requires bounded normal components"));
            }
            depth = depth.checked_add(1).ok_or_else(limit)?;
            if depth > self.limits.max_depth {
                return Err(limit());
            }
        }
        Ok(bytes)
    }

    fn same_device(&self, metadata: &libc::stat) -> io::Result<()> {
        if metadata.st_dev != self.initial_root.st_dev {
            return Err(io::Error::from_raw_os_error(libc::EXDEV));
        }
        Ok(())
    }

    fn directory(&self, relative: &Path) -> io::Result<File> {
        let bytes = self.validate_path(relative, true)?;
        self.root_stat()?;
        // Open a new file description, rather than dup: enumeration must not
        // change the root descriptor's offset or another reader's offset.
        let mut current = linux::directory(self.root, Path::new("."))?;
        for component in bytes
            .split(|byte| *byte == b'/')
            .filter(|name| !name.is_empty())
        {
            let next = linux::directory(&current, Path::new(OsStr::from_bytes(component)))?;
            self.same_device(&linux::stat(&next)?)?;
            current = next;
        }
        Ok(current)
    }

    /// Credited native source acquisition may pin a directory below its already
    /// authenticated Host anchor. Every intermediate component is no-follow.
    pub fn open_directory(&self, relative: &Path) -> io::Result<File> {
        self.directory(relative)
    }

    fn parent<'path>(&self, relative: &'path Path) -> io::Result<(File, &'path Path)> {
        let bytes = self.validate_path(relative, false)?;
        let (parent, name) = match bytes.iter().rposition(|byte| *byte == b'/') {
            Some(index) => (&bytes[..index], &bytes[index + 1..]),
            None => (&b""[..], bytes),
        };
        Ok((
            self.directory(Path::new(OsStr::from_bytes(parent)))?,
            Path::new(OsStr::from_bytes(name)),
        ))
    }

    /// Empty relative path denotes the actual root descriptor itself.
    pub fn lstat(&self, relative: &Path) -> io::Result<libc::stat> {
        if self.validate_path(relative, true)?.is_empty() {
            return self.root_stat();
        }
        let (parent, name) = self.parent(relative)?;
        let metadata = linux::lstat(&parent, name)?;
        self.same_device(&metadata)?;
        Ok(metadata)
    }

    /// The caller supplies max_link_bytes+1 scratch so exact and truncated
    /// targets are distinguishable without an allocation or unbounded retry.
    pub fn read_link<'buffer>(
        &self,
        relative: &Path,
        buffer: &'buffer mut [u8],
    ) -> io::Result<&'buffer [u8]> {
        let needed = self
            .limits
            .max_link_bytes
            .checked_add(1)
            .ok_or_else(limit)?;
        if buffer.len() < needed {
            return Err(invalid("readlink scratch lacks the overflow byte"));
        }
        let (parent, name) = self.parent(relative)?;
        let before = linux::lstat(&parent, name)?;
        self.same_device(&before)?;
        if before.st_mode & libc::S_IFMT != libc::S_IFLNK {
            return Err(invalid("readlink target is not a literal link"));
        }
        let count = linux::read_link(&parent, name, &mut buffer[..needed])?;
        if count > self.limits.max_link_bytes {
            return Err(limit());
        }
        if !unchanged(&before, &linux::lstat(&parent, name)?) {
            return Err(stale());
        }
        Ok(&buffer[..count])
    }

    /// Visits immediate child names and no-follow metadata. Order is native;
    /// deterministic retained ordering belongs to the bounded caller capture.
    /// Entry/name limits are checked before metadata IO or invoking the visitor.
    pub fn read_directory(
        &self,
        relative: &Path,
        active: &dyn Fn() -> bool,
        mut visitor: impl FnMut(&OsStr, &libc::stat) -> io::Result<()>,
    ) -> io::Result<()> {
        check_active(active)?;
        let directory = self.directory(relative)?;
        let before = linux::stat(&directory)?;
        let prefix = relative.as_os_str().as_bytes();
        let depth = if prefix.is_empty() {
            0
        } else {
            prefix.split(|b| *b == b'/').count()
        };
        let mut entries = 0usize;
        let mut name_bytes = 0usize;
        linux::read_directory(&directory, |name| {
            check_active(active)?;
            let bytes = name.as_bytes();
            if bytes.is_empty()
                || bytes.len() > self.limits.max_name_bytes
                || bytes.contains(&b'/')
                || bytes.contains(&0)
            {
                return Err(invalid("invalid rooted directory entry name"));
            }
            let child_bytes = prefix
                .len()
                .checked_add(usize::from(!prefix.is_empty()))
                .and_then(|length| length.checked_add(bytes.len()))
                .ok_or_else(limit)?;
            if child_bytes > self.limits.max_path_bytes || depth + 1 > self.limits.max_depth {
                return Err(limit());
            }
            entries = entries.checked_add(1).ok_or_else(limit)?;
            name_bytes = name_bytes.checked_add(bytes.len()).ok_or_else(limit)?;
            if entries > self.limits.max_entries
                || name_bytes > self.limits.max_directory_name_bytes
            {
                return Err(limit());
            }
            let metadata = linux::lstat(&directory, Path::new(name))?;
            self.same_device(&metadata)?;
            visitor(name, &metadata)
        })?;
        check_active(active)?;
        if !unchanged(&before, &linux::stat(&directory)?) {
            return Err(stale());
        }
        Ok(())
    }

    /// The returned descriptor stays internal to native IO. Its outer native
    /// reader must retain the backing lease and reserve its descriptor credit.
    pub fn open_read_regular(&self, relative: &Path) -> io::Result<File> {
        let (parent, name) = self.parent(relative)?;
        self.check_regular_metadata(&linux::lstat(&parent, name)?)?;
        let file = linux::open_regular(&parent, name, false, None)?;
        let opened = self.check_regular(&file)?;
        if !unchanged(&opened, &linux::lstat(&parent, name)?) {
            return Err(stale());
        }
        Ok(file)
    }

    fn check_regular(&self, file: &File) -> io::Result<libc::stat> {
        let metadata = linux::stat(file)?;
        self.check_regular_metadata(&metadata)?;
        Ok(metadata)
    }

    fn check_regular_metadata(&self, metadata: &libc::stat) -> io::Result<()> {
        self.same_device(metadata)?;
        if metadata.st_mode & libc::S_IFMT != libc::S_IFREG || metadata.st_nlink != 1 {
            return Err(invalid("rooted file must be regular with one link"));
        }
        Ok(())
    }

    /// Stream actual bytes through a caller-owned credited capture, computing
    /// SHA-256 over those same bytes. Regularity, size, EOF, pathname association
    /// and opened-file identity must all remain stable before success.
    pub fn read_file(
        &self,
        relative: &Path,
        active: &dyn Fn() -> bool,
        capture: impl FnMut(&[u8]) -> io::Result<()>,
    ) -> io::Result<FileCapture> {
        let (metadata, digest) = self.read_file_kernel::<true>(relative, active, capture)?;
        Ok(FileCapture {
            metadata,
            digest: digest.ok_or_else(|| invalid("required rooted digest was not derived"))?,
        })
    }

    /// Stream complete actual file contents with the same admission, EOF and
    /// opened/named-frame checks, returning authenticated metadata only.
    pub fn read_file_contents(
        &self,
        relative: &Path,
        active: &dyn Fn() -> bool,
        capture: impl FnMut(&[u8]) -> io::Result<()>,
    ) -> io::Result<libc::stat> {
        self.read_file_kernel::<false>(relative, active, capture)
            .map(|(metadata, _)| metadata)
    }

    fn read_file_kernel<const DERIVE_DIGEST: bool>(
        &self,
        relative: &Path,
        active: &dyn Fn() -> bool,
        mut capture: impl FnMut(&[u8]) -> io::Result<()>,
    ) -> io::Result<(libc::stat, Option<[u8; 32]>)> {
        check_active(active)?;
        let mut file = self.open_read_regular(relative)?;
        let before = self.check_regular(&file)?;
        let mut remaining = u64::try_from(before.st_size).map_err(|_| limit())?;
        self.admit_file_size(remaining)?;
        let mut buffer = [0u8; 64 * 1024];
        // Only a digest-returning API requests SHA; both result classes retain
        // the same complete byte stream and opened/named-frame checks.
        let mut digest = DERIVE_DIGEST.then(Sha256::new);
        while remaining != 0 {
            check_active(active)?;
            let length = usize::try_from(remaining.min(buffer.len() as u64)).unwrap();
            let count = file.read(&mut buffer[..length])?;
            if count == 0 {
                return Err(stale());
            }
            #[cfg(test)]
            read_work_for_test::record_chunk(DERIVE_DIGEST, count, digest.is_some());
            if let Some(digest) = &mut digest {
                digest.update(&buffer[..count]);
            }
            capture(&buffer[..count])?;
            remaining -= count as u64;
        }
        check_active(active)?;
        if file.read(&mut [0u8; 1])? != 0
            || !unchanged(&before, &self.check_regular(&file)?)
            || !unchanged(&before, &self.lstat(relative)?)
        {
            return Err(stale());
        }
        check_active(active)?;
        Ok((before, digest.map(|digest| digest.finalize().into())))
    }

    pub fn digest_file(
        &self,
        relative: &Path,
        active: &dyn Fn() -> bool,
    ) -> io::Result<FileCapture> {
        self.read_file(relative, active, |_| Ok(()))
    }

    /// Ensure normal directory components, never traversing an existing link.
    /// mkdirat honors the current umask, like the existing Path adapter.
    /// Existing directory modes are preserved. No mutation of the root mode.
    pub fn ensure_directory(&self, relative: &Path, mode: u32) -> io::Result<()> {
        let mode = checked_mode(mode)?;
        let bytes = self.validate_path(relative, true)?;
        self.root_stat()?;
        let mut current = linux::directory(self.root, Path::new("."))?;
        for component in bytes
            .split(|byte| *byte == b'/')
            .filter(|name| !name.is_empty())
        {
            let name = Path::new(OsStr::from_bytes(component));
            linux::mkdir(&current, name, mode)?;
            let next = linux::directory(&current, name)?;
            self.same_device(&linux::stat(&next)?)?;
            current = next;
        }
        Ok(())
    }

    /// Streaming archive adapter: opens without O_TRUNC, verifies a single-link
    /// regular file, then truncates. The caller owns decoded/physical/write
    /// budgets, cancellation and FileTimes on the returned File; this method
    /// does not bound arbitrary subsequent writes performed by that caller.
    pub fn open_write_regular(&self, relative: &Path, mode: WriteMode) -> io::Result<File> {
        let (parent, name) = self.parent(relative)?;
        match linux::lstat(&parent, name) {
            Ok(metadata) => self.check_regular_metadata(&metadata)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let file = linux::open_regular(&parent, name, true, Some(mode))?;
        let opened = self.check_regular(&file)?;
        if !unchanged(&opened, &linux::lstat(&parent, name)?) {
            return Err(stale());
        }
        file.set_len(0)?;
        Ok(file)
    }

    /// Change mode on an FD just opened by this adapter. This is an internal
    /// native helper, not a source/membership authority factory.
    pub fn chmod_regular(&self, file: &File, mode: u32) -> io::Result<()> {
        self.check_regular(file)?;
        linux::chmod(file, checked_mode(mode)?)
    }

    /// Convenience replacement for already captured file/template bytes. Errors
    /// can leave a partial mutation; the caller must discard that generation.
    /// Stream a preadmitted logical body without materializing a full file.
    /// Mode and size are checked before opening/truncation, as in write_file.
    pub fn write_file_chunks(
        &self,
        relative: &Path,
        length: u64,
        mode: u32,
        write_mode: WriteMode,
        active: &dyn Fn() -> bool,
        produce: impl FnOnce(&mut dyn FnMut(&[u8]) -> io::Result<()>) -> io::Result<()>,
    ) -> io::Result<()> {
        let mode = checked_mode(mode)?;
        self.admit_file_size(length)?;
        check_active(active)?;
        let mut file = self.open_write_regular(relative, write_mode)?;
        let mut written = 0u64;
        produce(&mut |chunk| {
            if chunk.len() > 64 * 1024 {
                return Err(limit());
            }
            let next = written
                .checked_add(chunk.len() as u64)
                .filter(|n| *n <= length)
                .ok_or_else(limit)?;
            let mut remaining = chunk;
            while !remaining.is_empty() {
                check_active(active)?;
                let count = file.write(remaining)?;
                if count == 0 {
                    return Err(io::Error::from(io::ErrorKind::WriteZero));
                }
                remaining = &remaining[count..];
            }
            written = next;
            Ok(())
        })?;
        check_active(active)?;
        if written != length {
            return Err(stale());
        }
        self.chmod_regular(&file, mode)?;
        let opened = self.check_regular(&file)?;
        if opened.st_size < 0
            || opened.st_size as u64 != length
            || !unchanged(&opened, &self.lstat(relative)?)
        {
            return Err(stale());
        }
        Ok(())
    }

    pub fn write_file(
        &self,
        relative: &Path,
        bytes: &[u8],
        mode: u32,
        write_mode: WriteMode,
        active: &dyn Fn() -> bool,
    ) -> io::Result<()> {
        let mode = checked_mode(mode)?;
        self.admit_file_size(bytes.len() as u64)?;
        check_active(active)?;
        let mut file = self.open_write_regular(relative, write_mode)?;
        let mut remaining = bytes;
        while !remaining.is_empty() {
            check_active(active)?;
            let length = remaining.len().min(64 * 1024);
            let count = file.write(&remaining[..length])?;
            if count == 0 {
                return Err(io::Error::from(io::ErrorKind::WriteZero));
            }
            remaining = &remaining[count..];
        }
        check_active(active)?;
        self.chmod_regular(&file, mode)?;
        let opened = self.check_regular(&file)?;
        if !unchanged(&opened, &self.lstat(relative)?) {
            return Err(stale());
        }
        Ok(())
    }

    /// Install a new literal relative link without following its destination.
    /// The caller owns complete-tree membership and immutable-source policy.
    /// Absolute or lexically escaping targets never reach the syscall.
    pub fn symlink_relative(&self, relative: &Path, target: &Path) -> io::Result<()> {
        self.validate_path(relative, false)?;
        let bytes = target.as_os_str().as_bytes();
        if bytes.is_empty()
            || bytes.len() > self.limits.max_link_bytes
            || bytes.contains(&0)
            || target.is_absolute()
        {
            return Err(invalid("invalid rooted literal link"));
        }
        let mut depth = relative.parent().map_or(0, |p| p.components().count());
        for component in target.components() {
            match component {
                std::path::Component::Normal(name)
                    if name.as_bytes().len() <= self.limits.max_name_bytes =>
                {
                    depth = depth.checked_add(1).ok_or_else(limit)?;
                    if depth > self.limits.max_depth {
                        return Err(limit());
                    }
                }
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    depth = depth
                        .checked_sub(1)
                        .ok_or_else(|| invalid("rooted link escapes root"))?;
                }
                _ => return Err(invalid("invalid rooted link component")),
            }
        }
        let (parent, name) = self.parent(relative)?;
        linux::symlink(&parent, name, target)?;
        let mut actual = [0u8; 4097];
        if self.read_link(relative, &mut actual)? != bytes {
            return Err(stale());
        }
        Ok(())
    }

    /// Install an absolute literal whose own/current-Source membership was
    /// admitted by the closed native prefix owner. Destination components stay
    /// nofollow; this creates no general absolute-link or traversal authority.
    pub fn symlink_validated_absolute(
        &self,
        relative: &Path,
        proof: &super::repository_physical_root::ValidatedAbsoluteLink<'_>,
    ) -> io::Result<()> {
        self.validate_path(relative, false)?;
        let target = proof.literal();
        let bytes = target.as_os_str().as_bytes();
        if !target.is_absolute()
            || bytes.is_empty()
            || bytes.len() > self.limits.max_link_bytes
            || bytes.contains(&0)
        {
            return Err(invalid("invalid admitted absolute link"));
        }
        let (parent, name) = self.parent(relative)?;
        linux::symlink(&parent, name, target)?;
        let mut actual = [0u8; 4097];
        if self.read_link(relative, &mut actual)? != bytes {
            return Err(stale());
        }
        Ok(())
    }

    /// Commit a privately staged regular replacement. Both names are walked
    /// without following links; caller retains the exact staged file handle.
    pub fn replace_regular(&self, staged: &Path, target: &Path, file: &File) -> io::Result<()> {
        let opened = self.check_regular(file)?;
        let (from_parent, from) = self.parent(staged)?;
        let (to_parent, to) = self.parent(target)?;
        if !unchanged(&opened, &linux::lstat(&from_parent, from)?) {
            return Err(stale());
        }
        self.check_regular_metadata(&linux::lstat(&to_parent, to)?)?;
        linux::rename(&from_parent, from, &to_parent, to)?;
        let moved = linux::stat(file)?;
        if opened.st_dev != moved.st_dev
            || opened.st_ino != moved.st_ino
            || opened.st_mode != moved.st_mode
            || opened.st_size != moved.st_size
            || !unchanged(&moved, &linux::lstat(&to_parent, to)?)
        {
            return Err(stale());
        }
        Ok(())
    }

    /// Remove the named file/link itself; never follow its literal target.
    pub fn unlink(&self, relative: &Path) -> io::Result<()> {
        let (parent, name) = self.parent(relative)?;
        let metadata = linux::lstat(&parent, name)?;
        self.same_device(&metadata)?;
        match metadata.st_mode & libc::S_IFMT {
            libc::S_IFREG if metadata.st_nlink == 1 => {}
            libc::S_IFLNK => {}
            _ => return Err(invalid("unlink requires an admitted regular file or link")),
        }
        linux::unlink(&parent, name, false)
    }

    /// Remove one empty normal directory. Recursive deletion belongs to the
    /// caller's explicitly bounded traversal; an empty path can never remove root.
    pub fn remove_directory(&self, relative: &Path) -> io::Result<()> {
        let (parent, name) = self.parent(relative)?;
        let metadata = linux::lstat(&parent, name)?;
        self.same_device(&metadata)?;
        if metadata.st_mode & libc::S_IFMT != libc::S_IFDIR {
            return Err(invalid("rmdir target is not a directory"));
        }
        linux::unlink(&parent, name, true)
    }

    pub fn set_mode(&self, relative: &Path, mode: u32) -> io::Result<()> {
        let mode = checked_mode(mode)?;
        let metadata = self.lstat(relative)?;
        let file = match metadata.st_mode & libc::S_IFMT {
            libc::S_IFDIR => self.directory(relative)?,
            libc::S_IFREG => self.open_read_regular(relative)?,
            _ => return Err(invalid("mode change requires a real directory or file")),
        };
        if !unchanged(&metadata, &linux::stat(&file)?) {
            return Err(stale());
        }
        linux::chmod(&file, mode)
    }
}

pub(super) fn unchanged(before: &libc::stat, after: &libc::stat) -> bool {
    before.st_dev == after.st_dev
        && before.st_ino == after.st_ino
        && before.st_mode == after.st_mode
        && before.st_nlink == after.st_nlink
        && before.st_size == after.st_size
        && before.st_mtime == after.st_mtime
        && before.st_mtime_nsec == after.st_mtime_nsec
        && before.st_ctime == after.st_ctime
        && before.st_ctime_nsec == after.st_ctime_nsec
}

fn checked_mode(mode: u32) -> io::Result<u32> {
    if mode & !0o777 != 0 {
        return Err(invalid("special repository mode bits are unsupported"));
    }
    Ok(mode)
}

fn check_active(active: &dyn Fn() -> bool) -> io::Result<()> {
    if !active() {
        return Err(io::Error::from(io::ErrorKind::Interrupted));
    }
    Ok(())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn limit() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "rooted IO operation exceeds its bound",
    )
}

fn stale() -> io::Error {
    io::Error::from_raw_os_error(libc::ESTALE)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "repository_root_io/read_work_for_test.rs"]
pub(crate) mod read_work_for_test;

#[cfg(test)]
#[path = "repository_root_io/framed_stream_tests.rs"]
mod framed_stream_tests;
