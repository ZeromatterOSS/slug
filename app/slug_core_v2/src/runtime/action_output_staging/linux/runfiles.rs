//! Core-only physical tree writer; links never enter generic transport staging.

use nix::fcntl::readlinkat;
use nix::unistd::symlinkat;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use super::*;

#[derive(Debug)]
pub(super) struct RunfilesTopology {
    entries: SmallMap<String, Option<String>>,
    directories: SmallSet<String>,
}

// Runfiles names are POSIX paths. Backslashes are literal filename bytes here;
// generic output-path validation deliberately keeps its existing policy.
fn components(path: &str) -> io::Result<Vec<&str>> {
    let parts = path.split('/').collect::<Vec<_>>();
    if parts.len() > 256
        || parts
            .iter()
            .any(|part| part.is_empty() || matches!(*part, "." | "..") || part.contains('\0'))
    {
        return Err(io::Error::other("noncanonical runfiles entry path"));
    }
    Ok(parts)
}

impl RunfilesTopology {
    fn new(workspace_name: &str, entries: &[(String, Option<String>)]) -> io::Result<Self> {
        let mut result = Self {
            entries: SmallMap::new(),
            directories: SmallSet::new(),
        };
        let workspace = components(workspace_name)?;
        for count in 1..=workspace.len() {
            result.directories.insert(workspace[..count].join("/"));
        }
        for (path, target) in entries {
            let parts = components(path)?;
            if let Some(target) = target
                && (!Path::new(target).is_absolute() || target.contains('\0'))
            {
                return Err(io::Error::other(
                    "runfiles link target must be an absolute path",
                ));
            }
            if result
                .entries
                .insert(path.clone(), target.clone())
                .is_some()
            {
                return Err(io::Error::other("duplicate runfiles entry"));
            }
            for count in 1..parts.len() {
                result.directories.insert(parts[..count].join("/"));
            }
        }
        if result
            .entries
            .keys()
            .any(|path| result.directories.contains(path))
        {
            return Err(io::Error::other(
                "runfiles leaf conflicts with a required directory",
            ));
        }
        Ok(result)
    }

    fn write(&self, root: &File) -> io::Result<()> {
        for path in &self.directories {
            walk(root, &components(path)?, true)?;
        }
        for (path, target) in &self.entries {
            let parts = components(path)?;
            let parent = walk(root, &parts[..parts.len() - 1], false)?;
            let name = *parts.last().unwrap();
            if let Some(target) = target {
                symlinkat(target.as_str(), &parent, name)?;
            } else {
                let _file = File::from(openat(
                    &parent,
                    name,
                    OFlag::O_WRONLY
                        | OFlag::O_CREAT
                        | OFlag::O_EXCL
                        | OFlag::O_NOFOLLOW
                        | OFlag::O_CLOEXEC,
                    Mode::from_bits_truncate(0o600),
                )?);
            }
        }
        Ok(())
    }

    pub(super) fn seal(&self, root: &File) -> io::Result<()> {
        // Only one directory descriptor per depth; symlink targets are never opened.
        struct RunfilesFrame {
            directory: File,
            path: String,
            children: std::vec::IntoIter<OsString>,
        }
        impl RunfilesFrame {
            fn new(directory: File, path: String) -> io::Result<Self> {
                let children = names(&directory)?.into_iter();
                Ok(Self {
                    directory,
                    path,
                    children,
                })
            }
        }
        let mut pending = vec![RunfilesFrame::new(root.try_clone()?, String::new())?];
        let mut leaves = 0;
        let mut directories = 0;
        while let Some(frame) = pending.last_mut() {
            if let Some(name) = frame.children.next() {
                let name = name
                    .to_str()
                    .ok_or_else(|| io::Error::other("unexpected non-Unicode runfiles entry"))?;
                let path = if frame.path.is_empty() {
                    name.to_owned()
                } else {
                    format!("{}/{name}", frame.path)
                };
                let stat = fstatat(&frame.directory, name, AtFlags::AT_SYMLINK_NOFOLLOW)?;
                match SFlag::from_bits_truncate(stat.st_mode) & SFlag::S_IFMT {
                    SFlag::S_IFDIR => {
                        if !self.directories.contains(&path) {
                            return Err(io::Error::other("unexpected runfiles directory"));
                        }
                        directories += 1;
                        let child = directory(&frame.directory, name, false)?;
                        pending.push(RunfilesFrame::new(child, path)?);
                    }
                    SFlag::S_IFLNK => {
                        let Some(Some(target)) = self.entries.get(&path) else {
                            return Err(io::Error::other("unexpected runfiles symlink"));
                        };
                        if readlinkat(&frame.directory, name)?.as_bytes() != target.as_bytes() {
                            return Err(io::Error::other(
                                "runfiles symlink target changed during staging",
                            ));
                        }
                        leaves += 1;
                    }
                    SFlag::S_IFREG => {
                        if !matches!(self.entries.get(&path), Some(None)) {
                            return Err(io::Error::other("unexpected runfiles regular file"));
                        }
                        let file = File::from(openat(
                            &frame.directory,
                            name,
                            OFlag::O_RDONLY
                                | OFlag::O_NONBLOCK
                                | OFlag::O_NOFOLLOW
                                | OFlag::O_CLOEXEC,
                            Mode::empty(),
                        )?);
                        let metadata = file.metadata()?;
                        if !metadata.is_file() || metadata.len() != 0 {
                            return Err(io::Error::other(
                                "runfiles empty file changed during staging",
                            ));
                        }
                        file.sync_all()?;
                        fchmod(&file, Mode::from_bits_truncate(0o444))?;
                        leaves += 1;
                    }
                    _ => return Err(io::Error::other("runfiles tree contains a special file")),
                }
            } else {
                frame.directory.sync_all()?;
                fchmod(&frame.directory, Mode::from_bits_truncate(0o755))?;
                pending.pop();
            }
        }
        if leaves != self.entries.len() || directories != self.directories.len() {
            return Err(io::Error::other(
                "runfiles tree is missing declared entries",
            ));
        }
        Ok(())
    }
}

/// Pure topology gate for whole-plan preflight, before backend or filesystem work.
pub(in crate::runtime::action_output_staging) fn validate_runfiles(
    workspace_name: &str,
    entries: &[(String, Option<String>)],
) -> io::Result<()> {
    RunfilesTopology::new(workspace_name, entries).map(|_| ())
}

impl Staging {
    pub(in crate::runtime::action_output_staging) fn new_runfiles(
        workspace: &Path,
        configuration: &SlugConfiguration,
        output: &ActionOutput,
        workspace_name: &str,
        entries: &[(String, Option<String>)],
        reserved: &SmallSet<&str>,
    ) -> io::Result<Self> {
        if output.kind() != ActionOutputKind::RunfilesTree {
            return Err(io::Error::other(
                "typed runfiles staging requires a RunfilesTree output",
            ));
        }
        let topology = RunfilesTopology::new(workspace_name, entries)?;
        // Reuse the physical directory allocation only. The wrapper retains the
        // original RunfilesTree declaration; no remote Directory result is made.
        let physical = ActionOutput::new(output.path(), ActionOutputKind::Directory);
        let mut staging = Self::new(workspace, configuration, &[physical], reserved)?;
        topology.write(&staging.destinations[0].staged)?;
        staging.destinations[0]
            .created
            .store(true, Ordering::Relaxed);
        staging.runfiles = Some(topology);
        Ok(staging)
    }
}

#[cfg(test)]
mod tests;
