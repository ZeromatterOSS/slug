//! Early, single-threaded PID 1 for the native repository runner. This entry
//! receives only an immutable sealed plan and an anonymous private channel;
//! immutable DICE observations never contain this authority.
use std::ffi::CStr;
use std::ffi::CString;
use std::os::fd::AsFd;
use std::time::Duration;
use std::time::Instant;

use wire::Frame;
use wire::Kind;
use wire::RunMode;
use wire::RunPlan;

use super::*;
use crate::runtime::repository_process_wire as wire;

#[path = "repository_process_helper/setup.rs"]
mod setup;
#[path = "repository_process_helper/supervisor.rs"]
mod supervisor;

#[derive(Debug)]
struct Failure {
    stage: u64,
    error: io::Error,
}
type Result<T> = std::result::Result<T, Failure>;
fn failure(stage: u64, error: io::Error) -> Failure {
    Failure { stage, error }
}
fn invalid(stage: u64) -> Failure {
    failure(
        stage,
        io::Error::new(
            io::ErrorKind::InvalidData,
            "repository helper refused setup",
        ),
    )
}
fn native(stage: u64, error: MountLeaseError) -> Failure {
    match error {
        MountLeaseError::Io(error) | MountLeaseError::Root(PhysicalRootError::Io(error)) => {
            failure(stage, error)
        }
        _ => invalid(stage),
    }
}
fn checked(stage: u64, value: i32) -> Result<()> {
    if value < 0 {
        Err(failure(stage, io::Error::last_os_error()))
    } else {
        Ok(())
    }
}
fn control() -> BorrowedFd<'static> {
    // run() validates that descriptor 3 is open and a seqpacket socket first.
    unsafe { BorrowedFd::borrow_raw(wire::CONTROL_FD) }
}
fn send(plan: &RunPlan, kind: Kind, words: [u64; 8], rights: &[BorrowedFd<'_>]) -> Result<()> {
    let frame = Frame {
        kind,
        nonce: plan.nonce,
        index: plan.index,
        words,
    };
    wire::send(control(), &frame, rights).map_err(|e| failure(8, e))
}
fn receive(plan: &RunPlan, kind: Kind, count: usize, deadline: Instant) -> Result<Vec<File>> {
    receive_words(plan, kind, count, [0; 8], deadline)
}
fn receive_words(
    plan: &RunPlan,
    kind: Kind,
    count: usize,
    words: [u64; 8],
    deadline: Instant,
) -> Result<Vec<File>> {
    loop {
        match wire::receive(control()) {
            Ok((frame, files)) => {
                if frame.kind != kind
                    || !frame.matches(plan)
                    || frame.words != words
                    || files.len() != count
                {
                    return Err(invalid(8));
                }
                return Ok(files);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(failure(8, io::Error::from(io::ErrorKind::TimedOut)));
                }
                let mut poll = libc::pollfd {
                    fd: wire::CONTROL_FD,
                    events: libc::POLLIN,
                    revents: 0,
                };
                checked(8, unsafe { libc::poll(&mut poll, 1, 10) })?;
            }
            Err(error) => return Err(failure(8, error)),
        }
    }
}

/// Called only by the reserved CLI dispatch before ordinary argument parsing or
/// Tokio startup. The runner owns and prepays this helper until actual death.
pub(in crate::runtime) fn run() -> i32 {
    if unsafe { libc::fcntl(wire::CONTROL_FD, libc::F_GETFD) } < 0
        || wire::check_socket(control()).is_err()
        || unsafe { libc::fcntl(wire::PLAN_FD, libc::F_GETFD) } < 0
    {
        return 125;
    }
    let input = unsafe { File::from_raw_fd(wire::PLAN_FD) };
    let plan = match RunPlan::read_sealed(&input) {
        Ok(plan) => plan,
        Err(_) => return 125,
    };
    drop(input);
    match run_checked(&plan) {
        Ok(()) => 0,
        Err(error) => {
            let mut words = [0; 8];
            words[0] = error.stage;
            words[1] = error.error.raw_os_error().unwrap_or(libc::EINVAL) as u64;
            let _ = send(&plan, Kind::SetupError, words, &[]);
            125
        }
    }
}
fn run_checked(plan: &RunPlan) -> Result<()> {
    if unsafe { libc::getpid() } != 1 {
        return Err(invalid(2));
    }
    setup::single_thread()?;
    setup::stdio()?;
    checked(2, unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) })?;
    checked(2, unsafe {
        libc::fcntl(wire::CONTROL_FD, libc::F_SETFD, libc::FD_CLOEXEC)
    })?;
    let deadline = Instant::now() + Duration::from_millis(plan.timeout_millis);
    let pidfd = unsafe { libc::syscall(libc::SYS_pidfd_open, 1, 0) };
    if pidfd < 0 {
        return Err(failure(2, io::Error::last_os_error()));
    }
    let pidfd = unsafe { File::from_raw_fd(pidfd as i32) };
    send(plan, Kind::Hello, [0; 8], &[pidfd.as_fd()])?;
    // Receipt of the sender's packet is not acceptance. No setup mutation or
    // selected-root creation occurs until the worker has retained this pidfd.
    receive(plan, Kind::HelloAck, 0, deadline)?;
    drop(pidfd);

    // The outer worker pays the complete plan/setup allowance before spawning.
    // This separate local bank bounds helper-owned intermediate objects; it is
    // never imported as the parent's source counters or shared backing guard.
    let resources = RepositoryRootResources::new(
        RepositoryRootResourceLimits::new(16 * 1024 * 1024, 256, ARCHIVE_BYTES)
            .map_err(|_| invalid(1))?,
    )
    .map_err(|_| invalid(1))?;
    let _plan = resources
        .reserve(wire::PLAN_RETAINED_BYTES, wire::MAX_MIRRORS + 8, 0)
        .map_err(|_| invalid(1))?;
    let workspace = slug_workspace_v2::NormalizedAbsolutePath::new(
        std::str::from_utf8(plan.workspace.as_bytes()).map_err(|_| invalid(1))?,
    )
    .map_err(|_| invalid(1))?;
    let scope = SourceServiceScope::for_repository_helper(
        workspace,
        deadline,
        deadline + Duration::from_secs(2),
    )
    .map_err(|_| invalid(1))?;
    setup::host_view(plan)?;
    setup::mountpoint(plan)?;
    send(plan, Kind::Ready, [0; 8], &[])?;
    let (initial, attached) = if plan.mode == RunMode::InitializeArchive {
        (Some(setup::initialize_archive(plan)?), None)
    } else {
        let mut files = receive(plan, Kind::Incoming, 3, deadline)?;
        let owner = files.pop().ok_or_else(|| invalid(4))?;
        let token = files.pop().ok_or_else(|| invalid(4))?;
        let original = files.pop().ok_or_else(|| invalid(4))?;
        let attached = attach_in_owning_namespace(
            token,
            &owner,
            Path::new(plan.display.to_str().map_err(|_| invalid(4))?),
            &resources,
            &scope,
            &|| true,
        )
        .map_err(|e| native(5, e))?;
        if !same_backing(
            identity(&original).map_err(|e| native(5, e))?,
            identity(&attached.file).map_err(|e| native(5, e))?,
        ) {
            return Err(invalid(5));
        }
        drop(original);
        drop(owner);
        (None, Some(attached))
    };
    let root = initial
        .as_ref()
        .unwrap_or_else(|| &attached.as_ref().expect("attached role").file);
    if plan.mode == RunMode::FirstExecute {
        let (before, after) = quota::shrink_attached(
            attached.as_ref().ok_or_else(|| invalid(6))?,
            Path::new(plan.display.to_str().map_err(|_| invalid(6))?),
            &resources,
            &scope,
            &|| true,
        )
        .map_err(|e| native(6, e))?;
        send(
            plan,
            Kind::Quota,
            [
                before.bytes,
                before.inodes,
                before.allocated,
                before.used_inodes,
                after.bytes,
                after.inodes,
                after.allocated,
                after.used_inodes,
            ],
            &[],
        )?;
    }
    let mode = if matches!(
        plan.mode,
        RunMode::InitializeArchive | RunMode::FinalizeArchive
    ) {
        RepositoryWorkspaceMode::Archive
    } else {
        RepositoryWorkspaceMode::Process
    };
    backing_identity(root, mode).map_err(|_| invalid(6))?;
    let successor = if plan.mode.is_finalize() {
        readonly::clone_readonly_from_attached_root(
            root,
            Path::new(plan.display.to_str().map_err(|_| invalid(7))?),
            &resources,
            &scope,
            &|| true,
        )
        .map_err(|e| native(7, e))?
        .into_clone()
    } else {
        clone_from_attached_root(
            root,
            Path::new(plan.display.to_str().map_err(|_| invalid(7))?),
            &resources,
            &scope,
            &|| true,
        )
        .map_err(|e| native(7, e))?
    };
    let a = successor.attached;
    let d = successor.detached;
    let n = successor.namespace;
    send(
        plan,
        Kind::Successor,
        [
            a.device, a.inode, a.mount, d.device, d.inode, d.mount, n.device, n.inode,
        ],
        &[
            root.as_fd(),
            successor.token.as_fd(),
            successor.owner.as_fd(),
        ],
    )?;
    transfer_views(plan, &resources, &scope, deadline)?;
    receive(plan, Kind::Ack, 0, deadline)?;
    setup::finish_mounts(plan)?;
    setup::cwd(plan, root, &resources, &scope)?;
    drop(successor);
    drop(initial);
    drop(attached);
    checked(
        9,
        unsafe { libc::syscall(libc::SYS_close_range, 4u32, u32::MAX, 0u32) } as i32,
    )?;
    setup::single_thread()?;
    setup::retire_privileges()?;
    setup::limits(plan.timeout_millis)?;
    if !plan.mode.has_payload() {
        let mut status = 0;
        let waited = unsafe { libc::waitpid(-1, &mut status, libc::__WALL | libc::WNOHANG) };
        if waited != -1 || io::Error::last_os_error().raw_os_error() != Some(libc::ECHILD) {
            return Err(invalid(12));
        }
        send(plan, Kind::Done, [0, 1, 0, 0, 0, 0, 0, 0], &[])?;
        return Ok(());
    }
    supervisor::execute(plan, deadline)
}

/// Trusted sequential handoff. Each temporary attachment is closed after its
/// successor is sent; namespace attachment itself lasts until actual helper
/// death. No payload can run until the parent validates the whole batch.
fn transfer_views(
    plan: &RunPlan,
    resources: &Arc<RepositoryRootResources>,
    scope: &Arc<SourceServiceScope>,
    deadline: Instant,
) -> Result<()> {
    for (slot, row) in plan.views.iter().enumerate() {
        let mut words = [0; 8];
        words[0] = slot as u64;
        let mut files = receive_words(plan, Kind::ViewIncoming, 1, words, deadline)?;
        let token = files.pop().ok_or_else(|| invalid(16))?;
        let incoming = identity(&token).map_err(|e| native(16, e))?;
        if incoming.device != row.device
            || incoming.inode != row.inode
            || incoming.mount != row.mount
        {
            return Err(invalid(16));
        }
        readonly::check_readonly(&token).map_err(|e| native(16, e))?;
        let _owner_credit = resources.reserve(1024, 1, 0).map_err(|_| invalid(16))?;
        let owner = current_owner(resources).map_err(|e| native(16, e))?;
        let display = Path::new(row.display.to_str().map_err(|_| invalid(16))?);
        let attached =
            attach_in_owning_namespace(token, &owner, display, resources, scope, &|| true)
                .map_err(|e| native(16, e))?;
        let flags = readonly::check_readonly(&attached.file).map_err(|e| native(16, e))?;
        let successor = readonly::clone_readonly_from_attached_root(
            &attached.file,
            display,
            resources,
            scope,
            &|| true,
        )
        .map_err(|e| native(16, e))?
        .into_clone();
        let a = successor.attached;
        let d = successor.detached;
        send(
            plan,
            Kind::ViewSuccessor,
            [
                slot as u64,
                a.device,
                a.inode,
                a.mount,
                d.device,
                d.inode,
                d.mount,
                flags,
            ],
            &[attached.file.as_fd(), successor.token.as_fd()],
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "repository_process_helper/tests.rs"]
mod tests;
