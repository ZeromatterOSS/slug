//! Temporary same-row/same-base coverage, minted only by the native parent.
use super::*;
use crate::bzl_environment::BzlEnvironmentInputs;
use crate::bzl_module::semantic_evaluation::budget::Budget;

pub(crate) struct OwnerReferenceInputs<'a> {
    pub(super) rows: &'a [Arc<LocalBzlInput>],
    pub(super) base: Budget,
}
impl<'a> OwnerReferenceInputs<'a> {
    // A fixed stack-only envelope, tested against the actual borrowed iterator.
    const ITERATOR_WORDS: usize = 32;
    const READS: usize = 12;
    pub(super) fn prepay(work: &mut PublicationShape) -> anyhow::Result<()> {
        // Two row-borrow/copy operations, three scalar base initializations,
        // three returned base scalar reads, two row-handle reads and two
        // iterator setup operations. Per-occurrence traversal remains charged
        // by the unchanged reference planner.
        add_shape(
            work,
            PublicationShape {
                visits: Self::READS,
                bytes: std::mem::size_of::<Self>()
                    + std::mem::size_of::<Budget>()
                    + Self::ITERATOR_WORDS * std::mem::size_of::<usize>(),
                ..zero_shape()
            },
            0,
        )
    }
    pub(crate) fn base(&self) -> Budget {
        self.base
    }
    pub(crate) fn environments(
        &self,
    ) -> impl Iterator<Item = &'a Arc<BzlEnvironmentInputs>> + Clone + use<'a> {
        environment_publication::uses(self.rows)
    }
}

#[cfg(test)]
mod tests;
