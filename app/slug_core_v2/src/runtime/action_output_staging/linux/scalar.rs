//! Private nonexecutable singleton sealing; no public permission override.
use std::os::unix::fs::FileExt;

use dupe::Dupe;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::*;
use crate::runtime::native_output_file::Identity;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct Fingerprint {
    identity: Identity,
    len: u64,
    mode: u32,
    links: u64,
    mtime: (i64, i64),
    ctime: (i64, i64),
}
fn fingerprint(file: &File) -> io::Result<Fingerprint> {
    let m = file.metadata()?;
    if !m.is_file() || m.nlink() != 1 || m.mode() & 0o444 == 0 {
        return Err(io::Error::other(
            "header must be readable regular and unaliased",
        ));
    }
    Ok(Fingerprint {
        identity: identity(file)?,
        len: m.len(),
        mode: m.mode(),
        links: m.nlink(),
        mtime: (m.mtime(), m.mtime_nsec()),
        ctime: (m.ctime(), m.ctime_nsec()),
    })
}
fn previous(stage: &Staging, output: &ActionOutput) -> io::Result<Option<Fingerprint>> {
    let destination = &stage.destinations[0];
    let name = output.root_relative_path().rsplit('/').next().unwrap();
    if entry(&destination.parent, name)? != destination.previous {
        return Err(io::Error::other("header destination changed"));
    }
    let Some(saved) = destination.previous else {
        return Ok(None);
    };
    if saved.kind != SFlag::S_IFREG {
        return Err(io::Error::other("header destination must be regular"));
    }
    let file = File::from(openat(
        &destination.parent,
        name,
        OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    )?);
    let facts = fingerprint(&file)?;
    if facts.identity != saved.identity || entry(&destination.parent, name)? != Some(saved) {
        return Err(io::Error::other("header destination descriptor changed"));
    }
    Ok(Some(facts))
}
#[derive(Debug)]
pub(super) struct SealedScalar {
    workspace: NormalizedAbsolutePath,
    staged: Fingerprint,
    previous: Option<Fingerprint>,
}
impl Staging {
    pub(in crate::runtime::action_output_staging) fn seal_nonexecutable(
        &mut self,
        outputs: &[ActionOutput],
        workspace: &NormalizedAbsolutePath,
        expected: &[u8],
    ) -> io::Result<()> {
        let [output] = outputs else {
            return Err(io::Error::other("header requires singleton output"));
        };
        if self.sealed
            || self.attempted
            || self.alias.is_some()
            || self.runfiles.is_some()
            || output.kind() != ActionOutputKind::File
        {
            return Err(io::Error::other("header requires unsealed scalar File"));
        }
        let destination = &self.destinations[0];
        if !destination.created.load(Ordering::Relaxed)
            || entry(&destination.parent, &destination.name)? != destination.cleanup
        {
            return Err(io::Error::other("header stage missing or changed"));
        }
        fingerprint(&destination.staged)?;
        compare(&destination.staged, expected)?;
        destination.staged.sync_all()?;
        fchmod(&destination.staged, Mode::from_bits_truncate(0o444))?;
        self.scalar = Some(SealedScalar {
            workspace: workspace.dupe(),
            staged: fingerprint(&destination.staged)?,
            previous: previous(self, output)?,
        });
        self.check_scalar(outputs)?;
        self.preflight(outputs)?;
        self.sealed = true;
        Ok(())
    }
    pub(super) fn check_scalar(&self, outputs: &[ActionOutput]) -> io::Result<()> {
        let Some(facts) = &self.scalar else {
            return Ok(());
        };
        let [output] = outputs else {
            return Err(io::Error::other("header requires singleton output"));
        };
        let current_workspace = File::options()
            .read(true)
            .custom_flags(nix::libc::O_DIRECTORY | nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
            .open(facts.workspace.as_path())?;
        if identity(&current_workspace)? != identity(&self.workspace)? {
            return Err(io::Error::other("header workspace changed"));
        }
        drop(current_workspace);
        let destination = &self.destinations[0];
        if facts.staged.mode & 0o777 != 0o444
            || fingerprint(&destination.staged)? != facts.staged
            || entry(&destination.parent, &destination.name)? != destination.cleanup
            || previous(self, output)? != facts.previous
        {
            return Err(io::Error::other("sealed header correspondence changed"));
        }
        Ok(())
    }
    pub(in crate::runtime::action_output_staging) fn check_scalar_bytes(
        &self,
        outputs: &[ActionOutput],
        expected: &[u8],
    ) -> io::Result<()> {
        if self.scalar.is_none() {
            return Err(io::Error::other("header has no scalar seal"));
        }
        self.check_scalar(outputs)?;
        compare(&self.destinations[0].staged, expected)?;
        self.check_scalar(outputs)
    }
    #[cfg(test)]
    pub(in crate::runtime::action_output_staging) fn scalar_raw_fds(&self) -> [i32; 6] {
        use std::os::fd::AsRawFd;
        [
            self.workspace.as_raw_fd(),
            self.bazel.as_raw_fd(),
            self.configured.as_raw_fd(),
            self.bin.as_raw_fd(),
            self.destinations[0].parent.as_raw_fd(),
            self.destinations[0].staged.as_raw_fd(),
        ]
    }
    #[cfg(test)]
    pub(in crate::runtime::action_output_staging) fn scalar_stage_name(&self) -> &str {
        &self.destinations[0].name
    }
}
fn compare(file: &File, expected: &[u8]) -> io::Result<()> {
    if file.metadata()?.len() != expected.len() as u64 {
        return Err(io::Error::other("header length differs"));
    }
    let mut scratch = [0; 1024];
    let mut cursor = 0;
    while cursor < expected.len() {
        let end = expected.len().min(cursor + scratch.len());
        let mut filled = 0;
        while filled < end - cursor {
            let n = file.read_at(&mut scratch[filled..end - cursor], (cursor + filled) as u64)?;
            if n == 0 {
                return Err(io::Error::other("header EOF differs"));
            }
            filled += n;
        }
        if scratch[..filled] != expected[cursor..end] {
            return Err(io::Error::other("header bytes differ"));
        }
        cursor = end;
    }
    if file.read_at(&mut scratch[..1], cursor as u64)? != 0 {
        return Err(io::Error::other("header has extra bytes"));
    }
    Ok(())
}
