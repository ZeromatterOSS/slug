//! Command-owned source acquisition/install. Source setup never fabricates a
//! General operation, completion record, owner certificate or transcript.
use super::super::super::mount_lease::source::SourceBootstrap;
use super::super::super::mount_lease::source::SourceFinalize;
use super::super::super::process_owner;
use super::super::super::source::SealedSourceSnapshot;
use super::super::super::source::SourceRootInput;
use super::*;
impl RepositoryWorkspaceCommand {
    pub(in crate::runtime) fn initialize_source_snapshot<'a>(
        mut self,
        reservation: Arc<slug_bzlmod_v2::RepositorySourceSnapshotReservation>,
        claim: Arc<crate::runtime::repository_io::source_snapshots::SourceSnapshotClaim>,
        input: SourceRootInput,
        scope: &'a Arc<SourceServiceScope>,
        active: &'a (dyn Fn() -> bool + Sync),
    ) -> impl std::future::Future<Output = Result<(Self, SealedSourceSnapshot), PhysicalRootError>>
    + Send
    + 'a {
        let input = input.with_retirement(self.retirement_observer());
        async move {
            self.check_scope(scope, active)?;
            input.check_binding(&reservation, &self.resources, scope, active)?;
            if reservation.lease() != self.epoch().lease().ok_or(PhysicalRootError::Reservation)?
                || reservation.workspace() != scope.workspace()
                || self
                    .roots
                    .len()
                    .checked_add(self.source_slots.len())
                    .ok_or(PhysicalRootError::Resource)?
                    >= self.max_roots
                || self
                    .source_slots
                    .iter()
                    .any(|s| s.reservation.request().repository() == reservation.repository())
                || self
                    .roots
                    .iter()
                    .any(|s| s.reservation.repository() == reservation.repository())
            {
                return Err(PhysicalRootError::Reservation);
            }
            let capacity = self
                .source_slots
                .len()
                .checked_add(1)
                .ok_or(PhysicalRootError::Resource)?;
            if self.source_slots.capacity() < capacity {
                self._control
                    .bytes
                    .try_grow(add(
                        capacity
                            .checked_mul(2 * std::mem::size_of::<NativeSourceSlot>())
                            .ok_or(PhysicalRootError::Resource)?,
                        64,
                    )?)
                    .map_err(|_| PhysicalRootError::Resource)?;
                self.source_slots
                    .try_reserve_exact(capacity - self.source_slots.len())
                    .map_err(|_| PhysicalRootError::Resource)?;
                if self.source_slots.capacity() > capacity {
                    return Err(PhysicalRootError::Resource);
                }
            }
            let observer = self.retirement_observer();
            let launch = SourceBootstrap::new(
                reservation.clone(),
                claim,
                self.resources.clone(),
                scope.clone(),
                self.mount_domain.clone(),
                observer.clone(),
                active,
            )?;
            let builder =
                process_owner::run(launch, super::super::super::initialize::platform()?, active)
                    .await?;
            let domain = builder.domain.clone();
            let captured = input.populate(&builder, scope, active)?;
            let (state, state_credit) =
                captured.into_state(reservation.clone(), &self.resources)?;
            let launch = SourceFinalize::new(builder, state, state_credit, observer)?;
            let finalized =
                process_owner::run(launch, super::super::super::initialize::platform()?, active)
                    .await?;
            self.check_scope(scope, active)?;
            let data = NativeReadViewData::from_source(&finalized.receipt)?;
            let dependencies = self
                .source_dependencies
                .merge(std::slice::from_ref(&data), &self.resources)?;
            self.source_slots.push(NativeSourceSlot {
                reservation,
                process_view: Some(selected::RetainedProcessView {
                    view: finalized.view,
                    data,
                }),
            });
            self.source_dependencies = dependencies;
            self.mount_domain = Some(domain);
            Ok((self, finalized.receipt))
        }
    }
}
