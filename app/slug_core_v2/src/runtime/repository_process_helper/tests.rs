use super::*;

#[test]
fn mountinfo_paths_preserve_kernel_escapes_and_refuse_invalid_or_oversized_frames() {
    assert_eq!(
        setup::decode_mount_path(b"/run/a\\040b/host\\134proc")
            .unwrap()
            .as_bytes(),
        b"/run/a b/host\\proc"
    );
    assert_eq!(
        setup::decode_mount_path(b"/run/a\\011b\\012c")
            .unwrap()
            .as_bytes(),
        b"/run/a\tb\nc"
    );
    for bytes in [
        b"relative".as_slice(),
        b"/a/../b",
        b"/a/./b",
        b"/a//b",
        b"/bad\\000path",
        b"/bad\\040\\",
        b"/bad\\x40",
    ] {
        assert!(setup::decode_mount_path(bytes).is_err(), "{bytes:?}");
    }
    assert!(setup::decode_mount_path(&vec![b'a'; 4097]).is_err());
}
fn plan(argv: &[&[u8]], env: &[&[u8]]) -> RunPlan {
    RunPlan {
        mode: RunMode::ContinueExecute,
        display: CString::new("/__slug_repositories__/test/root").unwrap(),
        workspace: CString::new("/workspace").unwrap(),
        cwd: CString::new("/__slug_repositories__/test/root").unwrap(),
        argv: argv.iter().map(|v| CString::new(*v).unwrap()).collect(),
        env: env.iter().map(|v| CString::new(*v).unwrap()).collect(),
        mirrors: Vec::new(),
        nonce: [7; 16],
        index: 3,
        timeout_millis: 1000,
    }
}
#[test]
fn executable_search_uses_complete_effective_path_preserving_literal_argv_and_cwd_segments() {
    let actual = plan(&[b"tool", b""], &[b"PATH=/one::relative:/two", b"EMPTY="]);
    let found = supervisor::candidates(&actual).unwrap();
    assert_eq!(
        found.iter().map(|v| v.as_bytes()).collect::<Vec<_>>(),
        [
            b"/one/tool".as_slice(),
            b"tool",
            b"relative/tool",
            b"/two/tool"
        ]
    );
    assert_eq!(actual.argv[0].as_bytes(), b"tool");
    assert_eq!(
        supervisor::candidates(&plan(&[b"dir/tool"], &[b"PATH=/ignored"])).unwrap()[0].as_bytes(),
        b"dir/tool"
    );
    assert_eq!(
        supervisor::candidates(&plan(&[b"/absolute/tool"], &[b"PATH=/ignored"])).unwrap()[0]
            .as_bytes(),
        b"/absolute/tool"
    );
    assert_eq!(
        supervisor::candidates(&plan(&[b"tool"], &[]))
            .unwrap()
            .iter()
            .map(|v| v.as_bytes())
            .collect::<Vec<_>>(),
        [b"/bin/tool".as_slice(), b"/usr/bin/tool"]
    );
    assert_eq!(
        supervisor::candidates(&plan(&[b"tool"], &[b"PATH="])).unwrap()[0].as_bytes(),
        b"tool"
    );
    let long = vec![b'x'; 4096];
    assert!(
        supervisor::candidates(&plan(&[b"tool"], &[&[b"PATH=".as_slice(), &long].concat()]))
            .is_err()
    );
}
fn evaluate(nr: i32, arch: u32, args: [u64; 6]) -> u32 {
    let code = supervisor::supervisor_policy();
    let mut input = [0u8; 64];
    input[..4].copy_from_slice(&nr.to_le_bytes());
    input[4..8].copy_from_slice(&arch.to_le_bytes());
    for (i, arg) in args.iter().enumerate() {
        input[16 + 8 * i..24 + 8 * i].copy_from_slice(&arg.to_le_bytes());
    }
    let mut pc = 0usize;
    let mut accumulator = 0u32;
    for _ in 0..=code.len() {
        assert!(pc < code.len());
        let row = &code[pc];
        pc += 1;
        match row.code {
            0x20 => {
                let offset = row.k as usize;
                assert!(offset <= 60 && offset % 4 == 0);
                accumulator = u32::from_le_bytes(input[offset..offset + 4].try_into().unwrap());
            }
            0x15 => pc += if accumulator == row.k { row.jt } else { row.jf } as usize,
            0x06 => return row.k,
            _ => panic!("unexpected supervisor opcode"),
        }
    }
    panic!("unterminated supervisor program")
}
#[test]
fn supervisor_policy_allows_only_private_channel_and_namespace_reap_operations() {
    const ARCH: u32 = 0xc000003e;
    const PERMIT: u32 = 0x7fff0000;
    const REFUSE: u32 = 0x00050000 | 1;
    assert_eq!(
        evaluate(libc::SYS_read as i32, 0, [4, 0, 0, 0, 0, 0]),
        0x80000000
    );
    for (call, fd) in [
        (libc::SYS_sendmsg, 3),
        (libc::SYS_read, 4),
        (libc::SYS_close, 4),
    ] {
        assert_eq!(evaluate(call as i32, ARCH, [fd, 0, 0, 0, 0, 0]), PERMIT);
        for wrong in [0, 1, 2, 5, fd | (1u64 << 32)] {
            assert_eq!(evaluate(call as i32, ARCH, [wrong, 0, 0, 0, 0, 0]), REFUSE);
        }
    }
    for minus in [u32::MAX as u64, u64::MAX] {
        assert_eq!(
            evaluate(libc::SYS_kill as i32, ARCH, [minus, 9, 0, 0, 0, 0]),
            PERMIT
        );
        assert_eq!(
            evaluate(
                libc::SYS_wait4 as i32,
                ARCH,
                [minus, 0, (libc::__WALL | libc::WNOHANG) as u64, 0, 0, 0]
            ),
            PERMIT
        );
    }
    for pid in [0, 1, 2, 64] {
        assert_eq!(
            evaluate(libc::SYS_kill as i32, ARCH, [pid, 9, 0, 0, 0, 0]),
            REFUSE
        );
    }
    assert_eq!(
        evaluate(libc::SYS_kill as i32, ARCH, [u64::MAX, 15, 0, 0, 0, 0]),
        REFUSE
    );
    assert_eq!(
        evaluate(
            libc::SYS_kill as i32,
            ARCH,
            [u64::MAX, 9 | (1u64 << 32), 0, 0, 0, 0]
        ),
        REFUSE
    );
    assert_eq!(
        evaluate(libc::SYS_wait4 as i32, ARCH, [u64::MAX, 0, 0, 0, 0, 0]),
        REFUSE
    );
    for call in [
        libc::SYS_openat,
        libc::SYS_clone,
        libc::SYS_execve,
        libc::SYS_mount,
        libc::SYS_ptrace,
        libc::SYS_socket,
        libc::SYS_seccomp,
    ] {
        assert_eq!(evaluate(call as i32, ARCH, [0; 6]), REFUSE);
    }
    for nr in [-1, 0x40000001, i32::MAX] {
        assert_eq!(evaluate(nr, ARCH, [0; 6]), REFUSE);
    }
}
