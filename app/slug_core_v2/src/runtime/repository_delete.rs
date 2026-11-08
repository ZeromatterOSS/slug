//! Bounded mutations of a command-private repository tree. No host paths or
//! symlink traversal are admitted; every failure discards the enclosing prefix.
use std::fs;
use std::path::Path;
use std::sync::Arc;

use super::ArchiveMaterializationError;
use super::SourceServiceScope;
use super::source_service;

const MAX_ENTRIES: usize = 131_072;
const MAX_PATH_BYTES: usize = 4 * 1024 * 1024;
const MAX_DEPTH: usize = 256;
// Conservative live frontier envelope: at most256 relative strings and native
// directory handles, each path <=4096 bytes, plus joined-path/stack scratch.
// This is modeled source-service credit, not a whole-process RSS assertion.
const FRONTIER_CREDIT: usize = MAX_PATH_BYTES + 64 * 1024;

#[derive(Default)]
pub(in crate::runtime) struct DeleteWork {
    entries: usize,
    path_bytes: usize,
}

impl DeleteWork {
    fn visit(&mut self, bytes: usize) -> Result<(), ArchiveMaterializationError> {
        let entries = self.entries.checked_add(1).filter(|n| *n <= MAX_ENTRIES);
        let paths = self
            .path_bytes
            .checked_add(bytes)
            .filter(|n| *n <= MAX_PATH_BYTES);
        let (Some(entries), Some(paths)) = (entries, paths) else {
            return Err(spec(
                "repository deletion exceeds per-invocation work limits",
            ));
        };
        self.entries = entries;
        self.path_bytes = paths;
        Ok(())
    }
}

struct Directory {
    relative: String,
    children: fs::ReadDir,
}

pub(super) fn delete(
    root: &Path,
    relative: &str,
    work: &mut DeleteWork,
    active: &dyn Fn() -> bool,
    scope: Option<&Arc<SourceServiceScope>>,
) -> Result<bool, ArchiveMaterializationError> {
    if !cfg!(target_os = "linux") {
        return Err(spec(
            "repository deletion is currently supported only on Linux",
        ));
    }
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
            "repository deletion requires a normalized nonroot relative path",
        ));
    }
    check(active, scope)?;
    let _frontier_credit = source_service::buffer(scope, FRONTIER_CREDIT)
        .map_err(ArchiveMaterializationError::materialization)?;
    // Check every ancestor without following symlinks. A missing ancestor proves
    // absence; a non-directory ancestor is an I/O error, not an absent target.
    for (index, byte) in relative.bytes().enumerate() {
        if byte != b'/' {
            continue;
        }
        check(active, scope)?;
        let ancestor = &relative[..index];
        work.visit(ancestor.len())?;
        match fs::symlink_metadata(root.join(ancestor)) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(spec(
                    "repository deletion symlink ancestors are unsupported",
                ));
            }
            Ok(meta) if !meta.is_dir() => {
                return Err(io_error("repository deletion has a non-directory ancestor"));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(io_error(error)),
        }
    }
    check(active, scope)?;
    work.visit(relative.len())?;
    let target = root.join(relative);
    let meta = match fs::symlink_metadata(&target) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(io_error(error)),
    };
    if meta.file_type().is_symlink() {
        return Err(spec("repository deletion of symlinks is unsupported"));
    }
    if meta.is_file() {
        check(active, scope)?;
        fs::remove_file(&target).map_err(io_error)?;
        check(active, scope)?;
        return Ok(true);
    }
    if !meta.is_dir() {
        return Err(spec("repository deletion of special files is unsupported"));
    }
    let mut stack = Vec::with_capacity(MAX_DEPTH);
    stack.push(Directory {
        relative: relative.to_owned(),
        children: fs::read_dir(&target).map_err(io_error)?,
    });
    while !stack.is_empty() {
        check(active, scope)?;
        let frame = stack.last_mut().expect("nonempty directory stack");
        match frame.children.next() {
            Some(entry) => {
                let entry = entry.map_err(io_error)?;
                let name = entry.file_name();
                let name = name
                    .to_str()
                    .ok_or_else(|| spec("repository deletion path is not Unicode"))?;
                let length = frame
                    .relative
                    .len()
                    .checked_add(1)
                    .and_then(|n| n.checked_add(name.len()))
                    .filter(|n| *n <= 4096)
                    .ok_or_else(|| spec("repository deletion path exceeds limit"))?;
                work.visit(length)?;
                // The complete path work is charged before allocating a joined path.
                let mut child = String::with_capacity(length);
                child.push_str(&frame.relative);
                child.push('/');
                child.push_str(name);
                let path = root.join(&child);
                let meta = fs::symlink_metadata(&path).map_err(io_error)?;
                if meta.file_type().is_symlink() {
                    return Err(spec("repository deletion of symlinks is unsupported"));
                }
                check(active, scope)?;
                if meta.is_dir() {
                    if child.split('/').count() > MAX_DEPTH {
                        return Err(spec("repository deletion depth exceeds limit"));
                    }
                    let children = fs::read_dir(&path).map_err(io_error)?;
                    stack.push(Directory {
                        relative: child,
                        children,
                    });
                } else if meta.is_file() {
                    fs::remove_file(path).map_err(io_error)?;
                } else {
                    return Err(spec("repository deletion of special files is unsupported"));
                }
            }
            None => {
                let directory = stack.pop().expect("nonempty directory stack");
                // Close the directory handle before removal.
                drop(directory.children);
                check(active, scope)?;
                fs::remove_dir(root.join(directory.relative)).map_err(io_error)?;
            }
        }
    }
    check(active, scope)?;
    Ok(true)
}

fn check(
    active: &dyn Fn() -> bool,
    scope: Option<&Arc<SourceServiceScope>>,
) -> Result<(), ArchiveMaterializationError> {
    if !active() {
        return Err(spec(
            "repository deletion command/session is no longer active",
        ));
    }
    source_service::check(scope).map_err(ArchiveMaterializationError::materialization)
}
fn spec(message: &str) -> ArchiveMaterializationError {
    ArchiveMaterializationError::spec(message)
}
fn io_error(error: impl std::fmt::Display) -> ArchiveMaterializationError {
    ArchiveMaterializationError::materialization(error.to_string())
}

#[cfg(target_os = "linux")]
#[path = "repository_delete/rooted.rs"]
mod rooted;

/// General workspace route; the existing Path consumer remains unchanged.
#[cfg(target_os = "linux")]
pub(in crate::runtime) fn delete_at(
    root: &crate::runtime::repository_root_io::RootIo<'_>,
    relative: &str,
    work: &mut DeleteWork,
    active: &dyn Fn() -> bool,
    scope: Option<&Arc<SourceServiceScope>>,
) -> Result<bool, ArchiveMaterializationError> {
    rooted::delete_at(root, relative, work, active, scope)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn delete_walk_returns_actual_presence_and_refuses_non_directory_or_symlink_ancestors() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("nested/inside")).unwrap();
        fs::write(root.path().join("nested/inside/file"), b"payload").unwrap();
        let mut work = DeleteWork::default();
        assert!(delete(root.path(), "nested", &mut work, &|| true, None).unwrap());
        assert!(!delete(root.path(), "nested", &mut work, &|| true, None).unwrap());
        fs::write(root.path().join("plain"), b"data").unwrap();
        assert!(
            delete(root.path(), "plain/child", &mut work, &|| true, None)
                .unwrap_err()
                .message
                .contains("non-directory")
        );
        assert!(root.path().join("plain").exists());
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("untouched"), b"outside").unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();
        assert!(delete(root.path(), "link/untouched", &mut work, &|| true, None).is_err());
        assert_eq!(
            fs::read(outside.path().join("untouched")).unwrap(),
            b"outside"
        );
    }

    #[test]
    fn delete_work_limits_accumulate_without_refund_and_cancellation_stops_io() {
        let root = tempfile::tempdir().unwrap();
        let mut work = DeleteWork {
            entries: MAX_ENTRIES - 1,
            path_bytes: 0,
        };
        assert!(!delete(root.path(), "absent", &mut work, &|| true, None).unwrap());
        assert!(
            delete(root.path(), "another", &mut work, &|| true, None)
                .unwrap_err()
                .message
                .contains("work limits")
        );
        let mut work = DeleteWork {
            entries: 0,
            path_bytes: MAX_PATH_BYTES - "absent".len(),
        };
        assert!(!delete(root.path(), "absent", &mut work, &|| true, None).unwrap());
        assert!(delete(root.path(), "one", &mut work, &|| true, None).is_err());
        fs::write(root.path().join("present"), b"unchanged").unwrap();
        assert!(
            delete(
                root.path(),
                "present",
                &mut DeleteWork::default(),
                &|| false,
                None
            )
            .is_err()
        );
        assert_eq!(fs::read(root.path().join("present")).unwrap(), b"unchanged");
        for path in ["", ".", "../host", "/outside", "a//b", "a/./b"] {
            assert!(
                delete(
                    root.path(),
                    path,
                    &mut DeleteWork::default(),
                    &|| true,
                    None
                )
                .is_err()
            );
        }
    }
    #[test]
    fn scoped_delete_frontier_credit_releases_on_success_error_and_cancellation() {
        use std::time::Duration;
        use std::time::Instant;

        use crate::runtime::dice::source_service::SourceServiceLimits;
        use crate::runtime::dice::source_service::SourceServiceScope;
        let root = tempfile::tempdir().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let scope = SourceServiceScope::new_for_test(
            slug_workspace_v2::NormalizedAbsolutePath::new(root.path()).unwrap(),
            deadline,
            deadline + Duration::from_secs(1),
            SourceServiceLimits::default(),
        )
        .unwrap();
        fs::write(root.path().join("file"), b"payload").unwrap();
        assert!(
            delete(
                root.path(),
                "file",
                &mut DeleteWork::default(),
                &|| true,
                Some(&scope)
            )
            .unwrap()
        );
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
        assert_eq!(scope.snapshot().peak_buffer_bytes, FRONTIER_CREDIT);
        fs::write(root.path().join("block"), b"unchanged").unwrap();
        assert!(
            delete(
                root.path(),
                "block/child",
                &mut DeleteWork::default(),
                &|| true,
                Some(&scope)
            )
            .is_err()
        );
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
        scope.cancel();
        assert!(
            delete(
                root.path(),
                "block",
                &mut DeleteWork::default(),
                &|| true,
                Some(&scope)
            )
            .is_err()
        );
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
        assert_eq!(fs::read(root.path().join("block")).unwrap(), b"unchanged");
    }
}
