//! Inactive general-workspace deletion consumer. The legacy Path lane is left
//! unchanged. Core supplies exclusive, quiescent RootIo mutation authority.

use std::io;

use super::*;
use crate::runtime::repository_root_io::RootIo;

#[derive(Clone, Copy)]
#[repr(u8)]
enum Kind {
    Directory,
    Leaf,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct Row {
    offset: u32,
    length: u16,
    kind: Kind,
    device: u64,
    inode: u64,
}

// Unlike the legacy live DFS frontier, this plan retains all paths and rows
// until complete discovery succeeds. Prepay both old/new allocation peaks when
// either Vec grows, plus fixed rooted directory/path/syscall scratch. This is
// positive modeled buffer credit, not allocator/kernel overhead or an RSS claim.
// On 64-bit Linux Row is24 bytes and this bound is14MiB+64KiB, below the existing
// 16MiB source buffer bank. No source-service limit is raised by this consumer.
const ROOTED_FRONTIER_CREDIT: usize =
    2 * (MAX_ENTRIES * std::mem::size_of::<Row>() + MAX_PATH_BYTES) + 64 * 1024;

#[derive(Default)]
struct Plan {
    rows: Vec<Row>,
    paths: Vec<u8>,
}

impl Plan {
    // The caller charges DeleteWork before every push. Capacity requests are
    // bounded before allocation; no implicit Vec growth follows those requests.
    fn push(
        &mut self,
        parent: &str,
        name: Option<&str>,
        metadata: &libc::stat,
    ) -> Result<(), ArchiveMaterializationError> {
        let kind = admitted_kind(metadata)?;
        let length = parent
            .len()
            .checked_add(name.map_or(0, |name| name.len() + 1))
            .filter(|length| *length <= 4096)
            .ok_or_else(|| spec("rooted deletion path exceeds limit"))?;
        let next_bytes = self
            .paths
            .len()
            .checked_add(length)
            .filter(|length| *length <= MAX_PATH_BYTES)
            .ok_or_else(|| spec("rooted deletion retained path storage exceeds limit"))?;
        let next_rows = self
            .rows
            .len()
            .checked_add(1)
            .filter(|length| *length <= MAX_ENTRIES)
            .ok_or_else(|| spec("rooted deletion retained entry storage exceeds limit"))?;
        if next_rows > self.rows.capacity() {
            let capacity = self
                .rows
                .capacity()
                .saturating_mul(2)
                .max(4)
                .min(MAX_ENTRIES);
            self.rows
                .try_reserve_exact(capacity - self.rows.len())
                .map_err(io_error)?;
        }
        if next_bytes > self.paths.capacity() {
            let capacity = self
                .paths
                .capacity()
                .saturating_mul(2)
                .max(next_bytes)
                .max(256)
                .min(MAX_PATH_BYTES);
            self.paths
                .try_reserve_exact(capacity - self.paths.len())
                .map_err(io_error)?;
        }
        let offset = self.paths.len() as u32;
        self.paths.extend_from_slice(parent.as_bytes());
        if let Some(name) = name {
            self.paths.push(b'/');
            self.paths.extend_from_slice(name.as_bytes());
        }
        self.rows.push(Row {
            offset,
            length: length as u16,
            kind,
            device: metadata.st_dev as u64,
            inode: metadata.st_ino as u64,
        });
        Ok(())
    }

    fn path(&self, row: Row) -> &str {
        let begin = row.offset as usize;
        std::str::from_utf8(&self.paths[begin..begin + row.length as usize])
            .expect("plan writer stores complete UTF-8 paths")
    }
}

fn admitted_kind(metadata: &libc::stat) -> Result<Kind, ArchiveMaterializationError> {
    match metadata.st_mode & libc::S_IFMT {
        libc::S_IFDIR => Ok(Kind::Directory),
        libc::S_IFREG if metadata.st_nlink == 1 => Ok(Kind::Leaf),
        libc::S_IFLNK => Ok(Kind::Leaf),
        libc::S_IFREG => Err(spec("rooted deletion of regular hardlinks is unsupported")),
        _ => Err(spec("rooted deletion of special files is unsupported")),
    }
}

fn validate_relative(relative: &str) -> Result<(), ArchiveMaterializationError> {
    let drive = relative.as_bytes().get(1) == Some(&b':')
        && relative
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic);
    if drive
        || relative.is_empty()
        || relative.len() > 4096
        || relative.split('/').count() > MAX_DEPTH
        || relative
            .split('/')
            .any(|part| matches!(part, "" | "." | ".."))
        || relative.contains(['\\', '\0'])
        || Path::new(relative).is_absolute()
    {
        return Err(spec(
            "rooted deletion requires a normalized nonroot relative path",
        ));
    }
    Ok(())
}

pub(super) fn delete_at(
    root: &RootIo<'_>,
    relative: &str,
    work: &mut DeleteWork,
    active: &dyn Fn() -> bool,
    scope: Option<&Arc<SourceServiceScope>>,
) -> Result<bool, ArchiveMaterializationError> {
    validate_relative(relative)?;
    check(active, scope)?;
    // Reserve before constructing either retained Vec or invoking rooted IO.
    let _frontier_credit = source_service::buffer(scope, ROOTED_FRONTIER_CREDIT)
        .map_err(ArchiveMaterializationError::materialization)?;
    for (index, byte) in relative.bytes().enumerate() {
        if byte != b'/' {
            continue;
        }
        check(active, scope)?;
        let ancestor = &relative[..index];
        work.visit(ancestor.len())?;
        match root.lstat(Path::new(ancestor)) {
            Ok(metadata) if metadata.st_mode & libc::S_IFMT == libc::S_IFLNK => {
                return Err(spec("rooted deletion symlink ancestors are unsupported"));
            }
            Ok(metadata) if metadata.st_mode & libc::S_IFMT != libc::S_IFDIR => {
                return Err(io_error("rooted deletion has a non-directory ancestor"));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(io_error(error)),
        }
    }
    check(active, scope)?;
    work.visit(relative.len())?;
    let metadata = match root.lstat(Path::new(relative)) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(io_error(error)),
    };
    let mut plan = Plan::default();
    plan.push(relative, None, &metadata)?;
    let mut cursor = 0usize;
    let mut path_scratch = [0u8; 4096];
    while cursor < plan.rows.len() {
        check(active, scope)?;
        let row = plan.rows[cursor];
        cursor += 1;
        if !matches!(row.kind, Kind::Directory) {
            continue;
        }
        let path = plan.path(row).as_bytes();
        path_scratch[..path.len()].copy_from_slice(path);
        let parent =
            std::str::from_utf8(&path_scratch[..path.len()]).expect("copied complete UTF-8 path");
        let parent_depth = parent.split('/').count();
        let mut visitor_error = None;
        let rooted_active = || active() && source_service::check(scope).is_ok();
        let listed = root.read_directory(Path::new(parent), &rooted_active, |name, metadata| {
            let result = (|| {
                check(active, scope)?;
                let name = name
                    .to_str()
                    .ok_or_else(|| spec("rooted deletion path is not Unicode"))?;
                let length = parent
                    .len()
                    .checked_add(1)
                    .and_then(|n| n.checked_add(name.len()))
                    .filter(|length| *length <= 4096)
                    .ok_or_else(|| spec("rooted deletion path exceeds limit"))?;
                if parent_depth >= MAX_DEPTH && metadata.st_mode & libc::S_IFMT == libc::S_IFDIR {
                    return Err(spec("rooted deletion depth exceeds limit"));
                }
                // Charge every discovered descendant before retaining its path
                // or row, including leaves and nodes later found unsupported.
                work.visit(length)?;
                plan.push(parent, Some(name), metadata)
            })();
            match result {
                Ok(()) => Ok(()),
                Err(error) => {
                    visitor_error = Some(error);
                    Err(io::Error::other("rooted deletion visitor refused"))
                }
            }
        });
        if let Some(error) = visitor_error {
            return Err(error);
        }
        check(active, scope)?;
        listed.map_err(io_error)?;
    }

    // Discovery is complete and all directory iterators have closed. Every
    // parent row precedes its children, so reverse order removes children first.
    // No mutation is performed in the stable-directory observation callback.
    for row in plan.rows.iter().rev().copied() {
        check(active, scope)?;
        let relative = Path::new(plan.path(row));
        let current = root.lstat(relative).map_err(io_error)?;
        let kind = admitted_kind(&current)?;
        if current.st_dev as u64 != row.device
            || current.st_ino as u64 != row.inode
            || matches!(kind, Kind::Directory) != matches!(row.kind, Kind::Directory)
        {
            return Err(io_error("rooted deletion tree changed after discovery"));
        }
        match row.kind {
            Kind::Directory => root.remove_directory(relative),
            Kind::Leaf => root.unlink(relative),
        }
        .map_err(io_error)?;
        check(active, scope)?;
    }
    check(active, scope)?;
    Ok(true)
}

#[cfg(test)]
#[path = "rooted/tests.rs"]
mod tests;
