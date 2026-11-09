//! Bounded ordered Lstat discovery. This is an operational lookahead window,
//! not native admission or a guarantee of serial-equivalent live-bank pressure.
use std::ops::ControlFlow;

use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_workspace_v2::NeedPathObservations;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationOperation;
use starlark_map::small_map::SmallMap;

mod file_bytes;
pub(crate) use file_bytes::HomogeneousPathLookahead;
#[cfg(test)]
pub(crate) use file_bytes::assert_observer_layout;
#[cfg(feature = "native-probe-observer")]
pub use file_bytes::file_bytes_lookahead_extended;

const MAX_ROWS: usize = 32;
const MAX_DEMANDS: usize = 32;
const MAX_METADATA: usize = 64 * 1024;
const MAX_PATH: usize = 4096;
const FIXED_SCRATCH: usize = 1024;

pub(crate) trait PathNeedBreak: Sized {
    fn into_need(self) -> Result<SourcePreparationNeeds, Self>;
    fn from_need(need: SourcePreparationNeeds) -> Self;
    #[cfg(feature = "native-probe-observer")]
    fn lookahead_started() {}
    #[cfg(feature = "native-probe-observer")]
    fn lookahead_inspected() {}
    #[cfg(feature = "native-probe-observer")]
    fn lookahead_extended(_added: usize) {}
}

pub(crate) struct PathLookahead {
    pending: Option<SourcePreparationNeeds>,
    inspected: usize,
}

impl PathLookahead {
    pub(crate) fn new() -> Self {
        Self {
            pending: None,
            inspected: 0,
        }
    }

    pub(crate) fn can_inspect_next(&self) -> bool {
        self.pending.is_none() || self.inspected < MAX_ROWS
    }

    pub(crate) fn observe<E: PathNeedBreak>(&mut self, result: Result<(), E>) -> ControlFlow<E> {
        debug_assert!(self.can_inspect_next());
        if self.pending.is_some() {
            self.inspected += 1;
            #[cfg(feature = "native-probe-observer")]
            E::lookahead_inspected();
        }
        match result.map_err(PathNeedBreak::into_need) {
            Ok(()) => ControlFlow::Continue(()),
            Err(Ok(need)) => {
                let Some(right) = Candidate::new(&need) else {
                    return ControlFlow::Break(self.earlier_or(E::from_need(need)));
                };
                if let Some(pending) = &self.pending {
                    let left = Candidate::new(pending).expect("pending Need was admitted");
                    let Some(unique) = forecast(&left, Some(&right)) else {
                        return ControlFlow::Break(self.take_pending());
                    };
                    #[cfg(feature = "native-probe-observer")]
                    let added = unique - left.paths.demands().len();
                    if unique != left.paths.demands().len() {
                        // The full prospective peak was checked before either
                        // the union Vec/Arc or new wrapper controls allocate.
                        let joined = left.paths.union(right.paths);
                        self.pending = Some(SourcePreparationNeeds::path(joined));
                    }
                    #[cfg(feature = "native-probe-observer")]
                    E::lookahead_extended(added);
                } else {
                    if forecast(&right, None).is_none() {
                        return ControlFlow::Break(E::from_need(need));
                    }
                    self.pending = Some(need);
                    self.inspected = 1;
                    #[cfg(feature = "native-probe-observer")]
                    E::lookahead_started();
                }
                ControlFlow::Continue(())
            }
            Err(Err(error)) => ControlFlow::Break(self.earlier_or(error)),
        }
    }

    fn earlier_or<E: PathNeedBreak>(&mut self, later: E) -> E {
        self.pending.take().map_or(later, E::from_need)
    }

    fn take_pending<E: PathNeedBreak>(&mut self) -> E {
        E::from_need(self.pending.take().expect("earlier path Need is pending"))
    }

    pub(crate) fn finish<E: PathNeedBreak>(mut self) -> Result<(), E> {
        match self.pending.take() {
            Some(need) => Err(E::from_need(need)),
            None => Ok(()),
        }
    }
}

struct Candidate<'a> {
    paths: &'a NeedPathObservations,
    path_bytes: usize,
    control_bytes: usize,
}

impl<'a> Candidate<'a> {
    fn new(need: &'a SourcePreparationNeeds) -> Option<Self> {
        let paths = need.only_path_observations()?;
        if paths.demands().len() > MAX_DEMANDS {
            return None;
        }
        let control_bytes = control_bytes(need)?;
        if control_bytes > MAX_METADATA {
            return None;
        }
        let mut path_bytes = 0usize;
        for demand in paths.demands() {
            if demand.operation() != PathObservationOperation::Lstat
                || demand.path().as_path().as_os_str().len() > MAX_PATH
            {
                return None;
            }
            // Includes actual PathBuf spare capacity and shared-owner headers.
            // Duplicate/shared backing is conservatively charged again.
            path_bytes = path_bytes.checked_add(demand.path().retained_storage_bytes()?)?;
            if path_bytes > MAX_METADATA {
                return None;
            }
        }
        Some(Self {
            paths,
            path_bytes,
            control_bytes,
        })
    }
}

fn map_control<K: Eq + std::hash::Hash, V>(map: &SmallMap<K, V>) -> Option<usize> {
    (2 * std::mem::size_of::<usize>())
        .checked_add(std::mem::size_of_val(map))?
        .checked_add(map.retained_storage_bytes_upper_bound()?)
}

fn control_bytes(need: &SourcePreparationNeeds) -> Option<usize> {
    let maps = [
        map_control(need.repository_materializations())?,
        map_control(need.selected_extension_downloads())?,
        map_control(need.repository_executes())?,
        map_control(need.repository_filesystem_operations())?,
        map_control(need.source_snapshots())?,
        map_control(need.workspace_reservations())?,
        map_control(need.workspace_operations())?,
    ];
    maps.into_iter().try_fold(
        std::mem::size_of::<SourcePreparationNeeds>(),
        usize::checked_add,
    )
}

fn peak_bytes(
    left: usize,
    right: usize,
    unique: usize,
    controls: usize,
    paths: usize,
) -> Option<usize> {
    let arc_header = (2 * std::mem::size_of::<usize>())
        .checked_add(std::mem::align_of::<PathObservationDemand>() - 1)?;
    // Both input arrays, L+R Vec capacity and the resulting Arc array coexist.
    let rows = left
        .checked_add(right)?
        .checked_mul(2)?
        .checked_add(unique)?;
    FIXED_SCRATCH
        .checked_add(controls.checked_mul(3)?)?
        .checked_add(std::mem::size_of::<Vec<PathObservationDemand>>())?
        .checked_add(arc_header.checked_mul(3)?)?
        .checked_add(rows.checked_mul(std::mem::size_of::<PathObservationDemand>())?)?
        .checked_add(paths)
}

fn forecast(left: &Candidate<'_>, right: Option<&Candidate<'_>>) -> Option<usize> {
    let mut unique = left.paths.demands().len();
    if let Some(right) = right {
        let mut old = left.paths.demands().iter().peekable();
        for demand in right.paths.demands() {
            while old.peek().is_some_and(|previous| *previous < demand) {
                old.next();
            }
            if old.peek().is_none_or(|previous| *previous != demand) {
                unique = unique.checked_add(1)?;
                if unique > MAX_DEMANDS {
                    return None;
                }
            }
        }
    }
    let right_count = right.map_or(0, |right| right.paths.demands().len());
    let controls = right.map_or(left.control_bytes, |right| {
        left.control_bytes.max(right.control_bytes)
    });
    let paths = left
        .path_bytes
        .checked_add(right.map_or(0, |right| right.path_bytes))?;
    (peak_bytes(
        left.paths.demands().len(),
        right_count,
        unique,
        controls,
        paths,
    )? <= MAX_METADATA)
        .then_some(unique)
}

#[cfg(test)]
mod tests {
    use dupe::Dupe;
    use slug_workspace_v2::NormalizedAbsolutePath;
    use slug_workspace_v2::PathObservationNamespace;

    use super::*;

    enum DriverBreak {
        Need(SourcePreparationNeeds),
    }
    impl PathNeedBreak for DriverBreak {
        fn into_need(self) -> Result<SourcePreparationNeeds, Self> {
            match self {
                Self::Need(need) => Ok(need),
            }
        }
        fn from_need(need: SourcePreparationNeeds) -> Self {
            Self::Need(need)
        }
    }

    fn need(indices: impl IntoIterator<Item = usize>) -> SourcePreparationNeeds {
        SourcePreparationNeeds::path(
            NeedPathObservations::try_from_iter(indices.into_iter().map(|i| {
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new(format!("/lookahead/{i}")).unwrap(),
                    PathObservationOperation::Lstat,
                )
            }))
            .unwrap(),
        )
    }

    fn stopped(flow: ControlFlow<DriverBreak>) -> SourcePreparationNeeds {
        let ControlFlow::Break(DriverBreak::Need(need)) = flow else {
            panic!("expected pending Need")
        };
        need
    }

    #[test]
    fn window_counts_first_hole_and_every_complete_slot() {
        let mut lookahead = PathLookahead::new();
        assert!(
            lookahead
                .observe::<DriverBreak>(Err(DriverBreak::Need(need([0]))))
                .is_continue()
        );
        for _ in 1..MAX_ROWS {
            assert!(lookahead.can_inspect_next());
            assert!(lookahead.observe::<DriverBreak>(Ok(())).is_continue());
        }
        assert!(!lookahead.can_inspect_next());
        assert!(matches!(
            lookahead.finish::<DriverBreak>(),
            Err(DriverBreak::Need(_))
        ));
        assert!(
            std::mem::size_of::<PathLookahead>()
                <= std::mem::size_of::<SourcePreparationNeeds>() + FIXED_SCRATCH
        );
    }

    #[test]
    fn unique_bound_and_later_operation_barriers_keep_only_earlier_paths() {
        let mut lookahead = PathLookahead::new();
        assert!(
            lookahead
                .observe::<DriverBreak>(Err(DriverBreak::Need(need(0..16))))
                .is_continue()
        );
        assert!(
            lookahead
                .observe::<DriverBreak>(Err(DriverBreak::Need(need(16..32))))
                .is_continue()
        );
        let pending = stopped(lookahead.observe::<DriverBreak>(Err(DriverBreak::Need(need([32])))));
        assert_eq!(
            pending.only_path_observations().unwrap().demands().len(),
            32
        );
        let mut lookahead = PathLookahead::new();
        assert!(
            lookahead
                .observe::<DriverBreak>(Err(DriverBreak::Need(need([0]))))
                .is_continue()
        );
        let bytes = SourcePreparationNeeds::path(NeedPathObservations::singleton(
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new("/later").unwrap(),
                PathObservationOperation::FileBytes,
            ),
        ));
        let pending = stopped(lookahead.observe::<DriverBreak>(Err(DriverBreak::Need(bytes))));
        assert_eq!(pending.only_path_observations().unwrap().demands().len(), 1);
        assert_eq!(
            pending.only_path_observations().unwrap().demands()[0]
                .path()
                .as_path(),
            std::path::Path::new("/lookahead/0")
        );
    }

    #[test]
    fn first_ineligible_or_large_spare_owner_returns_original_need() {
        for need in [
            need(0..33),
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new("/bytes").unwrap(),
                    PathObservationOperation::FileBytes,
                ),
            )),
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new(format!(
                        "/{}/../short",
                        "x".repeat(MAX_METADATA + 1)
                    ))
                    .unwrap(),
                    PathObservationOperation::Lstat,
                ),
            )),
        ] {
            let original = need.dupe();
            let mut lookahead = PathLookahead::new();
            let returned = stopped(lookahead.observe::<DriverBreak>(Err(DriverBreak::Need(need))));
            assert!(std::ptr::eq(
                original.path_observations().unwrap().demands(),
                returned.path_observations().unwrap().demands()
            ));
        }
    }

    #[test]
    fn duplicate_only_extension_keeps_pending_backing_and_unique_count() {
        let original = need([0, 1]);
        let retained = original.dupe();
        let foreign_equal = need([0, 1]);
        assert!(!std::ptr::eq(
            retained.path_observations().unwrap().demands(),
            foreign_equal.path_observations().unwrap().demands(),
        ));
        let mut lookahead = PathLookahead::new();
        assert!(
            lookahead
                .observe::<DriverBreak>(Err(DriverBreak::Need(original)))
                .is_continue()
        );
        assert!(
            lookahead
                .observe::<DriverBreak>(Err(DriverBreak::Need(foreign_equal)))
                .is_continue()
        );
        assert_eq!(lookahead.inspected, 2);
        let Err(DriverBreak::Need(pending)) = lookahead.finish::<DriverBreak>() else {
            panic!("pending earlier paths must prevent Complete")
        };
        assert_eq!(pending.only_path_observations().unwrap().demands().len(), 2);
        assert!(std::ptr::eq(
            retained.path_observations().unwrap().demands(),
            pending.path_observations().unwrap().demands(),
        ));
    }

    #[test]
    fn metadata_peak_has_exact_boundary_and_checked_overflow() {
        let fixed = peak_bytes(1, 1, 2, 0, 0).unwrap();
        assert_eq!(
            peak_bytes(1, 1, 2, 0, MAX_METADATA - fixed),
            Some(MAX_METADATA)
        );
        assert_eq!(
            peak_bytes(1, 1, 2, 0, MAX_METADATA - fixed + 1),
            Some(MAX_METADATA + 1)
        );
        assert!(peak_bytes(usize::MAX, 1, 1, 0, 0).is_none());
        assert!(peak_bytes(1, 1, 2, usize::MAX, 0).is_none());
    }
}
