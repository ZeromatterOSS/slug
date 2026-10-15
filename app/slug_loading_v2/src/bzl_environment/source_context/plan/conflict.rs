//! Error-only bounded diagnostics; source conflicts still fail closed.
use std::fmt;

use super::*;
use crate::bzl_module::semantic_evaluation::budget::limit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TailDifference {
    Origin,
    Digest,
    MappingLength,
    MappingEntry(usize),
}
impl fmt::Display for TailDifference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Origin => f.write_str("source origin"),
            Self::Digest => f.write_str("source digest"),
            Self::MappingLength => f.write_str("repository mapping length"),
            Self::MappingEntry(ordinal) => write!(f, "repository mapping entry {ordinal}"),
        }
    }
}
fn forecast(label: &CanonicalLabel) -> Result<usize, String> {
    // Include the complete label even when Display omits its mapping ID.
    // Eightfold credit covers formatting growth and old/new allocation peaks;
    // fixed credit covers delimiters, tag/ordinal, String/header and alignment.
    [
        label.package().repo().as_str().len(),
        label.package().package().as_str().len(),
        label.target().as_str().len(),
        label.mapping_id().map_or(0, |id| id.as_str().len()),
    ]
    .into_iter()
    .try_fold(64usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or_else(limit)
    })?
    .checked_mul(8)
    .and_then(|bytes| bytes.checked_add(256))
    .ok_or_else(limit)
}
pub(super) fn message(
    label: &CanonicalLabel,
    difference: TailDifference,
    work: &mut Budget,
) -> Result<String, String> {
    work.charge(1, forecast(label)?)?;
    Ok(format!(
        "conflicting complete environment source provenance for {label}: {difference}"
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::bzl_module::semantic_evaluation::budget::BYTE_LIMIT;

    #[test]
    fn conflict_error_admission_accepts_exact_peak_and_refuses_one_byte_less() {
        let label = CanonicalLabel::parse("@@dep+//pkg:file.bzl").unwrap();
        let credit = forecast(&label).unwrap();
        let mut exact = Budget {
            bytes: BYTE_LIMIT - credit,
            ..Budget::default()
        };
        let message = message(&label, TailDifference::MappingEntry(17), &mut exact).unwrap();
        assert_eq!(exact.bytes, BYTE_LIMIT);
        assert_eq!(exact.records, 1);
        assert!(message.contains("@@dep+//pkg:file.bzl: repository mapping entry 17"));
        let mut denied = Budget {
            bytes: BYTE_LIMIT - credit + 1,
            ..Budget::default()
        };
        assert_eq!(
            super::message(&label, TailDifference::Origin, &mut denied).unwrap_err(),
            limit()
        );
    }
}
