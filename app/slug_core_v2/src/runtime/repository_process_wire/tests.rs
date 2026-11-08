use std::os::fd::AsFd;

use super::*;
fn plan() -> RunPlan {
    RunPlan {
        mode: RunMode::FirstExecute,
        display: CString::new("/__slug_repositories__/example").unwrap(),
        workspace: CString::new("/workspace").unwrap(),
        cwd: CString::new("/__slug_repositories__/example/sub").unwrap(),
        argv: vec![
            CString::new("/bin/sh").unwrap(),
            CString::new("literal whitespace;$(no shell)").unwrap(),
            CString::new("").unwrap(),
        ],
        env: vec![
            CString::new("PATH=/bin").unwrap(),
            CString::new("EMPTY=").unwrap(),
        ],
        mirrors: vec![MirrorEntry {
            name: CString::new("usr").unwrap(),
            fd: 6,
            device: 1,
            inode: 2,
            mode: libc::S_IFDIR | 0o755,
            link: None,
        }],
        nonce: [27; 16],
        index: 45,
        timeout_millis: 27000,
    }
}
fn sockets() -> (OwnedFd, OwnedFd) {
    let mut fds = [0; 2];
    assert_eq!(
        unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
                0,
                fds.as_mut_ptr(),
            )
        },
        0
    );
    unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
}
#[test]
fn plan_roundtrip_preserves_literal_arguments_environment_and_mirror_identity() {
    let p = plan();
    let encoded = p.encode().unwrap();
    let decoded = RunPlan::decode(&encoded).unwrap();
    assert_eq!(decoded.encode().unwrap(), encoded);
    assert_eq!(
        decoded.argv[1].as_bytes(),
        b"literal whitespace;$(no shell)"
    );
    assert!(decoded.argv[2].as_bytes().is_empty());
    assert_eq!(decoded.env[1].as_bytes(), b"EMPTY=");
    assert_eq!(decoded.mirrors[0].fd, 6);
    assert_eq!(decoded.index, 45);
}
#[test]
fn plan_refuses_truncation_trailing_bytes_version_and_forged_counts() {
    let encoded = plan().encode().unwrap();
    for length in 0..encoded.len() {
        assert!(RunPlan::decode(&encoded[..length]).is_err(), "{length}");
    }
    let mut extra = encoded.clone();
    extra.push(0);
    assert!(RunPlan::decode(&extra).is_err());
    let mut version = encoded.clone();
    version[8] = 2;
    assert!(RunPlan::decode(&version).is_err());
    let mut count = encoded;
    let mut offset = 42;
    for _ in 0..3 {
        let len = u32::from_le_bytes(count[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4 + len;
    }
    count[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(RunPlan::decode(&count).is_err());
}
#[test]
fn plan_path_scope_environment_roles_and_native_caps_fail_closed() {
    let mut p = plan();
    p.display = CString::new("/__slug_repositories__/../escape").unwrap();
    assert!(p.validate().is_err());
    p = plan();
    p.cwd = CString::new("/__slug_repositories__/example-other").unwrap();
    assert!(p.validate().is_err());
    p = plan();
    p.display = CString::new("/__slug_repositories__").unwrap();
    assert!(p.validate().is_err());
    p = plan();
    p.timeout_millis = 27001;
    assert!(p.validate().is_err());
    p = plan();
    p.env.push(CString::new("PATH=other").unwrap());
    assert!(p.validate().is_err());
    p = plan();
    p.mode = RunMode::InitializeArchive;
    assert!(p.validate().is_err());
    p.argv.clear();
    p.env.clear();
    p.index = 0;
    assert!(p.validate().is_ok());
    p = plan();
    p.argv = vec![CString::new(vec![b'x'; MAX_PLAN]).unwrap()];
    assert!(p.encode().is_err());
    p = plan();
    p.mirrors[0].name = CString::new("proc").unwrap();
    assert!(p.validate().is_err());
    p = plan();
    p.mirrors[0].fd = 7;
    assert!(p.validate().is_err());
}
#[test]
fn only_complete_immutable_memfd_plans_are_admitted() {
    let p = plan();
    let file = p.sealed_file().unwrap();
    assert_eq!(
        RunPlan::read_sealed(&file).unwrap().encode().unwrap(),
        p.encode().unwrap()
    );
    assert_eq!(unsafe { libc::ftruncate(file.as_raw_fd(), 0) }, -1);
    assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EPERM));
    let mut ordinary = tempfile::tempfile().unwrap();
    ordinary.write_all(&p.encode().unwrap()).unwrap();
    assert!(RunPlan::read_sealed(&ordinary).is_err());
    let fd = unsafe {
        libc::memfd_create(
            c"unsealed".as_ptr(),
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
        )
    };
    assert!(fd >= 0);
    let mut unsealed = unsafe { File::from_raw_fd(fd) };
    unsealed.write_all(&p.encode().unwrap()).unwrap();
    assert!(RunPlan::read_sealed(&unsealed).is_err());
}
#[test]
fn private_seqpacket_transfers_exact_correlation_and_cloexec_owned_rights() {
    let (a, b) = sockets();
    check_socket(a.as_fd()).unwrap();
    let mut f = Frame::new(Kind::Successor, [4; 16], 9);
    f.words = [1, 2, 3, 4, 5, 6, 7, 8];
    let file = File::open("/dev/null").unwrap();
    send(a.as_fd(), &f, &[file.as_fd(), file.as_fd(), file.as_fd()]).unwrap();
    let (g, rights) = receive(b.as_fd()).unwrap();
    assert_eq!(g, f);
    assert_eq!(rights.len(), 3);
    for right in rights {
        assert_ne!(
            unsafe { libc::fcntl(right.as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC,
            0
        );
    }
    assert_eq!(
        receive(b.as_fd()).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}
fn fd_count() -> usize {
    std::fs::read_dir("/proc/self/fd").unwrap().count()
}
#[test]
fn excessive_or_malformed_scm_packets_close_every_received_descriptor() {
    let (a, b) = sockets();
    let file = File::open("/dev/null").unwrap();
    let before = fd_count();
    let mut control = Control([0; 128]);
    let bytes = Frame::new(Kind::Incoming, [0; 16], 1).bytes();
    let mut iov = libc::iovec {
        iov_base: bytes.as_ptr().cast_mut().cast(),
        iov_len: bytes.len(),
    };
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control.0.as_mut_ptr().cast();
    msg.msg_controllen = unsafe { libc::CMSG_SPACE(16) } as usize;
    unsafe {
        let c = libc::CMSG_FIRSTHDR(&msg);
        (*c).cmsg_level = libc::SOL_SOCKET;
        (*c).cmsg_type = libc::SCM_RIGHTS;
        (*c).cmsg_len = libc::CMSG_LEN(16) as usize;
        for i in 0..4 {
            libc::CMSG_DATA(c)
                .cast::<i32>()
                .add(i)
                .write_unaligned(file.as_raw_fd());
        }
        assert_eq!(
            libc::sendmsg(a.as_raw_fd(), &msg, libc::MSG_NOSIGNAL),
            bytes.len() as isize
        );
    }
    assert!(receive(b.as_fd()).is_err());
    assert_eq!(fd_count(), before);
    let mut malformed = bytes;
    malformed[8] = 99;
    iov.iov_base = malformed.as_mut_ptr().cast();
    msg.msg_controllen = unsafe { libc::CMSG_SPACE(4) } as usize;
    unsafe {
        let c = libc::CMSG_FIRSTHDR(&msg);
        (*c).cmsg_len = libc::CMSG_LEN(4) as usize;
        assert_eq!(
            libc::sendmsg(a.as_raw_fd(), &msg, libc::MSG_NOSIGNAL),
            malformed.len() as isize
        );
    }
    assert!(receive(b.as_fd()).is_err());
    assert_eq!(fd_count(), before);
}
