//! Fixed-scalar context for an already-refused operation; no retained state.
use std::fmt;

use super::BYTE_LIMIT;
use super::Budget;
use super::DEPTH_LIMIT;
use super::PeakContext;
use super::RECORD_LIMIT;
use super::limit;

#[derive(Clone, Copy)]
enum Total {
    Value(usize),
    Overflow,
    NotReached,
}
impl fmt::Display for Total {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(value) => value.fmt(f),
            Self::Overflow => f.write_str("overflow"),
            Self::NotReached => f.write_str("not-reached"),
        }
    }
}

pub(super) fn charge(
    context: PeakContext,
    before: Budget,
    after: Budget,
    records: usize,
    bytes: usize,
    error: String,
) -> String {
    // Reconstruct only the arithmetic actually reached by the legacy mutator.
    // In particular, both additions precede either ordinary cap predicate.
    let (kind, record_total, byte_total) = match before.records.checked_add(records) {
        None => ("records-overflow", Total::Overflow, Total::NotReached),
        Some(record_total) => match before.bytes.checked_add(bytes) {
            None => (
                "bytes-overflow",
                Total::Value(record_total),
                Total::Overflow,
            ),
            Some(byte_total) => (
                if record_total > RECORD_LIMIT {
                    "records-cap"
                } else {
                    "bytes-cap"
                },
                Total::Value(record_total),
                Total::Value(byte_total),
            ),
        },
    };
    format!(
        "{error} (operation=charge kind={kind} owner={} phase={} first_ledger={} before_records={} before_bytes={} before_depth={} requested_records={records} requested_bytes={bytes} record_total={record_total} byte_total={byte_total} after_records={} after_bytes={} after_depth={} record_limit={RECORD_LIMIT} byte_limit={BYTE_LIMIT} depth_limit={DEPTH_LIMIT})",
        context.owner(),
        context.phase(),
        context.first_ledger(),
        before.records,
        before.bytes,
        before.depth,
        after.records,
        after.bytes,
        after.depth,
    )
}

pub(super) fn depth(
    context: PeakContext,
    before: Budget,
    after: Budget,
    requested: usize,
    error: String,
) -> String {
    format!(
        "{error} (operation=depth kind=depth-cap owner={} phase={} first_ledger={} before_records={} before_bytes={} before_depth={} requested_depth={requested} after_records={} after_bytes={} after_depth={} depth_limit={DEPTH_LIMIT})",
        context.owner(),
        context.phase(),
        context.first_ledger(),
        before.records,
        before.bytes,
        before.depth,
        after.records,
        after.bytes,
        after.depth,
    )
}

pub(super) fn arithmetic(context: PeakContext) -> String {
    format!(
        "{} (operation=checked-arithmetic kind=checked-arithmetic owner={} phase={} first_ledger={})",
        limit(),
        context.owner(),
        context.phase(),
        context.first_ledger(),
    )
}

pub(super) fn peak_overflow(
    context: PeakContext,
    logical: Budget,
    scratch: Budget,
    extra: usize,
    base: Option<usize>,
) -> String {
    let (kind, base_total, extra_total) = match base {
        None => (
            "logical-scratch-overflow",
            Total::Overflow,
            Total::NotReached,
        ),
        Some(base) => ("extra-overflow", Total::Value(base), Total::Overflow),
    };
    format!(
        "{} (operation=joint-peak kind={kind} owner={} phase={} first_ledger={} logical_records={} logical_bytes={} scratch_records={} scratch_bytes={} extra_bytes={extra} base_total={base_total} extra_total={extra_total} byte_limit={BYTE_LIMIT})",
        limit(),
        context.owner(),
        context.phase(),
        context.first_ledger(),
        logical.records,
        logical.bytes,
        scratch.records,
        scratch.bytes,
    )
}

#[cfg(test)]
mod tests;
