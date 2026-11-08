//! Inactive consuming native process driver. No DICE transaction, callback,
//! printer, immutable-result constructor or GCC recipe enters this owner.
//! One owned thread exists BEFORE native launch and outlives async cancellation.
use std::ffi::CString;
use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::os::fd::AsFd;
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use slug_bzlmod_v2::RepositoryWorkspaceOperation;
use slug_bzlmod_v2::RepositoryWorkspaceRequest;
use slug_bzlmod_v2::RepositoryWorkspaceReservation;
use wire::Frame;
use wire::Kind;
use wire::RunMode;
use wire::RunPlan;

use super::super::repository_process_wire as wire;
use super::*;
#[path = "repository_process_owner/capture.rs"]
mod capture;
#[path = "repository_process_owner/lifecycle.rs"]
mod lifecycle;
#[path = "repository_process_owner/spawn.rs"]
mod spawn;
pub(super) use capture::ReapedBinding;
pub(super) use capture::ReapedCapture;

/// Only the closed bootstrap/first/continuation owners in command glue implement
/// this private trait. No consumer can substitute an immutable process result.
pub(super) trait ProcessLaunch: Send + 'static {
    type Output: Send + 'static;
    fn mode(&self) -> RunMode;
    fn reservation(&self) -> &Arc<RepositoryWorkspaceReservation>;
    fn request(&self) -> Option<&RepositoryWorkspaceRequest>;
    fn covered_request_owner(&self) -> Option<Arc<CoveredWorkspaceRequest>>;
    fn root_identity(&self) -> Option<RootIdentity>;
    fn scope(&self) -> &Arc<SourceServiceScope>;
    fn resources(&self) -> &Arc<RepositoryRootResources>;
    fn retirement_observer(
        &self,
    ) -> Option<super::progress::command::RepositoryWorkspaceRetirementObserver> {
        None
    }
    #[cfg(test)]
    fn process_probe(&self) -> Option<super::progress::command::RepositoryWorkspaceProcessProbe> {
        None
    }
    fn bind_attempt(&mut self, _nonce: &[u8; 16]) -> Result<(), PhysicalRootError> {
        Ok(())
    }
    fn views(&self) -> &[wire::ViewEntry] {
        &[]
    }
    fn transfer_view(
        &mut self,
        _slot: usize,
        _socket: BorrowedFd<'_>,
        _nonce: &[u8; 16],
    ) -> Result<(), PhysicalRootError> {
        Err(PhysicalRootError::Mode)
    }
    fn verify_view_successor(
        &mut self,
        _slot: usize,
        _attached: File,
        _token: File,
        _words: [u64; 8],
    ) -> Result<(), PhysicalRootError> {
        Err(PhysicalRootError::Mode)
    }
    fn namespace(&self) -> Result<Option<BorrowedFd<'_>>, PhysicalRootError>;
    fn transfer(
        &mut self,
        socket: BorrowedFd<'_>,
        nonce: &[u8; 16],
    ) -> Result<(), PhysicalRootError>;
    fn verify_successor(
        &mut self,
        attached: File,
        token: File,
        userns: File,
        words: [u64; 8],
        quota: Option<[u64; 8]>,
    ) -> Result<(), PhysicalRootError>;
    fn finish_reaped(self, capture: ReapedCapture) -> Result<Self::Output, PhysicalRootError>;
}

const THREAD_STACK: usize = 256 * 1024;
const CONTROL_BYTES: usize = THREAD_STACK + wire::PLAN_RETAINED_BYTES + 512 * 1024;
// Control, stdio, image pins, directory cursor, PID1 and up to three
// transient received rights are positively covered; mirror pins/normalization
// have separate credits. No existing source bank/default is raised.
// Covers the monitor's additional mirror/image FD copies before its
// close_extra_fds runs, held until actual monitor reap. Not a kernel-wide FD
// or RSS claim; caller still supplies the finite native resource bank.
const CONTROL_FDS: usize = 48 + wire::MAX_MIRRORS + 2;
// Virtual allowances for plan/newroot/scaffold/tmp, minimal dev mount and
// the bounded2048-entry Host-proc masking inventory. Not a kernel-RAM claim.
const TEMP_BACKING: u64 = (2 + 1 + 1 + 16) * 1024 * 1024 + 4096 + 2048 * 4096;
#[cfg(test)]
pub(super) fn temporary_backing_for_test() -> u64 {
    TEMP_BACKING
}
const TOTAL_TIME: Duration = Duration::from_secs(30);
const CLEANUP_TIME: Duration = Duration::from_secs(3);

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn failure(message: &'static str) -> PhysicalRootError {
    invalid(message).into()
}

/// Paths are chosen by Slug's trusted installation, never by an Execute argv.
/// Current executable startup must be protected against external writers.
pub(super) struct ProcessPlatform {
    bwrap: PathBuf,
    helper: PathBuf,
    test_images: bool,
    #[cfg(test)]
    observer: Option<Arc<AtomicUsize>>,
    #[cfg(test)]
    hold_before_hello: bool,
    #[cfg(test)]
    hold_after_view_transfer: bool,
}
impl ProcessPlatform {
    pub(super) fn current() -> io::Result<Self> {
        let bwrap = if Path::new("/usr/bin/bwrap").is_file() {
            PathBuf::from("/usr/bin/bwrap")
        } else {
            PathBuf::from("/home/linuxbrew/.linuxbrew/Cellar/bubblewrap/0.11.2/bin/bwrap")
        };
        Ok(Self {
            bwrap,
            helper: std::env::current_exe()?,
            test_images: false,
            #[cfg(test)]
            observer: None,
            #[cfg(test)]
            hold_before_hello: false,
            #[cfg(test)]
            hold_after_view_transfer: false,
        })
    }
    /// Observation/gate only: no execution plan, payload bytes or policy is
    /// changed. Native tests can cancel an actually armed owner deterministically.
    #[cfg(test)]
    pub(super) fn observe_for_test(
        mut self,
        stage: Arc<AtomicUsize>,
        hold_before_hello: bool,
    ) -> Self {
        self.observer = Some(stage);
        self.hold_before_hello = hold_before_hello;
        self
    }
    /// Hold only after a real RO token has crossed SCM, before final ACK.
    #[cfg(test)]
    pub(super) fn observe_view_transfer_for_test(mut self, stage: Arc<AtomicUsize>) -> Self {
        self.observer = Some(stage);
        self.hold_after_view_transfer = true;
        self
    }
    #[cfg(test)]
    pub(super) fn compiled_cli(helper: PathBuf, bwrap: PathBuf) -> Self {
        Self {
            bwrap,
            helper,
            test_images: true,
            observer: None,
            hold_before_hello: false,
            hold_after_view_transfer: false,
        }
    }
}

fn environment(
    execute: &slug_bzlmod_v2::RepositoryWorkspaceExecute,
    mut visit: impl FnMut(&str, &str) -> Result<(), PhysicalRootError>,
) -> Result<(), PhysicalRootError> {
    use slug_bzlmod_v2::RepositoryWorkspaceEnvironmentValue;
    for entry in execute.environment().iter() {
        if let Some(value) = execute.effective_value(entry.name()) {
            visit(entry.name(), value)?;
        }
    }
    for (name, value) in execute.overrides().iter() {
        if execute.environment().get(name).is_none() {
            if let RepositoryWorkspaceEnvironmentValue::Override(value) = value {
                visit(name, value)?;
            }
        }
    }
    Ok(())
}
fn plan<L: ProcessLaunch>(
    launch: &L,
    mirrors: Vec<wire::MirrorEntry>,
    nonce: [u8; 16],
) -> Result<RunPlan, PhysicalRootError> {
    let display = launch
        .reservation()
        .display_root()
        .as_path()
        .as_os_str()
        .as_bytes();
    let workspace = launch
        .reservation()
        .workspace()
        .as_path()
        .as_os_str()
        .as_bytes();
    let execute = match launch.request() {
        Some(request) => match request.operation() {
            RepositoryWorkspaceOperation::Execute(e) => Some(e),
            _ => return Err(PhysicalRootError::Mode),
        },
        None => None,
    };
    if launch.mode().has_payload() != execute.is_some() {
        return Err(PhysicalRootError::Mode);
    }
    let cwd = execute.map_or(display, |e| e.working_directory().path_str().as_bytes());
    // Forecast BEFORE cloning a single request string or making env="key=value".
    // The codec header has42bytes, three item lengths, two array counts and one
    // mirror count. Item/mirror payload arithmetic is checked against MAX_PLAN.
    let mut forecast = PlanForecast::new();
    forecast.item(display.len())?;
    forecast.item(workspace.len())?;
    forecast.item(cwd.len())?;
    let mut env_count = 0usize;
    if let Some(e) = execute {
        if e.argv().len() > wire::MAX_ITEMS {
            return Err(PhysicalRootError::Resource);
        }
        for arg in e.argv() {
            forecast.item(arg.text().len())?;
        }
        environment(e, |name, value| {
            env_count = env_count
                .checked_add(1)
                .filter(|n| *n <= wire::MAX_ITEMS)
                .ok_or(PhysicalRootError::Resource)?;
            forecast.item(
                name.len()
                    .checked_add(1)
                    .and_then(|n| n.checked_add(value.len()))
                    .ok_or(PhysicalRootError::Resource)?,
            )
        })?;
    }
    for row in &mirrors {
        forecast.item(row.name.as_bytes().len())?;
        forecast.raw(24)?;
        forecast.item(row.link.as_ref().map_or(0, |l| l.as_bytes().len()))?;
    }
    if launch.views().len() > wire::MAX_VIEWS {
        return Err(PhysicalRootError::Resource);
    }
    for view in launch.views() {
        forecast.item(view.display.as_bytes().len())?;
        forecast.raw(32)?;
    }
    // Native execution consumes the ORIGINAL source scope's remaining time;
    // the immutable requested timeout (e.g.600) is never rewritten or refreshed.
    launch.scope().check()?;
    let remaining = u64::try_from(
        launch
            .scope()
            .work_deadline()
            .saturating_duration_since(Instant::now())
            .as_millis(),
    )
    .map_err(|_| PhysicalRootError::Resource)?;
    let requested = if let Some(e) = execute {
        u64::try_from(e.timeout_seconds())
            .map_err(|_| PhysicalRootError::Mode)?
            .checked_mul(1000)
            .ok_or(PhysicalRootError::Resource)?
    } else {
        27000
    };
    let timeout_millis = requested.min(27000).min(remaining);
    if timeout_millis == 0 {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "repository source scope has no execution time remaining",
        )
        .into());
    }
    let cstring = |s: &[u8]| CString::new(s).map_err(|_| failure("invalid execution string"));
    let mut argv = Vec::new();
    let mut env = Vec::new();
    if let Some(e) = execute {
        argv.try_reserve_exact(e.argv().len())
            .map_err(|_| PhysicalRootError::Resource)?;
        for arg in e.argv() {
            argv.push(cstring(arg.text().as_bytes())?);
        }
        env.try_reserve_exact(env_count)
            .map_err(|_| PhysicalRootError::Resource)?;
        environment(e, |name, value| {
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(name.len() + 1 + value.len())
                .map_err(|_| PhysicalRootError::Resource)?;
            bytes.extend_from_slice(name.as_bytes());
            bytes.push(b'=');
            bytes.extend_from_slice(value.as_bytes());
            env.push(CString::new(bytes).map_err(|_| failure("invalid execution environment"))?);
            Ok(())
        })?;
    }
    let mut views = Vec::new();
    views
        .try_reserve_exact(launch.views().len())
        .map_err(|_| PhysicalRootError::Resource)?;
    for view in launch.views() {
        views.push(wire::ViewEntry {
            display: cstring(view.display.as_bytes())?,
            instance: view.instance,
            device: view.device,
            inode: view.inode,
            mount: view.mount,
        });
    }
    let plan = RunPlan {
        mode: launch.mode(),
        display: cstring(display)?,
        workspace: cstring(workspace)?,
        cwd: cstring(cwd)?,
        argv,
        env,
        mirrors,
        views,
        nonce,
        index: launch.request().map_or(0, |r| r.index() as u64),
        timeout_millis,
    };
    plan.validate()?;
    Ok(plan)
}
fn nonce() -> io::Result<[u8; 16]> {
    let mut value = [0; 16];
    let mut offset = 0;
    while offset < value.len() {
        let n = unsafe {
            libc::getrandom(value[offset..].as_mut_ptr().cast(), value.len() - offset, 0)
        };
        if n < 0 {
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e);
        }
        if n == 0 {
            return Err(invalid("empty native nonce"));
        }
        offset += n as usize;
    }
    Ok(value)
}

struct Cancel(Arc<AtomicBool>);
impl Drop for Cancel {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
/// No native child exists if worker creation fails. A dropped future only sets
/// cancellation; the existing worker retains every command/root/credit owner.
pub(super) async fn run<L: ProcessLaunch>(
    launch: L,
    platform: ProcessPlatform,
    active: &(dyn Fn() -> bool + Sync),
) -> Result<L::Output, PhysicalRootError> {
    check(launch.scope(), active)?;
    #[cfg(test)]
    let platform = match launch.process_probe() {
        Some(probe) => platform.observe_for_test(probe.stage, probe.hold_before_hello),
        None => platform,
    };
    let credit = launch
        .resources()
        .reserve(CONTROL_BYTES, CONTROL_FDS, TEMP_BACKING)?;
    let stop = Arc::new(AtomicBool::new(false));
    let cancel = Cancel(stop.clone());
    let (send, mut receive) = tokio::sync::oneshot::channel();
    let scope = launch.scope().clone();
    let deadline = Instant::now() + TOTAL_TIME;
    let worker_retirement = launch
        .retirement_observer()
        .map(|observer| observer.worker_guard());
    std::thread::Builder::new()
        .name("slug-repository-owner".into())
        .stack_size(THREAD_STACK)
        .spawn(move || {
            let result = drive(launch, platform, stop, credit);
            let _ = send.send(result);
            // A refused receiver drops the complete output before this guard.
            drop(worker_retirement);
        })
        .map_err(PhysicalRootError::Io)?;
    loop {
        tokio::select! {
            result=&mut receive=>{
                let output=result.map_err(|_|failure("repository worker failed"))?;
                check(&scope,active)?;
                drop(cancel);
                return output;
            }
            _=tokio::time::sleep(Duration::from_millis(10))=>{
                if !active(){cancel.0.store(true,Ordering::Release);return Err(PhysicalRootError::Inactive);}
                if let Err(error)=scope.check(){cancel.0.store(true,Ordering::Release);return Err(error.into());}
                if Instant::now()>=deadline{cancel.0.store(true,Ordering::Release);return Err(io::Error::new(io::ErrorKind::TimedOut,"repository process runtime limit").into());}
            }
        }
    }
}

struct Driver<L: ProcessLaunch> {
    launch: Option<L>,
    child: Option<spawn::Spawned>,
    life: Option<lifecycle::Lifecycle>,
    captures: Option<capture::Captures>,
    mirror: Option<spawn::HostMirror>,
    plan: Option<RunPlan>,
    credit: Option<resources::ResourceCredit>,
}
impl<L: ProcessLaunch> Drop for Driver<L> {
    fn drop(&mut self) {
        if let Some(life) = &mut self.life {
            // Cleanup timeouts are infrastructure failures, never permission to
            // abandon native authority. The ALREADY EXISTING worker keeps the
            // entire driver (command, pins, endpoints, pidfd and ALL credits)
            // armed and retries until actual death/reap is established. This
            // also protects panic unwinding; no cleanup thread/runtime is born.
            while !life.cleanup(
                self.child.as_ref().unwrap().control.as_fd(),
                self.child.as_ref().unwrap().status.as_fd(),
                self.plan.as_ref().unwrap(),
            ) {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Hello,
    Ready,
    Successor,
    AfterQuota,
    Views(usize),
    Running,
    Done,
}
fn zero(words: &[u64]) -> bool {
    words.iter().all(|v| *v == 0)
}
fn eof(fd: BorrowedFd<'_>) -> io::Result<bool> {
    let mut byte = 0u8;
    loop {
        let n = unsafe {
            libc::recv(
                fd.as_raw_fd(),
                (&mut byte as *mut u8).cast(),
                1,
                libc::MSG_PEEK | libc::MSG_DONTWAIT,
            )
        };
        if n >= 0 {
            return Ok(n == 0);
        }
        let e = io::Error::last_os_error();
        match e.kind() {
            io::ErrorKind::Interrupted => continue,
            io::ErrorKind::WouldBlock => return Ok(false),
            _ => return Err(e),
        }
    }
}
fn drive<L: ProcessLaunch>(
    launch: L,
    platform: ProcessPlatform,
    stop: Arc<AtomicBool>,
    credit: resources::ResourceCredit,
) -> Result<L::Output, PhysicalRootError> {
    let mut driver = Driver {
        launch: Some(launch),
        child: None,
        life: None,
        captures: None,
        mirror: None,
        plan: None,
        credit: Some(credit),
    };
    let attempt_nonce = nonce()?;
    driver
        .launch
        .as_mut()
        .unwrap()
        .bind_attempt(&attempt_nonce)?;
    let owner = driver.launch.as_ref().unwrap();
    let captures = capture::Captures::new(owner.resources())?;
    driver.captures = Some(captures);
    let (mirror, rows) = spawn::HostMirror::collect(owner.resources(), &stop, owner.scope())?;
    driver.mirror = Some(mirror);
    driver.plan = Some(plan(owner, rows, attempt_nonce)?);
    if stop.load(Ordering::Acquire) {
        return Err(PhysicalRootError::Inactive);
    }
    owner.scope().check()?;
    let child = spawn::launch(
        &platform,
        driver.plan.as_ref().unwrap(),
        driver.mirror.as_ref().unwrap(),
        owner.namespace()?,
        owner.resources(),
    )?;
    driver.life = Some(lifecycle::Lifecycle::new(child.pid));
    driver.child = Some(child);
    #[cfg(test)]
    {
        if let Some(stage) = &platform.observer {
            stage.store(1, Ordering::Release);
        }
        // The private HELLO is never received/ACKed in this gate. Bootstrap
        // therefore cannot create its selected root before test cancellation.
        while platform.hold_before_hello {
            if stop.load(Ordering::Acquire) {
                return Err(PhysicalRootError::Inactive);
            }
            driver.launch.as_ref().unwrap().scope().check()?;
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    let start = Instant::now();
    let mut phase = Phase::Hello;
    let mut quota = None;
    let mut code = None;
    let mut control_closed = false;
    loop {
        if stop.load(Ordering::Acquire) {
            return Err(PhysicalRootError::Inactive);
        }
        driver.launch.as_ref().unwrap().scope().check()?;
        if start.elapsed() >= TOTAL_TIME {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "repository process runtime limit",
            )
            .into());
        }
        let child = driver.child.as_ref().unwrap();
        let captures = driver.captures.as_mut().unwrap();
        captures.drain(child.stdout.as_fd(), false)?;
        captures.drain(child.stderr.as_fd(), true)?;
        #[cfg(test)]
        if !captures.stdout.is_empty() {
            if let Some(stage) = &platform.observer {
                stage.store(3, Ordering::Release);
            }
        }
        if !control_closed {
            for _ in 0..16 {
                if eof(child.control.as_fd())? {
                    control_closed = true;
                    if phase != Phase::Done {
                        return Err(failure("helper closed before completion"));
                    }
                    break;
                }
                let received = wire::receive(child.control.as_fd());
                let (frame, mut rights) = match received {
                    Ok(v) => v,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(e) => return Err(e.into()),
                };
                if !frame.matches(driver.plan.as_ref().unwrap()) {
                    return Err(failure("foreign helper frame"));
                }
                let owner = driver.launch.as_mut().unwrap();
                let life = driver.life.as_mut().unwrap();
                match frame.kind {
                    Kind::Hello
                        if phase == Phase::Hello && rights.len() == 1 && zero(&frame.words) =>
                    {
                        life.hello(rights.pop().unwrap())?;
                        wire::send(
                            child.control.as_fd(),
                            &Frame::new(Kind::HelloAck, frame.nonce, frame.index),
                            &[],
                        )?;
                        phase = Phase::Ready;
                    }
                    Kind::Ready
                        if phase == Phase::Ready && rights.is_empty() && zero(&frame.words) =>
                    {
                        if owner.mode() != RunMode::InitializeArchive {
                            life.sent_authority()?;
                            owner.transfer(child.control.as_fd(), &frame.nonce)?;
                        }
                        phase = Phase::Successor;
                    }
                    Kind::Quota
                        if phase == Phase::Successor
                            && owner.mode() == RunMode::FirstExecute
                            && rights.is_empty() =>
                    {
                        quota = Some(frame.words);
                        phase = Phase::AfterQuota;
                    }
                    Kind::Successor
                        if rights.len() == 3
                            && ((phase == Phase::AfterQuota
                                && owner.mode() == RunMode::FirstExecute)
                                || (phase == Phase::Successor
                                    && owner.mode() != RunMode::FirstExecute)) =>
                    {
                        let array: [File; 3] = rights
                            .try_into()
                            .map_err(|_| failure("invalid successor rights"))?;
                        let [attached, token, userns] = array;
                        owner.verify_successor(
                            attached,
                            token,
                            userns,
                            frame.words,
                            quota.take(),
                        )?;
                        if owner.views().is_empty() {
                            wire::send(
                                child.control.as_fd(),
                                &Frame::new(Kind::Ack, frame.nonce, frame.index),
                                &[],
                            )?;
                            phase = Phase::Running;
                        } else {
                            owner.transfer_view(0, child.control.as_fd(), &frame.nonce)?;
                            view_transfer_gate(&platform, &stop, owner.scope())?;
                            phase = Phase::Views(0);
                        }
                    }
                    Kind::ViewSuccessor
                        if matches!(phase, Phase::Views(_)) && rights.len() == 2 =>
                    {
                        let Phase::Views(slot) = phase else {
                            unreachable!()
                        };
                        if frame.words[0] != slot as u64 || slot >= owner.views().len() {
                            return Err(failure("foreign readonly view slot"));
                        }
                        let [attached, token]: [File; 2] = rights
                            .try_into()
                            .map_err(|_| failure("invalid view successor rights"))?;
                        owner.verify_view_successor(slot, attached, token, frame.words)?;
                        let next = slot + 1;
                        if next == owner.views().len() {
                            wire::send(
                                child.control.as_fd(),
                                &Frame::new(Kind::Ack, frame.nonce, frame.index),
                                &[],
                            )?;
                            phase = Phase::Running;
                        } else {
                            owner.transfer_view(next, child.control.as_fd(), &frame.nonce)?;
                            view_transfer_gate(&platform, &stop, owner.scope())?;
                            phase = Phase::Views(next);
                        }
                    }
                    Kind::Done
                        if phase == Phase::Running
                            && rights.is_empty()
                            && frame.words[1] == 1
                            && zero(&frame.words[2..]) =>
                    {
                        code = Some(normal_status(frame.words)?);
                        life.note_done();
                        phase = Phase::Done;
                    }
                    Kind::ExecError | Kind::SetupError => {
                        return Err(io::Error::other(format!(
                            "repository helper stage {} errno {}",
                            frame.words[0], frame.words[1]
                        ))
                        .into());
                    }
                    _ => return Err(failure("unexpected helper phase or rights")),
                }
            }
        }
        let captures = driver.captures.as_ref().unwrap();
        if let Some(proof) = driver
            .life
            .as_mut()
            .unwrap()
            .proof(captures.out_eof && captures.err_eof && control_closed)?
        {
            if phase != Phase::Done {
                return Err(failure("unbound native completion"));
            }
            driver.launch.as_ref().unwrap().scope().check()?;
            if stop.load(Ordering::Acquire) {
                return Err(PhysicalRootError::Inactive);
            }
            let owner = driver.launch.take().unwrap();
            let root = owner
                .root_identity()
                .ok_or_else(|| failure("missing completed physical root"))?;
            let capture = driver.captures.take().unwrap().finish(
                code.unwrap(),
                owner.mode(),
                driver.plan.as_ref().unwrap().nonce,
                owner.covered_request_owner(),
                root,
                proof,
            );
            return owner.finish_reaped(capture);
        }
        let child = driver.child.as_ref().unwrap();
        let captures = driver.captures.as_ref().unwrap();
        let mut polls = [
            libc::pollfd {
                fd: if control_closed {
                    -1
                } else {
                    child.control.as_raw_fd()
                },
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: if captures.out_eof {
                    -1
                } else {
                    child.stdout.as_raw_fd()
                },
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: if captures.err_eof {
                    -1
                } else {
                    child.stderr.as_raw_fd()
                },
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        if unsafe { libc::poll(polls.as_mut_ptr(), polls.len() as libc::nfds_t, 10) } < 0 {
            let e = io::Error::last_os_error();
            if e.kind() != io::ErrorKind::Interrupted {
                return Err(e.into());
            }
        }
    }
}

fn view_transfer_gate(
    _platform: &ProcessPlatform,
    _stop: &AtomicBool,
    _scope: &Arc<SourceServiceScope>,
) -> Result<(), PhysicalRootError> {
    #[cfg(test)]
    if _platform.hold_after_view_transfer {
        if let Some(stage) = &_platform.observer {
            stage.store(2, Ordering::Release);
        }
        loop {
            if _stop.load(Ordering::Acquire) {
                return Err(PhysicalRootError::Inactive);
            }
            _scope.check()?;
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "repository_process_owner/tests.rs"]
mod tests;

fn normal_status(words: [u64; 8]) -> Result<i32, PhysicalRootError> {
    if words[0] > 0xff00 || words[0] & 0xff != 0 || words[1] != 1 || !zero(&words[2..]) {
        return Err(failure("repository payload did not exit normally"));
    }
    Ok((words[0] >> 8) as i32)
}

struct PlanForecast {
    bytes: usize,
}
impl PlanForecast {
    fn new() -> Self {
        Self { bytes: 58 }
    }
    fn raw(&mut self, bytes: usize) -> Result<(), PhysicalRootError> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|n| *n <= wire::MAX_PLAN)
            .ok_or(PhysicalRootError::Resource)?;
        Ok(())
    }
    fn item(&mut self, bytes: usize) -> Result<(), PhysicalRootError> {
        self.raw(bytes.checked_add(4).ok_or(PhysicalRootError::Resource)?)
    }
}
