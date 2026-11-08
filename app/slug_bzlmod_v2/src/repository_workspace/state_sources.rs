//! Bounded, borrowed source classification. Retention visitors deliberately do
//! not grant candidates: only the exact operation prefix does.
use super::*;

pub(super) struct BindingSelection<'a> {
    rows: [Option<&'a RepositoryWorkspaceReadView>; 128],
    used: [bool; 128],
    len: usize,
}
fn overlaps(a: &std::path::Path, b: &std::path::Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}
impl<'a> BindingSelection<'a> {
    pub(super) fn new(
        reservation: &RepositoryWorkspaceReservation,
        entries: &[RepositoryWorkspaceEntry],
        candidates: impl IntoIterator<Item = &'a RepositoryWorkspaceReadView>,
    ) -> Result<Self, RepositoryWorkspaceError> {
        let mut selected = Self {
            rows: [None; 128],
            used: [false; 128],
            len: 0,
        };
        for view in candidates {
            if view.source_state().is_none()
                || view.workspace() != reservation.workspace()
                || view.lease() != reservation.lease()
                || view.repository() == reservation.repository()
                || view.instance() == reservation.instance()
                || overlaps(
                    view.display_root().as_path(),
                    reservation.display_root().as_path(),
                )
            {
                return Err(RepositoryWorkspaceError::ForeignReservation);
            }
            let mut duplicate = false;
            for old in selected.rows[..selected.len].iter().flatten() {
                if old.repository() == view.repository() {
                    if *old != view {
                        return Err(RepositoryWorkspaceError::ReplayConflict);
                    }
                    duplicate = true;
                } else if old.instance() == view.instance()
                    || overlaps(old.display_root().as_path(), view.display_root().as_path())
                {
                    return Err(RepositoryWorkspaceError::ForeignReservation);
                }
            }
            if duplicate {
                continue;
            }
            if selected.len == 128 {
                return Err(RepositoryWorkspaceError::StateLimit);
            }
            selected.rows[selected.len] = Some(view);
            selected.len += 1;
        }
        for entry in entries {
            let RepositoryWorkspaceNode::Link(target) = entry.node() else {
                continue;
            };
            if !target.starts_with('/') {
                continue;
            } // base validation checks relative escape
            let path = std::path::Path::new(target.as_str());
            let own_root = reservation
                .display_root()
                .as_path()
                .to_str()
                .ok_or(RepositoryWorkspaceError::InvalidPath)?;
            if target
                .strip_prefix(own_root)
                .is_some_and(|tail| tail.is_empty() || tail.starts_with('/'))
            {
                continue;
            }
            // Cross-root literals must be canonical, without normalization that
            // could conceal an escape or a different admitted source.
            if !target[1..]
                .split('/')
                .all(|p| !p.is_empty() && p != "." && p != "..")
            {
                return Err(RepositoryWorkspaceError::InvalidPath);
            }
            let Some(index) = selected.rows[..selected.len]
                .iter()
                .position(|view| path.starts_with(view.unwrap().display_root().as_path()))
            else {
                return Err(RepositoryWorkspaceError::InvalidPath);
            };
            selected.used[index] = true;
        }
        Ok(selected)
    }
    pub(super) fn for_request(
        request: &'a RepositoryWorkspaceRequest,
        entries: &[RepositoryWorkspaceEntry],
    ) -> Result<Self, RepositoryWorkspaceError> {
        Self::new(
            request.reservation(),
            entries,
            request
                .predecessor()
                .source_bindings()
                .iter()
                .chain(request.operation().direct_source_candidates()),
        )
    }
    pub(super) fn for_state(
        state: &'a RepositoryWorkspaceState,
    ) -> Result<Self, RepositoryWorkspaceError> {
        let selected = Self::new(
            state.reservation(),
            state.entries(),
            state.source_bindings(),
        )?;
        if selected.len != state.source_bindings().len()
            || selected.used[..selected.len].iter().any(|used| !used)
            || !state
                .source_bindings()
                .windows(2)
                .all(|p| p[0].display_root().as_path() < p[1].display_root().as_path())
        {
            return Err(RepositoryWorkspaceError::NonCanonicalEntries);
        }
        Ok(selected)
    }
    pub(super) fn owned_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        let count = self.used[..self.len].iter().filter(|used| **used).count();
        // Vec and Arc slice coexist at transfer; include all copied reference
        // metadata conservatively, preserving the existing source-request charge.
        let mut bytes = add(
            array_bytes::<RepositoryWorkspaceReadView>(count)?,
            array_bytes::<RepositoryWorkspaceReadView>(count)?,
        )?;
        let mut walk =
            crate::source_preparation::workspace_identity::WorkspaceSpecStorageWalk::default();
        for (index, view) in self.rows[..self.len].iter().enumerate() {
            if self.used[index] {
                bytes = add(bytes, view.unwrap().owned_metadata_bytes_with(&mut walk)?)?;
            }
        }
        Ok(bytes)
    }
    pub(super) fn visit(&self, visit: &mut impl FnMut(&RepositoryWorkspaceReadView)) {
        for (index, view) in self.rows[..self.len].iter().enumerate() {
            if self.used[index] {
                visit(view.unwrap());
            }
        }
    }
    pub(super) fn into_vec(self) -> Vec<RepositoryWorkspaceReadView> {
        let mut views =
            Vec::with_capacity(self.used[..self.len].iter().filter(|used| **used).count());
        for (index, view) in self.rows[..self.len].iter().enumerate() {
            if self.used[index] {
                views.push(view.unwrap().clone());
            }
        }
        views.sort_unstable_by(|a, b| a.display_root().as_path().cmp(b.display_root().as_path()));
        views
    }
}
impl RepositoryWorkspaceState {
    pub fn bindings_for_entries(
        reservation: &RepositoryWorkspaceReservation,
        entries: &[RepositoryWorkspaceEntry],
        candidates: &[RepositoryWorkspaceReadView],
    ) -> Result<Vec<RepositoryWorkspaceReadView>, RepositoryWorkspaceError> {
        Self::validate_base(reservation, RepositoryWorkspaceMode::Archive, entries)?;
        let selected = BindingSelection::new(reservation, entries, candidates)?;
        selected.owned_bytes()?;
        Ok(selected.into_vec())
    }
}
