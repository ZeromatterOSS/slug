//! Trusted PID 1 waits for the entire namespace, independently of stdio EOF.
//! Payload and supervisor install different closed policies after their fork.
use super::*;

fn instruction(code: u16, jt: u8, jf: u8, k: u32) -> libc::sock_filter {
    libc::sock_filter { code, jt, jf, k }
}
const LD: u16 = 0x20;
const JEQ: u16 = 0x15;
const RET: u16 = 0x06;
const ALLOW: u32 = libc::SECCOMP_RET_ALLOW;
const DENY: u32 = libc::SECCOMP_RET_ERRNO | libc::EPERM as u32;
const KILL: u32 = libc::SECCOMP_RET_KILL_PROCESS;
// Constructed before fork and before any policy is installed. The trusted
// supervisor cannot open files, create processes, enter namespaces or exec.
pub(super) fn supervisor_policy() -> Vec<libc::sock_filter> {
    let mut code = Vec::with_capacity(128);
    code.extend([
        instruction(LD, 0, 0, 4),
        instruction(JEQ, 1, 0, 0xc000003e),
        instruction(RET, 0, 0, KILL),
    ]);
    for syscall in [
        libc::SYS_poll,
        libc::SYS_clock_gettime,
        libc::SYS_exit,
        libc::SYS_exit_group,
    ] {
        code.extend([
            instruction(LD, 0, 0, 0),
            instruction(JEQ, 0, 1, syscall as u32),
            instruction(RET, 0, 0, ALLOW),
        ]);
    }
    for (syscall, fd) in [
        (libc::SYS_sendmsg, 3),
        (libc::SYS_read, 4),
        (libc::SYS_close, 4),
    ] {
        code.extend([
            instruction(LD, 0, 0, 0),
            instruction(JEQ, 0, 5, syscall as u32),
            instruction(LD, 0, 0, 20),
            instruction(JEQ, 0, 3, 0),
            instruction(LD, 0, 0, 16),
            instruction(JEQ, 0, 1, fd),
            instruction(RET, 0, 0, ALLOW),
        ]);
    }
    for syscall in [libc::SYS_kill, libc::SYS_wait4] {
        let expected = if syscall == libc::SYS_kill {
            libc::SIGKILL as u32
        } else {
            (libc::__WALL | libc::WNOHANG) as u32
        };
        let offset = if syscall == libc::SYS_kill { 24 } else { 32 };
        // libc may zero-extend or sign-extend a negative int syscall argument.
        // Both encodings mean -1, never an arbitrary positive target PID.
        code.extend([
            instruction(LD, 0, 0, 0),
            instruction(JEQ, 0, 12, syscall as u32),
            instruction(LD, 0, 0, 16),
            instruction(JEQ, 0, 10, u32::MAX),
            instruction(LD, 0, 0, 20),
            instruction(JEQ, 1, 0, 0),
            instruction(JEQ, 0, 7, u32::MAX),
            instruction(LD, 0, 0, offset + 4),
            instruction(JEQ, 0, 5, 0),
            instruction(LD, 0, 0, offset),
            instruction(JEQ, 0, 3, expected),
            instruction(RET, 0, 0, ALLOW),
            instruction(RET, 0, 0, DENY),
            instruction(RET, 0, 0, DENY),
        ]);
    }
    code.push(instruction(RET, 0, 0, DENY));
    code
}
fn install(code: &[libc::sock_filter]) -> io::Result<()> {
    let program = libc::sock_fprog {
        len: code.len() as u16,
        filter: code.as_ptr().cast_mut(),
    };
    if unsafe {
        libc::syscall(
            libc::SYS_seccomp,
            libc::SECCOMP_SET_MODE_FILTER,
            0,
            &program,
        )
    } != 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
pub(super) fn candidates(plan: &RunPlan) -> Result<Vec<CString>> {
    let executable = plan.argv.first().ok_or_else(|| invalid(11))?;
    if executable.as_bytes().contains(&b'/') {
        return Ok(vec![executable.clone()]);
    }
    // Effective environment, not execvpe's calling-process PATH. Empty PATH
    // components designate the selected cwd and argv[0] remains literal.
    let path = plan
        .env
        .iter()
        .find_map(|row| row.as_bytes().strip_prefix(b"PATH="))
        .unwrap_or(b"/bin:/usr/bin");
    let mut result = Vec::new();
    let mut retained = 0usize;
    for directory in path.split(|b| *b == b':') {
        if result.len() >= wire::MAX_ITEMS {
            return Err(invalid(11));
        }
        let size = directory
            .len()
            .checked_add(executable.as_bytes().len())
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| invalid(11))?;
        retained = retained.checked_add(size).ok_or_else(|| invalid(11))?;
        if size > 4096 || retained > wire::MAX_PLAN {
            return Err(invalid(11));
        }
        let mut bytes = Vec::with_capacity(size);
        if !directory.is_empty() {
            bytes.extend_from_slice(directory);
            bytes.push(b'/');
        }
        bytes.extend_from_slice(executable.as_bytes());
        result.push(CString::new(bytes).map_err(|_| invalid(11))?);
    }
    Ok(result)
}
fn child_error(stage: u64, errno: i32) -> ! {
    child_error_fd(3, stage, errno)
}
fn child_error_fd(fd: i32, stage: u64, errno: i32) -> ! {
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&stage.to_le_bytes());
    bytes[8..].copy_from_slice(&(errno as u64).to_le_bytes());
    loop {
        let count = unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) };
        if count < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
            continue;
        }
        break;
    }
    unsafe { libc::_exit(127) }
}
fn child_exec(
    executables: &[CString],
    argv: &[*const libc::c_char],
    env: &[*const libc::c_char],
    write: i32,
    policy: &[libc::sock_filter],
) -> ! {
    // Only async-signal-safe syscalls and immutable prebuilt buffers after fork.
    if unsafe { libc::dup3(write, 3, libc::O_CLOEXEC) } != 3 {
        child_error_fd(
            write,
            9,
            io::Error::last_os_error()
                .raw_os_error()
                .unwrap_or(libc::EIO),
        );
    }
    if unsafe { libc::syscall(libc::SYS_close_range, 4u32, u32::MAX, 0u32) } != 0 {
        child_error(
            9,
            io::Error::last_os_error()
                .raw_os_error()
                .unwrap_or(libc::EIO),
        );
    }
    // Rust startup ignores SIGPIPE. Restore ordinary payload disposition;
    // this helper never installed any payload-specific signal handlers.
    let mut disposition = unsafe { std::mem::zeroed::<libc::sigaction>() };
    disposition.sa_sigaction = libc::SIG_DFL;
    unsafe { libc::sigemptyset(&mut disposition.sa_mask) };
    if unsafe { libc::sigaction(libc::SIGPIPE, &disposition, std::ptr::null_mut()) } != 0 {
        child_error(
            9,
            io::Error::last_os_error()
                .raw_os_error()
                .unwrap_or(libc::EIO),
        );
    }
    let mut mask = unsafe { std::mem::zeroed::<libc::sigset_t>() };
    unsafe { libc::sigemptyset(&mut mask) };
    if unsafe { libc::sigprocmask(libc::SIG_SETMASK, &mask, std::ptr::null_mut()) } != 0 {
        child_error(
            9,
            io::Error::last_os_error()
                .raw_os_error()
                .unwrap_or(libc::EIO),
        );
    }
    if let Err(error) = install(policy) {
        child_error(13, error.raw_os_error().unwrap_or(libc::EIO));
    }
    let mut denied = false;
    for path in executables {
        unsafe { libc::execve(path.as_ptr(), argv.as_ptr(), env.as_ptr()) };
        let error = io::Error::last_os_error()
            .raw_os_error()
            .unwrap_or(libc::EIO);
        match error {
            libc::ENOENT | libc::ENOTDIR => {}
            libc::EACCES => denied = true,
            _ => child_error(11, error),
        }
    }
    child_error(11, if denied { libc::EACCES } else { libc::ENOENT })
}
fn terminal(plan: &RunPlan, kind: Kind, words: [u64; 8], exit: i32) -> ! {
    let _ = send(plan, kind, words, &[]);
    unsafe { libc::_exit(exit) }
}
fn kill_all() {
    unsafe { libc::kill(-1, libc::SIGKILL) };
}

pub(super) fn execute(plan: &RunPlan, deadline: Instant) -> Result<()> {
    let executables = candidates(plan)?;
    let argv: Vec<_> = plan
        .argv
        .iter()
        .map(|v| v.as_ptr())
        .chain(std::iter::once(std::ptr::null()))
        .collect();
    let env: Vec<_> = plan
        .env
        .iter()
        .map(|v| v.as_ptr())
        .chain(std::iter::once(std::ptr::null()))
        .collect();
    let supervisor = supervisor_policy();
    let payload = crate::runtime::repository_process_policy::linux_read_only_host_v1();
    payload.validate().map_err(|_| invalid(13))?;
    if Instant::now() >= deadline {
        return Err(failure(10, io::Error::from(io::ErrorKind::TimedOut)));
    }
    let mut disposition = unsafe { std::mem::zeroed::<libc::sigaction>() };
    disposition.sa_sigaction = libc::SIG_DFL;
    unsafe { libc::sigemptyset(&mut disposition.sa_mask) };
    checked(10, unsafe {
        libc::sigaction(libc::SIGCHLD, &disposition, std::ptr::null_mut())
    })?;
    let mut pair = [-1; 2];
    checked(10, unsafe {
        libc::pipe2(pair.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK)
    })?;
    let read = unsafe { File::from_raw_fd(pair[0]) };
    let write = unsafe { File::from_raw_fd(pair[1]) };
    if pair != [4, 5] {
        return Err(invalid(10));
    }
    let main = unsafe { libc::fork() };
    if main < 0 {
        return Err(failure(10, io::Error::last_os_error()));
    }
    if main == 0 {
        child_exec(
            &executables,
            &argv,
            &env,
            write.as_raw_fd(),
            payload.instructions(),
        );
    }
    drop(write);
    // PID1 must not keep output writers alive or leak setup handles to payload.
    unsafe {
        libc::close(1);
        libc::close(2)
    };
    if install(&supervisor).is_err() {
        kill_all();
        // No result is authenticated on a supervisor-policy infrastructure
        // failure. PID1's death makes the kernel tear down the entire namespace.
        unsafe { libc::_exit(125) };
    }
    let mut error_bytes = [0u8; 17];
    let mut error_len = 0usize;
    let mut error_eof = false;
    let mut error_stage = None;
    let mut failed = None;
    let mut main_status = None;
    let mut read_open = true;
    loop {
        if Instant::now() >= deadline && failed.is_none() {
            failed = Some((12, libc::ETIMEDOUT));
        }
        if !error_eof {
            let count = unsafe {
                libc::read(
                    read.as_raw_fd(),
                    error_bytes[error_len..].as_mut_ptr().cast(),
                    error_bytes.len() - error_len,
                )
            };
            if count > 0 {
                error_len += count as usize;
                if error_len > 16 {
                    failed = Some((11, libc::EPROTO));
                }
            } else if count == 0 {
                error_eof = true;
                if error_len == 16 {
                    let stage = u64::from_le_bytes(error_bytes[..8].try_into().unwrap());
                    let errno = u64::from_le_bytes(error_bytes[8..16].try_into().unwrap());
                    if stage > 15 || errno == 0 || errno > 4095 {
                        failed = Some((11, libc::EPROTO));
                    } else {
                        error_stage = Some((stage, errno as i32));
                    }
                } else if error_len != 0 {
                    failed = Some((11, libc::EPROTO));
                }
                unsafe { libc::close(4) };
                read_open = false;
            } else {
                let errno = io::Error::last_os_error()
                    .raw_os_error()
                    .unwrap_or(libc::EIO);
                if errno != libc::EINTR && errno != libc::EAGAIN {
                    failed = Some((11, errno));
                }
            }
        }
        if main_status.is_some() || failed.is_some() || error_stage.is_some() {
            kill_all();
        }
        let mut none = false;
        loop {
            let mut status = 0;
            let child = unsafe { libc::waitpid(-1, &mut status, libc::__WALL | libc::WNOHANG) };
            if child > 0 {
                if child == main {
                    main_status = Some(status);
                }
                continue;
            }
            if child == 0 {
                break;
            }
            let errno = io::Error::last_os_error()
                .raw_os_error()
                .unwrap_or(libc::EIO);
            if errno == libc::EINTR {
                continue;
            }
            if errno == libc::ECHILD {
                none = true;
                break;
            }
            failed = Some((12, errno));
            break;
        }
        if none && error_eof {
            if let Some((stage, errno)) = error_stage {
                terminal(
                    plan,
                    Kind::ExecError,
                    [stage, errno as u64, 0, 0, 0, 0, 0, 0],
                    125,
                );
            }
            if let Some((stage, errno)) = failed {
                terminal(
                    plan,
                    Kind::SetupError,
                    [stage, errno as u64, 0, 0, 0, 0, 0, 0],
                    125,
                );
            }
            if let Some(status) = main_status {
                terminal(plan, Kind::Done, [status as u64, 1, 0, 0, 0, 0, 0, 0], 0);
            }
            terminal(
                plan,
                Kind::SetupError,
                [12, libc::ECHILD as u64, 0, 0, 0, 0, 0, 0],
                125,
            );
        }
        let mut polls = [
            libc::pollfd {
                fd: 3,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: if read_open { 4 } else { -1 },
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let count = unsafe { libc::poll(polls.as_mut_ptr(), polls.len() as libc::nfds_t, 10) };
        if count < 0 && io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
            failed = Some((12, libc::EIO));
        }
        if polls[0].revents != 0 {
            failed = Some((12, libc::EPIPE));
        }
    }
}
