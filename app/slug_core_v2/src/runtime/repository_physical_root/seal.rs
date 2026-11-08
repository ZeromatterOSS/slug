//! Consuming verification against an already admitted immutable final state.
//! No second tree or file-body copy is built; trusted quiescence is prerequisite.
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use slug_bzlmod_v2::RepositoryWorkspaceNode;

use super::*;

pub(super) fn seal(
    mut builder: RepositoryPhysicalRootBuilder,
    realization: &RepositoryWorkspaceRealization,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<Arc<RepositoryPhysicalRoot>, PhysicalRootError> {
    check(scope, active)?;
    if realization.reservation().as_ref() != builder.reservation.as_ref()
        || realization.final_state().reservation().as_ref() != builder.reservation.as_ref()
    {
        return Err(PhysicalRootError::Reservation);
    }
    let state = realization.final_state();
    if state.mode() != builder.mode {
        return Err(PhysicalRootError::Mode);
    }
    // The native root prolongs the entire final allocation, including bodies.
    // This producer bound includes full foreign reservation guards and may
    // conservatively count shared file bodies repeatedly. No transcript kept.
    let retained = state
        .retained_storage_bytes()
        .map_err(|_| PhysicalRootError::Resource)?;
    builder
        .credit
        .bytes
        .try_grow(retained)
        .map_err(|_| PhysicalRootError::Resource)?;
    let _scratch = builder.resources.reserve(IO_SCRATCH, 3, 0)?;
    if backing_identity(&builder.file, builder.mode)? != builder.identity {
        return Err(PhysicalRootError::RootIdentity);
    }
    let io = RootIo::new(&builder.file, io_limits(builder.mode, &builder.reservation))?;
    verify(&io, state, scope, active)?;
    if backing_identity(&builder.file, builder.mode)? != builder.identity {
        return Err(PhysicalRootError::RootIdentity);
    }
    check(scope, active)?;
    let anchor = MaterializationPathAnchor::new(
        PathObservationNamespace::Materialization(builder.reservation.instance()),
        builder.reservation.display_root().clone(),
    )
    .map_err(|_| PhysicalRootError::Reservation)?;
    Ok(Arc::new(RepositoryPhysicalRoot {
        file: builder.file,
        state: state.clone(),
        reservation: builder.reservation,
        identity: builder.identity,
        mode: builder.mode,
        anchor,
        resources: builder.resources,
        credit: builder.credit,
    }))
}

fn verify(
    io: &RootIo<'_>,
    state: &RepositoryWorkspaceState,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<(), PhysicalRootError> {
    let running = || active() && scope.check().is_ok();
    let mut joined = [0u8; 4096];
    let mut link = [0u8; 4097];
    let mut observed_entries = 1usize;
    for entry in state.entries() {
        check(scope, active)?;
        let path = Path::new(entry.path());
        let before = io.lstat(path)?;
        check(scope, active)?;
        if (before.st_mode & 0o7777) as u16 != entry.mode() {
            return Err(PhysicalRootError::Snapshot);
        }
        match entry.node() {
            RepositoryWorkspaceNode::Directory => {
                if before.st_mode & libc::S_IFMT != libc::S_IFDIR {
                    return Err(PhysicalRootError::Snapshot);
                }
                let mut visitor_error = None;
                let listed = io.read_directory(path, &running, |name, stat| {
                    let result = (|| {
                        check(scope, active)?;
                        let prefix = entry.path().as_bytes();
                        let slash = usize::from(!prefix.is_empty());
                        let length = add(add(prefix.len(), slash)?, name.as_bytes().len())?;
                        if length > joined.len() {
                            return Err(PhysicalRootError::Snapshot);
                        }
                        joined[..prefix.len()].copy_from_slice(prefix);
                        if slash != 0 {
                            joined[prefix.len()] = b'/';
                        }
                        joined[prefix.len() + slash..length].copy_from_slice(name.as_bytes());
                        let text = std::str::from_utf8(&joined[..length])
                            .map_err(|_| PhysicalRootError::Snapshot)?;
                        let expected = state.entry(text).ok_or(PhysicalRootError::Snapshot)?;
                        if (stat.st_mode & 0o7777) as u16 != expected.mode()
                            || !same_kind(expected.node(), stat)
                        {
                            return Err(PhysicalRootError::Snapshot);
                        }
                        observed_entries = observed_entries
                            .checked_add(1)
                            .filter(|n| *n <= state.entries().len())
                            .ok_or(PhysicalRootError::Snapshot)?;
                        Ok(())
                    })();
                    match result {
                        Ok(()) => Ok(()),
                        Err(error) => {
                            visitor_error = Some(error);
                            Err(io::Error::other(
                                "physical root directory differs from final state",
                            ))
                        }
                    }
                });
                if let Some(error) = visitor_error {
                    return Err(error);
                }
                check(scope, active)?;
                listed?;
                if !unchanged(&before, &io.lstat(path)?) {
                    return Err(PhysicalRootError::Snapshot);
                }
            }
            RepositoryWorkspaceNode::File(body) => {
                if !same_kind(entry.node(), &before)
                    || before.st_size < 0
                    || before.st_size as u64 != body.len() as u64
                {
                    return Err(PhysicalRootError::Snapshot);
                }
                let mut offset = 0usize;
                let mut mismatch = false;
                let captured = io.read_file(path, &running, |chunk| {
                    let end = offset.checked_add(chunk.len());
                    if end.is_none_or(|end| end > body.len() || body[offset..end] != *chunk) {
                        mismatch = true;
                        return Err(io::Error::other(
                            "physical root file differs from final state",
                        ));
                    }
                    offset = end.unwrap();
                    Ok(())
                });
                check(scope, active)?;
                if mismatch {
                    return Err(PhysicalRootError::Snapshot);
                }
                let captured = captured?;
                if offset != body.len() || !unchanged(&before, &captured.metadata) {
                    return Err(PhysicalRootError::Snapshot);
                }
            }
            RepositoryWorkspaceNode::Link(target) => {
                if !same_kind(entry.node(), &before) {
                    return Err(PhysicalRootError::Snapshot);
                }
                let actual = io.read_link(path, &mut link)?;
                if actual != target.as_bytes() || !unchanged(&before, &io.lstat(path)?) {
                    return Err(PhysicalRootError::Snapshot);
                }
            }
        }
    }
    check(scope, active)?;
    if observed_entries != state.entries().len() {
        return Err(PhysicalRootError::Snapshot);
    }
    Ok(())
}

fn same_kind(expected: &RepositoryWorkspaceNode, actual: &libc::stat) -> bool {
    match expected {
        RepositoryWorkspaceNode::Directory => actual.st_mode & libc::S_IFMT == libc::S_IFDIR,
        RepositoryWorkspaceNode::File(_) => {
            actual.st_mode & libc::S_IFMT == libc::S_IFREG && actual.st_nlink == 1
        }
        RepositoryWorkspaceNode::Link(_) => actual.st_mode & libc::S_IFMT == libc::S_IFLNK,
    }
}

pub(super) fn unchanged(a: &libc::stat, b: &libc::stat) -> bool {
    a.st_dev == b.st_dev
        && a.st_ino == b.st_ino
        && a.st_mode == b.st_mode
        && a.st_nlink == b.st_nlink
        && a.st_size == b.st_size
        && a.st_mtime == b.st_mtime
        && a.st_mtime_nsec == b.st_mtime_nsec
        && a.st_ctime == b.st_ctime
        && a.st_ctime_nsec == b.st_ctime_nsec
}

#[cfg(test)]
#[path = "verification_tests.rs"]
mod verification_tests;
