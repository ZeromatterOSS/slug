//! Inactive linear rolling-mount primitive. Only trusted native setup can mint
//! a clone from a checked, actually attached root. fstat/statx do not attest that
//! an incoming descriptor is detached or establish its creation history.
//!
//! Immutable requests/records bind structural identity, never native authority.
//! The closed native runner binds actual reap to the exact request/root before
//! snapshot/record acceptance; deterministic progress alone cannot mint it.
//! The command owns every usable token and advances trusted mutation records.
use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;
use std::os::fd::FromRawFd;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::Arc;

use slug_bzlmod_v2::RepositoryWorkspaceOperation;
use slug_bzlmod_v2::RepositoryWorkspaceRecord;
use slug_bzlmod_v2::RepositoryWorkspaceRequest;
use slug_bzlmod_v2::RepositoryWorkspaceState;

use super::*;
#[path = "repository_process_view_batch.rs"]
#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
pub(super) mod batch;

#[path = "repository_mount_domain.rs"]
pub(super) mod domain;
#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
#[path = "repository_process_helper.rs"]
pub(in crate::runtime) mod helper;
#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
#[path = "repository_process_launch.rs"]
pub(super) mod launch;
#[path = "repository_quota_transition.rs"]
pub(super) mod quota;
#[path = "repository_process_views.rs"]
pub(super) mod readonly;
#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
#[path = "repository_source_root/launch.rs"]
pub(super) mod source;

const NS_GET_USERNS: libc::c_ulong = 0xb701;
const NS_GET_NSTYPE: libc::c_ulong = 0xb703;
const CLONE: u32 = 1;
const MOVE_FROM_EMPTY: u32 = 4;

#[derive(Debug)]
pub(super) enum MountLeaseError {
    Io(io::Error),
    Root(PhysicalRootError),
    Namespace,
    AttachedRoot,
    Request,
    UnsupportedOperation,
    Consumed,
    Completion,
}
impl From<io::Error> for MountLeaseError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<PhysicalRootError> for MountLeaseError {
    fn from(error: PhysicalRootError) -> Self {
        Self::Root(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct NamespaceIdentity {
    device: u64,
    inode: u64,
}
fn namespace_identity(file: &File) -> Result<NamespaceIdentity, MountLeaseError> {
    let stat = nix::sys::stat::fstat(file).map_err(io::Error::from)?;
    if unsafe { libc::ioctl(file.as_raw_fd(), NS_GET_NSTYPE) } != libc::CLONE_NEWUSER {
        return Err(MountLeaseError::Namespace);
    }
    cloexec(file)?;
    Ok(NamespaceIdentity {
        device: stat.st_dev as u64,
        inode: stat.st_ino as u64,
    })
}
fn cloexec(file: &File) -> Result<(), MountLeaseError> {
    let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFD) };
    if flags < 0 {
        return Err(io::Error::last_os_error().into());
    }
    if flags & libc::FD_CLOEXEC == 0 {
        return Err(MountLeaseError::Namespace);
    }
    Ok(())
}
fn identity(file: &File) -> Result<RootIdentity, MountLeaseError> {
    let stat = nix::sys::stat::fstat(file).map_err(io::Error::from)?;
    let mut extended = std::mem::MaybeUninit::<libc::statx>::zeroed();
    if unsafe {
        libc::statx(
            file.as_raw_fd(),
            c"".as_ptr(),
            libc::AT_EMPTY_PATH | libc::AT_SYMLINK_NOFOLLOW,
            libc::STATX_BASIC_STATS | libc::STATX_MNT_ID,
            extended.as_mut_ptr(),
        )
    } != 0
    {
        return Err(io::Error::last_os_error().into());
    }
    let extended = unsafe { extended.assume_init() };
    let attribute = libc::STATX_ATTR_MOUNT_ROOT as u64;
    if stat.st_mode & libc::S_IFMT != libc::S_IFDIR
        || stat.st_ino != 1
        || extended.stx_mask & libc::STATX_MNT_ID == 0
        || extended.stx_mnt_id == 0
        || extended.stx_attributes_mask & attribute == 0
        || extended.stx_attributes & attribute == 0
        || extended.stx_ino != stat.st_ino
    {
        return Err(MountLeaseError::AttachedRoot);
    }
    cloexec(file)?;
    Ok(RootIdentity {
        device: stat.st_dev as u64,
        inode: stat.st_ino as u64,
        mount: extended.stx_mnt_id,
    })
}
fn same_backing(left: RootIdentity, right: RootIdentity) -> bool {
    left.device == right.device && left.inode == right.inode
}
fn path_identity(path: &Path, expected: RootIdentity) -> Result<(), MountLeaseError> {
    let path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| MountLeaseError::AttachedRoot)?;
    let mut stat = std::mem::MaybeUninit::<libc::statx>::zeroed();
    if unsafe {
        libc::statx(
            libc::AT_FDCWD,
            path.as_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
            libc::STATX_BASIC_STATS | libc::STATX_MNT_ID,
            stat.as_mut_ptr(),
        )
    } != 0
    {
        return Err(io::Error::last_os_error().into());
    }
    let stat = unsafe { stat.assume_init() };
    let attribute = libc::STATX_ATTR_MOUNT_ROOT as u64;
    if !path.as_bytes().starts_with(b"/")
        || stat.stx_mode as u32 & libc::S_IFMT != libc::S_IFDIR
        || stat.stx_mask & libc::STATX_MNT_ID == 0
        || stat.stx_mnt_id != expected.mount
        || stat.stx_ino != expected.inode
        || libc::makedev(stat.stx_dev_major, stat.stx_dev_minor) != expected.device
        || stat.stx_attributes_mask & attribute == 0
        || stat.stx_attributes & attribute == 0
    {
        return Err(MountLeaseError::AttachedRoot);
    }
    Ok(())
}
fn current_owner(resources: &Arc<RepositoryRootResources>) -> Result<File, MountLeaseError> {
    // Retained callers prepay the returned owner FD; this temporary allowance
    // additionally covers mntns, self-userns and the ioctl result before transfer.
    let _temporary = resources.reserve(1024, 3, 0)?;
    let mount = File::open("/proc/self/ns/mnt")?;
    let current = File::open("/proc/self/ns/user")?;
    let descriptor = unsafe { libc::ioctl(mount.as_raw_fd(), NS_GET_USERNS) };
    if descriptor < 0 {
        return Err(io::Error::last_os_error().into());
    }
    let owner = unsafe { File::from_raw_fd(descriptor) };
    if unsafe { libc::fcntl(owner.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error().into());
    }
    if namespace_identity(&owner)? != namespace_identity(&current)? {
        return Err(MountLeaseError::Namespace);
    }
    Ok(owner)
}

/// Opaque output of trusted attached-root setup. No Clone or raw-FD accessor.
#[derive(Debug)]
pub(super) struct AttachedMountClone {
    token: File,
    owner: File,
    attached: RootIdentity,
    detached: RootIdentity,
    namespace: NamespaceIdentity,
    resources: Arc<RepositoryRootResources>,
    backing: Option<Arc<RepositoryRootBacking>>,
    credit: resources::ResourceCredit,
}

/// Runs only in trusted setup's actual owning user/mount namespace. The path
/// check names its selected attachment in THAT namespace; it is not a Host alias
/// or a data reopen. Never call this on a detached anchor.
pub(super) fn clone_from_attached_root(
    root: &File,
    display: &Path,
    resources: &Arc<RepositoryRootResources>,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<AttachedMountClone, MountLeaseError> {
    check(scope, active)?;
    if display.as_os_str().len() > 4096 {
        return Err(MountLeaseError::AttachedRoot);
    }
    let credit = resources.reserve(std::mem::size_of::<AttachedMountClone>(), 2, 0)?;
    let _scratch = resources.reserve(IO_SCRATCH, 0, 0)?;
    let attached = identity(root)?;
    path_identity(display, attached)?;
    let owner = current_owner(resources)?;
    let namespace = namespace_identity(&owner)?;
    let descriptor = unsafe {
        libc::syscall(
            libc::SYS_open_tree,
            root.as_raw_fd(),
            c"".as_ptr(),
            CLONE | libc::O_CLOEXEC as u32 | libc::AT_EMPTY_PATH as u32,
        )
    };
    if descriptor < 0 {
        return Err(io::Error::last_os_error().into());
    }
    let token = unsafe { File::from_raw_fd(descriptor as i32) };
    let detached = identity(&token)?;
    if !same_backing(attached, detached) || attached.mount == detached.mount {
        return Err(MountLeaseError::AttachedRoot);
    }
    path_identity(display, attached)?;
    check(scope, active)?;
    Ok(AttachedMountClone {
        token,
        owner,
        attached,
        detached,
        namespace,
        resources: resources.clone(),
        backing: None,
        credit,
    })
}

/// Consumes the one incoming token. Its detached creation is a trusted setup
/// provenance precondition; kernel metadata alone does not establish that fact.
/// Returns an actually attached root; it mints no successor automatically.
#[derive(Debug)]
pub(super) struct AttachedSetupRoot {
    file: File,
    _credit: resources::ResourceCredit,
}

pub(super) fn attach_in_owning_namespace(
    incoming: File,
    owner: &File,
    display: &Path,
    resources: &Arc<RepositoryRootResources>,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<AttachedSetupRoot, MountLeaseError> {
    use std::os::unix::fs::OpenOptionsExt;
    check(scope, active)?;
    if display.as_os_str().len() > 4096 {
        return Err(MountLeaseError::AttachedRoot);
    }
    let retained = resources.reserve(std::mem::size_of::<AttachedSetupRoot>(), 1, 0)?;
    let _temporary = resources.reserve(IO_SCRATCH, 4, 0)?;
    let current = current_owner(resources)?;
    if namespace_identity(owner)? != namespace_identity(&current)? {
        return Err(MountLeaseError::Namespace);
    }
    let expected = identity(&incoming)?;
    let path = std::ffi::CString::new(display.as_os_str().as_bytes())
        .map_err(|_| MountLeaseError::AttachedRoot)?;
    if !display.is_absolute() {
        return Err(MountLeaseError::AttachedRoot);
    }
    if unsafe {
        libc::syscall(
            libc::SYS_move_mount,
            incoming.as_raw_fd(),
            c"".as_ptr(),
            libc::AT_FDCWD,
            path.as_ptr(),
            MOVE_FROM_EMPTY,
        )
    } != 0
    {
        return Err(io::Error::last_os_error().into());
    }
    let root = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(display)?;
    if identity(&root)? != expected {
        return Err(MountLeaseError::AttachedRoot);
    }
    path_identity(display, expected)?;
    check(scope, active)?;
    // Close consumed token before returning the checked attached descriptor.
    drop(incoming);
    Ok(AttachedSetupRoot {
        file: root,
        _credit: retained,
    })
}

/// Never-used archive setup authority. No execution index or fabricated
/// process record exists until the whole command mints first-execute admission.
#[derive(Debug)]
pub(super) struct InitialRepositoryMount {
    clone: AttachedMountClone,
    _control: resources::ResourceCredit,
    retirement: Option<super::progress::command::RepositoryWorkspaceRetirementGuard>,
}
impl InitialRepositoryMount {
    pub(super) fn bind(
        root: &RepositoryPhysicalRootBuilder,
        clone: AttachedMountClone,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, MountLeaseError> {
        let control = root.resources.reserve(
            std::mem::size_of::<Self>() - std::mem::size_of::<AttachedMountClone>(),
            0,
            0,
        )?;
        let initial = Self {
            clone,
            _control: control,
            retirement: None,
        };
        initial.check_binding(root, scope, active)?;
        Ok(initial)
    }
    pub(super) fn with_retirement_guard(
        mut self,
        guard: super::progress::command::RepositoryWorkspaceRetirementGuard,
    ) -> Self {
        self.retirement = Some(guard);
        self
    }
    pub(super) fn into_clone_for_finalize(
        self,
        progress: &super::progress::RepositoryWorkspaceProgress,
        domain: &Arc<domain::NativeMountDomain>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<AttachedMountClone, MountLeaseError> {
        let root = progress.builder();
        self.check_binding(root, scope, active)?;
        domain.check_root(&root.reservation, &root.resources, scope, active)?;
        domain.check_clone(&self.clone, scope, active)?;
        if progress.transcript().reservation() != &root.reservation
            || progress.transcript().current().mode() != RepositoryWorkspaceMode::Archive
        {
            return Err(MountLeaseError::Request);
        }
        Ok(self.clone)
    }
    pub(super) fn check_binding(
        &self,
        root: &RepositoryPhysicalRootBuilder,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), MountLeaseError> {
        check(scope, active)?;
        if root.mode != RepositoryWorkspaceMode::Archive
            || self.clone.attached != root.identity
            || identity(&root.file)? != root.identity
            || !same_backing(self.clone.detached, root.identity)
            || !Arc::ptr_eq(&self.clone.resources, &root.resources)
            || !self
                .clone
                .backing
                .as_ref()
                .is_some_and(|backing| Arc::ptr_eq(backing, &root.backing))
        {
            return Err(MountLeaseError::Request);
        }
        root.backing
            .matches(&root.resources, RepositoryWorkspaceMode::Archive)?;
        if backing_identity(&root.file, RepositoryWorkspaceMode::Archive)? != root.identity {
            return Err(MountLeaseError::AttachedRoot);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(super) struct RepositoryMountLease {
    reservation: Arc<RepositoryWorkspaceReservation>,
    root: RootIdentity,
    index: usize,
    predecessor: Arc<RepositoryWorkspaceState>,
    clone: AttachedMountClone,
}
#[derive(Debug)]
pub(super) struct RepositoryMountAttempt {
    request: Arc<CoveredWorkspaceRequest>,
    lease: RepositoryMountLease,
}
#[derive(Debug)]
pub(super) struct MountInFlight {
    reservation: Arc<RepositoryWorkspaceReservation>,
    root: RootIdentity,
    request: Arc<CoveredWorkspaceRequest>,
    owner: File,
    namespace: NamespaceIdentity,
    incoming: RootIdentity,
    backing: Arc<RepositoryRootBacking>,
    _credit: resources::ResourceCredit,
}
#[derive(Debug)]
pub(super) struct PendingMountLease {
    reservation: Arc<RepositoryWorkspaceReservation>,
    root: RootIdentity,
    request: Arc<CoveredWorkspaceRequest>,
    successor: AttachedMountClone,
}

/// Constructed only from a closed request/root-bound reap proof and actual
/// native progress after bounded snapshot/immutable record acceptance. Fixed
/// primitive test drivers exercise the same sequence with their actual reaper.
#[derive(Debug)]
pub(super) struct QuiescedMountCompletion<'record> {
    record: &'record RepositoryWorkspaceRecord,
    root: RootIdentity,
}

impl<'a> QuiescedMountCompletion<'a> {
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    pub(super) fn from_reaped_record(
        progress: &'a super::progress::RepositoryWorkspaceProgress,
        proof: &super::process_owner::ReapedBinding,
    ) -> Result<Self, PhysicalRootError> {
        let record = progress
            .transcript()
            .records()
            .last()
            .ok_or(PhysicalRootError::Snapshot)?;
        let root = progress.builder().identity;
        proof.verify(record.request(), root)?;
        Ok(Self { record, root })
    }
}
impl RepositoryMountLease {
    pub(super) fn into_clone_for_finalize(
        self,
        progress: &super::progress::RepositoryWorkspaceProgress,
        domain: &Arc<domain::NativeMountDomain>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<AttachedMountClone, MountLeaseError> {
        let root = progress.builder();
        domain.check_root(&root.reservation, &root.resources, scope, active)?;
        domain.check_clone(&self.clone, scope, active)?;
        if self.reservation != root.reservation
            || progress.transcript().reservation() != &self.reservation
            || self.index != progress.transcript().records().len()
            || &self.predecessor != progress.transcript().current()
            || progress.transcript().current().mode() != RepositoryWorkspaceMode::Process
            || root.identity != self.root
            || identity(&root.file)? != self.root
            || !same_backing(self.clone.detached, self.root)
            || !self
                .clone
                .backing
                .as_ref()
                .is_some_and(|backing| Arc::ptr_eq(backing, &root.backing))
            || backing_identity(&root.file, RepositoryWorkspaceMode::Process)? != root.identity
        {
            return Err(MountLeaseError::Request);
        }
        Ok(self.clone)
    }
    /// Native deterministic mutation has already captured the actual root and
    /// accepted precisely this next record. No child or usable alias is live.
    pub(super) fn advance_trusted(
        mut self,
        progress: &super::progress::RepositoryWorkspaceProgress,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, MountLeaseError> {
        check(scope, active)?;
        let root = progress.builder();
        let transcript = progress.transcript();
        let record = transcript
            .records()
            .last()
            .ok_or(MountLeaseError::Completion)?;
        if record.request().reservation() != &self.reservation
            || record.request().index() != self.index
            || record.request().predecessor() != &self.predecessor
            || matches!(
                record.request().operation(),
                RepositoryWorkspaceOperation::Execute(_)
            )
            || root.identity != self.root
            || identity(&root.file)? != self.root
            || transcript.records().len()
                != self
                    .index
                    .checked_add(1)
                    .ok_or(MountLeaseError::Completion)?
            || !self
                .clone
                .backing
                .as_ref()
                .is_some_and(|backing| Arc::ptr_eq(backing, &root.backing))
            || backing_identity(&root.file, RepositoryWorkspaceMode::Process)? != root.identity
        {
            return Err(MountLeaseError::Completion);
        }
        self.clone
            .credit
            .bytes
            .try_grow(
                record
                    .next()
                    .retained_storage_bytes()
                    .map_err(|_| PhysicalRootError::Resource)?,
            )
            .map_err(|_| PhysicalRootError::Resource)?;
        self.index = self
            .index
            .checked_add(1)
            .ok_or(MountLeaseError::Completion)?;
        self.predecessor = record.next().clone();
        Ok(self)
    }
    fn validate_request(
        &self,
        root: &RepositoryPhysicalRootBuilder,
        request: &RepositoryWorkspaceRequest,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), MountLeaseError> {
        check(scope, active)?;
        if request.reservation() != &self.reservation
            || request.reservation() != &root.reservation
            || request.index() != self.index
            || request.predecessor() != &self.predecessor
            || root.identity != self.root
            || identity(&root.file)? != self.root
            || !self
                .clone
                .backing
                .as_ref()
                .is_some_and(|backing| Arc::ptr_eq(backing, &root.backing))
        {
            return Err(MountLeaseError::Request);
        }
        if !matches!(
            request.operation(),
            RepositoryWorkspaceOperation::Execute(_)
        ) {
            return Err(MountLeaseError::UnsupportedOperation);
        }
        Ok(())
    }
    pub(super) fn begin(
        self,
        root: &RepositoryPhysicalRootBuilder,
        request: RepositoryWorkspaceRequest,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<RepositoryMountAttempt, MountLeaseError> {
        let request = CoveredWorkspaceRequest::empty(request, &root.resources)?;
        self.begin_covered(root, request, scope, active)
    }
    pub(super) fn begin_covered(
        mut self,
        root: &RepositoryPhysicalRootBuilder,
        request: Arc<CoveredWorkspaceRequest>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<RepositoryMountAttempt, MountLeaseError> {
        self.validate_request(root, request.request(), scope, active)?;
        self.clone
            .credit
            .bytes
            .try_grow(request.owned_bytes())
            .map_err(|_| PhysicalRootError::Resource)?;
        Ok(RepositoryMountAttempt {
            lease: self,
            request,
        })
    }
}
impl RepositoryMountAttempt {
    /// Only fixed native launch setup may borrow this descriptor for bwrap's
    /// --userns argument. A source/root reader has no route to this type.
    pub(super) fn owning_user_namespace(&self) -> BorrowedFd<'_> {
        use std::os::fd::AsFd;
        self.lease.clone.owner.as_fd()
    }
    /// Test/native transport consumes the token on success OR failure. No
    /// callback/reader is retained. The retained owner keeps exact setup context.
    fn transfer(
        self,
        root: &RepositoryPhysicalRootBuilder,
        socket: BorrowedFd<'_>,
        nonce: &[u8; 16],
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<MountInFlight, MountLeaseError> {
        check(scope, active)?;
        if root.identity != self.lease.root || root.reservation != self.lease.reservation {
            return Err(MountLeaseError::Request);
        }
        let backing = self
            .lease
            .clone
            .backing
            .as_ref()
            .ok_or(MountLeaseError::Request)?
            .clone();
        send_incoming(
            socket,
            &root.file,
            &self.lease.clone.token,
            &self.lease.clone.owner,
            nonce,
            self.request.request().index(),
        )?;
        check(scope, active)?;
        // Keep the consumed token closed, while retaining its prepaid owner and
        // metadata credits until the successor replaces the old attempt.
        let AttachedMountClone {
            token,
            owner,
            attached,
            detached,
            namespace,
            resources,
            backing: _prior_backing,
            credit,
        } = self.lease.clone;
        drop(token);
        drop(resources);
        let _ = attached;
        Ok(MountInFlight {
            owner,
            namespace,
            incoming: detached,
            backing,
            _credit: credit,
            reservation: self.lease.reservation,
            root: self.lease.root,
            request: self.request,
        })
    }
}
impl MountInFlight {
    fn stage(
        self,
        mut successor: AttachedMountClone,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<PendingMountLease, MountLeaseError> {
        check(scope, active)?;
        if !self.backing.belongs_to(&successor.resources)
            || successor.namespace != self.namespace
            || successor.attached != self.incoming
            || !same_backing(successor.detached, self.root)
            || successor.detached.mount == successor.attached.mount
        {
            return Err(MountLeaseError::AttachedRoot);
        }
        if successor
            .backing
            .as_ref()
            .is_some_and(|backing| !Arc::ptr_eq(backing, &self.backing))
        {
            return Err(MountLeaseError::AttachedRoot);
        }
        successor.backing = Some(self.backing.clone());
        successor
            .credit
            .bytes
            .try_grow(add(
                std::mem::size_of::<PendingMountLease>(),
                self.request.owned_bytes(),
            )?)
            .map_err(|_| PhysicalRootError::Resource)?;
        Ok(PendingMountLease {
            successor,
            reservation: self.reservation,
            root: self.root,
            request: self.request,
        })
    }
}
impl PendingMountLease {
    // Initial trusted setup also stages its pre-payload successor; it does not
    // grant a usable lease until the first real completion is accepted.
    fn bootstrap(
        root: &RepositoryPhysicalRootBuilder,
        request: RepositoryWorkspaceRequest,
        mut successor: AttachedMountClone,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, MountLeaseError> {
        let request = CoveredWorkspaceRequest::empty(request, &root.resources)?;
        check(scope, active)?;
        if !Arc::ptr_eq(&successor.resources, &root.resources)
            || request.request().reservation() != &root.reservation
            || successor.attached != root.identity
            || identity(&root.file)? != root.identity
            || !same_backing(successor.detached, root.identity)
        {
            return Err(MountLeaseError::Request);
        }
        if !matches!(
            request.request().operation(),
            RepositoryWorkspaceOperation::Execute(_)
        ) {
            return Err(MountLeaseError::UnsupportedOperation);
        }
        if successor
            .backing
            .as_ref()
            .is_some_and(|backing| !Arc::ptr_eq(backing, &root.backing))
        {
            return Err(MountLeaseError::Request);
        }
        successor.backing = Some(root.backing.clone());
        successor
            .credit
            .bytes
            .try_grow(add(std::mem::size_of::<Self>(), request.owned_bytes())?)
            .map_err(|_| PhysicalRootError::Resource)?;
        Ok(Self {
            reservation: root.reservation.clone(),
            root: root.identity,
            request,
            successor,
        })
    }
    pub(super) fn accept(
        mut self,
        completion: QuiescedMountCompletion<'_>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<RepositoryMountLease, MountLeaseError> {
        check(scope, active)?;
        if self.successor.backing.is_none()
            || completion.root != self.root
            || completion.record.request() != self.request.request()
            || completion.record.next().reservation() != &self.reservation
        {
            return Err(MountLeaseError::Completion);
        }
        let index = self
            .request
            .request()
            .index()
            .checked_add(1)
            .ok_or(MountLeaseError::Completion)?;
        self.successor
            .credit
            .bytes
            .try_grow(
                completion
                    .record
                    .next()
                    .retained_storage_bytes()
                    .map_err(|_| PhysicalRootError::Resource)?,
            )
            .map_err(|_| PhysicalRootError::Resource)?;
        let clone = self.successor;
        Ok(RepositoryMountLease {
            clone,
            reservation: self.reservation,
            root: self.root,
            index,
            predecessor: completion.record.next().clone(),
        })
    }
}

fn send_incoming(
    socket: BorrowedFd<'_>,
    root: &File,
    token: &File,
    owner: &File,
    nonce: &[u8; 16],
    index: usize,
) -> Result<(), MountLeaseError> {
    use std::os::fd::AsFd;

    use crate::runtime::repository_process_wire::Frame;
    use crate::runtime::repository_process_wire::Kind;
    let frame = Frame::new(
        Kind::Incoming,
        *nonce,
        u64::try_from(index).map_err(|_| MountLeaseError::Request)?,
    );
    crate::runtime::repository_process_wire::send(
        socket,
        &frame,
        &[root.as_fd(), token.as_fd(), owner.as_fd()],
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "repository_mount_lease/native_fixture.rs"]
mod native_fixture;
#[cfg(test)]
#[path = "repository_mount_lease/native_tests.rs"]
mod native_tests;
#[cfg(test)]
#[path = "repository_mount_lease/tests.rs"]
mod tests;
