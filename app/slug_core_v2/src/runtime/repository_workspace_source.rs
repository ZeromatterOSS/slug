//! Command-owned source acquisition/install. Source setup never fabricates a
//! General operation, completion record, owner certificate or transcript.
use super::super::super::mount_lease::source::SourceBootstrap;
use super::super::super::mount_lease::source::SourceFinalize;
use super::super::super::process_owner;
use super::super::super::source::SealedSourceSnapshot;
use super::super::super::source::SourceRootInput;
use super::*;
#[cfg(all(test, feature = "native-probe-observer"))]
use crate::runtime::probe_observer::workspace_progress::Stage;
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
            // The authentic driver's block_on polls this future on the probe
            // thread. The spans include the caller's wait for the owned worker.
            #[cfg(all(test, feature = "native-probe-observer"))]
            let probe = crate::runtime::probe_observer::capture();
            #[cfg(all(test, feature = "native-probe-observer"))]
            let span = |stage| {
                probe
                    .as_ref()
                    .and_then(|probe| probe.workspace_progress_interval(stage))
            };
            #[cfg(all(test, feature = "native-probe-observer"))]
            let mut bootstrap = span(Stage::SourceBootstrap);
            let launch = SourceBootstrap::new(
                reservation.clone(),
                claim,
                self.resources.clone(),
                scope.clone(),
                self.mount_domain.clone(),
                observer.clone(),
                active,
            );
            #[cfg(all(test, feature = "native-probe-observer"))]
            if matches!(&launch, Err(PhysicalRootError::Resource))
                && let Some(probe) = crate::runtime::probe_observer::capture()
            {
                drop(bootstrap.take());
                probe.record_backing_inventory(self.backing_inventory_for_probe(&input));
            }
            let launch = launch?;
            let builder =
                process_owner::run(launch, super::super::super::initialize::platform()?, active)
                    .await?;
            #[cfg(all(test, feature = "native-probe-observer"))]
            if let Some(bootstrap) = bootstrap {
                bootstrap.finish();
            }
            let domain = builder.domain.clone();
            let captured = input.populate(&builder, scope, active)?;
            #[cfg(all(test, feature = "native-probe-observer"))]
            let state_span = span(Stage::SourceState);
            let (state, state_credit) =
                captured.into_state(reservation.clone(), &self.resources)?;
            #[cfg(all(test, feature = "native-probe-observer"))]
            if let Some(state_span) = state_span {
                state_span.finish();
            }
            #[cfg(all(test, feature = "native-probe-observer"))]
            let finalize = span(Stage::SourceFinalize);
            let launch = SourceFinalize::new(builder, state, state_credit, observer)?;
            let finalized =
                process_owner::run(launch, super::super::super::initialize::platform()?, active)
                    .await?;
            #[cfg(all(test, feature = "native-probe-observer"))]
            if let Some(finalize) = finalize {
                finalize.finish();
            }
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
            #[cfg(all(test, feature = "native-probe-observer"))]
            if let Some(class) = finalized.receipt.root().frozen_archive_quota()
                && let Some(probe) = crate::runtime::probe_observer::capture()
            {
                probe.record_closed_archive_quota(class.quota(), true);
            }
            Ok((self, finalized.receipt))
        }
    }
}
