//! Closed selected-source images. Source manifests retain digests, never file
//! bodies or General repository histories. Setup owners stay outside data leases.
use std::path::Path;

use compact_str::CompactString;
use slug_bzlmod_v2::RepositorySourceSnapshotEntry as Entry;
use slug_bzlmod_v2::RepositorySourceSnapshotNode as Node;
use slug_bzlmod_v2::RepositorySourceSnapshotRequest as Request;
use slug_bzlmod_v2::RepositorySourceSnapshotReservation as Reservation;
use slug_bzlmod_v2::RepositorySourceSnapshotState as State;
use slug_workspace_v2::FileContentDigest;

use super::*;
#[path = "repository_source_root/input.rs"]
#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
mod input;
#[path = "repository_source_root/reader.rs"]
mod reader;
#[path = "repository_source_root/tree.rs"]
mod tree;
#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
pub(in crate::runtime) use input::SourceRootInput;
pub(in crate::runtime) use reader::RepositorySourceReader;

pub(in crate::runtime) struct RepositorySourceRoot {
    pub(super) file: File,
    state: Arc<State>,
    pub(super) identity: RootIdentity,
    pub(super) resources: Arc<RepositoryRootResources>,
    pub(super) backing: Arc<RepositoryRootBacking>,
    pub(super) origin_scope: Arc<SourceServiceScope>,
    anchor: MaterializationPathAnchor,
    _credit: ResourceCredit,
}
impl std::fmt::Debug for RepositorySourceRoot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RepositorySourceRoot")
            .field("identity", &self.identity)
            .field("entries", &self.state.entries().len())
            .finish_non_exhaustive()
    }
}
#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
pub(in crate::runtime::repository_physical_root) struct SourceRootBuilder {
    pub(super) file: File,
    pub(super) reservation: Arc<Reservation>,
    pub(super) identity: RootIdentity,
    pub(super) resources: Arc<RepositoryRootResources>,
    pub(super) backing: Arc<RepositoryRootBacking>,
    pub(super) scope: Arc<SourceServiceScope>,
    pub(super) domain: Arc<mount_lease::domain::NativeMountDomain>,
    pub(super) seed: Option<mount_lease::source::SourceMountSeed>,
    pub(super) credit: ResourceCredit,
    // Last: queued source setup/results cannot signal retirement prematurely.
    pub(super) retirement: progress::command::RepositoryWorkspaceRetirementGuard,
}
pub(in crate::runtime) struct SealedSourceSnapshot {
    root: Arc<RepositorySourceRoot>,
    host_observations: PathObservationEpoch,
    _credit: ResourceCredit,
}
impl SealedSourceSnapshot {
    pub(in crate::runtime) fn root(&self) -> &Arc<RepositorySourceRoot> {
        &self.root
    }
    pub(in crate::runtime) fn state(&self) -> &Arc<State> {
        &self.root.state
    }
    pub(in crate::runtime) fn reservation(&self) -> &Arc<Reservation> {
        self.root.reservation()
    }
    pub(in crate::runtime) fn host_observations(&self) -> &PathObservationEpoch {
        &self.host_observations
    }
}
pub(super) fn limits(reservation: &Reservation) -> Result<RootIoLimits, PhysicalRootError> {
    let prefix = reservation.display_root().as_path().as_os_str().len();
    if prefix > 4096 {
        return Err(PhysicalRootError::Reservation);
    }
    Ok(RootIoLimits {
        max_path_bytes: 256.min(4096 - prefix - usize::from(prefix < 4096)),
        max_name_bytes: 255,
        max_depth: 32,
        max_entries: 8192,
        max_directory_name_bytes: 1024 * 1024,
        max_file_bytes: 64 * 1024 * 1024,
        max_link_bytes: 256,
    })
}
impl RepositorySourceRoot {
    pub(in crate::runtime) fn source_claim(
        &self,
    ) -> Option<Arc<crate::runtime::repository_io::source_snapshots::SourceSnapshotClaim>> {
        self.backing.source_snapshot_claim()
    }
    pub(in crate::runtime) fn state(&self) -> &Arc<State> {
        &self.state
    }
    pub(in crate::runtime) fn reservation(&self) -> &Arc<Reservation> {
        self.state.reservation()
    }
    pub(in crate::runtime) fn anchor(&self) -> &MaterializationPathAnchor {
        &self.anchor
    }
    pub(in crate::runtime) fn check_origin(
        &self,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        super::check_origin(&self.origin_scope, scope, active)
    }
    pub(super) fn validate_backing(&self) -> Result<(), PhysicalRootError> {
        if backing_identity(&self.file, RepositoryWorkspaceMode::Archive)? != self.identity {
            return Err(PhysicalRootError::RootIdentity);
        }
        Ok(())
    }
    pub(in crate::runtime) fn observe(
        &self,
        demands: &[PathObservationDemand],
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<PathObservationEpoch, PhysicalRootError> {
        self.check_origin(scope, active)?;
        let _scratch = self.resources.reserve(IO_SCRATCH, 3, 0)?;
        self.validate_backing()?;
        let io = RootIo::new(&self.file, limits(self.reservation())?)?;
        let result = crate::runtime::path_observation::rooted::observe_rooted(
            &io,
            &self.anchor,
            demands,
            scope,
            active,
        )
        .map_err(PhysicalRootError::Observation)?;
        self.validate_backing()?;
        self.check_origin(scope, active)?;
        Ok(result)
    }
    pub(in crate::runtime) fn open_member(
        self: &Arc<Self>,
        relative: &str,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<RepositorySourceReader, PhysicalRootError> {
        reader::open(self, relative, scope, active)
    }
}
#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
impl SourceRootBuilder {
    pub(super) fn from_bootstrap(
        file: File,
        reservation: Arc<Reservation>,
        resources: Arc<RepositoryRootResources>,
        backing: Arc<RepositoryRootBacking>,
        scope: Arc<SourceServiceScope>,
        domain: Arc<mount_lease::domain::NativeMountDomain>,
        seed: mount_lease::source::SourceMountSeed,
        observer: &progress::command::RepositoryWorkspaceRetirementObserver,
    ) -> Result<Self, PhysicalRootError> {
        struct Incoming {
            file: File,
            seed: mount_lease::source::SourceMountSeed,
            domain: Arc<mount_lease::domain::NativeMountDomain>,
            backing: Arc<RepositoryRootBacking>,
        }
        // Arm actual FD/token custody before any fallible retained admission.
        // In particular, a refused credit cannot refund backing before close.
        let incoming = Incoming {
            file,
            seed,
            domain,
            backing,
        };
        let credit = resources.reserve(
            add(
                add(
                    std::mem::size_of::<Self>(),
                    arc_bytes::<RepositorySourceRoot>()?,
                )?,
                reservation
                    .retained_storage_bytes()
                    .map_err(PhysicalRootError::Protocol)?,
            )?,
            1,
            0,
        )?;
        let mut owner = Self {
            file: incoming.file,
            reservation,
            identity: RootIdentity {
                device: 0,
                inode: 0,
                mount: 0,
            },
            resources,
            backing: incoming.backing,
            scope,
            domain: incoming.domain,
            seed: Some(incoming.seed),
            credit,
            retirement: observer.worker_guard(),
        };
        check(&owner.scope, &|| true)?;
        check_bank(&owner.scope, &owner.resources)?;
        owner.identity = backing_identity(&owner.file, RepositoryWorkspaceMode::Archive)?;
        owner
            .backing
            .bind_source(owner.identity, &owner.reservation)?;
        let io = RootIo::new(&owner.file, limits(&owner.reservation)?)?;
        if io.root_stat()?.st_mode & 0o7777 != 0o755 {
            return Err(PhysicalRootError::Snapshot);
        }
        let mut empty = true;
        io.read_directory(Path::new(""), &|| true, |_, _| {
            empty = false;
            Ok(())
        })?;
        if !empty {
            return Err(PhysicalRootError::Snapshot);
        }
        Ok(owner)
    }
    pub(super) fn check(
        &self,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        super::check_origin(&self.scope, scope, active)?;
        if backing_identity(&self.file, RepositoryWorkspaceMode::Archive)? != self.identity {
            return Err(PhysicalRootError::RootIdentity);
        }
        Ok(())
    }
    pub(super) fn capture(
        &self,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<tree::CapturedTree, PhysicalRootError> {
        self.check(scope, active)?;
        let io = RootIo::new(&self.file, limits(&self.reservation)?)?;
        tree::capture(&io, &self.reservation, None, &self.resources, scope, active)
    }
    pub(super) fn seal(
        self,
        expected: Arc<State>,
        host_observations: PathObservationEpoch,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<SealedSourceSnapshot, PhysicalRootError> {
        self.check(scope, active)?;
        if self.reservation != *expected.reservation() || self.seed.is_some() {
            return Err(PhysicalRootError::Reservation);
        }
        let actual = self.capture(scope, active)?;
        if actual.entries() != expected.entries() {
            return Err(PhysicalRootError::Snapshot);
        }
        let retained = expected
            .retained_storage_bytes()
            .map_err(PhysicalRootError::Protocol)?;
        let credit =
            self.resources
                .reserve(add(arc_bytes::<RepositorySourceRoot>()?, retained)?, 1, 0)?;
        let receipt = self.resources.reserve(
            add(
                std::mem::size_of::<SealedSourceSnapshot>(),
                tree::observation_storage(&host_observations)?,
            )?,
            0,
            0,
        )?;
        let anchor = MaterializationPathAnchor::new(
            PathObservationNamespace::Materialization(self.reservation.instance()),
            self.reservation.display_root().clone(),
        )
        .map_err(|_| PhysicalRootError::Reservation)?;
        let root = Arc::new(RepositorySourceRoot {
            file: self.file,
            state: expected,
            identity: self.identity,
            resources: self.resources,
            backing: self.backing,
            origin_scope: self.scope,
            anchor,
            _credit: credit,
        });
        Ok(SealedSourceSnapshot {
            root,
            host_observations,
            _credit: receipt,
        })
    }
}

#[cfg(all(test, target_arch = "x86_64", target_pointer_width = "64"))]
#[path = "repository_source_root/tests.rs"]
mod tests;
