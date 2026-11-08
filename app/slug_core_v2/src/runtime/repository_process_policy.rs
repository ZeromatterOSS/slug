//! Inactive LinuxReadOnlyHostV1 payload syscall policy. No installer lives here.
//!
//! This policy is supported only for the native x86_64 ABI. It requires the
//! separately verified recursive RO/nosuid/nodev Host view, protected private
//! proc/minimal devices, quota root/bounded tmp, private PID namespace with a
//! nondumpable PID1 supervisor, irreversible capability retirement, no_new_privs,
//! finite inherited hard limits, and ONLY descriptors 0/1/2 at payload exec.
//! It neither establishes path confinement nor bounds kernel memory/processes.
//! The trusted supervisor needs a DIFFERENT policy for namespace-wide cleanup.
//!
//! Primary ABI sources: Linux UAPI linux/{seccomp,filter,sched,prctl}.h,
//! asm/unistd_64.h, asm/prctl.h, asm-generic/fcntl.h; syscall numbers use libc.
#![cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]

const ARCH: u32 = 0xc000_003e; // AUDIT_ARCH_X86_64
const X32: u32 = 0x4000_0000;
const ALLOW: u32 = libc::SECCOMP_RET_ALLOW;
const DENY: u32 = libc::SECCOMP_RET_ERRNO | libc::EPERM as u32;
const NO_SYS: u32 = libc::SECCOMP_RET_ERRNO | libc::ENOSYS as u32;
const KILL: u32 = libc::SECCOMP_RET_KILL_PROCESS;
const CAPACITY: usize = 2048; // Strictly below Linux BPF_MAXINSNS=4096.
const LD: u16 = 0x20; // BPF_LD | BPF_W | BPF_ABS
const AND: u16 = 0x54; // BPF_ALU | BPF_AND | BPF_K
const JEQ: u16 = 0x15;
const JGT: u16 = 0x25;
const JGE: u16 = 0x35;
const JSET: u16 = 0x45;
const JA: u16 = 0x05;
const RET: u16 = 0x06;
const NR: u32 = 0;
const ARCH_OFFSET: u32 = 4;
const fn low(arg: usize) -> u32 {
    16 + 8 * arg as u32
}
const fn high(arg: usize) -> u32 {
    low(arg) + 4
}

// libc O_LARGEFILE is zero on x86_64, but F_GETFL returns this kernel UAPI bit.
const KERNEL_O_LARGEFILE: u32 = 0o100000;
const SETFL_MASK: u32 = (libc::O_ACCMODE
    | libc::O_APPEND
    | libc::O_NONBLOCK
    | libc::O_DIRECT
    | libc::O_NOATIME
    | libc::O_DSYNC
    | libc::O_SYNC) as u32
    | KERNEL_O_LARGEFILE;
const CLONE_MASK: u32 = (libc::CSIGNAL
    | libc::CLONE_VM
    | libc::CLONE_FS
    | libc::CLONE_FILES
    | libc::CLONE_SIGHAND
    | libc::CLONE_THREAD
    | libc::CLONE_SYSVSEM
    | libc::CLONE_SETTLS
    | libc::CLONE_PARENT_SETTID
    | libc::CLONE_CHILD_SETTID
    | libc::CLONE_CHILD_CLEARTID
    | libc::CLONE_VFORK) as u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProcessPolicyProfile {
    LinuxReadOnlyHostV1,
}

pub(super) struct ProcessPolicy {
    code: [libc::sock_filter; CAPACITY],
    len: usize,
}
impl ProcessPolicy {
    pub(super) const fn profile(&self) -> ProcessPolicyProfile {
        ProcessPolicyProfile::LinuxReadOnlyHostV1
    }
    pub(super) const fn version(&self) -> u32 {
        1
    }
    pub(super) fn instructions(&self) -> &[libc::sock_filter] {
        &self.code[..self.len]
    }
    /// Structural verification of the emitted subset, not a confinement proof.
    pub(super) fn validate(&self) -> Result<(), PolicyValidationError> {
        validate(self.instructions())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PolicyValidationError {
    Length,
    Instruction,
    Load,
    Jump,
    Action,
    Fallthrough,
}

fn validate(code: &[libc::sock_filter]) -> Result<(), PolicyValidationError> {
    use PolicyValidationError::*;
    if code.is_empty() || code.len() > CAPACITY || code.len() > libc::BPF_MAXINSNS as usize {
        return Err(Length);
    }
    for (i, ins) in code.iter().enumerate() {
        if !matches!(ins.code, JEQ | JGT | JGE | JSET) && (ins.jt != 0 || ins.jf != 0) {
            return Err(Instruction);
        }
        match ins.code {
            LD => {
                if !(ins.k == NR
                    || ins.k == ARCH_OFFSET
                    || (16..64).contains(&ins.k) && ins.k % 4 == 0)
                {
                    return Err(Load);
                }
            }
            AND => {}
            JA => {
                if ins.k as usize >= code.len() - i - 1 {
                    return Err(Jump);
                }
            }
            JEQ | JGT | JGE | JSET => {
                if ins.jt as usize >= code.len() - i - 1 || ins.jf as usize >= code.len() - i - 1 {
                    return Err(Jump);
                }
            }
            RET => {
                if !matches!(ins.k, ALLOW | DENY | NO_SYS | KILL) {
                    return Err(Action);
                }
            }
            _ => return Err(Instruction),
        }
        if i + 1 == code.len() && ins.code != RET {
            return Err(Fallthrough);
        }
    }
    // Every edge moves strictly forward within the slice; a final return and
    // the local edge checks therefore imply termination on every path.
    Ok(())
}

const ZERO: libc::sock_filter = libc::sock_filter {
    code: RET,
    jt: 0,
    jf: 0,
    k: DENY,
};
struct Build {
    policy: ProcessPolicy,
}
impl Build {
    const fn new() -> Self {
        Self {
            policy: ProcessPolicy {
                code: [ZERO; CAPACITY],
                len: 0,
            },
        }
    }
    const fn emit(&mut self, code: u16, jt: u8, jf: u8, k: u32) -> usize {
        assert!(self.policy.len < CAPACITY);
        let i = self.policy.len;
        self.policy.code[i] = libc::sock_filter { code, jt, jf, k };
        self.policy.len += 1;
        i
    }
    const fn load(&mut self, offset: u32) {
        self.emit(LD, 0, 0, offset);
    }
    const fn ret(&mut self, action: u32) {
        self.emit(RET, 0, 0, action);
    }
    const fn skip_unless(&mut self, offset: u32, value: u32) -> usize {
        self.load(offset);
        self.emit(JEQ, 1, 0, value);
        self.emit(JA, 0, 0, 0)
    }
    const fn end(&mut self, jump: usize) {
        self.policy.code[jump].k = (self.policy.len - jump - 1) as u32;
    }
    const fn eq(&mut self, offset: u32, value: u32) {
        self.load(offset);
        self.emit(JEQ, 1, 0, value);
        self.ret(DENY);
    }
    const fn arg_eq(&mut self, arg: usize, value: u32) {
        self.eq(high(arg), 0);
        self.eq(low(arg), value);
    }
    const fn range(&mut self, arg: usize, min: u32, max: u32) {
        self.eq(high(arg), 0);
        self.load(low(arg));
        self.emit(JGE, 1, 0, min);
        self.ret(DENY);
        self.emit(JGT, 0, 1, max);
        self.ret(DENY);
    }
    const fn mask(&mut self, arg: usize, mask: u32) {
        self.eq(high(arg), 0);
        self.load(low(arg));
        self.emit(AND, 0, 0, !mask);
        self.emit(JEQ, 1, 0, 0);
        self.ret(DENY);
    }
    const fn choices(&mut self, offset: u32, values: &[u32]) {
        assert!(values.len() <= 32);
        let mut exits = [0; 32];
        self.load(offset);
        let mut i = 0;
        while i < values.len() {
            self.emit(JEQ, 0, 1, values[i]);
            exits[i] = self.emit(JA, 0, 0, 0);
            i += 1;
        }
        self.ret(DENY);
        i = 0;
        while i < values.len() {
            self.end(exits[i]);
            i += 1;
        }
    }
    const fn arg_choices(&mut self, arg: usize, values: &[u32]) {
        self.eq(high(arg), 0);
        self.choices(low(arg), values);
    }
    const fn rule(&mut self, syscall: libc::c_long) -> usize {
        self.skip_unless(NR, syscall as u32)
    }
    const fn allow_rule(&mut self, syscall: libc::c_long) {
        let r = self.rule(syscall);
        self.ret(ALLOW);
        self.end(r);
    }
    const fn done(&mut self, r: usize) {
        self.ret(ALLOW);
        self.end(r);
    }
}

// Capability families, independent of tool/caller/source/profile names. All
// absent syscalls (including future syscall numbers) are denied by default.
const UNCONDITIONAL: &[libc::c_long] = &[
    libc::SYS_read,
    libc::SYS_write,
    libc::SYS_readv,
    libc::SYS_writev,
    libc::SYS_pread64,
    libc::SYS_pwrite64,
    libc::SYS_preadv,
    libc::SYS_pwritev,
    libc::SYS_preadv2,
    libc::SYS_pwritev2,
    libc::SYS_open,
    libc::SYS_openat,
    libc::SYS_openat2,
    libc::SYS_creat,
    libc::SYS_close,
    libc::SYS_dup,
    libc::SYS_dup2,
    libc::SYS_lseek,
    libc::SYS_stat,
    libc::SYS_lstat,
    libc::SYS_fstat,
    libc::SYS_newfstatat,
    libc::SYS_statx,
    libc::SYS_access,
    libc::SYS_faccessat,
    libc::SYS_faccessat2,
    libc::SYS_getdents,
    libc::SYS_getdents64,
    libc::SYS_readlink,
    libc::SYS_readlinkat,
    libc::SYS_getcwd,
    libc::SYS_chdir,
    libc::SYS_fchdir,
    libc::SYS_statfs,
    libc::SYS_fstatfs,
    libc::SYS_fsync,
    libc::SYS_fdatasync,
    libc::SYS_sync_file_range,
    libc::SYS_truncate,
    libc::SYS_ftruncate,
    libc::SYS_fallocate,
    libc::SYS_mkdir,
    libc::SYS_mkdirat,
    libc::SYS_rmdir,
    libc::SYS_unlink,
    libc::SYS_unlinkat,
    libc::SYS_rename,
    libc::SYS_renameat,
    libc::SYS_renameat2,
    libc::SYS_link,
    libc::SYS_linkat,
    libc::SYS_symlink,
    libc::SYS_symlinkat,
    libc::SYS_chmod,
    libc::SYS_fchmod,
    libc::SYS_fchmodat,
    libc::SYS_fchmodat2,
    libc::SYS_umask,
    libc::SYS_utime,
    libc::SYS_utimes,
    libc::SYS_futimesat,
    libc::SYS_utimensat,
    libc::SYS_getxattr,
    libc::SYS_lgetxattr,
    libc::SYS_fgetxattr,
    libc::SYS_listxattr,
    libc::SYS_llistxattr,
    libc::SYS_flistxattr,
    libc::SYS_flock,
    libc::SYS_pipe,
    libc::SYS_pipe2,
    libc::SYS_sendfile,
    libc::SYS_copy_file_range,
    libc::SYS_splice,
    libc::SYS_tee,
    libc::SYS_vmsplice,
    libc::SYS_poll,
    libc::SYS_ppoll,
    libc::SYS_select,
    libc::SYS_pselect6,
    libc::SYS_epoll_create,
    libc::SYS_epoll_create1,
    libc::SYS_epoll_ctl,
    libc::SYS_epoll_wait,
    libc::SYS_epoll_pwait,
    libc::SYS_epoll_pwait2,
    libc::SYS_eventfd,
    libc::SYS_eventfd2,
    libc::SYS_signalfd,
    libc::SYS_signalfd4,
    libc::SYS_timerfd_create,
    libc::SYS_timerfd_gettime,
    libc::SYS_timerfd_settime,
    libc::SYS_brk,
    libc::SYS_mmap,
    libc::SYS_mprotect,
    libc::SYS_munmap,
    libc::SYS_mremap,
    libc::SYS_madvise,
    libc::SYS_mincore,
    libc::SYS_futex,
    libc::SYS_futex_waitv,
    libc::SYS_set_tid_address,
    libc::SYS_set_robust_list,
    libc::SYS_rseq,
    libc::SYS_membarrier,
    libc::SYS_sched_yield,
    libc::SYS_restart_syscall,
    libc::SYS_fork,
    libc::SYS_vfork,
    libc::SYS_setsid, // Session changes remain inside the PID1-owned namespace.
    libc::SYS_execve,
    libc::SYS_execveat,
    libc::SYS_wait4,
    libc::SYS_exit,
    libc::SYS_exit_group,
    libc::SYS_rt_sigaction,
    libc::SYS_rt_sigprocmask,
    libc::SYS_rt_sigpending,
    libc::SYS_rt_sigtimedwait,
    libc::SYS_rt_sigsuspend,
    libc::SYS_rt_sigreturn,
    libc::SYS_sigaltstack,
    libc::SYS_clock_gettime,
    libc::SYS_clock_getres,
    libc::SYS_clock_nanosleep,
    libc::SYS_nanosleep,
    libc::SYS_gettimeofday,
    libc::SYS_time,
    libc::SYS_times,
    libc::SYS_alarm,
    libc::SYS_getitimer,
    libc::SYS_setitimer,
    libc::SYS_timer_create,
    libc::SYS_timer_delete,
    libc::SYS_timer_getoverrun,
    libc::SYS_timer_gettime,
    libc::SYS_timer_settime,
    libc::SYS_getrandom,
    libc::SYS_getpid,
    libc::SYS_getppid,
    libc::SYS_gettid,
    libc::SYS_getuid,
    libc::SYS_geteuid,
    libc::SYS_getgid,
    libc::SYS_getegid,
    libc::SYS_getresuid,
    libc::SYS_getresgid,
    libc::SYS_getgroups,
    libc::SYS_uname,
    libc::SYS_sysinfo,
    libc::SYS_getcpu,
];

const fn build() -> ProcessPolicy {
    let mut b = Build::new();
    b.load(ARCH_OFFSET);
    b.emit(JEQ, 1, 0, ARCH);
    b.ret(KILL);
    b.load(NR);
    b.emit(JSET, 0, 1, 0x8000_0000 | X32);
    b.ret(KILL);
    let mut i = 0;
    while i < UNCONDITIONAL.len() {
        b.allow_rule(UNCONDITIONAL[i]);
        i += 1;
    }

    let r = b.rule(libc::SYS_clone3);
    b.ret(NO_SYS);
    b.end(r);
    let r = b.rule(libc::SYS_clone);
    b.mask(0, CLONE_MASK);
    b.load(low(0));
    b.emit(AND, 0, 0, libc::CSIGNAL as u32);
    // A is the masked signal, so use comparisons directly, not a reload.
    b.emit(JEQ, 2, 0, 0);
    b.emit(JEQ, 1, 0, libc::SIGCHLD as u32);
    b.ret(DENY);
    b.done(r);

    i = 0;
    let signals = [libc::SYS_kill, libc::SYS_tkill, libc::SYS_tgkill];
    while i < signals.len() {
        let r = b.rule(signals[i]);
        b.range(0, 2, i32::MAX as u32);
        if signals[i] == libc::SYS_tgkill {
            b.range(1, 2, i32::MAX as u32);
            b.range(2, 0, 64);
        } else {
            b.range(1, 0, 64);
        }
        b.done(r);
        i += 1;
    }

    let r = b.rule(libc::SYS_fcntl);
    b.range(0, 0, i32::MAX as u32);
    b.eq(high(1), 0);
    let cmds = [
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
    ];
    i = 0;
    while i < cmds.len() {
        let cmd = cmds[i];
        let c = b.skip_unless(low(1), cmd as u32);
        if cmd == libc::F_DUPFD || cmd == libc::F_DUPFD_CLOEXEC {
            b.range(2, 0, i32::MAX as u32);
        } else if cmd == libc::F_SETFD {
            b.mask(2, libc::FD_CLOEXEC as u32);
        } else if cmd == libc::F_SETFL {
            b.mask(2, SETFL_MASK);
        }
        b.done(c);
        i += 1;
    }
    b.ret(DENY);
    b.end(r);

    let r = b.rule(libc::SYS_dup3);
    b.arg_choices(2, &[0, libc::O_CLOEXEC as u32]);
    b.done(r);
    let r = b.rule(libc::SYS_close_range);
    b.eq(high(0), 0);
    b.eq(high(1), 0);
    b.mask(2, libc::CLOSE_RANGE_CLOEXEC | libc::CLOSE_RANGE_UNSHARE);
    b.done(r);

    let r = b.rule(libc::SYS_arch_prctl);
    // asm/prctl.h native x86_64 selectors, not x32 compatibility selectors.
    b.arg_choices(0, &[0x1002, 0x1003, 0x1004]);
    b.done(r);

    let r = b.rule(libc::SYS_prctl);
    b.eq(high(0), 0);
    let cmds = [
        libc::PR_SET_NAME,
        libc::PR_GET_NAME,
        libc::PR_GET_DUMPABLE,
        libc::PR_GET_NO_NEW_PRIVS,
        libc::PR_GET_SECCOMP,
        libc::PR_GET_PDEATHSIG,
        libc::PR_SET_NO_NEW_PRIVS,
    ];
    i = 0;
    while i < cmds.len() {
        let cmd = cmds[i];
        let c = b.skip_unless(low(0), cmd as u32);
        if cmd == libc::PR_SET_NO_NEW_PRIVS || cmd == libc::PR_GET_NO_NEW_PRIVS {
            b.arg_eq(
                1,
                if cmd == libc::PR_SET_NO_NEW_PRIVS {
                    1
                } else {
                    0
                },
            );
            b.arg_eq(2, 0);
            b.arg_eq(3, 0);
            b.arg_eq(4, 0);
        }
        // Kernel-ignored variadic slots are deliberately not inspected for other
        // commands. Name/PDEATHSIG output arguments are full-width pointers.
        b.done(c);
        i += 1;
    }
    b.ret(DENY);
    b.end(r);

    let r = b.rule(libc::SYS_prlimit64);
    b.arg_eq(0, 0);
    b.range(1, 0, libc::RLIM_NLIMITS as u32 - 1);
    b.done(r);
    let limits = [libc::SYS_getrlimit, libc::SYS_setrlimit];
    i = 0;
    while i < limits.len() {
        let r = b.rule(limits[i]);
        b.range(0, 0, libc::RLIM_NLIMITS as u32 - 1);
        b.done(r);
        i += 1;
    }
    // New limit pointers are kernel-validated: a fully cap-dropped payload
    // cannot expand inherited hard limits. The helper must make them finite.
    let self_only = [
        libc::SYS_sched_getaffinity,
        libc::SYS_sched_setaffinity,
        libc::SYS_get_robust_list,
    ];
    i = 0;
    while i < self_only.len() {
        let r = b.rule(self_only[i]);
        b.arg_eq(0, 0);
        b.done(r);
        i += 1;
    }
    let r = b.rule(libc::SYS_getpriority);
    b.arg_eq(0, libc::PRIO_PROCESS);
    b.arg_eq(1, 0);
    b.done(r);

    let r = b.rule(libc::SYS_getrusage);
    let zero = b.skip_unless(high(0), 0);
    b.choices(
        low(0),
        &[
            libc::RUSAGE_SELF as u32,
            libc::RUSAGE_THREAD as u32,
            u32::MAX,
        ],
    );
    b.ret(ALLOW);
    b.end(zero);
    b.eq(high(0), u32::MAX);
    b.eq(low(0), u32::MAX);
    b.done(r);

    let r = b.rule(libc::SYS_waitid);
    b.eq(high(0), 0);
    let c = b.skip_unless(low(0), libc::P_ALL);
    b.arg_eq(1, 0);
    b.done(c);
    let c = b.skip_unless(low(0), libc::P_PID);
    b.range(1, 2, i32::MAX as u32);
    b.done(c);
    let c = b.skip_unless(low(0), libc::P_PGID);
    b.range(1, 0, i32::MAX as u32);
    b.done(c);
    b.ret(DENY);
    b.end(r);

    b.ret(DENY);
    b.policy
}
static POLICY: ProcessPolicy = build();
pub(super) fn linux_read_only_host_v1() -> &'static ProcessPolicy {
    &POLICY
}

#[cfg(test)]
#[path = "repository_process_policy/tests.rs"]
mod tests;
