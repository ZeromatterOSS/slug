//! Fixed trusted rolling setup, with shared bounded process/control plumbing.
use std::io::Read;
use std::os::fd::AsFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::process::Stdio;
use std::time::Instant;

use super::super::native_fixture::BWRAP;
use super::super::native_fixture::DEADLINE;
use super::super::native_fixture::Reap;
use super::super::native_fixture::STDERR_BYTES;
use super::super::native_fixture::TEXT_BYTES;
use super::super::native_fixture::drain;
use super::super::native_fixture::nonblocking;
use super::super::native_fixture::pause;
use super::super::native_fixture::preflight_for;
use super::super::native_fixture::receive_bounded;
use super::super::tests::resources;
use super::super::tests::scope;
use super::*;

pub(super) const HELPER: &str =
    "runtime::repository_physical_root::mount_lease::native_fixture::trusted_rolling_mount_helper";
const MAGIC: &[u8; 8] = b"SLUG425M";
const FRAME: usize = 98;
pub(super) const BODIES: [&[u8]; 3] = [b"one", b"two", b"three"];
fn fail(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

fn frame(kind: u8, phase: usize, nonce: &[u8; 16], facts: [u64; 8]) -> [u8; FRAME] {
    let mut bytes = [0; FRAME];
    bytes[..8].copy_from_slice(MAGIC);
    bytes[8] = 1;
    bytes[9] = kind;
    bytes[10..18].copy_from_slice(&(phase as u64).to_le_bytes());
    bytes[18..34].copy_from_slice(nonce);
    for (i, value) in facts.into_iter().enumerate() {
        bytes[34 + i * 8..42 + i * 8].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}
fn send(socket: i32, bytes: &[u8], fds: &[i32]) -> io::Result<()> {
    #[repr(C, align(8))]
    struct Control([u8; 32]);
    let mut control = Control([0; 32]);
    let mut iov = libc::iovec {
        iov_base: bytes.as_ptr().cast_mut().cast(),
        iov_len: bytes.len(),
    };
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;
    if !fds.is_empty() {
        message.msg_control = control.0.as_mut_ptr().cast();
        message.msg_controllen = unsafe { libc::CMSG_SPACE((fds.len() * 4) as u32) } as usize;
        unsafe {
            let header = libc::CMSG_FIRSTHDR(&message);
            (*header).cmsg_level = libc::SOL_SOCKET;
            (*header).cmsg_type = libc::SCM_RIGHTS;
            (*header).cmsg_len = libc::CMSG_LEN((fds.len() * 4) as u32) as usize;
            for (i, fd) in fds.iter().enumerate() {
                std::ptr::write_unaligned(libc::CMSG_DATA(header).cast::<i32>().add(i), *fd);
            }
        }
    }
    let count = unsafe { libc::sendmsg(socket, &message, libc::MSG_NOSIGNAL) };
    if count < 0 {
        return Err(io::Error::last_os_error());
    }
    if count as usize != bytes.len() {
        return Err(fail("short fixed rolling frame"));
    }
    Ok(())
}
fn wait_packet(socket: i32, max_fds: usize) -> io::Result<(Vec<u8>, Vec<OwnedFd>)> {
    let deadline = Instant::now() + DEADLINE;
    loop {
        if let Some(value) = receive_bounded(socket, max_fds)? {
            return Ok(value);
        }
        if Instant::now() >= deadline {
            return Err(fail("fixed helper receive timeout"));
        }
        pause()?;
    }
}
fn verify_before_payload(file: &File, phase: usize) -> io::Result<()> {
    let limits = super::super::RootIoLimits {
        max_path_bytes: 4096,
        max_name_bytes: 255,
        max_depth: 64,
        max_entries: 8192,
        max_directory_name_bytes: 1024 * 1024,
        max_file_bytes: 64 * 1024 * 1024,
        max_link_bytes: 4096,
    };
    let root = RootIo::new(file, limits)?;
    if phase == 0 {
        if !matches!(root.lstat(Path::new("out")),Err(error)if error.kind()==io::ErrorKind::NotFound)
        {
            return Err(fail("bootstrap payload ran before handoff"));
        }
    } else {
        let mut bytes = Vec::with_capacity(16);
        root.read_file(Path::new("out"), &|| true, |chunk| {
            if bytes.len() + chunk.len() > 16 {
                return Err(fail("fixed body overflow"));
            }
            bytes.extend_from_slice(chunk);
            Ok(())
        })?;
        if bytes != BODIES[phase - 1] {
            return Err(fail("next payload ran before successor handoff"));
        }
    }
    Ok(())
}

pub(super) struct Receipt {
    pub root: File,
    pub successor: AttachedMountClone,
    pub backing: Arc<RepositoryRootBacking>,
    pub _receipt: resources::ResourceCredit,
}
/// Returns only after the fixed helper and namespace launcher are reaped. The
/// successor was received and old bytes verified BEFORE the payload ACK.
pub(super) fn launch(
    phase: usize,
    display: &Path,
    mut attempt: Option<RepositoryMountAttempt>,
    root: Option<&RepositoryPhysicalRootBuilder>,
    resources: &Arc<RepositoryRootResources>,
    scope: &Arc<SourceServiceScope>,
) -> io::Result<(Receipt, Option<MountInFlight>)> {
    scope.check().map_err(|e| fail(format!("{e:?}")))?;
    // The root may be created only after its full backing allowance is paid.
    // Later launches retain the exact guard rather than reserving quota again.
    let backing = if let Some(root) = root {
        root.backing.clone()
    } else {
        RepositoryRootBacking::new(resources, RepositoryWorkspaceMode::Process)
            .map_err(|e| fail(format!("{e:?}")))?
    };
    backing
        .matches(resources, RepositoryWorkspaceMode::Process)
        .map_err(|e| fail(format!("{e:?}")))?;
    let binary = std::env::current_exe()?;
    {
        let _preflight_credit = resources
            .reserve(16 * 1024, 8, 0)
            .map_err(|e| fail(format!("{e:?}")))?;
        preflight_for(&binary, HELPER)?;
    }
    if !Path::new(BWRAP).is_file() {
        return Err(fail("required bwrap unavailable"));
    }
    let _control_credit = resources
        .reserve(128 * 1024, 3, 0)
        .map_err(|e| fail(format!("{e:?}")))?;
    let receive_credit = resources
        .reserve(1024, 3, 0)
        .map_err(|e| fail(format!("{e:?}")))?;
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
    let mut nonce = [0; 16];
    File::open("/dev/urandom")?.read_exact(&mut nonce)?;
    let text = nonce.iter().map(|v| format!("{v:02x}")).collect::<String>();
    let mut command = Command::new(BWRAP);
    if let Some(attempt) = attempt.as_ref() {
        let owner = attempt.owning_user_namespace().as_raw_fd();
        command.args(["--userns", &owner.to_string()]);
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(owner, libc::F_SETFD, 0) < 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
    } else {
        command.arg("--unshare-user");
    }
    command
        .args([
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
            "SLUG_WP425_MOUNT_FIXTURE",
            "v1",
            "--setenv",
            "SLUG_WP425_MOUNT_PHASE",
            &phase.to_string(),
            "--setenv",
            "SLUG_WP425_MOUNT_NONCE",
            &text,
            "--setenv",
            "SLUG_WP425_MOUNT_DISPLAY",
        ])
        .arg(display)
        .arg("--")
        .arg(binary)
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
        .process_group(0);
    let mut child = Reap(command.spawn()?);
    drop(command);
    let mut stderr = child.0.stderr.take().unwrap();
    nonblocking(stderr.as_raw_fd())?;
    let mut err = Vec::with_capacity(STDERR_BYTES);
    let deadline = Instant::now() + DEADLINE;
    let mut noise = 0usize;
    let mut ready = false;
    let mut end = false;
    let mut receipt = None;
    let mut inflight = None;
    loop {
        scope.check().map_err(|e| fail(format!("{e:?}")))?;
        let stderr_end = drain(&mut stderr, &mut err, STDERR_BYTES)?;
        while !end {
            let Some((bytes, mut fds)) = receive_bounded(socket.as_raw_fd(), 3)? else {
                break;
            };
            if bytes.is_empty() {
                if !fds.is_empty() {
                    return Err(fail("rights on EOF"));
                }
                end = true;
                break;
            }
            if !bytes.starts_with(MAGIC) {
                noise += bytes.len();
                if noise > TEXT_BYTES || !bytes.is_ascii() || !fds.is_empty() {
                    return Err(fail("bad rolling harness text"));
                }
                continue;
            }
            if bytes == frame(0, phase, &nonce, [0; 8]) {
                if ready || !fds.is_empty() {
                    return Err(fail("bad READY"));
                }
                ready = true;
                if phase > 0 {
                    let attempt = attempt
                        .take()
                        .ok_or_else(|| fail("missing linear incoming lease"))?;
                    inflight = Some(
                        attempt
                            .transfer(
                                root.ok_or_else(|| fail("missing root association"))?,
                                socket.as_fd(),
                                &nonce,
                                scope,
                                &|| true,
                            )
                            .map_err(|e| fail(format!("{e:?}")))?,
                    );
                }
                continue;
            }
            if bytes.len() != FRAME || !ready || receipt.is_some() || fds.len() != 3 {
                return Err(fail("bad successor frame"));
            }
            let owner = File::from(fds.pop().unwrap());
            let token = File::from(fds.pop().unwrap());
            let attached_file = File::from(fds.pop().unwrap());
            let attached = identity(&attached_file).map_err(|e| fail(format!("{e:?}")))?;
            let detached = identity(&token).map_err(|e| fail(format!("{e:?}")))?;
            let namespace = namespace_identity(&owner).map_err(|e| fail(format!("{e:?}")))?;
            let facts = [
                attached.device,
                attached.inode,
                attached.mount,
                detached.device,
                detached.inode,
                detached.mount,
                namespace.device,
                namespace.inode,
            ];
            if bytes != frame(1, phase, &nonce, facts)
                || !same_backing(attached, detached)
                || attached.mount == detached.mount
            {
                return Err(fail("successor identity/nonce mismatch"));
            }
            {
                let _verification_credit = resources
                    .reserve(IO_SCRATCH, 3, 0)
                    .map_err(|e| fail(format!("{e:?}")))?;
                verify_before_payload(&attached_file, phase)?;
            }
            let credit = resources
                .reserve(std::mem::size_of::<AttachedMountClone>(), 2, 0)
                .map_err(|e| fail(format!("{e:?}")))?;
            let successor = AttachedMountClone {
                token,
                owner,
                attached,
                detached,
                namespace,
                resources: resources.clone(),
                backing: Some(backing.clone()),
                credit,
            };
            receipt = Some((attached_file, successor));
            send(socket.as_raw_fd(), &frame(2, phase, &nonce, [0; 8]), &[])?;
        }
        if let Some(status) = child.0.try_wait()? {
            if end && stderr_end {
                if !status.success() {
                    return Err(fail(format!(
                        "trusted rolling child failed {status}: {}",
                        String::from_utf8_lossy(&err)
                    )));
                }
                let (root, successor) = receipt.ok_or_else(|| fail("no pre-payload successor"))?;
                return Ok((
                    Receipt {
                        root,
                        successor,
                        backing,
                        _receipt: receive_credit,
                    },
                    inflight,
                ));
            }
        }
        if Instant::now() >= deadline {
            return Err(fail(format!(
                "rolling setup timed out: {}",
                String::from_utf8_lossy(&err)
            )));
        }
        pause()?;
    }
}

#[test]
#[ignore = "trusted fixed rolling helper; requires parent-authenticated environment"]
fn trusted_rolling_mount_helper() {
    assert_eq!(
        std::env::var("SLUG_WP425_MOUNT_FIXTURE").as_deref(),
        Ok("v1")
    );
    let phase: usize = std::env::var("SLUG_WP425_MOUNT_PHASE")
        .unwrap()
        .parse()
        .unwrap();
    assert!(phase < 3);
    let display = std::path::PathBuf::from(std::env::var_os("SLUG_WP425_MOUNT_DISPLAY").unwrap());
    assert!(
        display.is_absolute()
            && display.file_name().unwrap() == "root"
            && display
                .parent()
                .unwrap()
                .file_name()
                .unwrap()
                .as_bytes()
                .starts_with(b"slug-wp425-rolling-")
    );
    let text = std::env::var("SLUG_WP425_MOUNT_NONCE").unwrap();
    assert_eq!(text.len(), 32);
    let mut nonce = [0; 16];
    for (i, v) in nonce.iter_mut().enumerate() {
        *v = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).unwrap();
    }
    assert_eq!(
        unsafe { libc::syscall(libc::SYS_close_range, 3u32, u32::MAX, 0u32) },
        0
    );
    let resources = resources();
    let scope = scope();
    let _incoming_credit = resources.reserve(8192, 3, 0).unwrap();
    send(1, &frame(0, phase, &nonce, [0; 8]), &[]).unwrap();
    let initial_credit;
    let attached;
    let initial;
    if phase == 0 {
        assert_eq!(unsafe { libc::sysconf(libc::_SC_PAGESIZE) }, 4096);
        let display = std::ffi::CString::new(display.as_os_str().as_bytes()).unwrap();
        assert_eq!(
            unsafe {
                libc::mount(
                    c"tmpfs".as_ptr(),
                    display.as_ptr(),
                    c"tmpfs".as_ptr(),
                    libc::MS_NOSUID | libc::MS_NODEV,
                    c"size=67108864,nr_inodes=8192,mode=0755".as_ptr().cast(),
                )
            },
            0,
            "{}",
            io::Error::last_os_error()
        );
        initial_credit = Some(resources.reserve(1024, 1, 0).unwrap());
        attached = None;
        initial = Some(
            std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW)
                .open(Path::new(std::str::from_utf8(display.as_bytes()).unwrap()))
                .unwrap(),
        );
    } else {
        let (bytes, mut fds) = wait_packet(1, 3).unwrap();
        assert_eq!(bytes.len(), 33);
        assert_eq!(&bytes[..8], TRANSFER_MAGIC);
        assert_eq!(bytes[8], 1);
        assert_eq!(&bytes[9..25], &nonce);
        assert_eq!(
            u64::from_le_bytes(bytes[25..33].try_into().unwrap()),
            phase as u64
        );
        assert_eq!(fds.len(), 3);
        let owner = File::from(fds.pop().unwrap());
        let token = File::from(fds.pop().unwrap());
        let original = File::from(fds.pop().unwrap());
        attached = Some(
            attach_in_owning_namespace(token, &owner, &display, &resources, &scope, &|| true)
                .unwrap(),
        );
        assert!(same_backing(
            identity(&original).unwrap(),
            identity(&attached.as_ref().unwrap().file).unwrap()
        ));
        initial_credit = None;
        // Borrow without descriptor duplication below; ownership stays in the
        // credited AttachedSetupRoot until after the payload and handoff.
        drop(owner);
        drop(original);
        initial = None;
    }
    let root = initial
        .as_ref()
        .unwrap_or_else(|| &attached.as_ref().unwrap().file);
    let successor = clone_from_attached_root(root, &display, &resources, &scope, &|| true).unwrap();
    let a = successor.attached;
    let d = successor.detached;
    let n = successor.namespace;
    send(
        1,
        &frame(
            1,
            phase,
            &nonce,
            [
                a.device, a.inode, a.mount, d.device, d.inode, d.mount, n.device, n.inode,
            ],
        ),
        &[
            root.as_raw_fd(),
            successor.token.as_raw_fd(),
            successor.owner.as_raw_fd(),
        ],
    )
    .unwrap();
    let (ack, fds) = wait_packet(1, 0).unwrap();
    assert!(fds.is_empty());
    assert_eq!(ack, frame(2, phase, &nonce, [0; 8]));
    assert_eq!(unsafe { libc::fchdir(root.as_raw_fd()) }, 0);
    // The selected cwd survives retirement of every setup handle. The fixed
    // payload begins with stdio only; its output FD is newly opened and paid.
    drop(successor);
    drop(initial);
    drop(attached);
    drop(initial_credit);
    drop(_incoming_credit);
    assert_eq!(resources.snapshot().native_fds, 0);
    {
        let redirect_credit = resources.reserve(1024, 1, 0).unwrap();
        let sink = std::fs::OpenOptions::new()
            .write(true)
            .open("/dev/null")
            .unwrap();
        assert_eq!(unsafe { libc::dup2(sink.as_raw_fd(), 1) }, 1);
        drop(sink);
        drop(redirect_credit);
    }
    assert_eq!(
        unsafe { libc::syscall(libc::SYS_close_range, 3u32, u32::MAX, 0u32) },
        0
    );
    assert_eq!(resources.snapshot().native_fds, 0);
    retire_capabilities();
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    let _output_credit = resources.reserve(1024, 1, 0).unwrap();
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o644)
        .open("out")
        .unwrap();
    output.write_all(BODIES[phase]).unwrap();
    output
        .set_permissions(std::fs::Permissions::from_mode(0o644))
        .unwrap();
    drop(output);
}

// Linux capability ABI v3 has two 32-bit words. All privilege and ambient
// capabilities retire before the trusted fixed write; this is not seccomp or
// a claim that arbitrary payloads are confined.
fn retire_capabilities() {
    #[repr(C)]
    struct Header {
        version: u32,
        pid: i32,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Data {
        effective: u32,
        permitted: u32,
        inheritable: u32,
    }
    let header = Header {
        version: 0x2008_0522,
        pid: 0,
    };
    let zero = Data {
        effective: 0,
        permitted: 0,
        inheritable: 0,
    };
    let mut values = [zero; 2];
    assert_eq!(
        unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) },
        0
    );
    assert_eq!(
        unsafe { libc::syscall(libc::SYS_capset, &header as *const Header, values.as_ptr()) },
        0
    );
    assert_eq!(
        unsafe {
            libc::prctl(
                libc::PR_CAP_AMBIENT,
                libc::PR_CAP_AMBIENT_CLEAR_ALL,
                0,
                0,
                0,
            )
        },
        0
    );
    assert_eq!(
        unsafe {
            libc::syscall(
                libc::SYS_capget,
                &header as *const Header,
                values.as_mut_ptr(),
            )
        },
        0
    );
    assert_eq!(values, [zero; 2]);
    assert_eq!(
        unsafe { libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) },
        1
    );
}
