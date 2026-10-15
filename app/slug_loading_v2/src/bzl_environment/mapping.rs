//! Borrow the actual tools mapping until its complete copy peak is admitted.
use std::alloc::Layout;
use std::mem::size_of;

use super::*;

type Pair = (ApparentRepoName, CanonicalRepoName);

#[derive(Debug, Clone, Allocative)]
pub(crate) struct BuiltinRepositoryMapping {
    pub(super) entries: Arc<[Pair]>,
}

impl BuiltinRepositoryMapping {
    // Existing callers already own this complete immutable input.
    pub(crate) fn new(entries: Arc<[Pair]>) -> Result<Self, String> {
        let mut b = Budget::default();
        b.array_with_context::<Pair>(
            budget_context(PeakPhase::ReferenceTables, FirstLedger::Logical),
            entries.len(),
        )?;
        for (a, c) in &*entries {
            b.charge_with_context(
                budget_context(PeakPhase::ReferenceTables, FirstLedger::Logical),
                1,
                a.as_str().len().saturating_add(c.as_str().len()),
            )?;
        }
        Ok(Self { entries })
    }

    pub(crate) fn into_entries(self) -> Arc<[Pair]> {
        self.entries
    }

    pub(crate) fn from_tools(
        mapping: &slug_bzlmod_v2::HostBuiltinBazelToolsRepositoryMapping,
    ) -> Result<Self, String> {
        admit_copy(mapping.iter())?;
        // The immutable producer guard covers both borrowed traversals. Do not
        // call entries(): it clones names and allocates before this admission.
        let entries = mapping
            .iter()
            .map(|(apparent, canonical)| (apparent.clone(), canonical.clone()))
            .collect::<Vec<_>>()
            .into();
        Ok(Self { entries })
    }
}

fn admit_copy<'a>(
    pairs: impl ExactSizeIterator<Item = (&'a ApparentRepoName, &'a CanonicalRepoName)>,
) -> Result<(), String> {
    let count = pairs.len();
    // Complete output plus Vec-to-Arc coexistence. Pair headers may coexist;
    // each name's new string buffer is allocated once and transferred intact.
    let buffers = Layout::array::<Pair>(count)
        .map_err(|_| {
            crate::bzl_module::semantic_evaluation::budget::limit_with_context(budget_context(
                PeakPhase::ReferenceTables,
                FirstLedger::Work,
            ))
        })?
        .size()
        .checked_mul(2)
        .ok_or_else(|| {
            crate::bzl_module::semantic_evaluation::budget::limit_with_context(budget_context(
                PeakPhase::ReferenceTables,
                FirstLedger::Work,
            ))
        })?;
    let headers = size_of::<BuiltinRepositoryMapping>()
        .checked_add(size_of::<Vec<Pair>>())
        .and_then(|v| v.checked_add(2 * size_of::<usize>()))
        .and_then(|v| v.checked_add(2 * (std::mem::align_of::<Pair>() - 1)))
        .ok_or_else(|| {
            crate::bzl_module::semantic_evaluation::budget::limit_with_context(budget_context(
                PeakPhase::ReferenceTables,
                FirstLedger::Work,
            ))
        })?;
    let mut budget = Budget::default();
    budget.charge_with_context(
        budget_context(PeakPhase::ReferenceTables, FirstLedger::Work),
        count
            .checked_mul(2)
            .and_then(|v| v.checked_add(1))
            .ok_or_else(|| {
                crate::bzl_module::semantic_evaluation::budget::limit_with_context(budget_context(
                    PeakPhase::ReferenceTables,
                    FirstLedger::Work,
                ))
            })?,
        buffers.checked_add(headers).ok_or_else(|| {
            crate::bzl_module::semantic_evaluation::budget::limit_with_context(budget_context(
                PeakPhase::ReferenceTables,
                FirstLedger::Work,
            ))
        })?,
    )?;
    for (apparent, canonical) in pairs {
        budget.charge_with_context(
            budget_context(PeakPhase::ReferenceTables, FirstLedger::Work),
            1,
            apparent
                .as_str()
                .len()
                .checked_add(canonical.as_str().len())
                .ok_or_else(|| {
                    crate::bzl_module::semantic_evaluation::budget::limit_with_context(
                        budget_context(PeakPhase::ReferenceTables, FirstLedger::Work),
                    )
                })?,
        )?;
    }
    Ok(())
}

const fn budget_context(phase: PeakPhase, ledger: FirstLedger) -> PeakContext {
    PeakContext::new(PeakOwner::BuiltinPreparation, phase, ledger)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bzl_module::semantic_evaluation::budget::RECORD_LIMIT;

    // Exact numeric migration: the first case refuses the last paid name;
    // the second refuses the fixed preallocation charge before any name read.
    fn expected_copy_refusal(count: usize, fixed: bool) -> String {
        let bytes = 2 * count * size_of::<Pair>()
            + size_of::<BuiltinRepositoryMapping>()
            + size_of::<Vec<Pair>>()
            + 2 * size_of::<usize>()
            + 2 * (std::mem::align_of::<Pair>() - 1);
        let initial_records = count * 2 + 1;
        let mut before = Budget::default();
        let (records, bytes) = if fixed {
            (initial_records, bytes)
        } else {
            let prior_names = RECORD_LIMIT - initial_records;
            before.records = RECORD_LIMIT;
            before.bytes = bytes + prior_names * ("apparent".len() + "canonical+".len());
            (1, "apparent".len() + "canonical+".len())
        };
        before
            .charge_with_context(
                budget_context(PeakPhase::ReferenceTables, FirstLedger::Work),
                records,
                bytes,
            )
            .unwrap_err()
    }

    #[test]
    fn complete_mapping_copy_admission_keeps_work_limit_and_precedes_materialization() {
        let a = ApparentRepoName::new("apparent").unwrap();
        let c = CanonicalRepoName::new("canonical+").unwrap();
        let boundary = (RECORD_LIMIT - 1) / 3;
        admit_copy(std::iter::repeat_n((&a, &c), boundary)).unwrap();
        assert_eq!(
            admit_copy(std::iter::repeat_n((&a, &c), boundary + 1)).unwrap_err(),
            expected_copy_refusal(boundary + 1, false)
        );
        let inspected = std::cell::Cell::new(0);
        let pairs = std::iter::repeat_n((&a, &c), RECORD_LIMIT).inspect(|_| {
            inspected.set(inspected.get() + 1);
        });
        assert_eq!(
            admit_copy(pairs).unwrap_err(),
            expected_copy_refusal(RECORD_LIMIT, true)
        );
        assert_eq!(
            inspected.get(),
            0,
            "fixed copy peak refuses before reading names"
        );
        assert_eq!(a.as_str(), "apparent");
        assert_eq!(c.as_str(), "canonical+");
    }
}
