//! Inactive FD-backed archive terminal manifest. Display paths are semantic
//! spellings for the inherited absolute-path/frontier accounting only. They
//! never reach filesystem IO; every operation uses RootIo and a relative path.

use std::io;
use std::os::unix::ffi::OsStrExt;

use super::*;
use crate::runtime::repository_root_io::RootIo;

pub(super) fn final_manifest_at(
    root: &RootIo<'_>,
    display_root: &Path,
    active: &dyn Fn() -> bool,
    scope: Option<&Arc<SourceServiceScope>>,
) -> Result<Arc<str>, ArchiveMaterializationError> {
    capture_at(root, display_root, active, scope, LIMITS)
}

fn capture_at(
    root: &RootIo<'_>,
    display_root: &Path,
    active: &dyn Fn() -> bool,
    scope: Option<&Arc<SourceServiceScope>>,
    limits: Limits,
) -> Result<Arc<str>, ArchiveMaterializationError> {
    check(active, scope)?;
    if !display_root.is_absolute() || display_root.as_os_str().len() > PATH_LIMIT {
        return Err(spec(
            "archive manifest root path exceeds budget or is not absolute",
        ));
    }
    // Reuse the exact old frontier, including cumulative PathBuf capacities,
    // Vec growth charges, discovered root/child count and reserved path scratch.
    // Its absolute PathBufs are display-only; no ambient path is ever opened.
    let _frontier_credit = source_service::buffer(scope, limits.paths).map_err(io_error)?;
    // The shared file kernel uses64KiB; RootIo directory enumeration uses8KiB.
    // Those phases do not overlap; the existing credit covers either scratch.
    let _buffer_credit = source_service::buffer(scope, 64 * 1024).map_err(io_error)?;
    let mut frontier = Frontier {
        paths: Vec::new(),
        discovered: 0,
        charged: 0,
        limits,
    };
    frontier.push(display_root.to_path_buf())?;
    let mut digest = Sha256::new();
    digest.update(b"slug.verified-generated-archive-output.v1\0");
    let mut remaining = limits.bytes;
    while let Some(display_path) = frontier.paths.pop() {
        check(active, scope)?;
        let relative = display_path.strip_prefix(display_root).map_err(io_error)?;
        let metadata = if relative.as_os_str().is_empty() {
            root.root_stat()
        } else {
            root.lstat(relative)
        }
        .map_err(io_error)?;
        admit_node(&metadata)?;
        let text = relative
            .to_str()
            .ok_or_else(|| spec("non-Unicode archive output path"))?;
        digest.update((text.len() as u64).to_be_bytes());
        digest.update(text.as_bytes());
        digest.update((metadata.st_mode & 0o777).to_be_bytes());
        if metadata.st_mode & libc::S_IFMT == libc::S_IFDIR {
            digest.update(b"dir\0");
            let start = frontier.paths.len();
            let mut visitor_error = None;
            let rooted_active = || active() && source_service::check(scope).is_ok();
            let listed = root.read_directory(relative, &rooted_active, |name, child_metadata| {
                let result = (|| {
                    check(active, scope)?;
                    if frontier.discovered == limits.entries {
                        return Err(spec("archive output manifest exceeds entry budget"));
                    }
                    let child_length = display_path
                        .as_os_str()
                        .len()
                        .checked_add(1)
                        .and_then(|length| length.checked_add(name.as_bytes().len()))
                        .ok_or_else(|| spec("archive manifest child path exceeds budget"))?;
                    if name.as_bytes().len() > 255 || child_length > PATH_LIMIT {
                        return Err(spec("archive manifest child path exceeds budget"));
                    }
                    if name.to_str().is_none() {
                        return Err(spec("non-Unicode archive output path"));
                    }
                    admit_node(child_metadata)?;
                    // Only bounded relative names produce display-only joins.
                    // PATH_SCRATCH prepays the temporary joined allocation;
                    // Frontier::push then charges its actual retained capacity.
                    frontier.push(display_path.join(name))
                })();
                match result {
                    Ok(()) => Ok(()),
                    Err(error) => {
                        visitor_error = Some(error);
                        Err(io::Error::other(
                            "rooted manifest directory visitor refused",
                        ))
                    }
                }
            });
            if let Some(error) = visitor_error {
                return Err(error);
            }
            check(active, scope)?;
            listed.map_err(io_error)?;
            if !unchanged(&metadata, &root.lstat(relative).map_err(io_error)?) {
                return Err(spec("archive manifest directory changed during capture"));
            }
            // Same DFS order and comparison as the existing Path lane.
            frontier.paths[start..].sort_unstable_by(|a, b| b.cmp(a));
        } else {
            digest.update(b"file\0");
            let size = u64::try_from(metadata.st_size)
                .map_err(|_| spec("archive manifest file has a negative length"))?;
            digest.update(size.to_be_bytes());
            if size > remaining {
                return Err(spec("archive output manifest exceeds byte budget"));
            }
            remaining -= size;
            stream_regular(root, relative, &metadata, size, &mut digest, active, scope)?;
        }
    }
    check(active, scope)?;
    Ok(format!(
        "verified-generated-archive:{}",
        hex::encode(digest.finalize())
    )
    .into())
}

fn stream_regular(
    root: &RootIo<'_>,
    relative: &Path,
    framed: &libc::stat,
    size: u64,
    digest: &mut Sha256,
    active: &dyn Fn() -> bool,
    scope: Option<&Arc<SourceServiceScope>>,
) -> Result<(), ArchiveMaterializationError> {
    check(active, scope)?;
    root.admit_file_size(size).map_err(io_error)?;
    let mut file = root.open_read_regular(relative).map_err(io_error)?;
    let opened = nix::sys::stat::fstat(&file).map_err(io_error)?;
    if !unchanged(framed, &opened) {
        return Err(spec("archive manifest file changed before body capture"));
    }
    // Reuse the existing exact-sized stream/EOF kernel and feed raw bodies into
    // the same global SHA framing. No second traversal, retained byte copy or
    // redundant per-file SHA computation is introduced. Scratch is live only
    // in this file phase, never during the directory-enumeration callback.
    let mut buffer = [0u8; 64 * 1024];
    super::read_file(&mut file, size, &mut buffer, digest, active, scope)?;
    let after = nix::sys::stat::fstat(&file).map_err(io_error)?;
    let named = root.lstat(relative).map_err(io_error)?;
    if !unchanged(framed, &after) || !unchanged(framed, &named) {
        return Err(spec("archive manifest file changed during body capture"));
    }
    check(active, scope)
}

fn admit_node(metadata: &libc::stat) -> Result<(), ArchiveMaterializationError> {
    match metadata.st_mode & libc::S_IFMT {
        libc::S_IFDIR => Ok(()),
        libc::S_IFREG if metadata.st_nlink == 1 => Ok(()),
        libc::S_IFREG => Err(spec(
            "regular hardlinks are unsupported in rooted archive manifests",
        )),
        _ => Err(spec("unsupported archive output manifest entry")),
    }
}

fn unchanged(before: &libc::stat, after: &libc::stat) -> bool {
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

#[cfg(test)]
#[path = "rooted/tests.rs"]
mod tests;
