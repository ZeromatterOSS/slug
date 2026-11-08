//! Linear process-only read views. These owners are separate from data roots:
//! a data reader can never obtain a namespace FD or an attachment operation.
use std::os::fd::AsFd;

use super::domain::NativeMountDomain;
use super::*;

const READONLY: u64 = 1;
const NOSUID: u64 = 2;
const NODEV: u64 = 4;

/// A trusted attached-root clone whose per-mount flags have been made readonly.
/// No metadata test proves detached provenance; the closed setup creates it.
#[derive(Debug)]
pub(in crate::runtime::repository_physical_root) struct ReadOnlyMountClone {
    pub(super) clone: AttachedMountClone,
}
impl ReadOnlyMountClone {
    pub(super) fn from_received_clone(clone: AttachedMountClone) -> Result<Self, MountLeaseError> {
        let owner = Self { clone };
        if identity(&owner.clone.token)? != owner.clone.detached {
            return Err(MountLeaseError::AttachedRoot);
        }
        check_readonly(&owner.clone.token)?;
        Ok(owner)
    }
    /// Only trusted helper/glue may strip the marker for bounded transport.
    pub(super) fn into_clone(self) -> AttachedMountClone {
        self.clone
    }
}

/// Checks the selected mount, including an open_tree O_PATH mount descriptor.
/// fstatvfs performs no path lookup or descriptor duplication.
pub(super) fn check_readonly(file: &File) -> Result<u64, MountLeaseError> {
    let flags = mount_flags(file)?;
    validate_flags(flags)?;
    Ok(flags)
}
fn mount_flags(file: &File) -> Result<u64, MountLeaseError> {
    let mut value = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
    if unsafe { libc::fstatvfs(file.as_raw_fd(), value.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(unsafe { value.assume_init() }.f_flag as u64)
}

fn validate_flags(flags: u64) -> Result<(), MountLeaseError> {
    let required = libc::ST_RDONLY as u64 | libc::ST_NOSUID as u64 | libc::ST_NODEV as u64;
    if flags & required != required || flags & libc::ST_NOEXEC as u64 != 0 {
        return Err(MountLeaseError::AttachedRoot);
    }
    Ok(())
}

/// Called only in the actual owning namespace, while the root is attached.
/// Attribute changes affect the new clone, never the mutable source mount or
/// the shared tmpfs superblock. Executable repository tools remain executable.
pub(super) fn clone_readonly_from_attached_root(
    root: &File,
    display: &Path,
    resources: &Arc<RepositoryRootResources>,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<ReadOnlyMountClone, MountLeaseError> {
    check(scope, active)?;
    let before = mount_flags(root)?;
    let clone = clone_from_attached_root(root, display, resources, scope, active)?;
    #[repr(C)]
    struct Attr {
        set: u64,
        clear: u64,
        propagation: u64,
        userns: u64,
    }
    let attr = Attr {
        set: READONLY | NOSUID | NODEV,
        clear: 0,
        propagation: 0,
        userns: 0,
    };
    if unsafe {
        libc::syscall(
            libc::SYS_mount_setattr,
            clone.token.as_raw_fd(),
            c"".as_ptr(),
            libc::AT_EMPTY_PATH | libc::AT_RECURSIVE,
            &attr,
            std::mem::size_of::<Attr>(),
        )
    } != 0
    {
        return Err(io::Error::last_os_error().into());
    }
    path_identity(display, clone.attached)?;
    if mount_flags(root)? != before {
        return Err(MountLeaseError::AttachedRoot);
    }
    check(scope, active)?;
    ReadOnlyMountClone::from_received_clone(clone)
}

/// A closed full-seal receipt binds the token only after the actual finalizing
/// helper has reaped. The native command/glue is the sole caller of bind.
#[derive(Clone, Debug)]
enum ViewDataRoot {
    Workspace(Arc<RepositoryPhysicalRoot>),
    Source(Arc<super::super::source::RepositorySourceRoot>),
}
impl ViewDataRoot {
    fn resources(&self) -> &Arc<RepositoryRootResources> {
        match self {
            Self::Workspace(r) => &r.resources,
            Self::Source(r) => &r.resources,
        }
    }
    fn identity(&self) -> RootIdentity {
        match self {
            Self::Workspace(r) => r.identity,
            Self::Source(r) => r.identity,
        }
    }
    fn backing(&self) -> &Arc<RepositoryRootBacking> {
        match self {
            Self::Workspace(r) => &r.backing,
            Self::Source(r) => &r.backing,
        }
    }
    fn mode(&self) -> RepositoryWorkspaceMode {
        match self {
            Self::Workspace(r) => r.mode,
            Self::Source(_) => RepositoryWorkspaceMode::Archive,
        }
    }
    fn frozen_quota(&self) -> Option<FrozenArchiveQuota> {
        match self {
            Self::Workspace(root) => root.frozen_archive_quota,
            Self::Source(_) => None,
        }
    }
    fn native_quota(&self) -> (u64, u64) {
        self.frozen_quota()
            .map_or_else(|| quota(self.mode()), |class| class.quota())
    }
    fn display(&self) -> &slug_workspace_v2::NormalizedAbsolutePath {
        match self {
            Self::Workspace(r) => r.reservation.display_root(),
            Self::Source(r) => r.reservation().display_root(),
        }
    }
    fn instance(&self) -> slug_workspace_v2::PathObservationInstanceId {
        match self {
            Self::Workspace(r) => r.reservation.instance(),
            Self::Source(r) => r.reservation().instance(),
        }
    }
    fn check_domain(
        &self,
        domain: &NativeMountDomain,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), MountLeaseError> {
        match self {
            Self::Workspace(r) => {
                r.check_origin(scope, active)?;
                domain.check_root(&r.reservation, &r.resources, scope, active)
            }
            Self::Source(r) => {
                r.check_origin(scope, active)?;
                domain.check_source(r.reservation(), &r.resources, scope, active)
            }
        }
    }
    fn validate_backing(&self) -> Result<(), PhysicalRootError> {
        match self {
            Self::Workspace(r) => r.validate_backing(),
            Self::Source(r) => r.validate_backing(),
        }
    }
}
#[derive(Debug)]
pub(in crate::runtime::repository_physical_root) struct FinalizedRepositoryView {
    mount: ReadOnlyMountClone,
    root: ViewDataRoot,
    domain: Arc<NativeMountDomain>,
    _credit: resources::ResourceCredit,
    // LAST: standalone/queued view custody can outlive its returned command.
    retirement: Option<super::super::progress::command::RepositoryWorkspaceRetirementGuard>,
}
impl FinalizedRepositoryView {
    pub(in crate::runtime::repository_physical_root) fn bind_source(
        sealed: &super::super::source::SealedSourceSnapshot,
        mount: ReadOnlyMountClone,
        domain: Arc<NativeMountDomain>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, MountLeaseError> {
        let credit = sealed
            .root()
            .resources
            .reserve(view_control_bytes(), 0, 0)?;
        let owner = Self {
            mount,
            root: ViewDataRoot::Source(sealed.root().clone()),
            domain,
            _credit: credit,
            retirement: None,
        };
        owner.validate(scope, active)?;
        Ok(owner)
    }
    pub(in crate::runtime::repository_physical_root) fn bind(
        sealed: &SealedWorkspaceMaterialization,
        mount: ReadOnlyMountClone,
        domain: Arc<NativeMountDomain>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, MountLeaseError> {
        let credit = sealed
            .root()
            .resources
            .reserve(view_control_bytes(), 0, 0)?;
        let owner = Self {
            mount,
            root: ViewDataRoot::Workspace(sealed.root().clone()),
            domain,
            _credit: credit,
            retirement: None,
        };
        let slug_bzlmod_v2::RepositoryMaterializationKind::GeneralWorkspace(realization) =
            &sealed.request().kind
        else {
            return Err(MountLeaseError::Request);
        };
        if realization.reservation() != sealed.root().reservation()
            || realization.final_state() != sealed.root().state()
        {
            return Err(MountLeaseError::Request);
        }
        owner.validate(scope, active)?;
        Ok(owner)
    }
    pub(in crate::runtime::repository_physical_root) fn with_retirement_observer(
        mut self,
        observer: super::super::progress::command::RepositoryWorkspaceRetirementObserver,
    ) -> Self {
        assert!(
            self.retirement.is_none(),
            "finalized view retirement already bound"
        );
        self.retirement = Some(observer.worker_guard());
        self
    }
    fn validate(
        &self,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), MountLeaseError> {
        self.root.check_domain(&self.domain, scope, active)?;
        self.domain.check_clone(&self.mount.clone, scope, active)?;
        if !same_backing(self.mount.clone.attached, self.root.identity())
            || !same_backing(self.mount.clone.detached, self.root.identity())
            || self.mount.clone.attached.mount == self.mount.clone.detached.mount
            || !self
                .mount
                .clone
                .backing
                .as_ref()
                .is_some_and(|backing| Arc::ptr_eq(backing, self.root.backing()))
            || identity(&self.mount.clone.token)? != self.mount.clone.detached
        {
            return Err(MountLeaseError::AttachedRoot);
        }
        self.root.validate_backing()?;
        super::quota::facts_for_quota(&self.mount.clone.token, self.root.native_quota())?;
        self.root.backing().matches_data(
            self.root.resources(),
            self.root.mode(),
            self.root.frozen_quota(),
        )?;
        check_readonly(&self.mount.clone.token)?;
        Ok(())
    }
    pub(in crate::runtime::repository_physical_root) fn display(
        &self,
    ) -> &slug_workspace_v2::NormalizedAbsolutePath {
        self.root.display()
    }
    pub(in crate::runtime::repository_physical_root) fn instance(
        &self,
    ) -> slug_workspace_v2::PathObservationInstanceId {
        self.root.instance()
    }
    pub(in crate::runtime::repository_physical_root) fn root_identity(&self) -> RootIdentity {
        self.root.identity()
    }
    pub(in crate::runtime::repository_physical_root) fn data_root(
        &self,
    ) -> Option<&Arc<RepositoryPhysicalRoot>> {
        match &self.root {
            ViewDataRoot::Workspace(root) => Some(root),
            _ => None,
        }
    }
    pub(in crate::runtime::repository_physical_root) fn source_data_root(
        &self,
    ) -> Option<&Arc<super::super::source::RepositorySourceRoot>> {
        match &self.root {
            ViewDataRoot::Source(root) => Some(root),
            _ => None,
        }
    }
    /// The incoming token's mount ID, rather than the original data FD's ID.
    pub(in crate::runtime::repository_physical_root) fn attachment_identity(&self) -> RootIdentity {
        self.mount.clone.detached
    }
    pub(in crate::runtime::repository_physical_root) fn domain(&self) -> &Arc<NativeMountDomain> {
        &self.domain
    }
    pub(in crate::runtime::repository_physical_root) fn begin(
        self,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<ReadOnlyViewAttempt, MountLeaseError> {
        self.validate(scope, active)?;
        Ok(ReadOnlyViewAttempt {
            view: Some(self),
            poisoned: false,
        })
    }
}

#[derive(Debug)]
pub(in crate::runtime::repository_physical_root) struct ReadOnlyViewAttempt {
    view: Option<FinalizedRepositoryView>,
    poisoned: bool,
}
#[derive(Debug)]
pub(super) struct ViewUseBinding {
    request: Arc<CoveredWorkspaceRequest>,
    root: RootIdentity,
    nonce: [u8; 16],
    _credit: resources::ResourceCredit,
}
impl ViewUseBinding {
    pub(super) fn new(
        request: Arc<CoveredWorkspaceRequest>,
        root: RootIdentity,
        nonce: [u8; 16],
        resources: &Arc<RepositoryRootResources>,
    ) -> Result<Arc<Self>, MountLeaseError> {
        let credit = resources.reserve(arc_bytes::<Self>()?, 0, 0)?;
        Ok(Arc::new(Self {
            request,
            root,
            nonce,
            _credit: credit,
        }))
    }
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    pub(super) fn check_capture(
        &self,
        capture: &super::super::process_owner::ReapedCapture,
    ) -> Result<(), MountLeaseError> {
        capture
            .binding()
            .verify_attempt(self.request.request(), self.root, &self.nonce)?;
        Ok(())
    }
}
#[derive(Debug)]
pub(in crate::runtime::repository_physical_root) struct ReadOnlyViewInFlight {
    binding: Option<Arc<ViewUseBinding>>,
    view: Option<FinalizedRepositoryView>,
    slot: usize,
    poisoned: bool,
}
#[derive(Debug)]
pub(in crate::runtime::repository_physical_root) struct PendingReadOnlyView {
    binding: Arc<ViewUseBinding>,
    view: Option<FinalizedRepositoryView>,
    poisoned: bool,
}
fn view_control_bytes() -> usize {
    std::mem::size_of::<FinalizedRepositoryView>()
        .max(std::mem::size_of::<ReadOnlyViewAttempt>())
        .max(std::mem::size_of::<ReadOnlyViewInFlight>())
        .max(std::mem::size_of::<PendingReadOnlyView>())
        + 64
}
impl ReadOnlyViewAttempt {
    pub(in crate::runtime::repository_physical_root) fn attachment_identity(&self) -> RootIdentity {
        self.view
            .as_ref()
            .expect("untransferred view")
            .attachment_identity()
    }
    pub(in crate::runtime::repository_physical_root) fn display(
        &self,
    ) -> &slug_workspace_v2::NormalizedAbsolutePath {
        self.view.as_ref().expect("untransferred view").display()
    }
    pub(in crate::runtime::repository_physical_root) fn instance(
        &self,
    ) -> slug_workspace_v2::PathObservationInstanceId {
        self.view.as_ref().expect("untransferred view").instance()
    }
    pub(in crate::runtime::repository_physical_root) fn domain(&self) -> &Arc<NativeMountDomain> {
        &self.view.as_ref().expect("untransferred view").domain
    }
    /// The worker keeps this phase armed through cleanup on every error. It
    /// cannot retry after transfer starts; no source backing retires on Err.
    pub(in crate::runtime::repository_physical_root) fn transfer(
        &mut self,
        socket: BorrowedFd<'_>,
        binding: Arc<ViewUseBinding>,
        slot: usize,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<ReadOnlyViewInFlight, MountLeaseError> {
        use crate::runtime::repository_process_wire::Frame;
        use crate::runtime::repository_process_wire::Kind;
        use crate::runtime::repository_process_wire::{self as wire};
        if self.poisoned {
            return Err(MountLeaseError::Consumed);
        }
        self.poisoned = true;
        let view = self.view.as_ref().ok_or(MountLeaseError::Consumed)?;
        view.validate(scope, active)?;
        view.domain.check_root(
            binding.request.request().reservation(),
            view.root.resources(),
            scope,
            active,
        )?;
        let mut frame = Frame::new(
            Kind::ViewIncoming,
            binding.nonce,
            u64::try_from(binding.request.request().index())
                .map_err(|_| MountLeaseError::Request)?,
        );
        frame.words[0] = u64::try_from(slot).map_err(|_| MountLeaseError::Request)?;
        wire::send(socket, &frame, &[view.mount.clone.token.as_fd()])?;
        check(scope, active)?;
        // No fallible step follows this move. On all earlier errors self still
        // owns data/backing and the consumed authority until the worker reaps.
        Ok(ReadOnlyViewInFlight {
            binding: Some(binding),
            view: self.view.take(),
            slot,
            poisoned: false,
        })
    }
}
impl ReadOnlyViewInFlight {
    /// Received rights have the driver's transient credit. Keep the original
    /// phase owner through every refusal; only a fully checked successor moves.
    pub(in crate::runtime::repository_physical_root) fn stage_successor(
        &mut self,
        attached: File,
        token: File,
        words: [u64; 8],
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<PendingReadOnlyView, MountLeaseError> {
        if self.poisoned {
            return Err(MountLeaseError::Consumed);
        }
        self.poisoned = true;
        let view = self.view.as_ref().ok_or(MountLeaseError::Consumed)?;
        view.validate(scope, active)?;
        let attached_id = identity(&attached)?;
        let detached = identity(&token)?;
        let flags = check_readonly(&token)?;
        check_readonly(&attached)?;
        if words
            != [
                self.slot as u64,
                attached_id.device,
                attached_id.inode,
                attached_id.mount,
                detached.device,
                detached.inode,
                detached.mount,
                flags,
            ]
            || attached_id != view.mount.clone.detached
            || !same_backing(detached, view.root.identity())
            || detached.mount == attached_id.mount
        {
            return Err(MountLeaseError::AttachedRoot);
        }
        let resources = view.root.resources();
        let credit = resources.reserve(view_control_bytes(), 0, 0)?;
        let clone_credit = resources.reserve(std::mem::size_of::<AttachedMountClone>(), 2, 0)?;
        let owner = view.domain.duplicate_namespace()?;
        let successor = AttachedMountClone {
            token,
            owner,
            attached: attached_id,
            detached,
            namespace: view.domain.namespace_identity(),
            resources: resources.clone(),
            backing: Some(view.root.backing().clone()),
            credit: clone_credit,
        };
        let mut next = FinalizedRepositoryView {
            mount: ReadOnlyMountClone::from_received_clone(successor)?,
            root: view.root.clone(),
            domain: view.domain.clone(),
            _credit: credit,
            // The original armed phase retains custody through every fallible
            // successor check. Move its guard only after validation succeeds.
            retirement: None,
        };
        next.validate(scope, active)?;
        next.retirement = self
            .view
            .as_mut()
            .expect("armed source view")
            .retirement
            .take();
        let binding = self.binding.take().expect("unconsumed view use");
        drop(self.view.take());
        Ok(PendingReadOnlyView {
            binding,
            view: Some(next),
            poisoned: false,
        })
    }
}
impl PendingReadOnlyView {
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    fn check_capture(
        &self,
        capture: &super::super::process_owner::ReapedCapture,
    ) -> Result<(), MountLeaseError> {
        self.binding.check_capture(capture)
    }
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    pub(super) fn check_completion(
        &self,
        capture: &super::super::process_owner::ReapedCapture,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), MountLeaseError> {
        if self.poisoned {
            return Err(MountLeaseError::Consumed);
        }
        let view = self.view.as_ref().ok_or(MountLeaseError::Consumed)?;
        view.domain.check_root(
            self.binding.request.request().reservation(),
            view.root.resources(),
            scope,
            active,
        )?;
        self.check_capture(capture)?;
        view.validate(scope, active)
    }
    #[cfg(all(test, target_arch = "x86_64", target_pointer_width = "64"))]
    pub(in crate::runtime::repository_physical_root) fn check_capture_for_test(
        &self,
        capture: &super::super::process_owner::ReapedCapture,
    ) -> Result<(), MountLeaseError> {
        self.check_capture(capture)
    }
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    pub(in crate::runtime::repository_physical_root) fn accept_after_reap(
        &mut self,
        capture: &super::super::process_owner::ReapedCapture,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<FinalizedRepositoryView, MountLeaseError> {
        if self.poisoned {
            return Err(MountLeaseError::Consumed);
        }
        let checked = self.check_completion(capture, scope, active);
        self.poisoned = true;
        checked?;
        Ok(self.view.take().expect("verified pending view"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readonly_flags_require_each_security_bit_and_preserve_tool_execution() {
        let flags = libc::ST_RDONLY as u64 | libc::ST_NOSUID as u64 | libc::ST_NODEV as u64;
        assert!(validate_flags(flags).is_ok());
        for bit in [libc::ST_RDONLY, libc::ST_NOSUID, libc::ST_NODEV] {
            assert!(matches!(
                validate_flags(flags & !(bit as u64)),
                Err(MountLeaseError::AttachedRoot)
            ));
        }
        assert!(matches!(
            validate_flags(flags | libc::ST_NOEXEC as u64),
            Err(MountLeaseError::AttachedRoot)
        ));
    }
    #[test]
    fn readonly_mint_refuses_host_and_cancelled_scope_without_leaking_credit() {
        let fixture = tempfile::tempdir().unwrap();
        let root = File::open(fixture.path()).unwrap();
        let resources = super::super::super::tests::resources();
        let scope = super::super::super::tests::scope();
        let baseline = resources.snapshot().retained_bytes;
        assert!(matches!(
            clone_readonly_from_attached_root(&root, fixture.path(), &resources, &scope, &|| true),
            Err(MountLeaseError::AttachedRoot)
        ));
        scope.cancel();
        assert!(matches!(
            clone_readonly_from_attached_root(&root, fixture.path(), &resources, &scope, &|| true),
            Err(MountLeaseError::Root(PhysicalRootError::SourceService(
                SourceServiceError::Cancelled
            )))
        ));
        let after = resources.snapshot();
        assert_eq!(
            (after.retained_bytes, after.native_fds, after.backing_bytes),
            (baseline, 0, 0)
        );
    }
}
