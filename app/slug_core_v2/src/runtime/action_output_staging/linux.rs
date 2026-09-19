use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::io::{self};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use nix::dir::Dir;
use nix::errno::Errno;
use nix::fcntl::AtFlags;
use nix::fcntl::OFlag;
use nix::fcntl::RenameFlags;
use nix::fcntl::openat;
use nix::fcntl::renameat2;
use nix::sys::stat::Mode;
use nix::sys::stat::SFlag;
use nix::sys::stat::fchmod;
use nix::sys::stat::fstatat;
use nix::sys::stat::mkdirat;
use nix::unistd::UnlinkatFlags;
use nix::unistd::unlinkat;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_configuration_v2::SlugConfiguration;
use tempfile::NamedTempFile;

mod alias;
mod runfiles;
mod source_backing;
pub(super) use runfiles::validate_runfiles;
pub(super) use source_backing::SourceBacking;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct Identity(u64, u64);
fn identity(file: &File) -> io::Result<Identity> {
    let value = file.metadata()?;
    Ok(Identity(value.dev(), value.ino()))
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct Entry {
    identity: Identity,
    kind: SFlag,
}
fn entry(parent: &File, name: &str) -> io::Result<Option<Entry>> {
    match fstatat(parent, name, AtFlags::AT_SYMLINK_NOFOLLOW) {
        Ok(stat) => {
            let kind = SFlag::from_bits_truncate(stat.st_mode) & SFlag::S_IFMT;
            if kind != SFlag::S_IFREG && kind != SFlag::S_IFDIR && kind != SFlag::S_IFLNK {
                return Err(io::Error::other("output namespace contains special file"));
            }
            Ok(Some(Entry {
                identity: Identity(stat.st_dev, stat.st_ino),
                kind,
            }))
        }
        Err(Errno::ENOENT) => Ok(None),
        Err(error) => Err(error.into()),
    }
}
fn component_path(path: &str) -> io::Result<Vec<&str>> {
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() > 256
        || parts.iter().any(|part| {
            part.is_empty() || matches!(*part, "." | "..") || part.contains(['\\', '\0'])
        })
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "noncanonical output path",
        ));
    }
    Ok(parts)
}
fn directory(parent: &File, name: &str, create: bool) -> io::Result<File> {
    if create {
        match mkdirat(parent, name, Mode::from_bits_truncate(0o755)) {
            Ok(()) | Err(Errno::EEXIST) => {}
            Err(error) => return Err(error.into()),
        }
    }
    openat(
        parent,
        name,
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(Into::into)
}
fn walk(parent: &File, parts: &[&str], create: bool) -> io::Result<File> {
    let mut current = parent.try_clone()?;
    for part in parts {
        current = directory(&current, part, create)?;
    }
    Ok(current)
}
fn fd_path(file: &File) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()))
}

/// Configuration markers use the same canonical collision contract as the
/// old writer, but every parent and final read is descriptor-confined.
fn claim_marker(bazel: &File, projection: &str, canonical: &[u8]) -> io::Result<()> {
    let markers = directory(bazel, ".slug-configurations", true)?;
    let name = format!("{projection}.canonical");
    let mut temp = NamedTempFile::new_in(fd_path(&markers))?;
    temp.write_all(canonical)?;
    temp.as_file().sync_all()?;
    match temp.persist_noclobber(fd_path(&markers).join(&name)) {
        Ok(_) => markers.sync_all(),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
            let mut file = File::from(openat(
                &markers,
                name.as_str(),
                OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )?);
            if !file.metadata()?.is_file() {
                return Err(io::Error::other(
                    "configuration marker is not a regular file",
                ));
            }
            let mut bytes = Vec::new();
            (&mut file)
                .take(canonical.len() as u64 + 1)
                .read_to_end(&mut bytes)?;
            if bytes == canonical {
                Ok(())
            } else {
                Err(io::Error::other("configuration projection collision"))
            }
        }
        Err(error) => Err(error.error),
    }
}

/// A hidden direct sibling permits same-parent rename of read-only trees.
/// `cleanup` follows the retired entry after exchange, never the published FD.
#[derive(Debug)]
struct Destination {
    parent: File,
    previous: Option<Entry>,
    name: String,
    staged: File,
    cleanup: Option<Entry>,
    created: AtomicBool,
}
impl Destination {
    fn new(
        parent: File,
        previous: Option<Entry>,
        kind: ActionOutputKind,
        reserved: &starlark_map::small_set::SmallSet<&str>,
    ) -> io::Result<Self> {
        let mut builder = tempfile::Builder::new();
        builder.prefix(".slug-output-stage-");
        // Exclude declared components, including not-yet-created package parents.
        let (staged, name, owned) = loop {
            if kind == ActionOutputKind::Directory {
                let temp = builder.tempdir_in(fd_path(&parent))?;
                let name = temp
                    .path()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned();
                if reserved.contains(name.as_str()) {
                    continue;
                }
                let file = directory(&parent, &name, false)?;
                let owned = Entry {
                    identity: identity(&file)?,
                    kind: SFlag::S_IFDIR,
                };
                temp.keep();
                break (file, name, owned);
            } else {
                let temp = builder.tempfile_in(fd_path(&parent))?;
                let name = temp
                    .path()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned();
                if reserved.contains(name.as_str()) {
                    continue;
                }
                let owned = Entry {
                    identity: identity(temp.as_file())?,
                    kind: SFlag::S_IFREG,
                };
                let (file, _) = temp.keep().map_err(|error| error.error)?;
                break (file, name, owned);
            }
        };
        Ok(Self {
            parent,
            previous,
            name,
            staged,
            cleanup: Some(owned),
            created: AtomicBool::new(false),
        })
    }
}
impl Drop for Destination {
    fn drop(&mut self) {
        let Some(expected) = self.cleanup else {
            return;
        };
        if !matches!(entry(&self.parent, &self.name), Ok(Some(current)) if current == expected) {
            return;
        }
        if expected.kind == SFlag::S_IFDIR {
            let Ok(retired) = directory(&self.parent, &self.name, false) else {
                return;
            };
            if identity(&retired).ok() != Some(expected.identity) {
                return;
            }
            if clear_directory(&retired).is_err() {
                return;
            }
            let _ = unlinkat(&self.parent, self.name.as_str(), UnlinkatFlags::RemoveDir);
        } else {
            let _ = unlinkat(&self.parent, self.name.as_str(), UnlinkatFlags::NoRemoveDir);
        }
    }
}

#[derive(Debug)]
pub(super) struct Staging {
    workspace: File,
    bazel: File,
    configured: File,
    bin: File,
    projection: String,
    destinations: Vec<Destination>,
    sealed: bool,
    attempted: bool,
    runfiles: Option<runfiles::RunfilesTopology>,
    alias: Option<String>,
}
impl Staging {
    pub(super) fn new(
        workspace: &Path,
        configuration: &SlugConfiguration,
        outputs: &[ActionOutput],
        reserved: &starlark_map::small_set::SmallSet<&str>,
    ) -> io::Result<Self> {
        for output in outputs {
            component_path(output.path())?;
            if !matches!(
                output.kind(),
                ActionOutputKind::File | ActionOutputKind::Directory
            ) {
                return Err(io::Error::other("unsupported output kind"));
            }
        }
        let workspace = File::options()
            .read(true)
            .custom_flags(nix::libc::O_DIRECTORY | nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
            .open(workspace)?;
        let bazel = directory(&workspace, "bazel-out", true)?;
        let projection = configuration.projection().path_component();
        claim_marker(&bazel, &projection, configuration.canonical_bytes())?;
        let configured = directory(&bazel, &projection, true)?;
        let bin = directory(&configured, "bin", true)?;
        let mut destinations = Vec::with_capacity(outputs.len());
        for output in outputs {
            let parts = component_path(output.path())?;
            let parent = walk(&bin, &parts[..parts.len() - 1], true)?;
            let previous = entry(&parent, parts.last().unwrap())?;
            destinations.push(Destination::new(parent, previous, output.kind(), reserved)?);
        }
        Ok(Self {
            workspace,
            bazel,
            configured,
            bin,
            projection,
            destinations,
            sealed: false,
            attempted: false,
            runfiles: None,
            alias: None,
        })
    }
    fn destination(
        &self,
        outputs: &[ActionOutput],
        index: usize,
        relative: &str,
        is_dir: bool,
    ) -> io::Result<&Destination> {
        if self.sealed || self.attempted || self.alias.is_some() {
            return Err(io::Error::other(
                "output staging is sealed or Core-owned alias",
            ));
        }
        let output = outputs
            .get(index)
            .ok_or_else(|| io::Error::other("unknown declared output index"))?;
        match output.kind() {
            ActionOutputKind::File if relative.is_empty() && !is_dir => {}
            ActionOutputKind::Directory => {
                if relative.is_empty() {
                    if !is_dir {
                        return Err(io::Error::other("tree root must remain a directory"));
                    }
                } else {
                    component_path(relative)?;
                }
            }
            _ => {
                return Err(io::Error::other(
                    "output kind or relative path differs from declaration",
                ));
            }
        }
        Ok(&self.destinations[index])
    }
    pub(super) fn create_file(
        &self,
        outputs: &[ActionOutput],
        index: usize,
        relative: &str,
    ) -> io::Result<File> {
        let destination = self.destination(outputs, index, relative, false)?;
        if relative.is_empty() {
            if destination.created.swap(true, Ordering::Relaxed) {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "selected file already staged",
                ));
            }
            return destination.staged.try_clone();
        }
        let parts = component_path(relative)?;
        let parent = walk(&destination.staged, &parts[..parts.len() - 1], true)?;
        let file = openat(
            &parent,
            *parts.last().unwrap(),
            OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::from_bits_truncate(0o600),
        )?;
        destination.created.store(true, Ordering::Relaxed);
        Ok(File::from(file))
    }
    pub(super) fn create_directory(
        &self,
        outputs: &[ActionOutput],
        index: usize,
        relative: &str,
    ) -> io::Result<()> {
        let destination = self.destination(outputs, index, relative, true)?;
        if !relative.is_empty() {
            walk(&destination.staged, &component_path(relative)?, true)?;
        }
        destination.created.store(true, Ordering::Relaxed);
        Ok(())
    }
    pub(super) fn seal(&mut self, outputs: &[ActionOutput]) -> io::Result<()> {
        if self.sealed || self.attempted {
            return Err(io::Error::other("output staging is already sealed"));
        }
        for destination in &self.destinations {
            if !destination.created.load(Ordering::Relaxed) {
                return Err(io::Error::other("selected output was not staged"));
            }
            if entry(&destination.parent, &destination.name)? != destination.cleanup {
                return Err(io::Error::other("staged output changed during transfer"));
            }
            if self.alias.is_some() {
                self.check_alias()?;
            } else if let Some(topology) = &self.runfiles {
                topology.seal(&destination.staged)?;
            } else {
                seal_entry(&destination.parent, &destination.name)?;
            }
        }
        self.preflight(outputs)?;
        self.sealed = true;
        Ok(())
    }
    fn preflight(&self, outputs: &[ActionOutput]) -> io::Result<()> {
        for (parent, name, expected) in [
            (&self.workspace, "bazel-out", &self.bazel),
            (&self.bazel, self.projection.as_str(), &self.configured),
            (&self.configured, "bin", &self.bin),
        ] {
            if identity(&directory(parent, name, false)?)? != identity(expected)? {
                return Err(io::Error::other(
                    "output namespace root changed during staging",
                ));
            }
        }
        for (output, saved) in outputs.iter().zip(&self.destinations) {
            let parts = component_path(output.path())?;
            let parent = walk(&self.bin, &parts[..parts.len() - 1], false)?;
            if identity(&parent)? != identity(&saved.parent)?
                || entry(&parent, parts.last().unwrap())? != saved.previous
            {
                return Err(io::Error::other(
                    "output destination changed during staging",
                ));
            }
        }
        Ok(())
    }
    pub(super) fn preflight_publication(&self, outputs: &[ActionOutput]) -> io::Result<()> {
        if !self.sealed || self.attempted {
            return Err(io::Error::other(
                "output publication requires one sealed stage",
            ));
        }
        self.check_alias()?;
        self.preflight(outputs)
    }
    pub(super) fn publish(&mut self, outputs: &[ActionOutput]) -> io::Result<()> {
        self.preflight_publication(outputs)?;
        self.attempted = true;
        for (output, destination) in outputs.iter().zip(&mut self.destinations) {
            let name = component_path(output.path())?.last().unwrap().to_string();
            let flags = if destination.previous.is_some() {
                RenameFlags::RENAME_EXCHANGE
            } else {
                RenameFlags::RENAME_NOREPLACE
            };
            renameat2(
                &destination.parent,
                destination.name.as_str(),
                &destination.parent,
                name.as_str(),
                flags,
            )?;
            destination.cleanup = destination.previous;
        }
        Ok(())
    }
}

fn names(directory: &File) -> io::Result<Vec<OsString>> {
    let mut dir = Dir::from_fd(openat(
        directory,
        ".",
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_CLOEXEC,
        Mode::empty(),
    )?)?;
    dir.iter()
        .filter_map(|entry| match entry {
            Ok(entry) if matches!(entry.file_name().to_bytes(), b"." | b"..") => None,
            Ok(entry) => Some(Ok(std::ffi::OsStr::from_bytes(
                entry.file_name().to_bytes(),
            )
            .to_owned())),
            Err(error) => Some(Err(error.into())),
        })
        .collect()
}

struct Frame {
    directory: File,
    name: Option<OsString>,
    children: std::vec::IntoIter<OsString>,
}
impl Frame {
    fn new(directory: File, name: Option<OsString>) -> io::Result<Self> {
        let children = names(&directory)?.into_iter();
        Ok(Self {
            directory,
            name,
            children,
        })
    }
}
fn seal_file(file: &File) -> io::Result<()> {
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("staged output contains a special file"));
    }
    file.sync_all()?;
    fchmod(file, Mode::from_bits_truncate(0o555))?;
    Ok(())
}
/// Descriptor use is proportional to depth, not the number of file children.
fn seal_entry(parent: &File, name: &str) -> io::Result<()> {
    let file = File::from(openat(
        parent,
        name,
        OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    )?);
    if !file.metadata()?.is_dir() {
        return seal_file(&file);
    }
    let mut pending = vec![Frame::new(file, None)?];
    while let Some(frame) = pending.last_mut() {
        if let Some(name) = frame.children.next() {
            let child = File::from(openat(
                &frame.directory,
                name.as_os_str(),
                OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )?);
            if child.metadata()?.is_dir() {
                pending.push(Frame::new(child, None)?);
            } else {
                seal_file(&child)?;
            }
        } else {
            frame.directory.sync_all()?;
            fchmod(&frame.directory, Mode::from_bits_truncate(0o555))?;
            pending.pop();
        }
    }
    Ok(())
}

fn clear_directory(root: &File) -> io::Result<()> {
    fchmod(root, Mode::from_bits_truncate(0o700))?;
    let mut pending = vec![Frame::new(root.try_clone()?, None)?];
    while let Some(frame) = pending.last_mut() {
        if let Some(name) = frame.children.next() {
            match openat(
                &frame.directory,
                name.as_os_str(),
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            ) {
                Ok(child) => {
                    let child = File::from(child);
                    fchmod(&child, Mode::from_bits_truncate(0o700))?;
                    pending.push(Frame::new(child, Some(name))?);
                }
                Err(Errno::ENOTDIR | Errno::ELOOP) => unlinkat(
                    &frame.directory,
                    name.as_os_str(),
                    UnlinkatFlags::NoRemoveDir,
                )?,
                Err(error) => return Err(error.into()),
            }
        } else {
            let frame = pending.pop().unwrap();
            if let Some(name) = frame.name {
                unlinkat(
                    &pending.last().unwrap().directory,
                    name.as_os_str(),
                    UnlinkatFlags::RemoveDir,
                )?;
            }
        }
    }
    Ok(())
}
