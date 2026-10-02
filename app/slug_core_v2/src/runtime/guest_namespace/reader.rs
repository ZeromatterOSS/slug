//! Trusted descriptor consumer. Guest metadata never comes from stage identity.
use std::fs::File;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::path::Component;
use std::path::Path;

use nix::fcntl::AtFlags;
use nix::fcntl::OFlag;
use nix::sys::stat::Mode;
use sha2::Digest;
use sha2::Sha256;
use slug_loading_v2::GuestDirectoryCheck;
use slug_loading_v2::GuestLookupError;
use slug_loading_v2::GuestNamespaceOperation;

use super::*;

#[derive(Debug)]
pub(super) enum ViewError {
    Lookup(GuestLookupError),
    Invalid,
    Stale,
    Cancelled,
    Resource,
    Io(std::io::Error),
    Sink,
}
impl From<std::io::Error> for ViewError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl ViewError {
    fn certified(&self) -> bool {
        matches!(
            self,
            Self::Lookup(
                GuestLookupError::Missing
                    | GuestLookupError::NotDirectory
                    | GuestLookupError::PermissionDenied
                    | GuestLookupError::ReadOnly
                    | GuestLookupError::NotSymlink
                    | GuestLookupError::IsDirectory
            )
        )
    }
}
struct Operation<'a> {
    view: &'a PreparedGuestNamespaceViewLease,
    complete: bool,
}
impl Drop for Operation<'_> {
    fn drop(&mut self) {
        if !self.complete {
            self.view.poison();
        }
    }
}
impl PreparedGuestNamespaceViewLease {
    fn operation<T>(&self, run: impl FnOnce() -> Result<T, ViewError>) -> Result<T, ViewError> {
        let mut operation = Operation {
            view: self,
            complete: false,
        };
        if !self.is_valid() {
            return Err(ViewError::Invalid);
        }
        let result = run();
        if result.as_ref().err().is_some_and(|e| !e.certified()) {
            return result;
        }
        if !self.is_valid() {
            return Err(ViewError::Invalid);
        }
        operation.complete = true;
        result
    }
    pub(super) fn lookup(
        &self,
        path: &str,
        op: GuestNamespaceOperation,
    ) -> Result<&GuestNamespaceNode, ViewError> {
        self.operation(|| self.lookup_checked(path, op, &|| true))
    }
    fn lookup_checked(
        &self,
        path: &str,
        op: GuestNamespaceOperation,
        active: &dyn Fn() -> bool,
    ) -> Result<&GuestNamespaceNode, ViewError> {
        let mut rejected = None;
        let result = self.facts().lookup_with(path, op, &mut |node, purpose| {
            let result = self.check_directory(node, purpose, active);
            if let Err(error) = result {
                rejected = Some(error);
                false
            } else {
                true
            }
        });
        if let Some(error) = rejected {
            return Err(error);
        }
        let node = result.map_err(ViewError::Lookup)?;
        if !matches!(node.binding(), GuestNamespaceBinding::Scaffold) {
            let _io_credit = self
                .budget()
                .try_reserve(8192)
                .map_err(|_| ViewError::Resource)?;
            self.with_bound_source(node, |root, relative| {
                check_named(root, relative, node).map(|_| ())
            })?;
        }
        check_active(self, active)?;
        Ok(node)
    }
    pub(super) fn list_directory(&self, path: &str) -> Result<&[usize], ViewError> {
        self.operation(|| {
            let node =
                self.lookup_checked(path, GuestNamespaceOperation::ListDirectory, &|| true)?;
            Ok(self.facts().children(node.index()))
        })
    }
    pub(super) fn read_regular(
        &self,
        path: &str,
        sink: &mut dyn FnMut(&[u8]) -> Result<(), ViewError>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), ViewError> {
        self.operation(|| {
            let node = self.lookup_checked(path, GuestNamespaceOperation::ReadRegular, active)?;
            let GuestNamespaceNodeKind::RegularFile(expected) = node.kind() else {
                return Err(ViewError::Stale);
            };
            // Fixed reader buffer and IO path/header allocation peaks are credited
            // before stack allocation/open. Sink chunks remain provisional.
            let _credit = self
                .budget()
                .try_reserve(64 * 1024 + 8192)
                .map_err(|_| ViewError::Resource)?;
            self.with_bound_source(node, |root, relative| {
                check_active(self, active)?;
                let before = check_named(root, relative, node)?;
                let (parent, name) = parent(root, relative)?;
                let mut file = open(&parent, name, OFlag::O_RDONLY)?;
                let opened = stat(&file)?;
                if !unchanged(&before, &opened) {
                    return Err(ViewError::Stale);
                }
                let mut hash = Sha256::new();
                let mut bytes = 0u64;
                let mut buffer = [0u8; 64 * 1024];
                loop {
                    check_active(self, active)?;
                    let remaining = expected.size_bytes().saturating_sub(bytes);
                    let length = remaining.min(buffer.len() as u64).max(1) as usize;
                    let count = file.read(&mut buffer[..length])?;
                    if count == 0 {
                        break;
                    }
                    bytes = bytes.checked_add(count as u64).ok_or(ViewError::Stale)?;
                    if bytes > expected.size_bytes() {
                        return Err(ViewError::Stale);
                    }
                    hash.update(&buffer[..count]);
                    sink(&buffer[..count])?;
                }
                let digest: [u8; 32] = hash.finalize().into();
                let after = stat(&file)?;
                let named = check_named(root, relative, node)?;
                if bytes != expected.size_bytes()
                    || digest != *expected.sha256()
                    || !unchanged(&opened, &after)
                    || !unchanged(&after, &named)
                {
                    return Err(ViewError::Stale);
                }
                check_active(self, active)
            })
        })
    }
    fn check_directory(
        &self,
        node: &GuestNamespaceNode,
        purpose: GuestDirectoryCheck,
        active: &dyn Fn() -> bool,
    ) -> Result<(), ViewError> {
        check_active(self, active)?;
        if matches!(node.binding(), GuestNamespaceBinding::Scaffold) {
            return Ok(());
        }
        let _io_credit = self
            .budget()
            .try_reserve(8192)
            .map_err(|_| ViewError::Resource)?;
        self.with_bound_source(node, |root, relative| {
            let before = check_named(root, relative, node)?;
            if !matches!(
                purpose,
                GuestDirectoryCheck::Membership | GuestDirectoryCheck::Listing
            ) {
                return Ok(());
            }
            let children = self.facts().children(node.index());
            let _credit = self
                .budget()
                .try_reserve(
                    children
                        .len()
                        .checked_add(8192 + 256)
                        .ok_or(ViewError::Resource)?,
                )
                .map_err(|_| ViewError::Resource)?;
            let mut seen = vec![false; children.len()];
            // A fresh open creates an independent directory cursor. dup/try_clone
            // would share its open-file-description and is not enumeration.
            let directory = if relative.as_os_str().is_empty() {
                open(root, Path::new("."), OFlag::O_RDONLY | OFlag::O_DIRECTORY)?
            } else {
                let (p, n) = parent(root, relative)?;
                open(&p, n, OFlag::O_RDONLY | OFlag::O_DIRECTORY)?
            };
            let opened = stat(&directory)?;
            if !unchanged(&before, &opened) {
                return Err(ViewError::Stale);
            }
            // Linux getdents64 fills this caller-owned fixed buffer directly;
            // no opaque libc DIR allocation can escape the credited bound.
            let mut buffer = [0u8; 8192];
            let mut count = 0usize;
            let mut records = 0usize;
            loop {
                check_active(self, active)?;
                let bytes = read_directory(&directory, &mut buffer)?;
                if bytes == 0 {
                    break;
                }
                let mut offset = 0usize;
                while offset < bytes {
                    check_active(self, active)?;
                    records = records.checked_add(1).ok_or(ViewError::Stale)?;
                    if records > children.len().checked_add(2).ok_or(ViewError::Stale)? {
                        return Err(ViewError::Stale);
                    }
                    let name = directory_name(&buffer[..bytes], &mut offset)?;
                    if name == b"." || name == b".." {
                        continue;
                    }
                    count = count.checked_add(1).ok_or(ViewError::Stale)?;
                    if count > children.len() {
                        return Err(ViewError::Stale);
                    }
                    let index = children
                        .binary_search_by(|index| {
                            self.facts()
                                .node(*index)
                                .expect("normal child")
                                .path()
                                .rsplit('/')
                                .next()
                                .unwrap()
                                .as_bytes()
                                .cmp(name)
                        })
                        .map_err(|_| ViewError::Stale)?;
                    if seen[index] {
                        return Err(ViewError::Stale);
                    }
                    seen[index] = true;
                    let child = self.facts().node(children[index]).ok_or(ViewError::Stale)?;
                    let name = std::str::from_utf8(name).map_err(|_| ViewError::Stale)?;
                    let current = named(&directory, Path::new(name))?;
                    compatible(&current, child)?;
                }
            }
            if count != children.len() || seen.iter().any(|v| !v) {
                return Err(ViewError::Stale);
            }
            let after = stat(&directory)?;
            let named = check_named(root, relative, node)?;
            if !unchanged(&opened, &after) || !unchanged(&after, &named) {
                return Err(ViewError::Stale);
            }
            Ok(())
        })
    }
}
fn check_active(
    view: &PreparedGuestNamespaceViewLease,
    active: &dyn Fn() -> bool,
) -> Result<(), ViewError> {
    if !active() {
        Err(ViewError::Cancelled)
    } else if !view.is_valid() {
        Err(ViewError::Invalid)
    } else {
        Ok(())
    }
}
fn open(root: &File, name: &Path, flags: OFlag) -> Result<File, ViewError> {
    nix::fcntl::openat(
        root,
        name,
        flags | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|e| ViewError::Io(e.into()))
}
fn parent<'p>(root: &File, relative: &'p Path) -> Result<(File, &'p Path), ViewError> {
    let mut current = open(root, Path::new("."), OFlag::O_RDONLY | OFlag::O_DIRECTORY)?;
    let mut parts = relative.components().peekable();
    let mut depth = 0;
    while let Some(part) = parts.next() {
        let Component::Normal(name) = part else {
            return Err(ViewError::Stale);
        };
        depth += 1;
        if depth > 32 {
            return Err(ViewError::Stale);
        }
        if parts.peek().is_none() {
            return Ok((current, Path::new(name)));
        }
        current = open(
            &current,
            Path::new(name),
            OFlag::O_RDONLY | OFlag::O_DIRECTORY,
        )?;
    }
    Err(ViewError::Stale)
}
fn stat(file: &File) -> Result<libc::stat, ViewError> {
    nix::sys::stat::fstat(file).map_err(|e| ViewError::Io(e.into()))
}
fn named(file: &File, name: &Path) -> Result<libc::stat, ViewError> {
    nix::sys::stat::fstatat(file, name, AtFlags::AT_SYMLINK_NOFOLLOW)
        .map_err(|e| ViewError::Io(e.into()))
}
fn check_named(
    root: &File,
    relative: &Path,
    node: &GuestNamespaceNode,
) -> Result<libc::stat, ViewError> {
    let s = if relative.as_os_str().is_empty() {
        stat(root)?
    } else {
        let (p, n) = parent(root, relative)?;
        named(&p, n)?
    };
    compatible(&s, node)?;
    Ok(s)
}
fn compatible(s: &libc::stat, node: &GuestNamespaceNode) -> Result<(), ViewError> {
    let kind = match node.kind() {
        GuestNamespaceNodeKind::Directory => libc::S_IFDIR,
        GuestNamespaceNodeKind::RegularFile(d) => {
            if s.st_size < 0 || s.st_size as u64 != d.size_bytes() {
                return Err(ViewError::Stale);
            }
            libc::S_IFREG
        }
    };
    if s.st_mode & libc::S_IFMT != kind || s.st_mode & 0o7777 != node.mode() {
        return Err(ViewError::Stale);
    }
    Ok(())
}
fn unchanged(a: &libc::stat, b: &libc::stat) -> bool {
    a.st_dev == b.st_dev
        && a.st_ino == b.st_ino
        && a.st_mode == b.st_mode
        && a.st_size == b.st_size
        && a.st_mtime == b.st_mtime
        && a.st_mtime_nsec == b.st_mtime_nsec
        && a.st_ctime == b.st_ctime
        && a.st_ctime_nsec == b.st_ctime_nsec
}
// Fixed Linux dirent64 ABI: inode64, offset64, reclen16, type8, name.
// This is trusted private filesystem IO, not a child syscall mediator.
fn read_directory(directory: &File, buffer: &mut [u8; 8192]) -> Result<usize, ViewError> {
    let count = unsafe {
        libc::syscall(
            libc::SYS_getdents64,
            directory.as_raw_fd(),
            buffer.as_mut_ptr(),
            buffer.len(),
        )
    };
    if count < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let count = usize::try_from(count).map_err(|_| ViewError::Stale)?;
    if count > buffer.len() {
        return Err(ViewError::Stale);
    }
    Ok(count)
}
fn directory_name<'a>(buffer: &'a [u8], offset: &mut usize) -> Result<&'a [u8], ViewError> {
    let record = buffer.get(*offset..).ok_or(ViewError::Stale)?;
    if record.len() < 20 {
        return Err(ViewError::Stale);
    }
    let length = usize::from(u16::from_ne_bytes([record[16], record[17]]));
    if length < 20 || length > record.len() {
        return Err(ViewError::Stale);
    }
    let name = &record[19..length];
    let end = name.iter().position(|v| *v == 0).ok_or(ViewError::Stale)?;
    if end == 0 || name[..end].contains(&b'/') {
        return Err(ViewError::Stale);
    }
    *offset = offset.checked_add(length).ok_or(ViewError::Stale)?;
    Ok(&name[..end])
}
