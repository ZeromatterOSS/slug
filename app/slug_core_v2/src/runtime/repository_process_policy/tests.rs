use super::PolicyValidationError;
use super::ProcessPolicyProfile;
use super::linux_read_only_host_v1;
use super::validate;

// Independent interpreter for the actual emitted cBPF, using literal Linux UAPI
// encodings and a serialized seccomp_data. It never calls a policy decision fn.
const PERMIT: u32 = 0x7fff0000;
const REFUSE: u32 = 0x00050000 | 1; // EPERM
const UNSUPPORTED: u32 = 0x00050000 | 38; // ENOSYS
const TERMINATE: u32 = 0x80000000;
fn evaluate_arch(nr: i32, arch: u32, args: [u64; 6]) -> u32 {
    let mut data = [0u8; 64];
    data[0..4].copy_from_slice(&nr.to_le_bytes());
    data[4..8].copy_from_slice(&arch.to_le_bytes());
    for (i, value) in args.iter().enumerate() {
        data[16 + i * 8..24 + i * 8].copy_from_slice(&value.to_le_bytes());
    }
    let code = linux_read_only_host_v1().instructions();
    let mut a = 0u32;
    let mut pc = 0;
    for _ in 0..=code.len() {
        let ins = &code[pc];
        pc += 1;
        match ins.code {
            0x20 => {
                let offset = ins.k as usize;
                a = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
            }
            0x54 => a &= ins.k,
            0x15 | 0x25 | 0x35 | 0x45 => {
                let matched = match ins.code {
                    0x15 => a == ins.k,
                    0x25 => a > ins.k,
                    0x35 => a >= ins.k,
                    0x45 => a & ins.k != 0,
                    _ => unreachable!(),
                };
                pc += if matched { ins.jt } else { ins.jf } as usize;
            }
            0x05 => pc += ins.k as usize,
            0x06 => return ins.k,
            opcode => panic!("unsupported emitted opcode {opcode:#x}"),
        }
    }
    panic!("policy did not terminate");
}
fn evaluate(nr: libc::c_long, args: [u64; 6]) -> u32 {
    evaluate_arch(nr as i32, 0xc000003e, args)
}
fn expect(nr: libc::c_long, args: [u64; 6], action: u32) {
    assert_eq!(evaluate(nr, args), action, "syscall {nr}, args {args:x?}");
}
fn upper_half_refused(nr: libc::c_long, args: [u64; 6], positions: &[usize]) {
    expect(nr, args, PERMIT);
    for &position in positions {
        for high in [1u64, 0x80000000, 0xffffffff] {
            let mut forged = args;
            forged[position] |= high << 32;
            expect(nr, forged, REFUSE);
        }
    }
}

#[test]
fn static_profile_and_emitted_control_flow_are_bounded() {
    let policy = linux_read_only_host_v1();
    assert_eq!(policy.profile(), ProcessPolicyProfile::LinuxReadOnlyHostV1);
    assert_eq!(policy.version(), 1);
    assert!(std::ptr::eq(policy, linux_read_only_host_v1()));
    assert!(policy.instructions().len() < 2048);
    assert_eq!(policy.validate(), Ok(()));
}

#[test]
fn wrong_abi_x32_negative_and_unknown_syscalls_fail_closed() {
    for arch in [0, 0x40000003, 0xc00000b7, 0x8000003e, u32::MAX] {
        assert_eq!(evaluate_arch(0, arch, [0; 6]), TERMINATE);
    }
    for nr in [-1, i32::MIN, 0x40000000, 0x40000001, 0x40000200, i32::MAX] {
        assert_eq!(evaluate_arch(nr, 0xc000003e, [0; 6]), TERMINATE);
    }
    for nr in [512, 547, 548, 1024, 100000, 0x3fffffff] {
        assert_eq!(evaluate_arch(nr, 0xc000003e, [0; 6]), REFUSE);
    }
    expect(libc::SYS_clone3, [0; 6], UNSUPPORTED);
    expect(libc::SYS_clone3, [u64::MAX; 6], UNSUPPORTED);
}

// Golden numbers are from native Linux asm/unistd_64.h, independently of the
// producer's libc constant list. This catches accidentally adding an allow rule
// as well as omitting one. Zero-valued arguments intentionally exercise the
// seccomp surface; the kernel may subsequently reject invalid syscall arguments.
const ZERO_ARGUMENT_ALLOWED: &[i32] = &[
    0,   // read
    1,   // write
    2,   // open
    3,   // close
    4,   // stat
    5,   // fstat
    6,   // lstat
    7,   // poll
    8,   // lseek
    9,   // mmap
    10,  // mprotect
    11,  // munmap
    12,  // brk
    13,  // rt_sigaction
    14,  // rt_sigprocmask
    15,  // rt_sigreturn
    17,  // pread64
    18,  // pwrite64
    19,  // readv
    20,  // writev
    21,  // access
    22,  // pipe
    23,  // select
    24,  // sched_yield
    25,  // mremap
    27,  // mincore
    28,  // madvise
    32,  // dup
    33,  // dup2
    35,  // nanosleep
    36,  // getitimer
    37,  // alarm
    38,  // setitimer
    39,  // getpid
    40,  // sendfile
    56,  // clone
    57,  // fork
    58,  // vfork
    59,  // execve
    60,  // exit
    61,  // wait4
    63,  // uname
    72,  // fcntl
    73,  // flock
    74,  // fsync
    75,  // fdatasync
    76,  // truncate
    77,  // ftruncate
    78,  // getdents
    79,  // getcwd
    80,  // chdir
    81,  // fchdir
    82,  // rename
    83,  // mkdir
    84,  // rmdir
    85,  // creat
    86,  // link
    87,  // unlink
    88,  // symlink
    89,  // readlink
    90,  // chmod
    91,  // fchmod
    95,  // umask
    96,  // gettimeofday
    97,  // getrlimit
    98,  // getrusage
    99,  // sysinfo
    100, // times
    102, // getuid
    104, // getgid
    107, // geteuid
    108, // getegid
    110, // getppid
    115, // getgroups
    118, // getresuid
    120, // getresgid
    127, // rt_sigpending
    128, // rt_sigtimedwait
    130, // rt_sigsuspend
    131, // sigaltstack
    132, // utime
    137, // statfs
    138, // fstatfs
    140, // getpriority
    160, // setrlimit
    186, // gettid
    191, // getxattr
    192, // lgetxattr
    193, // fgetxattr
    194, // listxattr
    195, // llistxattr
    196, // flistxattr
    201, // time
    202, // futex
    203, // sched_setaffinity
    204, // sched_getaffinity
    213, // epoll_create
    217, // getdents64
    218, // set_tid_address
    219, // restart_syscall
    222, // timer_create
    223, // timer_settime
    224, // timer_gettime
    225, // timer_getoverrun
    226, // timer_delete
    228, // clock_gettime
    229, // clock_getres
    230, // clock_nanosleep
    231, // exit_group
    232, // epoll_wait
    233, // epoll_ctl
    235, // utimes
    247, // waitid
    257, // openat
    258, // mkdirat
    261, // futimesat
    262, // newfstatat
    263, // unlinkat
    264, // renameat
    265, // linkat
    266, // symlinkat
    267, // readlinkat
    268, // fchmodat
    269, // faccessat
    270, // pselect6
    271, // ppoll
    273, // set_robust_list
    274, // get_robust_list
    275, // splice
    276, // tee
    277, // sync_file_range
    278, // vmsplice
    280, // utimensat
    281, // epoll_pwait
    282, // signalfd
    283, // timerfd_create
    284, // eventfd
    285, // fallocate
    286, // timerfd_settime
    287, // timerfd_gettime
    289, // signalfd4
    290, // eventfd2
    291, // epoll_create1
    292, // dup3
    293, // pipe2
    295, // preadv
    296, // pwritev
    302, // prlimit64
    309, // getcpu
    316, // renameat2
    318, // getrandom
    322, // execveat
    324, // membarrier
    326, // copy_file_range
    327, // preadv2
    328, // pwritev2
    332, // statx
    334, // rseq
    436, // close_range
    437, // openat2
    439, // faccessat2
    441, // epoll_pwait2
    449, // futex_waitv
    452, // fchmodat2
];

#[test]
fn complete_native_number_space_has_exact_reviewed_default_deny_surface() {
    for nr in 0..=511 {
        let expected = if nr == 435 {
            UNSUPPORTED
        } else if ZERO_ARGUMENT_ALLOWED.contains(&nr) {
            PERMIT
        } else {
            REFUSE
        };
        assert_eq!(
            evaluate_arch(nr, 0xc000003e, [0; 6]),
            expected,
            "syscall {nr}"
        );
    }
    // Unconditional calls do not silently acquire pointer/scalar restrictions.
    for &nr in ZERO_ARGUMENT_ALLOWED {
        // These are the independent UAPI numbers of argument-filtered calls.
        if ![
            56, 61, 72, 97, 98, 140, 160, 203, 204, 247, 274, 292, 302, 436,
        ]
        .contains(&nr)
        {
            assert_eq!(
                evaluate_arch(nr, 0xc000003e, [u64::MAX; 6]),
                PERMIT,
                "syscall {nr}"
            );
        }
    }
}

#[test]
fn escape_syscalls_are_denied_even_with_plausible_arguments() {
    let forbidden = [
        libc::SYS_socket,
        libc::SYS_socketpair,
        libc::SYS_connect,
        libc::SYS_sendmsg,
        libc::SYS_recvmsg,
        libc::SYS_ioctl,
        libc::SYS_mount,
        libc::SYS_umount2,
        libc::SYS_pivot_root,
        libc::SYS_chroot,
        libc::SYS_open_tree,
        libc::SYS_move_mount,
        libc::SYS_fsopen,
        libc::SYS_fsconfig,
        libc::SYS_fsmount,
        libc::SYS_fspick,
        libc::SYS_mount_setattr,
        libc::SYS_setns,
        libc::SYS_unshare,
        libc::SYS_ptrace,
        libc::SYS_process_vm_readv,
        libc::SYS_process_vm_writev,
        libc::SYS_process_madvise,
        libc::SYS_process_mrelease,
        libc::SYS_migrate_pages,
        libc::SYS_move_pages,
        libc::SYS_mbind,
        libc::SYS_get_mempolicy,
        libc::SYS_set_mempolicy,
        libc::SYS_pidfd_open,
        libc::SYS_pidfd_getfd,
        libc::SYS_pidfd_send_signal,
        libc::SYS_io_uring_setup,
        libc::SYS_io_uring_enter,
        libc::SYS_io_uring_register,
        libc::SYS_io_setup,
        libc::SYS_io_submit,
        libc::SYS_keyctl,
        libc::SYS_add_key,
        libc::SYS_request_key,
        libc::SYS_bpf,
        libc::SYS_perf_event_open,
        libc::SYS_fanotify_init,
        libc::SYS_fanotify_mark,
        libc::SYS_name_to_handle_at,
        libc::SYS_open_by_handle_at,
        libc::SYS_mknod,
        libc::SYS_mknodat,
        libc::SYS_capset,
        libc::SYS_setuid,
        libc::SYS_setgid,
        libc::SYS_setgroups,
        libc::SYS_seccomp,
        libc::SYS_memfd_create,
        libc::SYS_memfd_secret,
        libc::SYS_shmget,
        libc::SYS_shmat,
        libc::SYS_shmctl,
        libc::SYS_shmdt,
        libc::SYS_semget,
        libc::SYS_semop,
        libc::SYS_semctl,
        libc::SYS_msgget,
        libc::SYS_msgsnd,
        libc::SYS_msgrcv,
        libc::SYS_msgctl,
        libc::SYS_setxattr,
        libc::SYS_fsetxattr,
        libc::SYS_lsetxattr,
        libc::SYS_removexattr,
        libc::SYS_fremovexattr,
        libc::SYS_lremovexattr,
        libc::SYS_rt_sigqueueinfo,
        libc::SYS_rt_tgsigqueueinfo,
    ];
    for nr in forbidden {
        expect(nr, [0; 6], REFUSE);
        expect(nr, [1; 6], REFUSE);
    }
}

#[test]
fn clone_admits_ordinary_fork_vfork_threads_and_rejects_each_other_bit() {
    let pthread = (libc::CLONE_VM
        | libc::CLONE_FS
        | libc::CLONE_FILES
        | libc::CLONE_SIGHAND
        | libc::CLONE_THREAD
        | libc::CLONE_SYSVSEM
        | libc::CLONE_SETTLS
        | libc::CLONE_PARENT_SETTID
        | libc::CLONE_CHILD_CLEARTID) as u64;
    for flags in [
        0,
        libc::SIGCHLD as u64,
        pthread,
        (libc::CLONE_VM | libc::CLONE_VFORK | libc::SIGCHLD) as u64,
    ] {
        upper_half_refused(
            libc::SYS_clone,
            [flags, u64::MAX, u64::MAX, u64::MAX, u64::MAX, 0],
            &[0],
        );
    }
    let permitted = 0x00000100u64
        | 0x00000200
        | 0x00000400
        | 0x00000800
        | 0x00004000
        | 0x00010000
        | 0x00040000
        | 0x00080000
        | 0x00100000
        | 0x00200000
        | 0x01000000;
    for bit in 8..64 {
        if permitted & (1u64 << bit) == 0 {
            expect(
                libc::SYS_clone,
                [(1u64 << bit) | libc::SIGCHLD as u64, 0, 0, 0, 0, 0],
                REFUSE,
            );
        }
    }
    for signal in 0..=255 {
        expect(
            libc::SYS_clone,
            [signal, 0, 0, 0, 0, 0],
            if signal == 0 || signal == 17 {
                PERMIT
            } else {
                REFUSE
            },
        );
    }
}

#[test]
fn signals_cannot_target_pid1_or_groups_and_check_every_scalar_half() {
    for nr in [libc::SYS_kill, libc::SYS_tkill, libc::SYS_tgkill] {
        let args = if nr == libc::SYS_tgkill {
            [2, 3, 15, 0, 0, 0]
        } else {
            [2, 15, 0, 0, 0, 0]
        };
        upper_half_refused(
            nr,
            args,
            if nr == libc::SYS_tgkill {
                &[0, 1, 2]
            } else {
                &[0, 1]
            },
        );
        for pid in [0, 1, 0xffffffff, u64::MAX, 0x80000000] {
            let mut invalid = args;
            invalid[0] = pid;
            expect(nr, invalid, REFUSE);
            if nr == libc::SYS_tgkill {
                invalid = args;
                invalid[1] = pid;
                expect(nr, invalid, REFUSE);
            }
        }
        for signal in [0, 64, 65, u64::MAX] {
            let mut candidate = args;
            candidate[if nr == libc::SYS_tgkill { 2 } else { 1 }] = signal;
            expect(nr, candidate, if signal <= 64 { PERMIT } else { REFUSE });
        }
    }
}

#[test]
fn fcntl_command_and_scalar_filters_preserve_only_reviewed_operations() {
    for cmd in [
        libc::F_DUPFD,
        libc::F_DUPFD_CLOEXEC,
        libc::F_GETFD,
        libc::F_SETFD,
        libc::F_GETFL,
        libc::F_SETFL,
        libc::F_GETLK,
        libc::F_SETLK,
        libc::F_SETLKW,
        libc::F_OFD_GETLK,
        libc::F_OFD_SETLK,
        libc::F_OFD_SETLKW,
        libc::F_GETPIPE_SZ,
    ] {
        upper_half_refused(libc::SYS_fcntl, [3, cmd as u64, 0, 0, 0, 0], &[0, 1]);
    }
    for cmd in [
        libc::F_GETFD,
        libc::F_GETFL,
        libc::F_GETPIPE_SZ,
        libc::F_GETLK,
        libc::F_SETLK,
        libc::F_SETLKW,
        libc::F_OFD_GETLK,
        libc::F_OFD_SETLK,
        libc::F_OFD_SETLKW,
    ] {
        expect(libc::SYS_fcntl, [3, cmd as u64, u64::MAX, 0, 0, 0], PERMIT);
    }
    for cmd in [libc::F_DUPFD, libc::F_DUPFD_CLOEXEC] {
        upper_half_refused(
            libc::SYS_fcntl,
            [3, cmd as u64, i32::MAX as u64, 0, 0, 0],
            &[2],
        );
        expect(
            libc::SYS_fcntl,
            [3, cmd as u64, 0x80000000, 0, 0, 0],
            REFUSE,
        );
    }
    for flags in [0, 1, 2, 3, 0x100000001] {
        expect(
            libc::SYS_fcntl,
            [3, libc::F_SETFD as u64, flags, 0, 0, 0],
            if flags <= 1 { PERMIT } else { REFUSE },
        );
    }
    for cmd in [
        libc::F_SETOWN,
        15, // F_SETOWN_EX: asm-generic/fcntl.h (not exported by libc).
        10, // F_SETSIG: asm-generic/fcntl.h (not exported by libc).
        libc::F_SETPIPE_SZ,
        libc::F_ADD_SEALS,
        -1,
        0x12345,
    ] {
        expect(libc::SYS_fcntl, [3, cmd as u64, 0, 0, 0, 0], REFUSE);
    }
    expect(
        libc::SYS_fcntl,
        [u64::MAX, libc::F_GETFD as u64, 0, 0, 0, 0],
        REFUSE,
    );
}

#[test]
fn fcntl_getfl_setfl_roundtrip_keeps_kernel_largefile_bit() {
    // Native libc O_LARGEFILE=0; the x86_64 kernel F_GETFL bit is 0o100000.
    assert_eq!(libc::O_LARGEFILE, 0);
    let expected_mask = (libc::O_ACCMODE
        | libc::O_APPEND
        | libc::O_NONBLOCK
        | libc::O_DIRECT
        | libc::O_NOATIME
        | libc::O_DSYNC
        | libc::O_SYNC) as u64
        | 0o100000;
    for flags in [0o100000, 0o100000 | libc::O_NONBLOCK as u64, expected_mask] {
        upper_half_refused(
            libc::SYS_fcntl,
            [3, libc::F_SETFL as u64, flags, 0, 0, 0],
            &[2],
        );
    }
    for bit in 0..64 {
        if expected_mask & (1u64 << bit) == 0 {
            expect(
                libc::SYS_fcntl,
                [3, libc::F_SETFL as u64, 1u64 << bit, 0, 0, 0],
                REFUSE,
            );
        }
    }
}

#[test]
fn duplication_close_range_and_tls_have_exact_flags() {
    for flags in [0, libc::O_CLOEXEC as u64] {
        upper_half_refused(libc::SYS_dup3, [3, 4, flags, 0, 0, 0], &[2]);
    }
    for bit in 0..64 {
        if 1u64 << bit != libc::O_CLOEXEC as u64 {
            expect(libc::SYS_dup3, [3, 4, 1u64 << bit, 0, 0, 0], REFUSE);
        }
    }
    for flags in [0, 2, 4, 6] {
        upper_half_refused(
            libc::SYS_close_range,
            [0, u32::MAX as u64, flags, 0, 0, 0],
            &[0, 1, 2],
        );
    }
    for bit in 0..64 {
        if bit != 1 && bit != 2 {
            expect(
                libc::SYS_close_range,
                [0, u32::MAX as u64, 1u64 << bit, 0, 0, 0],
                REFUSE,
            );
        }
    }
    for cmd in [0x1002, 0x1003, 0x1004] {
        upper_half_refused(libc::SYS_arch_prctl, [cmd, u64::MAX, 0, 0, 0, 0], &[0]);
    }
    for cmd in [0, 0x1001, 0x1011, 0x1012, 0x3001, 0x5001] {
        expect(libc::SYS_arch_prctl, [cmd, 0, 0, 0, 0, 0], REFUSE);
    }
}

#[test]
fn prctl_excludes_privilege_and_dumpability_setters() {
    for cmd in [
        libc::PR_SET_NAME,
        libc::PR_GET_NAME,
        libc::PR_GET_DUMPABLE,
        libc::PR_GET_SECCOMP,
        libc::PR_GET_PDEATHSIG,
    ] {
        // Name/PDEATHSIG arguments are full pointers; ignored variadic slots
        // may contain arbitrary register contents from ordinary libc callers.
        upper_half_refused(
            libc::SYS_prctl,
            [cmd as u64, u64::MAX, u64::MAX, u64::MAX, u64::MAX, u64::MAX],
            &[0],
        );
    }
    for (cmd, value) in [
        (libc::PR_SET_NO_NEW_PRIVS, 1),
        (libc::PR_GET_NO_NEW_PRIVS, 0),
    ] {
        let args = [cmd as u64, value, 0, 0, 0, u64::MAX];
        upper_half_refused(libc::SYS_prctl, args, &[0, 1, 2, 3, 4]);
        for slot in 1..=4 {
            let mut invalid = args;
            invalid[slot] = if slot == 1 { value ^ 1 } else { 1 };
            expect(libc::SYS_prctl, invalid, REFUSE);
        }
    }
    for cmd in [
        libc::PR_SET_DUMPABLE,
        libc::PR_SET_SECCOMP,
        libc::PR_SET_PTRACER,
        libc::PR_CAP_AMBIENT,
        libc::PR_CAPBSET_DROP,
        libc::PR_SET_PDEATHSIG,
        -1,
        0x12345,
    ] {
        expect(libc::SYS_prctl, [cmd as u64, 0, 0, 0, 0, 0], REFUSE);
    }
}

#[test]
fn resource_affinity_and_robust_queries_cannot_target_supervisor() {
    for resource in 0..16 {
        upper_half_refused(
            libc::SYS_prlimit64,
            [0, resource, u64::MAX, u64::MAX, 0, 0],
            &[0, 1],
        );
        for nr in [libc::SYS_getrlimit, libc::SYS_setrlimit] {
            upper_half_refused(nr, [resource, u64::MAX, 0, 0, 0, 0], &[0]);
        }
    }
    for resource in [16, 17, u64::MAX] {
        expect(libc::SYS_prlimit64, [0, resource, 0, 0, 0, 0], REFUSE);
        for nr in [libc::SYS_getrlimit, libc::SYS_setrlimit] {
            expect(nr, [resource, 0, 0, 0, 0, 0], REFUSE);
        }
    }
    for nr in [
        libc::SYS_sched_getaffinity,
        libc::SYS_sched_setaffinity,
        libc::SYS_get_robust_list,
    ] {
        upper_half_refused(nr, [0, u64::MAX, u64::MAX, 0, 0, 0], &[0]);
        for pid in [1, 2, 0xffffffff, u64::MAX] {
            expect(nr, [pid, 0, 0, 0, 0, 0], REFUSE);
        }
    }
    for pid in [1, 2, 0xffffffff, u64::MAX] {
        expect(libc::SYS_prlimit64, [pid, 0, 0, 0, 0, 0], REFUSE);
    }
    upper_half_refused(libc::SYS_getpriority, [0; 6], &[0, 1]);
    expect(libc::SYS_getpriority, [1, 0, 0, 0, 0, 0], REFUSE);
    expect(libc::SYS_getpriority, [0, 1, 0, 0, 0, 0], REFUSE);
    for selector in [0, 1, 0xffffffff, u64::MAX] {
        expect(
            libc::SYS_getrusage,
            [selector, u64::MAX, 0, 0, 0, 0],
            PERMIT,
        );
    }
    for selector in [2, 0xfffffffe, 0x100000000, 0xffffffff00000000, 0x100000001] {
        expect(libc::SYS_getrusage, [selector, 0, 0, 0, 0, 0], REFUSE);
    }
}

#[test]
fn waitid_preserves_child_wait_selectors_but_excludes_pidfd() {
    for args in [
        [0; 6],
        [1, 2, u64::MAX, u64::MAX, 0, 0],
        [1, i32::MAX as u64, 0, 0, 0, 0],
        [2, 0, 0, 0, 0, 0],
        [2, 1, 0, 0, 0, 0],
    ] {
        upper_half_refused(libc::SYS_waitid, args, &[0, 1]);
    }
    for args in [
        [0, 1, 0, 0, 0, 0],
        [1, 0, 0, 0, 0, 0],
        [1, 1, 0, 0, 0, 0],
        [1, 0x80000000, 0, 0, 0, 0],
        [2, 0x80000000, 0, 0, 0, 0],
        [3, 3, 0, 0, 0, 0],
        [u64::MAX, 0, 0, 0, 0, 0],
    ] {
        expect(libc::SYS_waitid, args, REFUSE);
    }
}

fn instruction(code: u16, jt: u8, jf: u8, k: u32) -> libc::sock_filter {
    libc::sock_filter { code, jt, jf, k }
}
#[test]
fn verifier_rejects_invalid_loads_jumps_actions_and_fallthrough() {
    use PolicyValidationError::*;
    let ret = instruction(0x06, 0, 0, REFUSE);
    assert_eq!(validate(&[]), Err(Length));
    assert_eq!(validate(&vec![ret; 2049]), Err(Length));
    assert_eq!(
        validate(&[instruction(0xffff, 0, 0, 0), ret]),
        Err(Instruction)
    );
    assert_eq!(
        validate(&[instruction(0x20, 1, 0, 0), ret]),
        Err(Instruction)
    );
    for offset in [1, 8, 12, 15, 17, 64, u32::MAX] {
        assert_eq!(validate(&[instruction(0x20, 0, 0, offset), ret]), Err(Load));
    }
    for opcode in [0x15, 0x25, 0x35, 0x45] {
        assert_eq!(validate(&[instruction(opcode, 1, 0, 0), ret]), Err(Jump));
        assert_eq!(validate(&[instruction(opcode, 0, 1, 0), ret]), Err(Jump));
    }
    for offset in [1, u32::MAX] {
        assert_eq!(validate(&[instruction(0x05, 0, 0, offset), ret]), Err(Jump));
    }
    assert_eq!(
        validate(&[instruction(0x06, 0, 0, 0x7fc00000)]),
        Err(Action)
    );
    assert_eq!(validate(&[instruction(0x54, 0, 0, 0)]), Err(Fallthrough));
    assert_eq!(validate(&[instruction(0x05, 0, 0, 0), ret]), Ok(()));
    for offset in [0, 4, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 60] {
        assert_eq!(validate(&[instruction(0x20, 0, 0, offset), ret]), Ok(()));
    }
}
