use std::fs::File;
use std::fs::Metadata;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::io::{self};
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use nix::fcntl::OFlag;
use nix::fcntl::RenameFlags;
use nix::fcntl::openat;
use nix::fcntl::renameat2;
use nix::sys::stat::Mode;
use nix::sys::stat::SFlag;
use nix::sys::stat::fchmod;
use nix::unistd::UnlinkatFlags;
use nix::unistd::unlinkat;

use super::PublicationOutcome;
use super::StablePublication;
use crate::runtime::native_output_file::Entry;
use crate::runtime::native_output_file::Identity;
use crate::runtime::native_output_file::directory;
use crate::runtime::native_output_file::entry;
use crate::runtime::native_output_file::fd_path;
use crate::runtime::native_output_file::identity;

mod receipt;
pub(super) use receipt::PublishedStatusFiles;

const NAMES: [&str; 2] = ["stable-status.txt", "volatile-status.txt"];
const MAX_STABLE: usize = 43 + 4096 + 4096;
const MAX_VOLATILE: usize = 69;

// atime may change on our bounded read; all other retained facts must remain current.
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
    let m: Metadata = file.metadata()?;
    if !m.is_file() || m.nlink() != 1 || m.mode() & 0o444 == 0 {
        return Err(io::Error::other(
            "status file must be readable regular and unaliased",
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
fn open_workspace(path: &Path) -> io::Result<File> {
    File::options()
        .read(true)
        .custom_flags(nix::libc::O_DIRECTORY | nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(path)
}
fn regular(parent: &File, name: &str, expected: Entry) -> io::Result<File> {
    if expected.kind != SFlag::S_IFREG {
        return Err(io::Error::other("status destination is not regular"));
    }
    let file = File::from(openat(
        parent,
        name,
        OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    )?);
    if identity(&file)? != expected.identity || entry(parent, name)? != Some(expected) {
        return Err(io::Error::other("status file name/descriptor changed"));
    }
    fingerprint(&file)?;
    Ok(file)
}
fn current(parent: &File, name: &str) -> io::Result<Option<Fingerprint>> {
    entry(parent, name)?
        .map(|e| fingerprint(&regular(parent, name, e)?))
        .transpose()
}
// Streaming equality reads at most desired length + one. No whole-file heap copy.
fn equal(file: &mut File, desired: &[u8]) -> io::Result<bool> {
    file.seek(SeekFrom::Start(0))?;
    let mut scratch = [0u8; 1024];
    let mut cursor = 0;
    while cursor < desired.len() {
        let size = scratch.len().min(desired.len() - cursor);
        let mut filled = 0;
        while filled < size {
            let n = file.read(&mut scratch[filled..size])?;
            if n == 0 {
                return Ok(false);
            }
            filled += n;
        }
        if scratch[..size] != desired[cursor..cursor + size] {
            return Ok(false);
        }
        cursor += size;
    }
    Ok(file.read(&mut scratch[..1])? == 0)
}

#[derive(Debug)]
struct Stage {
    parent: File,
    file: File,
    name: String,
    cleanup: Option<Entry>,
    sealed: Option<Fingerprint>,
}
impl Stage {
    fn new(parent: &File, bytes: &[u8]) -> io::Result<Self> {
        let parent = parent.try_clone()?;
        let temp = tempfile::Builder::new()
            .prefix(".slug-status-stage-")
            .disable_cleanup(true)
            .tempfile_in(fd_path(&parent))?;
        // Until the FD identity is known there is no authority to unlink a name.
        // An identity-read failure leaves an unclaimed file rather than deleting
        // a potentially substituted entry. Automatic pathname cleanup is disabled.
        let owned = Entry {
            identity: identity(temp.as_file())?,
            kind: SFlag::S_IFREG,
        };
        let name = temp
            .path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let (file, _) = temp.keep().map_err(|e| e.error)?;
        // Install conditional ownership before any write, sync or chmod can fail.
        let mut stage = Self {
            parent,
            file,
            name,
            cleanup: Some(owned),
            sealed: None,
        };
        if entry(&stage.parent, &stage.name)? != stage.cleanup {
            return Err(io::Error::other("status stage creation changed"));
        }
        stage.file.write_all(bytes)?;
        stage.file.sync_all()?;
        fchmod(&stage.file, Mode::from_bits_truncate(0o444))?;
        stage.sealed = Some(fingerprint(&stage.file)?);
        Ok(stage)
    }
    fn check(&mut self, bytes: &[u8]) -> io::Result<()> {
        if entry(&self.parent, &self.name)? != self.cleanup
            || Some(fingerprint(&self.file)?) != self.sealed
            || !equal(&mut self.file, bytes)?
            || Some(fingerprint(&self.file)?) != self.sealed
        {
            return Err(io::Error::other("sealed status stage changed"));
        }
        Ok(())
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        if self.cleanup.is_some() && entry(&self.parent, &self.name).ok().flatten() == self.cleanup
        {
            let _ = unlinkat(&self.parent, self.name.as_str(), UnlinkatFlags::NoRemoveDir);
        }
    }
}

#[derive(Debug)]
pub(super) struct Staging {
    workspace: File,
    bazel: File,
    previous: [Option<Fingerprint>; 2],
    stages: [Stage; 2],
    attempted: bool,
    #[cfg(test)]
    pub(super) fail_second: bool,
}
impl Staging {
    pub(super) fn new(workspace: &Path, stable: &[u8], volatile: &[u8]) -> io::Result<Self> {
        if stable.len() > MAX_STABLE || volatile.len() > MAX_VOLATILE {
            return Err(io::Error::other("status producer size bound exceeded"));
        }
        let workspace = open_workspace(workspace)?;
        let bazel = directory(&workspace, "bazel-out", true)?;
        // Refuse both destinations before making a provisional file.
        let previous = [current(&bazel, NAMES[0])?, current(&bazel, NAMES[1])?];
        let stages = [Stage::new(&bazel, stable)?, Stage::new(&bazel, volatile)?];
        Ok(Self {
            workspace,
            bazel,
            previous,
            stages,
            attempted: false,
            #[cfg(test)]
            fail_second: false,
        })
    }
    fn check_roots(&self, workspace: &Path) -> io::Result<()> {
        if identity(&open_workspace(workspace)?)? != identity(&self.workspace)?
            || identity(&directory(&self.workspace, "bazel-out", false)?)? != identity(&self.bazel)?
        {
            return Err(io::Error::other("status publication parent changed"));
        }
        Ok(())
    }
    fn check_destination(&self, index: usize) -> io::Result<()> {
        if current(&self.bazel, NAMES[index])? != self.previous[index] {
            return Err(io::Error::other(
                "status destination changed during staging",
            ));
        }
        Ok(())
    }
    pub(super) fn publish(
        &mut self,
        workspace: &Path,
        stable: &[u8],
        volatile: &[u8],
    ) -> io::Result<PublicationOutcome> {
        if self.attempted {
            return Err(io::Error::other("status stage already attempted"));
        }
        self.check_roots(workspace)?;
        let bytes = [stable, volatile];
        // Both roles preflight before the first visible replacement.
        for (i, bytes) in bytes.iter().enumerate() {
            self.check_destination(i)?;
            self.stages[i].check(bytes)?;
        }
        let preserve = if let Some(saved) = self.previous[0] {
            let e = Entry {
                identity: saved.identity,
                kind: SFlag::S_IFREG,
            };
            let mut file = regular(&self.bazel, NAMES[0], e)?;
            let same = equal(&mut file, stable)?;
            if fingerprint(&file)? != saved {
                return Err(io::Error::other("stable status changed during comparison"));
            }
            self.check_destination(0)?;
            same
        } else {
            false
        };
        self.attempted = true;
        for i in 0..2 {
            self.check_roots(workspace)?;
            self.check_destination(i)?;
            self.stages[i].check(bytes[i])?;
            if i == 0 && preserve {
                continue;
            }
            #[cfg(test)]
            if i == 1 && self.fail_second {
                return Err(io::Error::other("injected second status rename failure"));
            }
            let stage = &mut self.stages[i];
            renameat2(
                &self.bazel,
                stage.name.as_str(),
                &self.bazel,
                NAMES[i],
                if self.previous[i].is_some() {
                    RenameFlags::RENAME_EXCHANGE
                } else {
                    RenameFlags::RENAME_NOREPLACE
                },
            )?;
            stage.cleanup = self.previous[i].map(|f| Entry {
                identity: f.identity,
                kind: SFlag::S_IFREG,
            });
        }
        Ok(PublicationOutcome {
            stable: if preserve {
                StablePublication::Preserved
            } else {
                StablePublication::Replaced
            },
        })
    }
    pub(super) fn capture_files(
        &self,
        workspace: &Path,
        outcome: PublicationOutcome,
        lengths: [usize; 2],
    ) -> io::Result<PublishedStatusFiles> {
        PublishedStatusFiles::capture(self, workspace, outcome, lengths)
    }
    #[cfg(test)]
    pub(super) fn names(&self) -> [&str; 2] {
        [&self.stages[0].name, &self.stages[1].name]
    }
}
