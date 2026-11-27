//! Installed setup images and their native descriptor credits.
use std::ffi::CStr;
use std::path::Component;

use super::spawn;
use super::*;

pub(super) const SYSTEM_BWRAP: &str = "/usr/bin/bwrap";
const PATH_BYTES: usize = libc::PATH_MAX as usize;

/// A protected image pair for the running CLI. Possessing the pins does not
/// establish that this host can complete a confined launch.
#[derive(Clone)]
pub(in crate::runtime) struct InstalledProcessPlatform(Arc<Images>);

struct Images {
    bwrap: PinnedImage,
    helper: PinnedImage,
    helper_path: PathBuf,
    closefrom: spawn::CloseFrom,
    resources: Arc<RepositoryRootResources>,
    _credit: resources::ResourceCredit,
}

struct PinnedImage {
    file: File,
    identity: libc::stat,
}

impl InstalledProcessPlatform {
    pub(in crate::runtime) fn acquire(
        resources: Arc<RepositoryRootResources>,
    ) -> Result<Self, PhysicalRootError> {
        require_rootless_process()?;
        let closefrom = spawn::resolve_closefrom()?;
        let credit = resources.reserve(add(arc_bytes::<Images>()?, PATH_BYTES)?, 2, 0)?;
        // Intermediate path spellings and at most two directory FDs plus the
        // named helper used to authenticate the kernel's current image.
        let _temporary = resources.reserve(4 * PATH_BYTES, 3, 0)?;
        let bwrap = pin_installed(Path::new(SYSTEM_BWRAP), true)?;
        let helper_path = std::env::current_exe()?;
        let named = pin_installed(&helper_path, false)?;
        let helper = open_at(
            libc::AT_FDCWD,
            c"/proc/self/exe",
            libc::O_RDONLY | libc::O_NONBLOCK,
        )?;
        let identity = spawn::stat(helper.as_fd())?;
        if !spawn::same_image(&named.identity, &identity) {
            return Err(invalid("installed helper differs from the running Slug image").into());
        }
        drop(named);
        Ok(Self(Arc::new(Images {
            bwrap,
            helper: PinnedImage {
                file: helper,
                identity,
            },
            helper_path,
            closefrom,
            resources,
            _credit: credit,
        })))
    }

    pub(in crate::runtime) fn resources(&self) -> &Arc<RepositoryRootResources> {
        &self.0.resources
    }

    pub(in crate::runtime::repository_physical_root) fn verify_resources(
        &self,
        resources: &Arc<RepositoryRootResources>,
    ) -> Result<(), PhysicalRootError> {
        if !Arc::ptr_eq(self.resources(), resources) {
            return Err(failure(
                "native setup image resources belong to another runtime",
            ));
        }
        Ok(())
    }

    pub(in crate::runtime::repository_physical_root) fn platform(&self) -> ProcessPlatform {
        ProcessPlatform::new(ProcessImages::Installed(self.clone()))
    }

    pub(super) fn launch_images(
        &self,
        display: &Path,
    ) -> io::Result<(BorrowedFd<'_>, BorrowedFd<'_>, spawn::CloseFrom)> {
        // Linux capabilities belong to individual threads. The current worker
        // inherits the requesting thread's credentials, not the acquiring thread's.
        require_rootless_process()?;
        if Path::new(SYSTEM_BWRAP).starts_with(display) || self.0.helper_path.starts_with(display) {
            return Err(invalid("invalid native setup image"));
        }
        let bwrap = self.0.bwrap.checked_fd()?;
        let helper = self.0.helper.checked_fd()?;
        Ok((bwrap, helper, self.0.closefrom))
    }
}

impl PinnedImage {
    fn checked_fd(&self) -> io::Result<BorrowedFd<'_>> {
        let fd = self.file.as_fd();
        if !spawn::same_image(&self.identity, &spawn::stat(fd)?) {
            return Err(invalid("installed setup image changed after acquisition"));
        }
        Ok(fd)
    }
}

fn pin_installed(path: &Path, root_owned: bool) -> io::Result<PinnedImage> {
    if !path.is_absolute()
        || path.as_os_str().len() >= PATH_BYTES
        || path.starts_with(wire::RESERVED_PREFIX)
    {
        return Err(invalid("invalid installed setup image path"));
    }
    let flags = libc::O_PATH | libc::O_DIRECTORY | libc::O_NOFOLLOW;
    let root = open_at(libc::AT_FDCWD, c"/", flags)?;
    pin_from_directory(
        root,
        path.strip_prefix("/").expect("checked absolute path"),
        root_owned,
    )
}

fn pin_from_directory(
    mut directory: File,
    path: &Path,
    root_owned: bool,
) -> io::Result<PinnedImage> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid("invalid installed setup image path"))?;
    let leaf = path
        .file_name()
        .ok_or_else(|| invalid("invalid installed setup image path"))?;
    let flags = libc::O_PATH | libc::O_DIRECTORY | libc::O_NOFOLLOW;
    require_protected_directory(&directory, root_owned)?;
    for part in parent.components() {
        let Component::Normal(name) = part else {
            return Err(invalid("invalid installed setup image directory"));
        };
        let name = CString::new(name.as_bytes())
            .map_err(|_| invalid("invalid installed setup image directory"))?;
        let next = open_at(directory.as_raw_fd(), &name, flags)?;
        require_protected_directory(&next, root_owned)?;
        directory = next;
    }
    let leaf =
        CString::new(leaf.as_bytes()).map_err(|_| invalid("invalid installed setup image name"))?;
    let file = open_at(
        directory.as_raw_fd(),
        &leaf,
        libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK,
    )?;
    let identity = spawn::stat(file.as_fd())?;
    if !spawn::admitted_image_mode(identity.st_mode, identity.st_uid, root_owned) {
        return Err(invalid("untrusted installed setup image"));
    }
    spawn::check_image_contents(&file)?;
    Ok(PinnedImage { file, identity })
}

fn require_protected_directory(directory: &File, root_owned: bool) -> io::Result<()> {
    let state = spawn::stat(directory.as_fd())?;
    let owner = state.st_uid == 0 || (!root_owned && state.st_uid == unsafe { libc::geteuid() });
    if state.st_mode & libc::S_IFMT != libc::S_IFDIR || state.st_mode & 0o022 != 0 || !owner {
        return Err(invalid("unprotected installed setup image directory"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::fs::symlink;

    use super::*;

    #[test]
    fn installed_image_walk_refuses_symlinks_and_writable_directories() {
        let temporary = tempfile::tempdir().unwrap();
        let root = || File::open(temporary.path()).unwrap();
        let directory = temporary.path().join("trusted");
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
        let image = directory.join("image");
        std::fs::write(&image, b"\x7fELF").unwrap();
        std::fs::set_permissions(&image, std::fs::Permissions::from_mode(0o555)).unwrap();
        let pin = pin_from_directory(root(), Path::new("trusted/image"), false).unwrap();
        pin.checked_fd().unwrap();

        symlink(&directory, temporary.path().join("hop")).unwrap();
        symlink(&image, directory.join("alias")).unwrap();
        for name in ["hop/image", "trusted/alias", "../trusted/image"] {
            assert!(
                pin_from_directory(root(), Path::new(name), false).is_err(),
                "{name}"
            );
        }
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o775)).unwrap();
        assert!(pin_from_directory(root(), Path::new("trusted/image"), false).is_err());
    }

    #[test]
    fn admitted_file_identity_never_reopens_and_refuses_in_place_changes() {
        let temporary = tempfile::tempdir().unwrap();
        let root = || File::open(temporary.path()).unwrap();
        let name = temporary.path().join("image");
        std::fs::write(&name, b"\x7fELF original").unwrap();
        std::fs::set_permissions(&name, std::fs::Permissions::from_mode(0o555)).unwrap();
        let pin = pin_from_directory(root(), Path::new("image"), false).unwrap();
        let old = temporary.path().join("old");
        std::fs::rename(&name, &old).unwrap();
        std::fs::write(&name, b"\x7fELF different").unwrap();
        assert_eq!(
            spawn::stat(pin.file.as_fd()).unwrap().st_ino,
            pin.identity.st_ino
        );
        assert_ne!(std::fs::metadata(&name).unwrap().ino(), pin.identity.st_ino);
        let pin = pin_from_directory(root(), Path::new("old"), false).unwrap();
        pin.checked_fd().unwrap();
        std::fs::set_permissions(&old, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(pin.checked_fd().is_err());
    }

    #[test]
    fn installed_images_require_their_own_bank_and_refund_when_the_last_owner_drops() {
        unsafe extern "C" fn unused_closefrom(
            _actions: *mut libc::posix_spawn_file_actions_t,
            _first_fd: i32,
        ) -> i32 {
            libc::ENOSYS
        }

        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("image");
        std::fs::write(&path, b"\x7fELF").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o555)).unwrap();
        let pin = || {
            let directory = File::open(temporary.path()).unwrap();
            pin_from_directory(directory, Path::new("image"), false).unwrap()
        };
        let make_bank = || {
            let limits = RepositoryRootResourceLimits::new(1024 * 1024, 16, 1024 * 1024).unwrap();
            RepositoryRootResources::new(limits).unwrap()
        };
        let resources = make_bank();
        let before = resources.snapshot();
        let credit = resources
            .reserve(arc_bytes::<Images>().unwrap() + PATH_BYTES, 2, 0)
            .unwrap();
        let owner = InstalledProcessPlatform(Arc::new(Images {
            bwrap: pin(),
            helper: pin(),
            helper_path: path,
            closefrom: unused_closefrom,
            resources: resources.clone(),
            _credit: credit,
        }));
        assert!(owner.verify_resources(&resources).is_ok());
        assert!(owner.verify_resources(&make_bank()).is_err());
        let retained = owner.clone();
        drop(owner);
        assert_eq!(resources.snapshot().native_fds, before.native_fds + 2);
        drop(retained);
        assert_eq!(resources.snapshot().native_fds, before.native_fds);
        assert_eq!(resources.snapshot().retained_bytes, before.retained_bytes);
    }
}

fn open_at(base: libc::c_int, name: &CStr, flags: libc::c_int) -> io::Result<File> {
    let fd = unsafe { libc::openat(base, name.as_ptr(), flags | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn require_rootless_process() -> io::Result<()> {
    let (mut uid, mut euid, mut suid) = (0, 0, 0);
    let (mut gid, mut egid, mut sgid) = (0, 0, 0);
    if unsafe { libc::getresuid(&mut uid, &mut euid, &mut suid) } != 0
        || unsafe { libc::getresgid(&mut gid, &mut egid, &mut sgid) } != 0
    {
        return Err(io::Error::last_os_error());
    }
    if uid == 0 || uid != euid || uid != suid || gid != egid || gid != sgid {
        return Err(invalid(
            "installed native setup requires an unprivileged CLI",
        ));
    }
    // Linux capability API v3 uses two triples of u32 values for effective,
    // permitted and inheritable sets. An ambient capability requires both
    // a permitted and an inheritable grant.
    #[repr(C)]
    struct CapabilityHeader {
        version: u32,
        pid: libc::pid_t,
    }
    let mut header = CapabilityHeader {
        version: 0x2008_0522,
        pid: 0,
    };
    let mut caps = [0u32; 6];
    if unsafe { libc::syscall(libc::SYS_capget, &mut header, caps.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if caps != [0; 6] {
        return Err(invalid(
            "installed native setup requires a CLI without process capabilities",
        ));
    }
    Ok(())
}
