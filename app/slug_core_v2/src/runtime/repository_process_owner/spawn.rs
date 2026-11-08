//! Bounded Host view acquisition and posix_spawn machinery. The FD aliases in
//! this leaf select pinned HOST mounts only; no workspace root is reopened.
//! Spawn actions/closefrom normalization are source-derived from the protected
//! repository_process.rs runner; no GCC argument/source/environment policy is
//! imported.
use std::ffi::CStr;
use std::ffi::c_void;
use std::mem::MaybeUninit;
use std::ptr;

use super::*;

struct Pin {
    file: File,
    _credit: resources::ResourceCredit,
}
struct Row {
    row: wire::MirrorEntry,
    pin: Pin,
}
pub(super) struct HostMirror {
    pins: Vec<Pin>,
}
struct Directory(*mut libc::DIR);
impl Drop for Directory {
    fn drop(&mut self) {
        unsafe {
            libc::closedir(self.0);
        }
    }
}
fn stat(fd: BorrowedFd<'_>) -> io::Result<libc::stat> {
    let mut value = MaybeUninit::zeroed();
    if unsafe { libc::fstat(fd.as_raw_fd(), value.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { value.assume_init() })
}
fn same(a: &libc::stat, b: &libc::stat) -> bool {
    a.st_dev == b.st_dev
        && a.st_ino == b.st_ino
        && a.st_mode == b.st_mode
        && a.st_mtime == b.st_mtime
        && a.st_mtime_nsec == b.st_mtime_nsec
        && a.st_ctime == b.st_ctime
        && a.st_ctime_nsec == b.st_ctime_nsec
}
impl HostMirror {
    pub(super) fn collect(
        resources: &Arc<RepositoryRootResources>,
        stop: &AtomicBool,
        scope: &SourceServiceScope,
    ) -> Result<(Self, Vec<wire::MirrorEntry>), PhysicalRootError> {
        let raw = unsafe {
            libc::open(
                c"/".as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        if raw < 0 {
            return Err(io::Error::last_os_error().into());
        }
        let root = unsafe { File::from_raw_fd(raw) };
        let before = stat(root.as_fd())?;
        let duplicate = unsafe { libc::fcntl(raw, libc::F_DUPFD_CLOEXEC, 3) };
        if duplicate < 0 {
            return Err(io::Error::last_os_error().into());
        }
        let directory = unsafe { libc::fdopendir(duplicate) };
        if directory.is_null() {
            unsafe {
                libc::close(duplicate);
            }
            return Err(io::Error::last_os_error().into());
        }
        let directory = Directory(directory);
        let mut rows = Vec::new();
        rows.try_reserve_exact(wire::MAX_MIRRORS)
            .map_err(|_| PhysicalRootError::Resource)?;
        let mut text = 0usize;
        let mut seen = 0usize;
        loop {
            if stop.load(Ordering::Acquire) {
                return Err(PhysicalRootError::Inactive);
            }
            scope.check()?;
            unsafe {
                *libc::__errno_location() = 0;
            }
            let entry = unsafe { libc::readdir(directory.0) };
            if entry.is_null() {
                let errno = unsafe { *libc::__errno_location() };
                if errno == libc::EINTR {
                    continue;
                }
                if errno != 0 {
                    return Err(io::Error::from_raw_os_error(errno).into());
                }
                break;
            }
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
            if name == b"." || name == b".." {
                continue;
            }
            seen += 1;
            if seen > wire::MAX_MIRRORS {
                return Err(PhysicalRootError::Resource);
            }
            if [
                b"proc".as_slice(),
                b"dev",
                b"tmp",
                &wire::RESERVED_PREFIX.as_bytes()[1..],
            ]
            .contains(&name)
            {
                continue;
            }
            if name.is_empty() || name.len() > 255 || name.contains(&b'/') {
                return Err(failure("invalid Host mirror name"));
            }
            text = text
                .checked_add(name.len())
                .filter(|n| *n <= wire::MAX_MIRROR_TEXT)
                .ok_or(PhysicalRootError::Resource)?;
            let name = CString::new(name).map_err(|_| failure("invalid Host mirror name"))?;
            let credit = resources.reserve(std::mem::size_of::<Pin>(), 2, 0)?;
            let raw = unsafe {
                libc::openat(
                    root.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_PATH | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if raw < 0 {
                return Err(io::Error::last_os_error().into());
            }
            let pin = Pin {
                file: unsafe { File::from_raw_fd(raw) },
                _credit: credit,
            };
            let frame = stat(pin.file.as_fd())?;
            let kind = frame.st_mode & libc::S_IFMT;
            if !matches!(kind, libc::S_IFDIR | libc::S_IFREG | libc::S_IFLNK) {
                return Err(failure("unsupported Host mirror node"));
            }
            let link = if kind == libc::S_IFLNK {
                let mut scratch = [0u8; 4097];
                let n = unsafe {
                    libc::readlinkat(
                        raw,
                        c"".as_ptr(),
                        scratch.as_mut_ptr().cast(),
                        scratch.len(),
                    )
                };
                if n < 0 {
                    return Err(io::Error::last_os_error().into());
                }
                if n as usize >= scratch.len() {
                    return Err(PhysicalRootError::Resource);
                }
                text = text
                    .checked_add(n as usize)
                    .filter(|n| *n <= wire::MAX_MIRROR_TEXT)
                    .ok_or(PhysicalRootError::Resource)?;
                Some(
                    CString::new(&scratch[..n as usize])
                        .map_err(|_| failure("invalid Host link"))?,
                )
            } else {
                None
            };
            let mut named = MaybeUninit::zeroed();
            if unsafe {
                libc::fstatat(
                    root.as_raw_fd(),
                    name.as_ptr(),
                    named.as_mut_ptr(),
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            } != 0
            {
                return Err(io::Error::last_os_error().into());
            }
            if !same(&frame, &unsafe { named.assume_init() })
                || !same(&frame, &stat(pin.file.as_fd())?)
            {
                return Err(failure("unstable Host mirror"));
            }
            rows.push(Row {
                row: wire::MirrorEntry {
                    name,
                    fd: 0,
                    device: frame.st_dev as u64,
                    inode: frame.st_ino as u64,
                    mode: frame.st_mode,
                    link,
                },
                pin,
            });
        }
        if !same(&before, &stat(root.as_fd())?) {
            return Err(failure("unstable Host root"));
        }
        rows.sort_unstable_by(|a, b| a.row.name.as_bytes().cmp(b.row.name.as_bytes()));
        let mut pins = Vec::new();
        let mut mirrors = Vec::new();
        pins.try_reserve_exact(rows.len())
            .map_err(|_| PhysicalRootError::Resource)?;
        mirrors
            .try_reserve_exact(rows.len())
            .map_err(|_| PhysicalRootError::Resource)?;
        for (i, mut row) in rows.into_iter().enumerate() {
            if mirrors
                .last()
                .is_some_and(|last: &wire::MirrorEntry| last.name == row.row.name)
            {
                return Err(failure("duplicate Host mirror"));
            }
            row.row.fd = 6 + i as i32;
            mirrors.push(row.row);
            pins.push(row.pin);
        }
        Ok((Self { pins }, mirrors))
    }
}

pub(super) fn nonblocking(fd: BorrowedFd<'_>) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
    if flags < 0
        || unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
fn pipe() -> io::Result<(File, File)> {
    let mut raw = [-1; 2];
    if unsafe { libc::pipe2(raw.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { (File::from_raw_fd(raw[0]), File::from_raw_fd(raw[1])) })
}
fn socket() -> io::Result<(File, File)> {
    let mut raw = [-1; 2];
    if unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
            0,
            raw.as_mut_ptr(),
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { (File::from_raw_fd(raw[0]), File::from_raw_fd(raw[1])) })
}
fn code(value: i32) -> io::Result<()> {
    if value == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(value))
    }
}
type CloseFrom = unsafe extern "C" fn(*mut libc::posix_spawn_file_actions_t, i32) -> i32;
pub(super) fn closefrom_with(resolve: impl FnOnce(&CStr) -> *mut c_void) -> io::Result<CloseFrom> {
    let address = resolve(c"posix_spawn_file_actions_addclosefrom_np");
    if address.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native closefrom spawn action unavailable",
        ));
    }
    Ok(unsafe { std::mem::transmute::<*mut c_void, CloseFrom>(address) })
}
struct Actions(libc::posix_spawn_file_actions_t);
impl Actions {
    fn new() -> io::Result<Self> {
        let mut value = MaybeUninit::uninit();
        code(unsafe { libc::posix_spawn_file_actions_init(value.as_mut_ptr()) })?;
        Ok(Self(unsafe { value.assume_init() }))
    }
    fn ptr(&mut self) -> *mut libc::posix_spawn_file_actions_t {
        &mut self.0
    }
}
impl Drop for Actions {
    fn drop(&mut self) {
        unsafe {
            libc::posix_spawn_file_actions_destroy(self.ptr());
        }
    }
}
struct Attributes(libc::posix_spawnattr_t);
impl Attributes {
    fn new() -> io::Result<Self> {
        let mut value = MaybeUninit::uninit();
        code(unsafe { libc::posix_spawnattr_init(value.as_mut_ptr()) })?;
        Ok(Self(unsafe { value.assume_init() }))
    }
    fn ptr(&mut self) -> *mut libc::posix_spawnattr_t {
        &mut self.0
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe {
            libc::posix_spawnattr_destroy(self.ptr());
        }
    }
}

pub(super) struct Spawned {
    pub pid: libc::pid_t,
    pub control: File,
    pub stdout: File,
    pub stderr: File,
    pub status: File,
}
pub(super) fn launch(
    platform: &ProcessPlatform,
    plan: &RunPlan,
    mirror: &HostMirror,
    namespace: Option<BorrowedFd<'_>>,
    resources: &Arc<RepositoryRootResources>,
) -> Result<Spawned, PhysicalRootError> {
    let closefrom =
        closefrom_with(|name| unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) })?;
    let bwrap_image = pin_image(&platform.bwrap, plan, !platform.test_images, false)?;
    let helper_image = pin_image(&platform.helper, plan, false, !platform.test_images)?;
    let argv = arguments(platform, plan, namespace.is_some())?;
    let pointers: Vec<*mut libc::c_char> = argv
        .iter()
        .map(|v| v.as_ptr().cast_mut())
        .chain(std::iter::once(ptr::null_mut()))
        .collect();
    let environment = [ptr::null_mut()];
    // This temporary FD credit precedes the file/normalization owners below.
    let _duplicate_credit = resources.reserve(4096, mirror.pins.len() + 8, 0)?;
    let plan_file = plan.sealed_file()?;
    let (stdout, stdout_write) = pipe()?;
    let (stderr, stderr_write) = pipe()?;
    let (control, child_control) = socket()?;
    let (status, status_write) = pipe()?;
    nonblocking(stdout.as_fd())?;
    nonblocking(stderr.as_fd())?;
    nonblocking(control.as_fd())?;
    nonblocking(status.as_fd())?;
    let minimum = 9 + mirror.pins.len() as i32;
    let mut normalized = Vec::new();
    normalized
        .try_reserve_exact(mirror.pins.len() + 8)
        .map_err(|_| PhysicalRootError::Resource)?;
    let mut add = |source: BorrowedFd<'_>, target: i32| -> io::Result<()> {
        let raw = unsafe { libc::fcntl(source.as_raw_fd(), libc::F_DUPFD_CLOEXEC, minimum) };
        if raw < minimum {
            return Err(io::Error::last_os_error());
        }
        normalized.push((unsafe { OwnedFd::from_raw_fd(raw) }, target));
        Ok(())
    };
    add(stdout_write.as_fd(), 1)?;
    add(stderr_write.as_fd(), 2)?;
    add(child_control.as_fd(), 3)?;
    add(plan_file.as_fd(), 4)?;
    if let Some(namespace) = namespace {
        add(namespace, 5)?;
    }
    for (i, pin) in mirror.pins.iter().enumerate() {
        add(pin.file.as_fd(), 6 + i as i32)?;
    }
    add(bwrap_image.as_fd(), 6 + mirror.pins.len() as i32)?;
    add(helper_image.as_fd(), 7 + mirror.pins.len() as i32)?;
    add(status_write.as_fd(), 8 + mirror.pins.len() as i32)?;
    let mut actions = Actions::new()?;
    code(unsafe {
        libc::posix_spawn_file_actions_addopen(
            actions.ptr(),
            0,
            c"/dev/null".as_ptr(),
            libc::O_RDONLY,
            0,
        )
    })?;
    if namespace.is_none() {
        code(unsafe {
            libc::posix_spawn_file_actions_addopen(
                actions.ptr(),
                5,
                c"/dev/null".as_ptr(),
                libc::O_RDONLY,
                0,
            )
        })?;
    }
    for (source, target) in &normalized {
        code(unsafe {
            libc::posix_spawn_file_actions_adddup2(actions.ptr(), source.as_raw_fd(), *target)
        })?;
    }
    code(unsafe { closefrom(actions.ptr(), minimum) })?;
    let mut attrs = Attributes::new()?;
    code(unsafe { libc::posix_spawnattr_setflags(attrs.ptr(), libc::POSIX_SPAWN_SETSID) })?;
    let mut pid = 0;
    let executable = CString::new(format!("/proc/self/fd/{}", 6 + mirror.pins.len()))
        .map_err(|_| failure("invalid pinned executable"))?;
    code(unsafe {
        libc::posix_spawn(
            &mut pid,
            executable.as_ptr(),
            actions.ptr(),
            attrs.ptr(),
            pointers.as_ptr(),
            environment.as_ptr(),
        )
    })?;
    if pid <= 0 {
        return Err(failure("invalid native launcher PID"));
    }
    Ok(Spawned {
        pid,
        control,
        stdout,
        stderr,
        status,
    })
}

fn pin_image(path: &Path, plan: &RunPlan, root_owned: bool, current: bool) -> io::Result<File> {
    use std::os::unix::fs::MetadataExt;
    if !path.is_absolute()
        || path.starts_with(Path::new(OsStr::from_bytes(plan.display.as_bytes())))
    {
        return Err(invalid("invalid native setup image"));
    }
    let canonical = path.canonicalize()?;
    let meta = canonical.metadata()?;
    if canonical.starts_with(Path::new(OsStr::from_bytes(plan.display.as_bytes())))
        || !meta.is_file()
        || meta.mode() & 0o111 == 0
        || meta.mode() & 0o022 != 0
        || (root_owned && meta.uid() != 0)
        || (!root_owned && meta.uid() != 0 && meta.uid() != unsafe { libc::geteuid() })
    {
        return Err(invalid("untrusted native setup image"));
    }
    let source = if current {
        c"/proc/self/exe".to_owned()
    } else {
        CString::new(canonical.as_os_str().as_bytes())
            .map_err(|_| invalid("invalid trusted image"))?
    };
    let raw = unsafe { libc::open(source.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    let file = unsafe { File::from_raw_fd(raw) };
    let actual = stat(file.as_fd())?;
    if actual.st_dev as u64 != meta.dev()
        || actual.st_ino as u64 != meta.ino()
        || actual.st_mode != meta.mode()
    {
        return Err(invalid("trusted image identity changed"));
    }
    use std::os::unix::fs::FileExt;
    let mut magic = [0u8; 4];
    file.read_exact_at(&mut magic, 0)?;
    if magic != *b"\x7fELF" {
        return Err(invalid("trusted setup image is not ELF"));
    }
    Ok(file)
}
fn arguments(
    platform: &ProcessPlatform,
    plan: &RunPlan,
    namespace: bool,
) -> io::Result<Vec<CString>> {
    let mut args = Vec::new();
    args.try_reserve_exact(64 + plan.mirrors.len() * 3)
        .map_err(|_| invalid("native argv allocation"))?;
    let mut add = |value: &[u8]| -> io::Result<()> {
        args.push(CString::new(value).map_err(|_| invalid("invalid native setup argument"))?);
        Ok(())
    };
    add(platform.bwrap.as_os_str().as_bytes())?;
    if namespace {
        add(b"--userns")?;
        add(b"5")?;
    } else {
        add(b"--unshare-user")?;
    }
    for arg in [
        "--unshare-pid",
        "--as-pid-1",
        "--unshare-net",
        "--unshare-ipc",
        "--unshare-uts",
        "--die-with-parent",
        "--new-session",
        "--uid",
        "0",
        "--gid",
        "0",
        "--clearenv",
        "--cap-drop",
        "ALL",
        "--cap-add",
        "CAP_SYS_ADMIN",
        "--cap-add",
        "CAP_SETPCAP",
        "--size",
        "1048576",
        "--tmpfs",
        "/",
    ] {
        add(arg.as_bytes())?;
    }
    for row in &plan.mirrors {
        let mut target = Vec::with_capacity(1 + row.name.as_bytes().len());
        target.push(b'/');
        target.extend_from_slice(row.name.as_bytes());
        if let Some(link) = &row.link {
            add(b"--symlink")?;
            add(link.as_bytes())?;
            add(&target)?;
        } else {
            let source = format!("/proc/self/fd/{}", row.fd);
            add(b"--ro-bind")?;
            add(source.as_bytes())?;
            add(&target)?;
        }
    }
    for arg in [
        "--proc",
        "/proc",
        "--size",
        "4096",
        "--tmpfs",
        "/dev",
        "--dev-bind",
        "/dev/null",
        "/dev/null",
        "--dev-bind",
        "/dev/zero",
        "/dev/zero",
        "--dev-bind",
        "/dev/random",
        "/dev/random",
        "--dev-bind",
        "/dev/urandom",
        "/dev/urandom",
        "--size",
        "16777216",
        "--tmpfs",
        "/tmp",
        "--size",
        "1048576",
        "--tmpfs",
        wire::RESERVED_PREFIX,
        "--chdir",
        "/",
    ] {
        add(arg.as_bytes())?;
    }
    add(b"--json-status-fd")?;
    let status_fd = (8 + plan.mirrors.len()).to_string();
    add(status_fd.as_bytes())?;
    add(b"--")?;
    let helper = format!("/proc/self/fd/{}", 7 + plan.mirrors.len());
    add(helper.as_bytes())?;
    add(wire::HELPER_ARGUMENT.as_bytes())?;
    Ok(args)
}
