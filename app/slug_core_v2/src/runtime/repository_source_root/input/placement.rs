//! Verified Source replacements stay on the eventual backing. No permission
//! change or writable open of an existing archive inode is required.
use std::io::Write as _;
use std::path::PathBuf;

use super::*;
use crate::runtime::repository_root_io::WriteMode;
use crate::runtime::repository_root_io::unchanged;

const STAGING_SLOTS: usize = 8192;

pub(super) fn place_capture(
    capture: &Capture,
    io: &RootIo<'_>,
    path: &str,
    mode: u32,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<(), PhysicalRootError> {
    check(scope, active)?;
    // Admit the complete expected file before directory or staging mutation.
    io.admit_file_size(capture.frame().len())?;
    let target = Path::new(path);
    if let Some(parent) = target.parent() {
        io.ensure_directory(parent, 0o755)?;
    }
    let before = match io.lstat(target) {
        Ok(stat) if stat.st_mode & libc::S_IFMT == libc::S_IFREG && stat.st_nlink == 1 => {
            Some(stat)
        }
        Ok(_) => return Err(PhysicalRootError::Snapshot),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let Some(before) = before else {
        let mut output = io.open_write_regular(target, WriteMode::CreateNew)?;
        write_capture(capture, io, &mut output, mode, scope, active)?;
        drop(output);
        return check(scope, active);
    };
    // Root-level staging avoids inheriting a deep target's path length. Every
    // existing node kind is a collision, including links and special files.
    let mut staged = None;
    for index in 0..STAGING_SLOTS {
        check(scope, active)?;
        let name = PathBuf::from(format!(".slug-source-placement-{index:04x}"));
        if name == target {
            continue;
        }
        match io.lstat(&name) {
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        match io.open_write_regular(&name, WriteMode::CreateNew) {
            Ok(file) => {
                staged = Some((name, file));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    let (name, mut output) = staged.ok_or(PhysicalRootError::Resource)?;
    write_capture(capture, io, &mut output, mode, scope, active)?;
    check(scope, active)?;
    let current = io.lstat(target)?;
    if !unchanged(&before, &current) {
        return Err(PhysicalRootError::Snapshot);
    }
    io.replace_regular(&name, target, &output)?;
    // No writer survives into the final inventory/hash or Source publication.
    drop(output);
    check(scope, active)
}

fn write_capture(
    capture: &Capture,
    io: &RootIo<'_>,
    output: &mut File,
    mode: u32,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<(), PhysicalRootError> {
    let mut hash = Sha256::new();
    capture.read_chunks(scope, active, |chunk| {
        crate::runtime::repository_io::source_service::write_counted(
            output,
            chunk,
            active,
            Some(scope),
        )?;
        hash.update(chunk);
        Ok(())
    })?;
    if <[u8; 32]>::from(hash.finalize()) != capture.digest {
        return Err(PhysicalRootError::Snapshot);
    }
    io.chmod_regular(output, mode)?;
    output.flush()?;
    check(scope, active)
}
