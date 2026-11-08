//! Trusted namespace setup only. Every refusal precedes arbitrary payload code.
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;

use super::*;
const MOUNTINFO_BYTES: usize = 256 * 1024;
const MOUNTINFO_ROWS: usize = 2048;
const TMP_BYTES: u64 = 16 * 1024 * 1024;

pub(super) fn single_thread() -> Result<()> {
    let mut count = 0usize;
    for entry in std::fs::read_dir("/proc/self/task").map_err(|e| failure(2, e))? {
        entry.map_err(|e| failure(2, e))?;
        count += 1;
        if count > 1 {
            return Err(invalid(2));
        }
    }
    if count != 1 {
        return Err(invalid(2));
    }
    Ok(())
}
fn cpath(bytes: &[u8]) -> Result<CString> {
    CString::new(bytes).map_err(|_| invalid(3))
}
fn lstat(path: &CStr) -> Result<libc::stat> {
    let mut value = std::mem::MaybeUninit::zeroed();
    checked(3, unsafe { libc::lstat(path.as_ptr(), value.as_mut_ptr()) })?;
    Ok(unsafe { value.assume_init() })
}
fn mount_id(path: &CStr) -> Result<u64> {
    let mut value = std::mem::MaybeUninit::<libc::statx>::zeroed();
    checked(3, unsafe {
        libc::statx(
            libc::AT_FDCWD,
            path.as_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
            libc::STATX_MNT_ID,
            value.as_mut_ptr(),
        )
    })?;
    let value = unsafe { value.assume_init() };
    if value.stx_mask & libc::STATX_MNT_ID == 0 || value.stx_mnt_id == 0 {
        return Err(invalid(3));
    }
    Ok(value.stx_mnt_id)
}
fn attributes(path: &CStr, recursive: bool, set: u64, clear: u64) -> Result<()> {
    #[repr(C)]
    struct Attr {
        set: u64,
        clear: u64,
        propagation: u64,
        userns: u64,
    }
    let attr = Attr {
        set,
        clear,
        propagation: 0,
        userns: 0,
    };
    let flags = libc::AT_SYMLINK_NOFOLLOW as u32
        | if recursive {
            libc::AT_RECURSIVE as u32
        } else {
            0
        };
    checked(3, unsafe {
        libc::syscall(
            libc::SYS_mount_setattr,
            libc::AT_FDCWD,
            path.as_ptr(),
            flags,
            &attr,
            std::mem::size_of::<Attr>(),
        )
    } as i32)?;
    let mut actual = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
    checked(3, unsafe {
        libc::statvfs(path.as_ptr(), actual.as_mut_ptr())
    })?;
    let actual = unsafe { actual.assume_init() }.f_flag as u64;
    // mount_setattr and statvfs have distinct UAPI flag domains. Translate the
    // four admitted attributes explicitly instead of comparing their numbers.
    if (set | clear) & !(RO | NOSUID | NODEV | NOEXEC) != 0 {
        return Err(invalid(3));
    }
    for (attribute, observed) in [
        (RO, libc::ST_RDONLY as u64),
        (NOSUID, libc::ST_NOSUID as u64),
        (NODEV, libc::ST_NODEV as u64),
        (NOEXEC, libc::ST_NOEXEC as u64),
    ] {
        if (set & attribute != 0 && actual & observed == 0)
            || (clear & attribute != 0 && actual & observed != 0)
        {
            return Err(invalid(3));
        }
    }
    Ok(())
}
const RO: u64 = 1;
const NOSUID: u64 = 2;
const NODEV: u64 = 4;
const NOEXEC: u64 = 8;
fn fs(path: &CStr) -> Result<libc::statfs> {
    let mut value = std::mem::MaybeUninit::zeroed();
    checked(3, unsafe {
        libc::statfs(path.as_ptr(), value.as_mut_ptr())
    })?;
    Ok(unsafe { value.assume_init() })
}
fn tmpfs(path: &CStr, bytes: u64, inodes: u64) -> Result<()> {
    let value = fs(path)?;
    let mut vfs = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
    checked(3, unsafe { libc::statvfs(path.as_ptr(), vfs.as_mut_ptr()) })?;
    let vfs = unsafe { vfs.assume_init() };
    if value.f_type != libc::TMPFS_MAGIC
        || value.f_bsize != 4096
        || vfs.f_frsize != 4096
        || (vfs.f_blocks as u64).checked_mul(4096) != Some(bytes)
        || vfs.f_files as u64 != inodes
    {
        return Err(invalid(3));
    }
    Ok(())
}
fn top(name: &CStr) -> Result<CString> {
    let mut bytes = Vec::with_capacity(name.to_bytes().len() + 1);
    bytes.push(b'/');
    bytes.extend_from_slice(name.to_bytes());
    cpath(&bytes)
}

pub(super) fn host_view(plan: &RunPlan) -> Result<()> {
    // No propagation from a concurrent Host mount may enter after inventory.
    checked(3, unsafe {
        libc::mount(
            std::ptr::null(),
            c"/".as_ptr(),
            std::ptr::null(),
            libc::MS_PRIVATE | libc::MS_REC,
            std::ptr::null(),
        )
    })?;
    // bwrap's --size controls blocks only. These three are fresh private
    // setup tmpfs mounts; establish explicit inode quotas before attestation.
    for (path, options) in [
        (c"/", c"size=1048576,nr_inodes=512"),
        (c"/__slug_repositories__", c"size=1048576,nr_inodes=512"),
        (c"/tmp", c"size=16777216,nr_inodes=4096"),
    ] {
        if fs(path)?.f_type != libc::TMPFS_MAGIC {
            return Err(invalid(3));
        }
        checked(3, unsafe {
            libc::mount(
                std::ptr::null(),
                path.as_ptr(),
                c"tmpfs".as_ptr(),
                libc::MS_REMOUNT | libc::MS_NOSUID | libc::MS_NODEV,
                options.as_ptr().cast(),
            )
        })?;
    }
    tmpfs(c"/", 1024 * 1024, 512)?;
    tmpfs(c"/__slug_repositories__", 1024 * 1024, 512)?;
    tmpfs(c"/tmp", TMP_BYTES, 4096)?;
    for row in &plan.mirrors {
        let mut pin = std::mem::MaybeUninit::<libc::stat>::zeroed();
        checked(3, unsafe { libc::fstat(row.fd, pin.as_mut_ptr()) })?;
        let pin = unsafe { pin.assume_init() };
        if pin.st_dev as u64 != row.device
            || pin.st_ino as u64 != row.inode
            || pin.st_mode != row.mode
        {
            return Err(invalid(3));
        }
        let path = top(&row.name)?;
        let actual = lstat(&path)?;
        if row.mode & libc::S_IFMT == libc::S_IFLNK {
            let mut target = [0u8; 4097];
            let count =
                unsafe { libc::readlink(path.as_ptr(), target.as_mut_ptr().cast(), target.len()) };
            if count < 0 {
                return Err(failure(3, io::Error::last_os_error()));
            }
            if actual.st_mode & libc::S_IFMT != libc::S_IFLNK
                || row.link.as_ref().map(|v| v.as_bytes()) != Some(&target[..count as usize])
            {
                return Err(invalid(3));
            }
        } else {
            if actual.st_dev as u64 != row.device
                || actual.st_ino as u64 != row.inode
                || actual.st_mode != row.mode
            {
                return Err(invalid(3));
            }
            attributes(&path, true, RO | NOSUID | NODEV, 0)?;
        }
    }
    // Check the complete namespace root, not just the requested mirror rows.
    let mut seen = 0usize;
    for item in std::fs::read_dir("/").map_err(|e| failure(3, e))? {
        let item = item.map_err(|e| failure(3, e))?;
        let name = item.file_name();
        let bytes = name.as_bytes();
        if [b"proc".as_slice(), b"dev", b"tmp", b"__slug_repositories__"].contains(&bytes) {
            continue;
        }
        if !plan.mirrors.iter().any(|row| row.name.as_bytes() == bytes) {
            return Err(invalid(3));
        }
        seen += 1;
        if seen > wire::MAX_MIRRORS {
            return Err(invalid(3));
        }
    }
    if seen != plan.mirrors.len() {
        return Err(invalid(3));
    }
    protect_proc()?;
    devices()?;
    attributes(c"/tmp", true, NOSUID | NODEV, 0)?;
    // Source Host pins are setup-only and must not survive READY. No target
    // root authority has been received at this point.
    checked(
        3,
        unsafe { libc::syscall(libc::SYS_close_range, 6u32, u32::MAX, 0u32) } as i32,
    )?;
    Ok(())
}

#[derive(Debug)]
struct ProcMount {
    id: u64,
    path: CString,
}
pub(super) fn decode_mount_path(bytes: &[u8]) -> Result<CString> {
    if bytes.len() > 4 * 4096 {
        return Err(invalid(3));
    }
    let mut path = Vec::with_capacity(bytes.len().min(4096));
    let mut i = 0;
    while i < bytes.len() {
        let byte = if bytes[i] == b'\\' {
            let code = bytes.get(i + 1..i + 4).ok_or_else(|| invalid(3))?;
            let decoded = match code {
                b"040" => b' ',
                b"011" => b'\t',
                b"012" => b'\n',
                b"134" => b'\\',
                _ => return Err(invalid(3)),
            };
            i += 4;
            decoded
        } else {
            let decoded = bytes[i];
            i += 1;
            decoded
        };
        if path.len() == 4096 {
            return Err(invalid(3));
        }
        path.push(byte);
    }
    if !path.starts_with(b"/")
        || (path != b"/"
            && path[1..]
                .split(|b| *b == b'/')
                .any(|v| v.is_empty() || v == b"." || v == b".."))
    {
        return Err(invalid(3));
    }
    cpath(&path)
}
fn proc_mounts() -> Result<Vec<ProcMount>> {
    let input = File::open("/proc/self/mountinfo").map_err(|e| failure(3, e))?;
    let mut bytes = Vec::with_capacity(MOUNTINFO_BYTES + 1);
    input
        .take((MOUNTINFO_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| failure(3, e))?;
    if bytes.len() > MOUNTINFO_BYTES {
        return Err(invalid(3));
    }
    let mut result = Vec::new();
    let mut rows = 0usize;
    let mut text = 0usize;
    for line in bytes.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
        rows += 1;
        if rows > MOUNTINFO_ROWS {
            return Err(invalid(3));
        }
        let mut fields = line.split(|b| *b == b' ');
        let id = fields.next().ok_or_else(|| invalid(3))?;
        fields.next().ok_or_else(|| invalid(3))?;
        fields.next().ok_or_else(|| invalid(3))?;
        let root = fields.next().ok_or_else(|| invalid(3))?;
        let path = fields.next().ok_or_else(|| invalid(3))?;
        fields.next().ok_or_else(|| invalid(3))?;
        loop {
            let field = fields.next().ok_or_else(|| invalid(3))?;
            if field == b"-" {
                break;
            }
            if field.starts_with(b"shared:")
                || field.starts_with(b"master:")
                || field.starts_with(b"propagate_from:")
            {
                return Err(invalid(3));
            }
        }
        if fields.next() != Some(b"proc".as_slice()) {
            continue;
        }
        let id = std::str::from_utf8(id)
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v != 0)
            .ok_or_else(|| invalid(3))?;
        let path = decode_mount_path(path)?;
        text = text
            .checked_add(path.as_bytes().len())
            .ok_or_else(|| invalid(3))?;
        if text > MOUNTINFO_BYTES {
            return Err(invalid(3));
        }
        // A proc root with a non-root source is still a proc alias and must be
        // masked/refused. Only private /proc itself must expose the true root.
        if path.as_bytes() == b"/proc" && root != b"/" {
            return Err(invalid(3));
        }
        result.push(ProcMount { id, path });
    }
    Ok(result)
}
fn beneath(path: &[u8], parent: &[u8]) -> bool {
    path == parent
        || path
            .strip_prefix(parent)
            .is_some_and(|tail| tail.starts_with(b"/"))
}
fn protect_proc() -> Result<()> {
    let mut own = [0u8; 32];
    let count =
        unsafe { libc::readlink(c"/proc/self".as_ptr(), own.as_mut_ptr().cast(), own.len()) };
    if count != 1 || own[0] != b'1' || fs(c"/proc")?.f_type != libc::PROC_SUPER_MAGIC {
        return Err(invalid(3));
    }
    let private = mount_id(c"/proc")?;
    let mut mounts = proc_mounts()?;
    if !mounts
        .iter()
        .any(|row| row.id == private && row.path.as_bytes() == b"/proc")
    {
        return Err(invalid(3));
    }
    mounts.sort_by_key(|row| row.path.as_bytes().len());
    let mut masks: Vec<CString> = Vec::new();
    for row in &mounts {
        if row.id == private && row.path.as_bytes() == b"/proc" {
            continue;
        }
        if masks
            .iter()
            .any(|mask| beneath(row.path.as_bytes(), mask.as_bytes()))
        {
            continue;
        }
        // Directory aliases are covered by an actual tiny RO filesystem. A
        // remaining file bind of proc is an unsupported setup, never ignored.
        if lstat(&row.path)?.st_mode & libc::S_IFMT != libc::S_IFDIR {
            return Err(invalid(3));
        }
        checked(3, unsafe {
            libc::mount(
                c"tmpfs".as_ptr(),
                row.path.as_ptr(),
                c"tmpfs".as_ptr(),
                libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
                c"size=4096,nr_inodes=16,mode=0000".as_ptr().cast(),
            )
        })?;
        attributes(&row.path, true, RO | NOSUID | NODEV | NOEXEC, 0)?;
        tmpfs(&row.path, 4096, 16)?;
        masks.push(row.path.clone());
    }
    attributes(c"/proc", true, RO | NOSUID | NODEV | NOEXEC, 0)?;
    // Covered lower proc mounts may remain in mountinfo. Every visible alias
    // must resolve through its actual covering tmpfs, not a path-only allowlist.
    for row in proc_mounts()? {
        if row.id == private && row.path.as_bytes() == b"/proc" {
            continue;
        }
        if !masks
            .iter()
            .any(|mask| beneath(row.path.as_bytes(), mask.as_bytes()))
        {
            return Err(invalid(3));
        }
        let mask = masks
            .iter()
            .find(|mask| beneath(row.path.as_bytes(), mask.as_bytes()))
            .ok_or_else(|| invalid(3))?;
        if fs(mask)?.f_type != libc::TMPFS_MAGIC {
            return Err(invalid(3));
        }
    }
    if mount_id(c"/proc")? != private {
        return Err(invalid(3));
    }
    Ok(())
}
fn devices() -> Result<()> {
    let devices = [
        (b"null".as_slice(), 3),
        (b"zero".as_slice(), 5),
        (b"random".as_slice(), 8),
        (b"urandom".as_slice(), 9),
    ];
    if fs(c"/dev")?.f_type != libc::TMPFS_MAGIC {
        return Err(invalid(3));
    }
    let mut count = 0;
    for item in std::fs::read_dir("/dev").map_err(|e| failure(3, e))? {
        let name = item.map_err(|e| failure(3, e))?.file_name();
        let Some((_, minor)) = devices
            .iter()
            .find(|(expected, _)| *expected == name.as_bytes())
        else {
            return Err(invalid(3));
        };
        let mut bytes = b"/dev/".to_vec();
        bytes.extend_from_slice(name.as_bytes());
        let path = cpath(&bytes)?;
        let stat = lstat(&path)?;
        if stat.st_mode & libc::S_IFMT != libc::S_IFCHR || stat.st_rdev != libc::makedev(1, *minor)
        {
            return Err(invalid(3));
        }
        attributes(&path, false, RO | NOSUID | NOEXEC, NODEV)?;
        count += 1;
    }
    if count != 4 {
        return Err(invalid(3));
    }
    attributes(c"/dev", false, RO | NOSUID | NOEXEC, 0)
}

pub(super) fn mountpoint(plan: &RunPlan) -> Result<()> {
    let mut current = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(wire::RESERVED_PREFIX)
        .map_err(|e| failure(3, e))?;
    let tail = plan
        .display
        .as_bytes()
        .strip_prefix(b"/__slug_repositories__/")
        .ok_or_else(|| invalid(3))?;
    for component in tail.split(|b| *b == b'/') {
        let component = cpath(component)?;
        let status = unsafe { libc::mkdirat(current.as_raw_fd(), component.as_ptr(), 0o755) };
        if status != 0 && io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST) {
            return Err(failure(3, io::Error::last_os_error()));
        }
        let raw = unsafe {
            libc::openat(
                current.as_raw_fd(),
                component.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        checked(3, raw)?;
        current = unsafe { File::from_raw_fd(raw) };
    }
    Ok(())
}
pub(super) fn initialize_archive(plan: &RunPlan) -> Result<File> {
    checked(5, unsafe {
        libc::mount(
            c"tmpfs".as_ptr(),
            plan.display.as_ptr(),
            c"tmpfs".as_ptr(),
            libc::MS_NOSUID | libc::MS_NODEV,
            c"size=2147483648,nr_inodes=262144,mode=0755"
                .as_ptr()
                .cast(),
        )
    })?;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(Path::new(plan.display.to_str().map_err(|_| invalid(5))?))
        .map_err(|e| failure(5, e))
}
pub(super) fn finish_mounts(_plan: &RunPlan) -> Result<()> {
    attributes(c"/__slug_repositories__", false, RO | NOSUID | NODEV, 0)?;
    attributes(c"/", false, RO | NOSUID | NODEV, 0)
}

pub(super) fn limits(timeout_millis: u64) -> Result<()> {
    let cpu = timeout_millis.div_ceil(1000).min(27);
    for (resource, maximum) in [
        (libc::RLIMIT_AS, 512 * 1024 * 1024u64),
        (libc::RLIMIT_CPU, cpu),
        (libc::RLIMIT_NPROC, 64),
        (libc::RLIMIT_NOFILE, 64),
        (libc::RLIMIT_FSIZE, PROCESS_BYTES),
        (libc::RLIMIT_CORE, 0),
        (libc::RLIMIT_STACK, 8 * 1024 * 1024),
    ] {
        let mut previous = std::mem::MaybeUninit::<libc::rlimit>::zeroed();
        checked(15, unsafe {
            libc::getrlimit(resource, previous.as_mut_ptr())
        })?;
        let previous = unsafe { previous.assume_init() };
        let limit = maximum
            .min(previous.rlim_cur as u64)
            .min(previous.rlim_max as u64) as libc::rlim_t;
        let next = libc::rlimit {
            rlim_cur: limit,
            rlim_max: limit,
        };
        checked(15, unsafe { libc::setrlimit(resource, &next) })?;
        let mut actual = std::mem::MaybeUninit::<libc::rlimit>::zeroed();
        checked(15, unsafe {
            libc::getrlimit(resource, actual.as_mut_ptr())
        })?;
        let actual = unsafe { actual.assume_init() };
        if actual.rlim_cur != limit || actual.rlim_max != limit {
            return Err(invalid(15));
        }
    }
    Ok(())
}
pub(super) fn retire_privileges() -> Result<()> {
    checked(9, unsafe {
        libc::prctl(
            libc::PR_CAP_AMBIENT,
            libc::PR_CAP_AMBIENT_CLEAR_ALL,
            0,
            0,
            0,
        )
    })?;
    let mut last = None;
    for cap in 0..64 {
        let value = unsafe { libc::prctl(libc::PR_CAPBSET_READ, cap, 0, 0, 0) };
        if value < 0 {
            if io::Error::last_os_error().raw_os_error() != Some(libc::EINVAL) {
                return Err(failure(9, io::Error::last_os_error()));
            }
            last = Some(cap);
            break;
        }
        checked(9, unsafe {
            libc::prctl(libc::PR_CAPBSET_DROP, cap, 0, 0, 0)
        })?;
    }
    let last = last.ok_or_else(|| invalid(9))?;
    #[repr(C)]
    struct Header {
        version: u32,
        pid: i32,
    }
    #[repr(C)]
    #[derive(Clone, Copy, PartialEq, Eq)]
    struct Caps {
        effective: u32,
        permitted: u32,
        inheritable: u32,
    }
    let header = Header {
        version: 0x20080522,
        pid: 0,
    };
    let zero = Caps {
        effective: 0,
        permitted: 0,
        inheritable: 0,
    };
    let mut caps = [zero; 2];
    checked(
        9,
        unsafe { libc::syscall(libc::SYS_capset, &header, caps.as_ptr()) } as i32,
    )?;
    checked(
        9,
        unsafe { libc::syscall(libc::SYS_capget, &header, caps.as_mut_ptr()) } as i32,
    )?;
    if caps != [zero; 2] {
        return Err(invalid(9));
    }
    for cap in 0..last {
        if unsafe { libc::prctl(libc::PR_CAPBSET_READ, cap, 0, 0, 0) } != 0
            || unsafe { libc::prctl(libc::PR_CAP_AMBIENT, libc::PR_CAP_AMBIENT_IS_SET, cap, 0, 0) }
                != 0
        {
            return Err(invalid(9));
        }
    }
    checked(9, unsafe {
        libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0)
    })?;
    if unsafe { libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) } != 1
        || unsafe { libc::prctl(libc::PR_GET_DUMPABLE, 0, 0, 0, 0) } != 0
    {
        return Err(invalid(9));
    }
    Ok(())
}

pub(super) fn stdio() -> Result<()> {
    for fd in 0..=2 {
        let mut stat = std::mem::MaybeUninit::<libc::stat>::zeroed();
        checked(2, unsafe { libc::fstat(fd, stat.as_mut_ptr()) })?;
        let stat = unsafe { stat.assume_init() };
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        checked(2, flags)?;
        if fd == 0 {
            if stat.st_mode & libc::S_IFMT != libc::S_IFCHR
                || stat.st_rdev != libc::makedev(1, 3)
                || flags & libc::O_ACCMODE != libc::O_RDONLY
            {
                return Err(invalid(2));
            }
        } else if stat.st_mode & libc::S_IFMT != libc::S_IFIFO
            || flags & libc::O_ACCMODE != libc::O_WRONLY
        {
            return Err(invalid(2));
        }
    }
    Ok(())
}
pub(super) fn cwd(
    plan: &RunPlan,
    root: &File,
    resources: &Arc<RepositoryRootResources>,
    scope: &Arc<SourceServiceScope>,
) -> Result<()> {
    let suffix = plan
        .cwd
        .as_bytes()
        .strip_prefix(plan.display.as_bytes())
        .ok_or_else(|| invalid(14))?;
    let relative = if suffix.is_empty() {
        b"".as_slice()
    } else {
        suffix.strip_prefix(b"/").ok_or_else(|| invalid(14))?
    };
    if plan.mode != RunMode::InitializeArchive {
        let _scratch = resources
            .reserve(IO_SCRATCH, 3, 0)
            .map_err(|_| invalid(14))?;
        scope.check().map_err(|_| invalid(14))?;
        let limit = RootIoLimits {
            max_path_bytes: 4096usize
                .saturating_sub(plan.display.as_bytes().len().saturating_add(1)),
            max_name_bytes: 255,
            max_depth: 64,
            max_entries: 8192,
            max_directory_name_bytes: 1024 * 1024,
            max_file_bytes: PROCESS_BYTES,
            max_link_bytes: 4096,
        };
        RootIo::new(root, limit)
            .map_err(|e| failure(14, e))?
            .ensure_directory(Path::new(std::ffi::OsStr::from_bytes(relative)), 0o755)
            .map_err(|e| failure(14, e))?;
        scope.check().map_err(|_| invalid(14))?;
    } else if !relative.is_empty() {
        return Err(invalid(14));
    }
    // The cwd is reached from the held actual root, never a Host display reopen.
    checked(14, unsafe { libc::fchdir(root.as_raw_fd()) })?;
    if !relative.is_empty() {
        let relative = cpath(relative)?;
        checked(14, unsafe { libc::chdir(relative.as_ptr()) })?;
    }
    Ok(())
}
