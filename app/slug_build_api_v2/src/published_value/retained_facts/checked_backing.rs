//! A construction-bound backing count; arbitrary drafts retain full validation.
use super::super::checked_draft::CheckedDraft;
use super::*;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::published_value::builder) enum CertificationPhase {
    ViewPrepay,
    SourceSetup,
    Rows,
    BackingEdges,
    RootSlots,
    StoredReferences,
    SourceCount,
}
impl CertificationPhase {
    pub(in crate::published_value::builder) fn label(self) -> &'static str {
        match self {
            Self::ViewPrepay => "viewPrepay",
            Self::SourceSetup => "sourceSetup",
            Self::Rows => "rows",
            Self::BackingEdges => "backingEdges",
            Self::RootSlots => "rootSlots",
            Self::StoredReferences => "storedReferences",
            Self::SourceCount => "sourceCount",
        }
    }
}

/// Three parent-field borrows, three accessor reads, entry control and Draft
/// assembly. The two view copies, resulting Draft and eight words cover their
/// simultaneous borrowed/control stack storage, not a retained allocation.
pub(in crate::published_value::builder) fn prepare_checked_draft(
    work: &mut usize,
    bytes: &mut usize,
    scratch: &mut usize,
    failed: &mut bool,
) -> Result<(), PublishedValueError> {
    let mut meter = Meter {
        work,
        bytes,
        scratch,
        failed,
        retained: 0,
        source: 0,
        native: 0,
    };
    for _ in 0..8 {
        meter.work()?;
    }
    meter.byte_parts(
        0,
        2 * std::mem::size_of::<CheckedDraft<'_>>()
            + std::mem::size_of::<Draft<'_>>()
            + 8 * std::mem::size_of::<usize>(),
    )
}

pub(super) trait Backing {
    fn count(draft: &Draft<'_>, meter: &mut Meter<'_>) -> Result<(), PublishedValueError>;
    fn slots(
        slots: &[Option<PublishedRootSlot>],
        root_count: usize,
        meter: &mut Meter<'_>,
    ) -> Result<(), PublishedValueError>;
}
pub(super) struct Raw;
impl Backing for Raw {
    fn count(draft: &Draft<'_>, meter: &mut Meter<'_>) -> Result<(), PublishedValueError> {
        for edge in draft.edges {
            draft.edge(*edge, meter)?;
        }
        Ok(())
    }
    fn slots(
        slots: &[Option<PublishedRootSlot>],
        root_count: usize,
        meter: &mut Meter<'_>,
    ) -> Result<(), PublishedValueError> {
        for slot in slots {
            meter.record()?;
            if slot.is_some_and(|slot| slot.index() >= root_count) {
                return Err(PublishedValueError::Invalid("original slot"));
            }
        }
        Ok(())
    }
}
struct Checked;
impl Backing for Checked {
    fn count(draft: &Draft<'_>, meter: &mut Meter<'_>) -> Result<(), PublishedValueError> {
        // The consumed same-builder view alone admits this path. Append already
        // checked each copied edge; row/import domains only grow. This removes
        // the second index scan, not facts or work that is still performed.
        meter.count_records(draft.edges)
    }
    fn slots(
        slots: &[Option<PublishedRootSlot>],
        _root_count: usize,
        meter: &mut Meter<'_>,
    ) -> Result<(), PublishedValueError> {
        // Consuming seal retains every slot fact now and validates its original
        // range and required presence in the unchanged final validator later.
        // The edge construction check supplies no slot-validity authority.
        meter.count_records(slots)
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::published_value::builder) fn checked_local_visits(
    view: CheckedDraft<'_>,
    slots: &[Option<PublishedRootSlot>],
    owner_count: usize,
    root_count: usize,
    work: &mut usize,
    bytes: &mut usize,
    scratch: &mut usize,
    failed: &mut bool,
    phase: &mut CertificationPhase,
) -> Result<(LocalRetainedFacts, Vec<CallableSourceAuthority>), PublishedValueError> {
    // Parent prepay precedes minting the view and these three borrowed reads.
    let draft = Draft {
        rows: view.rows(),
        edges: view.edges(),
        imports: view.imports(),
    };
    local_visits_with::<Checked>(
        draft,
        slots,
        owner_count,
        root_count,
        work,
        bytes,
        scratch,
        failed,
        phase,
    )
}

#[cfg(test)]
#[path = "checked_backing/tests.rs"]
mod tests;
