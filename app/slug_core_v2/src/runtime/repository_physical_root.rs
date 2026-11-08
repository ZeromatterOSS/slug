//! Inactive, closed Linux physical-root ownership. Trusted native setup selects
//! this exact root and excludes every external writer before factory/seal.
//! Filesystem verification does not prove descendant death or mount provenance.
//! This owner exposes no ambient-path/proc/registry mutation or activation.
//! Trusted setup and process lifecycle stay in closed native child leaves.
use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::sync::Arc;

use slug_bzlmod_v2::RepositoryWorkspaceMode;
use slug_bzlmod_v2::RepositoryWorkspaceRealization;
use slug_bzlmod_v2::RepositoryWorkspaceReservation;
use slug_bzlmod_v2::RepositoryWorkspaceState;
use slug_workspace_v2::MaterializationPathAnchor;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationNamespace;

use super::dice::source_service::SourceServiceError;
use super::dice::source_service::SourceServiceScope;
use super::path_observation::rooted::RootedObservationError;
use super::repository_root_io::RootIo;
use super::repository_root_io::RootIoLimits;

#[path = "repository_workspace_archive.rs"]
mod archive;
#[path = "repository_mount_lease.rs"]
mod mount_lease;
#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
#[path = "repository_process_owner.rs"]
mod process_owner;
#[path = "repository_workspace_progress.rs"]
mod progress;
pub(in crate::runtime) use progress::command::RepositoryWorkspaceCommand;
pub(in crate::runtime) use progress::command::RepositoryWorkspaceRetirementObserver;
mod image;
#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
mod initialize;
mod materialization;
pub(in crate::runtime) use materialization::SealedWorkspaceMaterialization;
mod reader;
mod resources;
mod seal;
#[path = "repository_workspace_snapshot.rs"]
mod snapshot;
pub(in crate::runtime) use reader::RepositoryRootReader;
pub(in crate::runtime) use resources::*;
#[path = "repository_view_data.rs"]
mod view_data;
use view_data::CoveredWorkspaceRequest;
#[path = "repository_view_comparison.rs"]
mod view_comparison;
pub(in crate::runtime) use view_comparison::ViewComparison;
pub(in crate::runtime) use view_data::NativeReadViewData;
pub(in crate::runtime) use view_data::NativeReadViewDependencies;
pub(in crate::runtime) use view_data::NativeWorkspaceHistoryGuard;

#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
pub(in crate::runtime) async fn run_selected_process(
    decision: mount_lease::launch::SelectedExecuteDecision,
    active: &(dyn Fn() -> bool + Sync),
) -> Result<RepositoryWorkspaceCommand, PhysicalRootError> {
    match decision {
        mount_lease::launch::SelectedExecuteDecision::Replay(command) => Ok(command),
        mount_lease::launch::SelectedExecuteDecision::Launch(launch) => {
            process_owner::run(launch, initialize::platform()?, active).await
        }
    }
}

const ARCHIVE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const PROCESS_BYTES: u64 = 64 * 1024 * 1024;
// Three temporary descriptors are positively counted in the separate native
// bank. Fixed credits cover actual native buffers/control, never kernel RAM.
const IO_SCRATCH: usize = 64 * 1024
    + 8192
    + 4 * 4096
    + 4096
    + 3 * std::mem::size_of::<File>()
    + 3 * std::mem::size_of::<libc::stat>()
    + std::mem::size_of::<libc::statx>()
    + std::mem::size_of::<libc::statfs>()
    + std::mem::size_of::<libc::statvfs>()
    + 1024;

#[derive(Debug)]
pub(in crate::runtime) enum PhysicalRootError {
    Io(io::Error),
    Reservation,
    Mode,
    Backing,
    RootIdentity,
    Snapshot,
    UnsupportedOperation,
    Mutation(super::repository_io::ArchiveMaterializationError),
    Protocol(slug_bzlmod_v2::RepositoryWorkspaceError),
    Resource,
    SourceService(SourceServiceError),
    Inactive,
    Observation(RootedObservationError),
}
impl PhysicalRootError {
    pub(super) fn mount(error: mount_lease::MountLeaseError) -> Self {
        match error {
            mount_lease::MountLeaseError::Root(error) => error,
            mount_lease::MountLeaseError::Io(error) => Self::Io(error),
            _ => Self::Backing,
        }
    }
}
impl From<io::Error> for PhysicalRootError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<SourceServiceError> for PhysicalRootError {
    fn from(e: SourceServiceError) -> Self {
        Self::SourceService(e)
    }
}

fn add(a: usize, b: usize) -> Result<usize, PhysicalRootError> {
    a.checked_add(b).ok_or(PhysicalRootError::Resource)
}
fn arc_bytes<T>() -> Result<usize, PhysicalRootError> {
    let (layout, _) = std::alloc::Layout::new::<[usize; 2]>()
        .extend(std::alloc::Layout::new::<T>())
        .map_err(|_| PhysicalRootError::Resource)?;
    Ok(layout.pad_to_align().size())
}
fn check(scope: &SourceServiceScope, active: &dyn Fn() -> bool) -> Result<(), PhysicalRootError> {
    scope.check()?;
    if !active() {
        return Err(PhysicalRootError::Inactive);
    }
    Ok(())
}
fn quota(mode: RepositoryWorkspaceMode) -> (u64, u64) {
    match mode {
        RepositoryWorkspaceMode::Archive => (ARCHIVE_BYTES, 262_144),
        RepositoryWorkspaceMode::Process => (PROCESS_BYTES, 8192),
    }
}
fn io_limits(
    mode: RepositoryWorkspaceMode,
    reservation: &RepositoryWorkspaceReservation,
) -> RootIoLimits {
    let process = mode == RepositoryWorkspaceMode::Process;
    RootIoLimits {
        max_path_bytes: 4096usize.saturating_sub(
            reservation
                .display_root()
                .as_path()
                .as_os_str()
                .len()
                .saturating_add(1),
        ),
        max_name_bytes: 255,
        max_depth: if process { 64 } else { 2048 },
        max_entries: if process { 8192 } else { 131_072 },
        max_directory_name_bytes: if process {
            1024 * 1024
        } else {
            4 * 1024 * 1024
        },
        max_file_bytes: if process {
            PROCESS_BYTES
        } else {
            1024 * 1024 * 1024
        },
        max_link_bytes: 4096,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RootIdentity {
    device: u64,
    inode: u64,
    mount: u64,
}

/// Mutable native progress owner. Intentionally no Clone, raw FD or arbitrary
/// IO/mutation callback. Rolling leases/remount authority are not source data.
#[derive(Debug)]
pub(in crate::runtime) struct RepositoryPhysicalRootBuilder {
    file: File,
    reservation: Arc<RepositoryWorkspaceReservation>,
    identity: RootIdentity,
    mode: RepositoryWorkspaceMode,
    resources: Arc<RepositoryRootResources>,
    backing: Arc<RepositoryRootBacking>,
    credit: resources::ResourceCredit,
    // A bootstrap result may remain queued after the awaiting future cancels.
    // Mutable authority and its credits retire before this counted guard.
    retirement: Option<progress::command::RepositoryWorkspaceRetirementGuard>,
}

/// Sealed same-root owner. Arc cloning creates no descriptor. Field order closes
/// actual handles/drops final bodies before releasing their lifetime credits.
#[derive(Debug)]
pub(in crate::runtime) struct RepositoryPhysicalRoot {
    file: File,
    state: Arc<RepositoryWorkspaceState>,
    reservation: Arc<RepositoryWorkspaceReservation>,
    identity: RootIdentity,
    mode: RepositoryWorkspaceMode,
    anchor: MaterializationPathAnchor,
    resources: Arc<RepositoryRootResources>,
    backing: Arc<RepositoryRootBacking>,
    credit: resources::ResourceCredit,
}

impl RepositoryPhysicalRootBuilder {
    pub(in crate::runtime) fn bind_source_claim(
        &mut self,
        claim: Arc<super::repository_io::reservations::SourceRootClaim>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        check(scope, active)?;
        self.backing.bind_claim(&self.reservation, claim)
    }
    /// Native factory precondition: trusted setup has selected this full
    /// reservation's actual backing and stopped/closed all external writers.
    /// Process-mode handoff represents already admitted native progress; this
    /// API performs no Archive→Process transition and proves no launch history.
    pub fn from_quiescent_tmpfs(
        input: File,
        reservation: Arc<RepositoryWorkspaceReservation>,
        mode: RepositoryWorkspaceMode,
        resources: Arc<RepositoryRootResources>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        check(scope, active)?;
        let backing = RepositoryRootBacking::new(&resources, mode)?;
        Self::from_quiescent_tmpfs_with_backing(
            input,
            reservation,
            mode,
            resources,
            backing,
            scope,
            active,
        )
    }

    /// The trusted producer already prepaid this exact backing before creating
    /// it. Incoming file ownership is armed before any further refusal.
    fn from_quiescent_tmpfs_with_backing(
        input: File,
        reservation: Arc<RepositoryWorkspaceReservation>,
        mode: RepositoryWorkspaceMode,
        resources: Arc<RepositoryRootResources>,
        backing: Arc<RepositoryRootBacking>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        struct Incoming {
            file: File,
            backing: Arc<RepositoryRootBacking>,
        }
        let incoming = Incoming {
            file: input,
            backing,
        };
        check(scope, active)?;
        incoming.backing.matches(&resources, mode)?;
        if reservation.instance().value() == 0 || reservation.lease() == 0 {
            return Err(PhysicalRootError::Reservation);
        }
        let retained = add(
            arc_bytes::<RepositoryPhysicalRoot>()?,
            add(
                std::mem::size_of::<Self>(),
                reservation
                    .retained_storage_bytes()
                    .map_err(|_| PhysicalRootError::Resource)?,
            )?,
        )?;
        let credit = resources.reserve(retained, 1, 0)?;
        // Arm handle-before-credit destruction immediately after admission, so
        // every later refusal closes the root before returning its FD credit.
        let mut owner = Self {
            file: incoming.file,
            reservation,
            identity: RootIdentity {
                device: 0,
                inode: 0,
                mount: 0,
            },
            mode,
            resources,
            backing: incoming.backing,
            credit,
            retirement: None,
        };
        let _scratch = owner.resources.reserve(IO_SCRATCH, 0, 0)?;
        owner.identity = backing_identity(&owner.file, mode)?;
        owner.backing.bind(owner.identity, &owner.reservation)?;
        check(scope, active)?;
        Ok(owner)
    }
    pub fn seal(
        self,
        realization: &RepositoryWorkspaceRealization,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Arc<RepositoryPhysicalRoot>, PhysicalRootError> {
        seal::seal(self, realization, scope, active)
    }
}
impl RepositoryPhysicalRoot {
    pub(in crate::runtime) fn source_claim(
        &self,
    ) -> Option<Arc<super::repository_io::reservations::SourceRootClaim>> {
        self.backing.source_claim()
    }
    pub fn reservation(&self) -> &Arc<RepositoryWorkspaceReservation> {
        &self.reservation
    }
    pub fn state(&self) -> &Arc<RepositoryWorkspaceState> {
        &self.state
    }
    pub fn anchor(&self) -> &MaterializationPathAnchor {
        &self.anchor
    }
    pub fn observe(
        &self,
        demands: &[PathObservationDemand],
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<PathObservationEpoch, PhysicalRootError> {
        check(scope, active)?;
        let _credit = self.resources.reserve(IO_SCRATCH, 3, 0)?;
        self.validate_backing()?;
        let io = RootIo::new(&self.file, io_limits(self.mode, &self.reservation))?;
        let result = super::path_observation::rooted::observe_rooted(
            &io,
            &self.anchor,
            demands,
            scope,
            active,
        )
        .map_err(PhysicalRootError::Observation)?;
        self.validate_backing()?;
        check(scope, active)?;
        Ok(result)
    }
    pub fn open_member(
        self: &Arc<Self>,
        relative: &str,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<RepositoryRootReader, PhysicalRootError> {
        reader::open(self, relative, scope, active)
    }
    fn validate_backing(&self) -> Result<(), PhysicalRootError> {
        if backing_identity(&self.file, self.mode)? != self.identity {
            return Err(PhysicalRootError::RootIdentity);
        }
        Ok(())
    }
}

fn backing_identity(
    file: &File,
    mode: RepositoryWorkspaceMode,
) -> Result<RootIdentity, PhysicalRootError> {
    let fd = file.as_raw_fd();
    let stat = nix::sys::stat::fstat(file).map_err(io::Error::from)?;
    if stat.st_mode & libc::S_IFMT != libc::S_IFDIR || stat.st_ino != 1 {
        return Err(PhysicalRootError::RootIdentity);
    }
    // A bind of a subdirectory can itself have MOUNT_ROOT; inode1 is required
    // too. Unsupported statx attributes/mount identity fail closed.
    let mut extended = std::mem::MaybeUninit::<libc::statx>::zeroed();
    let rc = unsafe {
        libc::statx(
            fd,
            c"".as_ptr(),
            libc::AT_EMPTY_PATH | libc::AT_SYMLINK_NOFOLLOW,
            libc::STATX_BASIC_STATS | libc::STATX_MNT_ID,
            extended.as_mut_ptr(),
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error().into());
    }
    let extended = unsafe { extended.assume_init() };
    let root_attr = libc::STATX_ATTR_MOUNT_ROOT as u64;
    let required = libc::STATX_MNT_ID | libc::STATX_INO | libc::STATX_TYPE;
    if extended.stx_mask & required != required
        || extended.stx_mnt_id == 0
        || extended.stx_attributes_mask & root_attr == 0
        || extended.stx_attributes & root_attr == 0
        || extended.stx_ino != stat.st_ino
        || extended.stx_mode as libc::mode_t & libc::S_IFMT != libc::S_IFDIR
    {
        return Err(PhysicalRootError::RootIdentity);
    }
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    let status = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || status < 0 {
        return Err(io::Error::last_os_error().into());
    }
    if flags & libc::FD_CLOEXEC == 0
        || status & libc::O_ACCMODE != libc::O_RDONLY
        || status & libc::O_PATH != 0
    {
        return Err(PhysicalRootError::Backing);
    }
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    let mut fs = std::mem::MaybeUninit::<libc::statfs>::zeroed();
    let mut vfs = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
    if unsafe { libc::fstatfs(fd, fs.as_mut_ptr()) } != 0
        || unsafe { libc::fstatvfs(fd, vfs.as_mut_ptr()) } != 0
    {
        return Err(io::Error::last_os_error().into());
    }
    let fs = unsafe { fs.assume_init() };
    let vfs = unsafe { vfs.assume_init() };
    let (bytes, inodes) = quota(mode);
    if page != 4096
        || fs.f_type != libc::TMPFS_MAGIC
        || fs.f_bsize != 4096
        || vfs.f_frsize != 4096
        || vfs.f_bsize != 4096
        || (vfs.f_blocks as u64).checked_mul(vfs.f_frsize as u64) != Some(bytes)
        || vfs.f_files as u64 != inodes
        || vfs.f_bfree > vfs.f_blocks
        || vfs.f_ffree > vfs.f_files
    {
        return Err(PhysicalRootError::Backing);
    }
    Ok(RootIdentity {
        device: stat.st_dev as u64,
        inode: stat.st_ino as u64,
        mount: extended.stx_mnt_id,
    })
}

#[cfg(test)]
mod native_fixture;
#[cfg(test)]
mod native_tests;
#[cfg(test)]
mod tests;

#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
pub(super) fn run_process_helper() -> i32 {
    mount_lease::helper::run()
}
