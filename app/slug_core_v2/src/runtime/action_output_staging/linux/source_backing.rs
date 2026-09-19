//! Persistent verified backing for materialized source link targets.

use sha2::Digest;
use sha2::Sha256;
use slug_workspace_v2::FileContentDigest;
use starlark_map::small_set::SmallSet;

use super::*;

#[derive(Debug)]
pub(in crate::runtime::action_output_staging) struct SourceBacking {
    workspace: File,
    bazel: File,
    cache: File,
    version: File,
    name: String,
    target: PathBuf,
    destination: Destination,
    mode: u32,
    sealed: bool,
    attempted: bool,
}

fn mode(permissions: i32) -> io::Result<u32> {
    if permissions < 0 {
        return Err(io::Error::other(
            "source backing requires observed POSIX permissions",
        ));
    }
    Ok(0o444 | (permissions as u32 & 0o111))
}

impl SourceBacking {
    pub(in crate::runtime::action_output_staging) fn target_path(
        workspace: &Path,
        digest: FileContentDigest,
        permissions: i32,
    ) -> io::Result<PathBuf> {
        if !workspace.is_absolute() {
            return Err(io::Error::other(
                "source backing workspace must be absolute",
            ));
        }
        Ok(workspace
            .join("bazel-out/.slug-runfiles-sources/v1")
            .join(format!(
                "{}-{}-{:03o}",
                hex::encode(digest.sha256()),
                digest.size_bytes(),
                mode(permissions)?
            )))
    }

    pub(in crate::runtime::action_output_staging) fn new(
        workspace: &Path,
        digest: FileContentDigest,
        permissions: i32,
        source: &mut File,
    ) -> io::Result<Self> {
        let target = Self::target_path(workspace, digest, permissions)?;
        if !source.metadata()?.is_file() {
            return Err(io::Error::other(
                "source backing requires a regular observed source",
            ));
        }
        let workspace = File::options()
            .read(true)
            .custom_flags(nix::libc::O_DIRECTORY | nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
            .open(workspace)?;
        let bazel = directory(&workspace, "bazel-out", true)?;
        let cache = directory(&bazel, ".slug-runfiles-sources", true)?;
        let version = directory(&cache, "v1", true)?;
        let name = target.file_name().unwrap().to_str().unwrap().to_owned();
        let previous = entry(&version, &name)?;
        if previous.is_some_and(|entry| entry.kind != SFlag::S_IFREG) {
            return Err(io::Error::other(
                "source backing destination is not a regular file",
            ));
        }
        let mut destination = Destination::new(
            version.try_clone()?,
            previous,
            ActionOutputKind::File,
            &SmallSet::new(),
        )?;
        let mut hash = Sha256::new();
        let mut length = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = source.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            length = length
                .checked_add(read as u64)
                .ok_or_else(|| io::Error::other("source backing byte count overflow"))?;
            if length > digest.size_bytes() {
                return Err(io::Error::other("source backing digest or size mismatch"));
            }
            hash.update(&buffer[..read]);
            destination.staged.write_all(&buffer[..read])?;
        }
        let actual: [u8; 32] = hash.finalize().into();
        if length != digest.size_bytes() || &actual != digest.sha256() {
            return Err(io::Error::other("source backing digest or size mismatch"));
        }
        Ok(Self {
            workspace,
            bazel,
            cache,
            version,
            name,
            target,
            destination,
            mode: mode(permissions)?,
            sealed: false,
            attempted: false,
        })
    }

    pub(in crate::runtime::action_output_staging) fn target(&self) -> &Path {
        &self.target
    }

    pub(in crate::runtime::action_output_staging) fn seal(&mut self) -> io::Result<()> {
        if self.sealed || self.attempted {
            return Err(io::Error::other("source backing is already sealed"));
        }
        if entry(&self.destination.parent, &self.destination.name)? != self.destination.cleanup {
            return Err(io::Error::other(
                "source backing stage changed during copying",
            ));
        }
        self.destination.staged.sync_all()?;
        fchmod(
            &self.destination.staged,
            Mode::from_bits_truncate(self.mode),
        )?;
        self.sealed = true;
        self.preflight()
    }

    pub(in crate::runtime::action_output_staging) fn preflight(&self) -> io::Result<()> {
        if !self.sealed || self.attempted {
            return Err(io::Error::other(
                "source backing publication requires one sealed stage",
            ));
        }
        for (parent, name, expected) in [
            (&self.workspace, "bazel-out", &self.bazel),
            (&self.bazel, ".slug-runfiles-sources", &self.cache),
            (&self.cache, "v1", &self.version),
        ] {
            if identity(&directory(parent, name, false)?)? != identity(expected)? {
                return Err(io::Error::other(
                    "source backing namespace changed during staging",
                ));
            }
        }
        if entry(&self.version, &self.name)? != self.destination.previous {
            return Err(io::Error::other(
                "source backing destination changed during staging",
            ));
        }
        if entry(&self.destination.parent, &self.destination.name)? != self.destination.cleanup {
            return Err(io::Error::other(
                "source backing stage changed during staging",
            ));
        }
        Ok(())
    }

    pub(in crate::runtime::action_output_staging) fn publish(&mut self) -> io::Result<()> {
        self.preflight()?;
        self.attempted = true;
        let flags = if self.destination.previous.is_some() {
            RenameFlags::RENAME_EXCHANGE
        } else {
            RenameFlags::RENAME_NOREPLACE
        };
        renameat2(
            &self.destination.parent,
            self.destination.name.as_str(),
            &self.destination.parent,
            self.name.as_str(),
            flags,
        )?;
        self.destination.cleanup = self.destination.previous;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
