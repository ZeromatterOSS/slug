//! Exclusive launcher/pidfd lifecycle. Source anchors: repository_process.rs
//! ChildReaper ownership, WNOWAIT before signals and EINTR retry. Unlike that
//! legacy owner, an actual PID1 pidfd is retained through namespace death.
use std::mem::MaybeUninit;

use super::*;

pub(super) struct NamespaceReaped {
    _private: (),
}
pub(super) struct Lifecycle {
    pid: libc::pid_t,
    owned: bool,
    monitor_status: Option<i32>,
    init: Option<File>,
    authority_sent: bool,
    done: bool,
    status: [u8; 4096],
    status_len: usize,
    status_eof: bool,
    status_bad: bool,
}
impl Lifecycle {
    pub(super) fn new(pid: libc::pid_t) -> Self {
        Self {
            pid,
            owned: true,
            monitor_status: None,
            init: None,
            authority_sent: false,
            done: false,
            status: [0; 4096],
            status_len: 0,
            status_eof: false,
            status_bad: false,
        }
    }
    pub(super) fn hello(&mut self, file: File) -> io::Result<()> {
        if self.init.is_some() {
            return Err(invalid("duplicate PID1 descriptor"));
        }
        // Signal zero validates actual pidfd type/permission without signalling.
        let checked = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                file.as_raw_fd(),
                0,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        };
        if checked != 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error);
            }
        }
        self.init = Some(file);
        Ok(())
    }
    pub(super) fn sent_authority(&mut self) -> io::Result<()> {
        if self.init.is_none() {
            return Err(invalid("root transfer before PID1 identity"));
        }
        self.authority_sent = true;
        Ok(())
    }
    pub(super) fn note_done(&mut self) {
        self.done = true;
    }
    pub(super) fn monitor(&mut self) -> io::Result<bool> {
        if !self.owned {
            return Ok(self.monitor_status.is_some());
        }
        loop {
            let mut status = 0;
            let result = unsafe { libc::waitpid(self.pid, &mut status, libc::WNOHANG) };
            if result == 0 {
                return Ok(false);
            }
            if result == self.pid {
                self.owned = false;
                self.monitor_status = Some(status);
                return Ok(true);
            }
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            if error.raw_os_error() == Some(libc::ECHILD) {
                self.owned = false;
            }
            return Err(error);
        }
    }
    pub(super) fn namespace_dead(&self) -> io::Result<bool> {
        let Some(init) = &self.init else {
            return Ok(false);
        };
        let mut poll = libc::pollfd {
            fd: init.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        loop {
            let result = unsafe { libc::poll(&mut poll, 1, 0) };
            if result < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e);
            }
            if poll.revents & libc::POLLNVAL != 0 {
                return Err(invalid("invalid PID1 pidfd"));
            }
            return Ok(poll.revents & libc::POLLIN != 0);
        }
    }
    fn kill_monitor(&mut self) {
        if !self.owned {
            return;
        }
        let mut info = MaybeUninit::<libc::siginfo_t>::zeroed();
        loop {
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    self.pid as libc::id_t,
                    info.as_mut_ptr(),
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            if result == 0 {
                let observed = unsafe { info.assume_init().si_pid() };
                if observed == 0 || observed == self.pid {
                    unsafe {
                        libc::kill(self.pid, libc::SIGKILL);
                    }
                }
                return;
            }
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            if e.raw_os_error() == Some(libc::ECHILD) {
                self.owned = false;
            }
            return;
        }
    }
    /// Fatal completion cannot return selected native authority. Unexpected
    /// ownership failure leaves the caller's entire command/backing quarantined.
    pub(super) fn cleanup(
        &mut self,
        control: BorrowedFd<'_>,
        status_fd: BorrowedFd<'_>,
        plan: &RunPlan,
    ) -> bool {
        if self.init.is_none() {
            // Cancellation can race delivery of HELLO. Recover its actual
            // kernel object before shutting down the control channel.
            if let Ok((frame, mut rights)) = wire::receive(control) {
                if frame.kind == Kind::Hello
                    && frame.matches(plan)
                    && zero(&frame.words)
                    && rights.len() == 1
                {
                    let _ = self.hello(rights.pop().unwrap());
                }
            }
        }
        // Before HELLO ACK the trusted helper cannot mutate even bootstrap
        // roots. EOF releases its ACK wait; do NOT SIGKILL the outer monitor
        // when no PID1 identity has yet been acquired.
        unsafe {
            libc::shutdown(control.as_raw_fd(), libc::SHUT_RDWR);
        }
        if let Some(init) = &self.init {
            unsafe {
                libc::syscall(
                    libc::SYS_pidfd_send_signal,
                    init.as_raw_fd(),
                    libc::SIGKILL,
                    std::ptr::null::<libc::siginfo_t>(),
                    0,
                );
            }
            self.kill_monitor();
        }
        let deadline = Instant::now() + CLEANUP_TIME;
        loop {
            self.drain_status(status_fd);
            let monitor = self.monitor();
            if self.init.is_some() {
                if self.namespace_dead().unwrap_or(false)
                    && (matches!(monitor, Ok(true)) || !self.owned)
                {
                    return true;
                }
            } else if matches!(monitor, Ok(true))
                && self.status_eof
                && !self.status_bad
                && self.monitor_status.is_some_and(|status| {
                    monitor_reaped_status(&self.status[..self.status_len], status)
                })
            {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn drain_status(&mut self, fd: BorrowedFd<'_>) {
        if self.status_eof || self.status_bad {
            return;
        }
        for _ in 0..16 {
            let mut extra = [0u8; 1];
            let output = if self.status_len < self.status.len() {
                &mut self.status[self.status_len..]
            } else {
                &mut extra
            };
            let n = unsafe { libc::read(fd.as_raw_fd(), output.as_mut_ptr().cast(), output.len()) };
            if n < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                if e.kind() != io::ErrorKind::WouldBlock {
                    self.status_bad = true;
                }
                return;
            }
            if n == 0 {
                self.status_eof = true;
                return;
            }
            if self.status_len == self.status.len() {
                self.status_bad = true;
                return;
            }
            self.status_len += n as usize;
        }
    }
    pub(super) fn proof(&mut self, streams_eof: bool) -> io::Result<Option<NamespaceReaped>> {
        let monitor = self.monitor()?;
        if !self.done || !streams_eof || !self.namespace_dead()? || !monitor {
            return Ok(None);
        }
        let status = self
            .monitor_status
            .ok_or_else(|| invalid("missing launcher status"))?;
        if !libc::WIFEXITED(status) || libc::WEXITSTATUS(status) != 0 {
            return Err(invalid("native supervisor failed"));
        }
        Ok(Some(NamespaceReaped { _private: () }))
    }
}

/// Private monitor channel, never inherited by the helper/payload. Source:
/// containers/bubblewrap v0.11.2 bubblewrap.c: eventfd only at2869 when
/// !opt_as_pid_1; report_child_exit_status at554 follows matching waitpid at547;
/// child closes opt_json_status_fd at3056 BEFORE helper exec. A mere ordinary
/// launcher exit (e.g. die_with_error before waitpid) is NOT accepted.
pub(super) fn monitor_reaped_status(bytes: &[u8], status: i32) -> bool {
    if bytes.len() > 4096 || bytes.last() != Some(&b'\n') || !libc::WIFEXITED(status) {
        return false;
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let mut lines = text.lines();
    let Some(first) = lines.next() else {
        return false;
    };
    let Some(second) = lines.next() else {
        return false;
    };
    if lines.next().is_some() {
        return false;
    }
    let mut seen = 0u8;
    let first = record(first.as_bytes(), |name, value| {
        let bit = match name {
            b"child-pid" => 0,
            b"cgroup-namespace" => 1,
            b"ipc-namespace" => 2,
            b"mnt-namespace" => 3,
            b"net-namespace" => 4,
            b"pid-namespace" => 5,
            b"uts-namespace" => 6,
            _ => return false,
        };
        if value == 0 || (bit == 0 && value > i32::MAX as u64) || seen & (1 << bit) != 0 {
            return false;
        }
        seen |= 1 << bit;
        true
    });
    let mut exit = false;
    first
        && seen & 1 != 0
        && record(second.as_bytes(), |name, value| {
            if name != b"exit-code" || exit || value != libc::WEXITSTATUS(status) as u64 {
                return false;
            }
            exit = true;
            true
        })
        && exit
}
// Deliberately NOT a general JSON parser. Pinned bwrap emits one flat object
// per record: quoted literal known keys and unsigned decimal integer scalars.
// Borrowed slices, no escapes/duplicates/nesting/alternate number syntax, no
// allocation and no new production dependency.
fn record(bytes: &[u8], mut visit: impl FnMut(&[u8], u64) -> bool) -> bool {
    fn whitespace(bytes: &[u8], p: &mut usize) {
        while bytes
            .get(*p)
            .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\r'))
        {
            *p += 1;
        }
    }
    let mut p = 0;
    whitespace(bytes, &mut p);
    if bytes.get(p) != Some(&b'{') {
        return false;
    }
    p += 1;
    whitespace(bytes, &mut p);
    loop {
        if bytes.get(p) != Some(&b'"') {
            return false;
        }
        p += 1;
        let begin = p;
        while bytes.get(p).is_some_and(|b| *b != b'"') {
            if bytes[p] == b'\\' || bytes[p] < 0x20 {
                return false;
            }
            p += 1;
        }
        if bytes.get(p) != Some(&b'"') {
            return false;
        }
        let key = &bytes[begin..p];
        p += 1;
        whitespace(bytes, &mut p);
        if bytes.get(p) != Some(&b':') {
            return false;
        }
        p += 1;
        whitespace(bytes, &mut p);
        let begin = p;
        let mut value = 0u64;
        while bytes.get(p).is_some_and(u8::is_ascii_digit) {
            let Some(next) = value
                .checked_mul(10)
                .and_then(|v| v.checked_add((bytes[p] - b'0') as u64))
            else {
                return false;
            };
            value = next;
            p += 1;
        }
        if begin == p || (p > begin + 1 && bytes[begin] == b'0') || !visit(key, value) {
            return false;
        }
        whitespace(bytes, &mut p);
        match bytes.get(p) {
            Some(b',') => {
                p += 1;
                whitespace(bytes, &mut p);
            }
            Some(b'}') => {
                p += 1;
                whitespace(bytes, &mut p);
                return p == bytes.len();
            }
            _ => return false,
        }
    }
}
