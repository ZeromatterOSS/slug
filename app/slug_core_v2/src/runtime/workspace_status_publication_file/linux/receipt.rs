//! Fixed readonly correspondence issued only after both status roles publish.
use std::os::unix::fs::FileExt;

use super::*;

#[derive(Debug)]
pub(in crate::runtime::workspace_status_publication_file) struct PublishedStatusFiles {
    workspace: File,
    bazel: File,
    files: [File; 2],
    fingerprints: [Fingerprint; 2],
}
impl PublishedStatusFiles {
    pub(super) fn capture(
        stage: &Staging,
        workspace: &Path,
        outcome: PublicationOutcome,
        lengths: [usize; 2],
    ) -> io::Result<Self> {
        stage.check_roots(workspace)?;
        let expected = [
            if outcome.stable == StablePublication::Preserved {
                stage.previous[0].unwrap().identity
            } else {
                stage.stages[0].sealed.unwrap().identity
            },
            stage.stages[1].sealed.unwrap().identity,
        ];
        let open = |i: usize| {
            regular(
                &stage.bazel,
                NAMES[i],
                Entry {
                    identity: expected[i],
                    kind: SFlag::S_IFREG,
                },
            )
        };
        let files = [open(0)?, open(1)?];
        let fingerprints = [fingerprint(&files[0])?, fingerprint(&files[1])?];
        for i in 0..2 {
            if fingerprints[i].len != lengths[i] as u64 || !readonly(&files[i])? {
                return Err(io::Error::other("published status correspondence differs"));
            }
        }
        let result = Self {
            workspace: stage.workspace.try_clone()?,
            bazel: stage.bazel.try_clone()?,
            files,
            fingerprints,
        };
        result.check(workspace)?;
        Ok(result)
    }
    fn check(&self, workspace: &Path) -> io::Result<()> {
        if identity(&open_workspace(workspace)?)? != identity(&self.workspace)?
            || identity(&directory(&self.workspace, "bazel-out", false)?)? != identity(&self.bazel)?
        {
            return Err(io::Error::other("status input parent changed"));
        }
        for i in 0..2 {
            let expected = self.fingerprints[i];
            if entry(&self.bazel, NAMES[i])?
                != Some(Entry {
                    identity: expected.identity,
                    kind: SFlag::S_IFREG,
                })
                || fingerprint(&self.files[i])? != expected
                || !readonly(&self.files[i])?
            {
                return Err(io::Error::other("published status input changed"));
            }
        }
        Ok(())
    }
    pub(in crate::runtime::workspace_status_publication_file) fn read<'a>(
        &self,
        index: usize,
        workspace: &Path,
        expected: &[u8],
        scratch: &'a mut [u8],
    ) -> io::Result<&'a [u8]> {
        if index >= 2
            || expected.len() > [MAX_STABLE, MAX_VOLATILE][index]
            || scratch.len() <= expected.len()
            || self.fingerprints[index].len != expected.len() as u64
        {
            return Err(io::Error::other("status input read bound differs"));
        }
        self.check(workspace)?;
        let count = read_bounded(&self.files[index], &mut scratch[..expected.len() + 1])?;
        if count != expected.len() || scratch[..count] != *expected {
            return Err(io::Error::other("status input bytes differ"));
        }
        self.check(workspace)?;
        Ok(&scratch[..count])
    }
    #[cfg(test)]
    pub(in crate::runtime::workspace_status_publication_file) fn historical_bytes(
        &self,
        index: usize,
    ) -> io::Result<Vec<u8>> {
        let length = usize::try_from(self.fingerprints[index].len).map_err(io::Error::other)?;
        let mut scratch = [0; MAX_STABLE + 1];
        let count = read_bounded(&self.files[index], &mut scratch[..length + 1])?;
        if count != length {
            return Err(io::Error::other("historical status length changed"));
        }
        Ok(scratch[..count].to_vec())
    }
    #[cfg(test)]
    pub(in crate::runtime::workspace_status_publication_file) fn raw_fds(&self) -> [i32; 4] {
        use std::os::fd::AsRawFd;
        [
            self.workspace.as_raw_fd(),
            self.bazel.as_raw_fd(),
            self.files[0].as_raw_fd(),
            self.files[1].as_raw_fd(),
        ]
    }
    #[cfg(test)]
    pub(in crate::runtime::workspace_status_publication_file) fn readonly_flags(
        &self,
    ) -> io::Result<[i32; 2]> {
        Ok([flags(&self.files[0])?, flags(&self.files[1])?])
    }
    #[cfg(test)]
    pub(in crate::runtime::workspace_status_publication_file) fn identities(
        &self,
    ) -> io::Result<[(u64, u64); 2]> {
        let a = self.files[0].metadata()?;
        let b = self.files[1].metadata()?;
        Ok([(a.dev(), a.ino()), (b.dev(), b.ino())])
    }
}
fn flags(file: &File) -> io::Result<i32> {
    use nix::fcntl::FcntlArg;
    use nix::fcntl::fcntl;
    Ok(fcntl(file, FcntlArg::F_GETFL)?)
}
fn readonly(file: &File) -> io::Result<bool> {
    Ok(flags(file)? & nix::libc::O_ACCMODE == nix::libc::O_RDONLY)
}
fn read_bounded(file: &File, scratch: &mut [u8]) -> io::Result<usize> {
    let mut count = 0;
    while count < scratch.len() {
        match file.read_at(&mut scratch[count..], count as u64) {
            Ok(0) => break,
            Ok(n) => count += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(count)
}
