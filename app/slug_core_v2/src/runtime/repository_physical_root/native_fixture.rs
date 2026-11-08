//! Trusted Rust-only fixture. No original or arbitrary payload runs here.
use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::fs::symlink;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use slug_bzlmod_v2::RepositoryWorkspaceMode;

const BWRAP: &str = "/home/linuxbrew/.linuxbrew/Cellar/bubblewrap/0.11.2/bin/bwrap";
const HELPER: &str =
    "runtime::repository_physical_root::native_fixture::trusted_tmpfs_fixture_helper";
const MAGIC: &[u8; 8] = b"SLUG425R";
const FRAME_BYTES: usize = 74;
const TEXT_BYTES: usize = 4096;
const STDERR_BYTES: usize = 16 * 1024;
const DEADLINE: Duration = Duration::from_secs(5);
pub(super) const BODY: &[u8] = b"wp425 trusted tmpfs\0\xff";

fn failure(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

struct Reap(Child);
impl Drop for Reap {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            // The launcher has its own process group. Its private PID namespace
            // and --die-with-parent additionally retire the fixed helper.
            unsafe {
                libc::kill(-(self.0.id() as i32), libc::SIGKILL);
            }
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn nonblocking(fd: i32) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn drain(input: &mut impl Read, output: &mut Vec<u8>, cap: usize) -> io::Result<bool> {
    let mut bytes = [0; 1024];
    loop {
        match input.read(&mut bytes) {
            Ok(0) => return Ok(true),
            Ok(count) => {
                if count > cap.saturating_sub(output.len()) {
                    return Err(failure("trusted fixture output exceeded its bound"));
                }
                output.extend_from_slice(&bytes[..count]);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
}

fn pause() -> io::Result<()> {
    if unsafe { libc::poll(std::ptr::null_mut(), 0, 2) } < 0 {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
    Ok(())
}

fn preflight(binary: &Path) -> io::Result<()> {
    let child = Command::new(binary)
        .args(["--list", "--ignored", "--exact", HELPER])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    let mut child = Reap(child);
    let mut stdout = child.0.stdout.take().unwrap();
    let mut stderr = child.0.stderr.take().unwrap();
    nonblocking(stdout.as_raw_fd())?;
    nonblocking(stderr.as_raw_fd())?;
    let mut out = Vec::with_capacity(TEXT_BYTES);
    let mut err = Vec::with_capacity(TEXT_BYTES);
    let deadline = Instant::now() + DEADLINE;
    loop {
        let out_end = drain(&mut stdout, &mut out, TEXT_BYTES)?;
        let err_end = drain(&mut stderr, &mut err, TEXT_BYTES)?;
        if let Some(status) = child.0.try_wait()? {
            if out_end && err_end {
                let text = std::str::from_utf8(&out).map_err(|error| failure(error.to_string()))?;
                let expected = format!("{HELPER}: test");
                if !status.success()
                    || !err.is_empty()
                    || text
                        .lines()
                        .filter(|line| line.ends_with(": test"))
                        .collect::<Vec<_>>()
                        != [expected.as_str()]
                {
                    return Err(failure("exact ignored trusted helper preflight failed"));
                }
                return Ok(());
            }
        }
        if Instant::now() >= deadline {
            return Err(failure("trusted helper preflight timed out"));
        }
        pause()?;
    }
}

fn mode_values(mode: RepositoryWorkspaceMode) -> (u8, u64, u64, &'static str) {
    match mode {
        RepositoryWorkspaceMode::Archive => (0, 2 * 1024 * 1024 * 1024, 262_144, "archive"),
        RepositoryWorkspaceMode::Process => (1, 64 * 1024 * 1024, 8192, "process"),
    }
}

fn facts(fd: i32, mode: RepositoryWorkspaceMode, bound_subdirectory: bool) -> io::Result<[u64; 6]> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    let mut fs = std::mem::MaybeUninit::<libc::statfs>::uninit();
    let mut vfs = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    let mut extended = std::mem::MaybeUninit::<libc::statx>::zeroed();
    if unsafe { libc::fstat(fd, stat.as_mut_ptr()) } != 0
        || unsafe { libc::fstatfs(fd, fs.as_mut_ptr()) } != 0
        || unsafe { libc::fstatvfs(fd, vfs.as_mut_ptr()) } != 0
        || unsafe {
            libc::statx(
                fd,
                c"".as_ptr(),
                libc::AT_EMPTY_PATH | libc::AT_SYMLINK_NOFOLLOW,
                libc::STATX_BASIC_STATS | libc::STATX_MNT_ID,
                extended.as_mut_ptr(),
            )
        } != 0
    {
        return Err(io::Error::last_os_error());
    }
    let (stat, fs, vfs, extended) = unsafe {
        (
            stat.assume_init(),
            fs.assume_init(),
            vfs.assume_init(),
            extended.assume_init(),
        )
    };
    let (_, expected_bytes, expected_inodes, _) = mode_values(mode);
    let bytes = vfs
        .f_blocks
        .checked_mul(vfs.f_frsize)
        .ok_or_else(|| failure("quota overflow"))?;
    // STATX_ATTR_MOUNT_ROOT, required separately from the inode=1 discriminator.
    const MOUNT_ROOT: u64 = 0x0000_2000;
    if unsafe { libc::sysconf(libc::_SC_PAGESIZE) } != 4096
        || fs.f_type as u64 != 0x0102_1994
        || fs.f_bsize != 4096
        || vfs.f_frsize != 4096
        || bytes != expected_bytes
        || vfs.f_files != expected_inodes
        || if bound_subdirectory {
            stat.st_ino <= 1
        } else {
            stat.st_ino != 1
        }
        || stat.st_mode & libc::S_IFMT != libc::S_IFDIR
        || extended.stx_mask & libc::STATX_MNT_ID == 0
        || extended.stx_mnt_id == 0
        || extended.stx_attributes_mask & MOUNT_ROOT == 0
        || extended.stx_attributes & MOUNT_ROOT == 0
        || extended.stx_ino != stat.st_ino
    {
        return Err(failure(
            "actual fixture is not the required tmpfs mount root/quota/page policy",
        ));
    }
    Ok([
        stat.st_dev,
        stat.st_ino,
        extended.stx_mnt_id,
        bytes,
        vfs.f_files,
        vfs.f_frsize,
    ])
}

fn frame(
    mode: RepositoryWorkspaceMode,
    nonce: &[u8; 16],
    facts: [u64; 6],
    bound_subdirectory: bool,
) -> [u8; FRAME_BYTES] {
    let mut frame = [0; FRAME_BYTES];
    frame[..8].copy_from_slice(MAGIC);
    frame[8] = 1;
    frame[9] = mode_values(mode).0 | if bound_subdirectory { 0x80 } else { 0 };
    frame[10..26].copy_from_slice(nonce);
    for (index, value) in facts.into_iter().enumerate() {
        frame[26 + index * 8..34 + index * 8].copy_from_slice(&value.to_le_bytes());
    }
    frame
}

#[repr(C, align(8))]
struct Control([u8; 128]);

fn send(fd: i32, bytes: &[u8]) -> io::Result<()> {
    let mut control = Control([0; 128]);
    let mut iov = libc::iovec {
        iov_base: bytes.as_ptr().cast_mut().cast(),
        iov_len: bytes.len(),
    };
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;
    message.msg_control = control.0.as_mut_ptr().cast();
    message.msg_controllen =
        unsafe { libc::CMSG_SPACE(std::mem::size_of::<i32>() as u32) } as usize;
    unsafe {
        let header = libc::CMSG_FIRSTHDR(&message);
        (*header).cmsg_level = libc::SOL_SOCKET;
        (*header).cmsg_type = libc::SCM_RIGHTS;
        (*header).cmsg_len = libc::CMSG_LEN(std::mem::size_of::<i32>() as u32) as usize;
        std::ptr::write_unaligned(libc::CMSG_DATA(header).cast::<i32>(), fd);
    }
    let sent = unsafe { libc::sendmsg(1, &message, libc::MSG_NOSIGNAL) };
    if sent < 0 {
        return Err(io::Error::last_os_error());
    }
    if sent as usize != bytes.len() {
        return Err(failure("short trusted root frame"));
    }
    Ok(())
}

fn receive(socket: i32) -> io::Result<Option<(Vec<u8>, Vec<OwnedFd>)>> {
    let mut bytes = [0; TEXT_BYTES + 1];
    let mut control = Control([0; 128]);
    let mut iov = libc::iovec {
        iov_base: bytes.as_mut_ptr().cast(),
        iov_len: bytes.len(),
    };
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;
    message.msg_control = control.0.as_mut_ptr().cast();
    message.msg_controllen = control.0.len();
    let count = unsafe {
        libc::recvmsg(
            socket,
            &mut message,
            libc::MSG_CMSG_CLOEXEC | libc::MSG_DONTWAIT,
        )
    };
    if count < 0 {
        let error = io::Error::last_os_error();
        return if matches!(
            error.kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
        ) {
            Ok(None)
        } else {
            Err(error)
        };
    }
    let mut rights = Vec::with_capacity(32);
    let mut bad_control = false;
    unsafe {
        let mut header = libc::CMSG_FIRSTHDR(&message);
        while !header.is_null() {
            if (*header).cmsg_level == libc::SOL_SOCKET && (*header).cmsg_type == libc::SCM_RIGHTS {
                let base = libc::CMSG_LEN(0) as usize;
                let length = (*header).cmsg_len.saturating_sub(base);
                if length % std::mem::size_of::<i32>() != 0 {
                    bad_control = true;
                }
                for index in 0..length / std::mem::size_of::<i32>() {
                    let descriptor =
                        std::ptr::read_unaligned(libc::CMSG_DATA(header).cast::<i32>().add(index));
                    rights.push(OwnedFd::from_raw_fd(descriptor));
                }
            } else {
                bad_control = true;
            }
            header = libc::CMSG_NXTHDR(&message, header);
        }
    }
    // Received descriptors are owned before any failure/flag/frame validation.
    if bad_control || message.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0 {
        return Err(failure("invalid/truncated trusted descriptor control"));
    }
    Ok(Some((bytes[..count as usize].to_vec(), rights)))
}

pub(super) fn root(mode: RepositoryWorkspaceMode) -> File {
    root_checked(mode, false)
        .unwrap_or_else(|error| panic!("required trusted tmpfs fixture failed: {error}"))
}

/// Negative discriminator: a real bind mount of a quota-sharing subdirectory
/// has MOUNT_ROOT too, but must never be admitted as the whole tmpfs root.
pub(super) fn bound_subdirectory(mode: RepositoryWorkspaceMode) -> File {
    root_checked(mode, true)
        .unwrap_or_else(|error| panic!("required bound-subdirectory fixture failed: {error}"))
}

fn root_checked(mode: RepositoryWorkspaceMode, bound_subdirectory: bool) -> io::Result<File> {
    let launcher = Path::new(BWRAP);
    let metadata = std::fs::metadata(launcher)?;
    if !metadata.is_file()
        || unsafe { libc::access(std::ffi::CString::new(BWRAP).unwrap().as_ptr(), libc::X_OK) } != 0
    {
        return Err(failure("required pinned bwrap path is not executable"));
    }
    let binary = std::env::current_exe()?;
    preflight(&binary)?;
    let reservation = tempfile::Builder::new()
        .prefix("slug-wp425-native-")
        .tempdir()?;
    let display = reservation.path().join("root");
    std::fs::create_dir(&display)?;
    let mut nonce = [0; 16];
    File::open("/dev/urandom")?.read_exact(&mut nonce)?;
    let nonce_text = nonce
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let mut pair = [-1; 2];
    if unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
            0,
            pair.as_mut_ptr(),
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    let (socket, child_socket) =
        unsafe { (OwnedFd::from_raw_fd(pair[0]), OwnedFd::from_raw_fd(pair[1])) };
    nonblocking(socket.as_raw_fd())?;
    let child = Command::new(launcher)
        .args([
            "--unshare-user",
            "--unshare-pid",
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
            "--ro-bind",
            "/",
            "/",
            "--proc",
            "/proc",
            "--dev",
            "/dev",
        ])
        .args([
            "--setenv",
            "SLUG_WP425_ROOT_FIXTURE",
            "v1",
            "--setenv",
            "SLUG_WP425_ROOT_MODE",
            mode_values(mode).3,
            "--setenv",
            "SLUG_WP425_ROOT_VIEW",
            if bound_subdirectory {
                "bound_subdirectory"
            } else {
                "mount_root"
            },
            "--setenv",
            "SLUG_WP425_ROOT_NONCE",
            &nonce_text,
            "--setenv",
            "SLUG_WP425_ROOT_DISPLAY",
        ])
        .arg(&display)
        .arg("--")
        .arg(&binary)
        .args([
            "--ignored",
            "--exact",
            HELPER,
            "--nocapture",
            "--test-threads=1",
        ])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::from(child_socket))
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    let mut child = Reap(child);
    let mut stderr = child.0.stderr.take().unwrap();
    nonblocking(stderr.as_raw_fd())?;
    let mut err = Vec::with_capacity(STDERR_BYTES);
    let deadline = Instant::now() + DEADLINE;
    let mut received = None;
    let mut text_bytes = 0usize;
    let mut socket_end = false;
    loop {
        let stderr_end = drain(&mut stderr, &mut err, STDERR_BYTES)?;
        while !socket_end {
            let Some((bytes, mut rights)) = receive(socket.as_raw_fd())? else {
                break;
            };
            if bytes.is_empty() {
                if !rights.is_empty() {
                    return Err(failure("descriptors on empty fixture packet"));
                }
                socket_end = true;
                break;
            }
            if bytes.starts_with(MAGIC) {
                if bytes.len() != FRAME_BYTES || received.is_some() || rights.len() != 1 {
                    return Err(failure("unexpected trusted root frame/count"));
                }
                let descriptor = rights.pop().unwrap();
                let actual = facts(descriptor.as_raw_fd(), mode, bound_subdirectory)?;
                if bytes != frame(mode, &nonce, actual, bound_subdirectory) {
                    return Err(failure("trusted root nonce/version/mode/metadata mismatch"));
                }
                if unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC
                    == 0
                {
                    return Err(failure("received root descriptor is not close-on-exec"));
                }
                received = Some(descriptor);
            } else {
                // Only bounded libtest pre/post text may surround the one frame.
                // Such text cannot carry descriptors or select fixture behavior.
                text_bytes = text_bytes
                    .checked_add(bytes.len())
                    .ok_or_else(|| failure("fixture text overflow"))?;
                if !rights.is_empty() || text_bytes > TEXT_BYTES || !bytes.is_ascii() {
                    return Err(failure("invalid trusted harness text"));
                }
            }
        }
        if let Some(status) = child.0.try_wait()? {
            if stderr_end && socket_end {
                if !status.success() {
                    return Err(failure(format!(
                        "trusted helper failed {status}: {}",
                        String::from_utf8_lossy(&err)
                    )));
                }
                let descriptor = received.ok_or_else(|| failure("trusted helper sent no root"))?;
                facts(descriptor.as_raw_fd(), mode, bound_subdirectory)?;
                // The helper and launcher are reaped before the File escapes.
                // Removing this Host reservation cannot remove the received root.
                drop(reservation);
                return Ok(File::from(descriptor));
            }
        }
        if Instant::now() >= deadline {
            return Err(failure(format!(
                "trusted helper timed out: {}",
                String::from_utf8_lossy(&err)
            )));
        }
        pause()?;
    }
}

#[test]
#[ignore = "trusted internal helper; requires parent-authenticated v1 fixture environment"]
fn trusted_tmpfs_fixture_helper() {
    assert_eq!(
        std::env::var("SLUG_WP425_ROOT_FIXTURE").as_deref(),
        Ok("v1")
    );
    let mode = match std::env::var("SLUG_WP425_ROOT_MODE").unwrap().as_str() {
        "archive" => RepositoryWorkspaceMode::Archive,
        "process" => RepositoryWorkspaceMode::Process,
        _ => panic!("invalid fixed fixture mode"),
    };
    let bound_subdirectory = match std::env::var("SLUG_WP425_ROOT_VIEW").unwrap().as_str() {
        "mount_root" => false,
        "bound_subdirectory" => true,
        _ => panic!("invalid fixed fixture view"),
    };
    let display = std::path::PathBuf::from(std::env::var_os("SLUG_WP425_ROOT_DISPLAY").unwrap());
    assert!(display.is_absolute() && display.file_name().unwrap() == "root");
    assert!(
        display
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .as_bytes()
            .starts_with(b"slug-wp425-native-")
    );
    assert!(std::fs::symlink_metadata(&display).unwrap().is_dir());
    let nonce_text = std::env::var("SLUG_WP425_ROOT_NONCE").unwrap();
    assert_eq!(nonce_text.len(), 32);
    let mut nonce = [0; 16];
    for (index, byte) in nonce.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&nonce_text[index * 2..index * 2 + 2], 16).unwrap();
    }
    assert_eq!(unsafe { libc::geteuid() }, 0);
    assert_eq!(unsafe { libc::getegid() }, 0);
    // No inherited setup/control descriptor reaches fixture operations beyond
    // stdio. This occurs after re-exec, never in an allocating pre_exec closure.
    assert_eq!(
        unsafe { libc::syscall(libc::SYS_close_range, 3u32, u32::MAX, 0u32) },
        0
    );
    let target = std::ffi::CString::new(display.as_os_str().as_bytes()).unwrap();
    let (_, bytes, inodes, _) = mode_values(mode);
    let options =
        std::ffi::CString::new(format!("size={bytes},nr_inodes={inodes},mode=0755")).unwrap();
    assert_eq!(
        unsafe {
            libc::mount(
                c"tmpfs".as_ptr(),
                target.as_ptr(),
                c"tmpfs".as_ptr(),
                libc::MS_NOSUID | libc::MS_NODEV,
                options.as_ptr().cast(),
            )
        },
        0,
        "trusted tmpfs mount failed: {}",
        io::Error::last_os_error()
    );
    let root = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&display)
        .unwrap();
    assert_eq!(unsafe { libc::fchdir(root.as_raw_fd()) }, 0);
    assert_eq!(unsafe { libc::fchmod(root.as_raw_fd(), 0o755) }, 0);
    std::fs::create_dir("dir").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions("dir", std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut body = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o644)
        .open("dir/body")
        .unwrap();
    use std::io::Write;
    body.write_all(BODY).unwrap();
    body.set_permissions(std::fs::Permissions::from_mode(0o644))
        .unwrap();
    drop(body);
    symlink("dir/body", "link").unwrap();
    let root = if bound_subdirectory {
        std::fs::create_dir("bound").unwrap();
        let source = std::ffi::CString::new(display.join("dir").as_os_str().as_bytes()).unwrap();
        let target = std::ffi::CString::new(display.join("bound").as_os_str().as_bytes()).unwrap();
        assert_eq!(
            unsafe {
                libc::mount(
                    source.as_ptr(),
                    target.as_ptr(),
                    std::ptr::null(),
                    libc::MS_BIND,
                    std::ptr::null(),
                )
            },
            0,
            "trusted subdirectory bind failed: {}",
            io::Error::last_os_error()
        );
        let subdirectory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(display.join("bound"))
            .unwrap();
        drop(root);
        subdirectory
    } else {
        root
    };
    let actual = facts(root.as_raw_fd(), mode, bound_subdirectory).unwrap();
    send(
        root.as_raw_fd(),
        &frame(mode, &nonce, actual, bound_subdirectory),
    )
    .unwrap();
    drop(root);
}
