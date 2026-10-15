//! Positive native row/key partitions, separate from old construction forecasts.
use super::*;
use crate::bzl_environment::publication::PreparedPublication;

pub(super) fn add_local(
    complete: &mut PublicationShape,
    intrinsic: &mut PublicationShape,
    group: PublicationShape,
    nesting: usize,
) -> anyhow::Result<()> {
    add_shape(complete, group, nesting)?;
    add_shape(intrinsic, group, nesting)
}
pub(super) fn prepare(
    intrinsic: PublicationShape,
    count: usize,
    children: impl Iterator<Item = PublicationShape>,
    work: &mut PublicationShape,
) -> anyhow::Result<PreparedPublication> {
    PreparedPublication::prepare(intrinsic, count, children, |visits, bytes| {
        add_shape(
            work,
            PublicationShape {
                visits,
                bytes,
                ..zero_shape()
            },
            0,
        )
        .map_err(|error| error.to_string())
    })
    .map_err(anyhow::Error::msg)
}
pub(super) fn precopy(retained: PublicationShape, work: PublicationShape) -> anyhow::Result<()> {
    use crate::bzl_module::semantic_evaluation::budget::Budget;
    use crate::bzl_module::semantic_evaluation::budget::admit_peak;
    admit_peak(
        PeakContext::new(
            PeakOwner::FunctionOwner,
            PeakPhase::NativeTransfer,
            FirstLedger::NativeRetained,
        ),
        Budget {
            records: retained.visits,
            bytes: retained.bytes,
            depth: retained.depth,
        },
        Budget {
            records: work.visits,
            bytes: work.bytes,
            depth: work.depth,
        },
        0,
    )
    .map_err(anyhow::Error::msg)
}

#[cfg(test)]
#[path = "key_publication/tests.rs"]
mod tests;
