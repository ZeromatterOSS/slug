//! Borrowed construction-checked domains, minted only by consuming builder seal.
use super::*;

/// append_edges validates before copying. Rows/imports only append, and imported
/// arenas are immutable, so those exact backing indices cannot become invalid.
/// This is completed validation, not a receipt for prepaid per-edge work.
pub(in crate::published_value) struct CheckedDraft<'a> {
    pub(super) rows: &'a [Option<PublishedNode>],
    pub(super) edges: &'a [PublishedEdge],
    pub(super) imports: &'a [PublishedValueArena],
}
impl<'a> CheckedDraft<'a> {
    pub(in crate::published_value) fn rows(&self) -> &'a [Option<PublishedNode>] {
        self.rows
    }
    pub(in crate::published_value) fn edges(&self) -> &'a [PublishedEdge] {
        self.edges
    }
    pub(in crate::published_value) fn imports(&self) -> &'a [PublishedValueArena] {
        self.imports
    }
}

#[cfg(test)]
#[path = "checked_draft/tests.rs"]
mod tests;
