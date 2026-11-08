//! Inactive linear rolling-mount primitive. Only trusted native setup can mint
//! a clone from a checked, actually attached root. fstat/statx do not attest that
//! an incoming descriptor is detached or establish its creation history.
//!
//! Immutable requests/records bind structural identity, never native authority.
//! There is deliberately no production constructor for quiescent completion:
//! general process reap/snapshot/record acceptance and non-execute advancement
//! still belong to the future common native progress owner.
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

const NS_GET_USERNS: libc::c_ulong = 0xb701;
const NS_GET_NSTYPE: libc::c_ulong = 0xb703;
const CLONE: u32 = 1;
const MOVE_FROM_EMPTY: u32 = 4;
const TRANSFER_MAGIC: &[u8; 8] = b"SLUG425I";

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
struct NamespaceIdentity {
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
    request: RepositoryWorkspaceRequest,
    lease: RepositoryMountLease,
}
#[derive(Debug)]
pub(super) struct MountInFlight {
    reservation: Arc<RepositoryWorkspaceReservation>,
    root: RootIdentity,
    request: RepositoryWorkspaceRequest,
    owner: File,
    namespace: NamespaceIdentity,
    incoming: RootIdentity,
    _credit: resources::ResourceCredit,
}
#[derive(Debug)]
pub(super) struct PendingMountLease {
    reservation: Arc<RepositoryWorkspaceReservation>,
    root: RootIdentity,
    request: RepositoryWorkspaceRequest,
    successor: AttachedMountClone,
}

/// Intentionally has no production constructor. The future general native
/// owner must mint it only after child/descendant reap, actual bounded snapshot
/// and exact immutable record acceptance. Test drivers use the same sequence.
#[derive(Debug)]
pub(super) struct QuiescedMountCompletion<'record> {
    record: &'record RepositoryWorkspaceRecord,
    root: RootIdentity,
}

impl RepositoryMountLease {
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
        mut self,
        root: &RepositoryPhysicalRootBuilder,
        request: RepositoryWorkspaceRequest,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<RepositoryMountAttempt, MountLeaseError> {
        self.validate_request(root, &request, scope, active)?;
        self.clone
            .credit
            .bytes
            .try_grow(
                request
                    .retained_storage_bytes()
                    .map_err(|_| PhysicalRootError::Resource)?,
            )
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
        send_incoming(
            socket,
            &root.file,
            &self.lease.clone.token,
            &self.lease.clone.owner,
            nonce,
            self.request.index(),
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
            credit,
        } = self.lease.clone;
        drop(token);
        drop(resources);
        let _ = attached;
        Ok(MountInFlight {
            owner,
            namespace,
            incoming: detached,
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
        if successor.namespace != self.namespace
            || successor.attached != self.incoming
            || !same_backing(successor.detached, self.root)
            || successor.detached.mount == successor.attached.mount
        {
            return Err(MountLeaseError::AttachedRoot);
        }
        successor
            .credit
            .bytes
            .try_grow(add(
                std::mem::size_of::<PendingMountLease>(),
                self.request
                    .retained_storage_bytes()
                    .map_err(|_| PhysicalRootError::Resource)?,
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
        check(scope, active)?;
        if request.reservation() != &root.reservation
            || successor.attached != root.identity
            || identity(&root.file)? != root.identity
            || !same_backing(successor.detached, root.identity)
        {
            return Err(MountLeaseError::Request);
        }
        if !matches!(
            request.operation(),
            RepositoryWorkspaceOperation::Execute(_)
        ) {
            return Err(MountLeaseError::UnsupportedOperation);
        }
        successor
            .credit
            .bytes
            .try_grow(add(
                std::mem::size_of::<Self>(),
                request
                    .retained_storage_bytes()
                    .map_err(|_| PhysicalRootError::Resource)?,
            )?)
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
        if completion.root != self.root
            || completion.record.request() != &self.request
            || completion.record.next().reservation() != &self.reservation
        {
            return Err(MountLeaseError::Completion);
        }
        let index = self
            .request
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

#[repr(C, align(8))]
struct TransferControl([u8; 32]);
fn send_incoming(
    socket: BorrowedFd<'_>,
    root: &File,
    token: &File,
    owner: &File,
    nonce: &[u8; 16],
    index: usize,
) -> Result<(), MountLeaseError> {
    let mut bytes = [0; 33];
    bytes[..8].copy_from_slice(TRANSFER_MAGIC);
    bytes[8] = 1;
    bytes[9..25].copy_from_slice(nonce);
    bytes[25..].copy_from_slice(
        &u64::try_from(index)
            .map_err(|_| MountLeaseError::Request)?
            .to_le_bytes(),
    );
    let mut control = TransferControl([0; 32]);
    let mut iov = libc::iovec {
        iov_base: bytes.as_mut_ptr().cast(),
        iov_len: bytes.len(),
    };
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;
    message.msg_control = control.0.as_mut_ptr().cast();
    message.msg_controllen = unsafe { libc::CMSG_SPACE(12) } as usize;
    unsafe {
        let header = libc::CMSG_FIRSTHDR(&message);
        (*header).cmsg_level = libc::SOL_SOCKET;
        (*header).cmsg_type = libc::SCM_RIGHTS;
        (*header).cmsg_len = libc::CMSG_LEN(12) as usize;
        let data = libc::CMSG_DATA(header).cast::<i32>();
        std::ptr::write_unaligned(data, root.as_raw_fd());
        std::ptr::write_unaligned(data.add(1), token.as_raw_fd());
        std::ptr::write_unaligned(data.add(2), owner.as_raw_fd());
    }
    let sent = unsafe { libc::sendmsg(socket.as_raw_fd(), &message, libc::MSG_NOSIGNAL) };
    if sent < 0 {
        return Err(io::Error::last_os_error().into());
    }
    if sent as usize != bytes.len() {
        return Err(MountLeaseError::Consumed);
    }
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
