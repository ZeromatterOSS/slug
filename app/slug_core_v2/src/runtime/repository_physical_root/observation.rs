//! Borrowed observation custody, minted only from the actual sealed owner.
use std::path::Path;

use slug_bzlmod_v2::RepositoryWorkspaceNode;

use super::*;

/// Contains its own RootIo borrow, so an expected body cannot be paired with
/// another root's descriptor. No File, callback or constructor escapes here.
pub(in crate::runtime) struct SealedGeneralObservation<'a> {
    root: &'a RepositoryPhysicalRoot,
    io: RootIo<'a>,
    scope: &'a Arc<SourceServiceScope>,
    active: &'a dyn Fn() -> bool,
}
impl<'a> SealedGeneralObservation<'a> {
    pub(super) fn new(
        root: &'a RepositoryPhysicalRoot,
        scope: &'a Arc<SourceServiceScope>,
        active: &'a dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        root.check_origin(scope, active)?;
        Ok(Self {
            io: RootIo::new(&root.file, io_limits(root.mode, &root.reservation))?,
            root,
            scope,
            active,
        })
    }
    pub(in crate::runtime) fn io(&self) -> &RootIo<'a> {
        &self.io
    }
    pub(in crate::runtime) fn anchor(&self) -> &MaterializationPathAnchor {
        &self.root.anchor
    }
    pub(in crate::runtime) fn scope(&self) -> &Arc<SourceServiceScope> {
        self.scope
    }
    pub(in crate::runtime) fn active(&self) -> &dyn Fn() -> bool {
        self.active
    }
    /// Only direct canonical regular members qualify. Links, missing paths and
    /// digest-only Source snapshots never become owned-body verification.
    pub(in crate::runtime) fn file(
        &self,
        relative: &Path,
    ) -> Option<(&RepositoryWorkspaceBody, u16)> {
        let entry = self.root.state.entry(relative.to_str()?)?;
        let RepositoryWorkspaceNode::File(body) = entry.node() else {
            return None;
        };
        Some((body, entry.mode()))
    }
}
