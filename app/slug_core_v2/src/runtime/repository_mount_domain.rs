//! Command-scoped setup authority. Sealed roots, backing and spelling claims
//! never point to this owner; data readers cannot retain its user namespace.
use std::os::fd::AsFd;

use super::*;

pub(in crate::runtime::repository_physical_root) struct NativeMountDomain {
    namespace: File,
    identity: NamespaceIdentity,
    lease: u64,
    scope: Arc<SourceServiceScope>,
    resources: Arc<RepositoryRootResources>,
    // Namespace descriptor and scope payload retire before their allowance.
    _credit: resources::ResourceCredit,
}
impl std::fmt::Debug for NativeMountDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeMountDomain")
            .field("identity", &self.identity)
            .field("lease", &self.lease)
            .finish_non_exhaustive()
    }
}
impl NativeMountDomain {
    pub(in crate::runtime::repository_physical_root) fn from_source(
        reservation: &slug_bzlmod_v2::RepositorySourceSnapshotReservation,
        clone: &AttachedMountClone,
        resources: &Arc<RepositoryRootResources>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Arc<Self>, MountLeaseError> {
        check(scope, active)?;
        if reservation.workspace() != scope.workspace()
            || reservation.lease() == 0
            || !Arc::ptr_eq(&clone.resources, resources)
        {
            return Err(MountLeaseError::Request);
        }
        let credit = resources.reserve(arc_bytes::<Self>()?, 1, 0)?;
        let namespace = duplicate_namespace(&clone.owner)?;
        let owner = Arc::new(Self {
            namespace,
            identity: clone.namespace,
            lease: reservation.lease(),
            scope: scope.clone(),
            resources: resources.clone(),
            _credit: credit,
        });
        owner.check_clone(clone, scope, active)?;
        Ok(owner)
    }
    pub(in crate::runtime::repository_physical_root) fn check_source(
        &self,
        reservation: &slug_bzlmod_v2::RepositorySourceSnapshotReservation,
        resources: &Arc<RepositoryRootResources>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), MountLeaseError> {
        self.check_scope(scope, active)?;
        if reservation.workspace() != self.scope.workspace()
            || reservation.lease() != self.lease
            || !Arc::ptr_eq(resources, &self.resources)
        {
            return Err(MountLeaseError::Request);
        }
        Ok(())
    }
    /// Only a verified, reaped bootstrap's seed may establish the domain. The
    /// consuming command calls this after its bootstrap lifecycle has settled.
    pub(in crate::runtime::repository_physical_root) fn from_initial(
        root: &RepositoryPhysicalRootBuilder,
        seed: &InitialRepositoryMount,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Arc<Self>, MountLeaseError> {
        seed.check_binding(root, scope, active)?;
        if root.reservation.lease() == 0 || root.reservation.workspace() != scope.workspace() {
            return Err(MountLeaseError::Request);
        }
        let credit = root.resources.reserve(arc_bytes::<Self>()?, 1, 0)?;
        let namespace = duplicate_namespace(&seed.clone.owner)?;
        let owner = Self {
            namespace,
            identity: seed.clone.namespace,
            lease: root.reservation.lease(),
            scope: scope.clone(),
            resources: root.resources.clone(),
            _credit: credit,
        };
        if namespace_identity(&owner.namespace)? != owner.identity {
            return Err(MountLeaseError::Namespace);
        }
        owner.check_clone(&seed.clone, scope, active)?;
        Ok(Arc::new(owner))
    }
    pub(in crate::runtime::repository_physical_root) fn check_initial(
        &self,
        root: &RepositoryPhysicalRootBuilder,
        seed: &InitialRepositoryMount,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), MountLeaseError> {
        seed.check_binding(root, scope, active)?;
        self.check_root(&root.reservation, &root.resources, scope, active)?;
        self.check_clone(&seed.clone, scope, active)
    }
    pub(in crate::runtime::repository_physical_root) fn check_root(
        &self,
        reservation: &RepositoryWorkspaceReservation,
        resources: &Arc<RepositoryRootResources>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), MountLeaseError> {
        self.check_scope(scope, active)?;
        if reservation.lease() != self.lease
            || reservation.workspace() != self.scope.workspace()
            || !Arc::ptr_eq(resources, &self.resources)
        {
            return Err(MountLeaseError::Request);
        }
        Ok(())
    }
    pub(in crate::runtime::repository_physical_root) fn check_clone(
        &self,
        clone: &AttachedMountClone,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), MountLeaseError> {
        self.check_scope(scope, active)?;
        if !Arc::ptr_eq(&clone.resources, &self.resources)
            || clone.namespace != self.identity
            || namespace_identity(&clone.owner)? != self.identity
            || namespace_identity(&self.namespace)? != self.identity
        {
            return Err(MountLeaseError::Namespace);
        }
        Ok(())
    }
    fn check_scope(
        &self,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), MountLeaseError> {
        crate::runtime::repository_io::source_service::check_identity(
            Some(&self.scope),
            Some(scope),
        )
        .map_err(PhysicalRootError::SourceService)?;
        check(scope, active)?;
        Ok(())
    }
    pub(in crate::runtime::repository_physical_root) fn borrow_namespace(&self) -> BorrowedFd<'_> {
        self.namespace.as_fd()
    }
    pub(super) fn namespace_identity(&self) -> NamespaceIdentity {
        self.identity
    }
    /// Caller prepays the returned FD before duplicating it. No data owner has
    /// access to this private setup-only method.
    pub(super) fn duplicate_namespace(&self) -> Result<File, MountLeaseError> {
        duplicate_namespace(&self.namespace)
    }
}
fn duplicate_namespace(input: &File) -> Result<File, MountLeaseError> {
    namespace_identity(input)?;
    let raw = unsafe { libc::fcntl(input.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
    if raw < 0 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(unsafe { File::from_raw_fd(raw) })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn namespace_dup_refuses_ordinary_files_and_missing_cloexec() {
        let ordinary = tempfile::tempfile().unwrap();
        assert!(matches!(
            duplicate_namespace(&ordinary),
            Err(MountLeaseError::Namespace)
        ));
        let namespace = File::open("/proc/self/ns/user").unwrap();
        assert_eq!(
            unsafe { libc::fcntl(namespace.as_raw_fd(), libc::F_SETFD, 0) },
            0
        );
        assert!(matches!(
            duplicate_namespace(&namespace),
            Err(MountLeaseError::Namespace)
        ));
    }
}
