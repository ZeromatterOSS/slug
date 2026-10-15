//! Isolated counter/kernel oracles; genuine consuming-builder cases live in
//! checked_draft tests. These helpers do not mint a publication certificate.
use super::*;

fn audit<B: Backing>(
    rows: &[Option<PublishedNode>],
    edges: &[PublishedEdge],
    slots: &[Option<PublishedRootSlot>],
    owners: usize,
    roots: usize,
    work: &mut usize,
    bytes: &mut usize,
    scratch: &mut usize,
    failed: &mut bool,
    phase: &mut CertificationPhase,
) -> Result<(LocalRetainedFacts, Vec<CallableSourceAuthority>), PublishedValueError> {
    local_visits_with::<B>(
        Draft {
            rows,
            edges,
            imports: &[],
        },
        slots,
        owners,
        roots,
        work,
        bytes,
        scratch,
        failed,
        phase,
    )
}

#[test]
fn raw_and_checked_backing_keep_full_facts_and_positive_work_bytes() {
    let rows = [Some(PublishedNode::Cell { contents: None })];
    let edge = PublishedEdge::Local(PublishedNodeId::new(0).unwrap());
    for count in [0, 1, 17, 4096] {
        let edges = vec![edge; count];
        let mut raw_work = 0;
        let mut raw_bytes = 0;
        let mut raw_scratch = 0;
        let (raw, raw_sources) = local_visits(
            &rows,
            &edges,
            &[None],
            &[],
            0,
            0,
            &mut raw_work,
            &mut raw_bytes,
            &mut raw_scratch,
            &mut false,
        )
        .unwrap();
        let mut work = 0;
        let mut bytes = 0;
        let mut scratch = 0;
        prepare_checked_draft(&mut work, &mut bytes, &mut scratch, &mut false).unwrap();
        let mut phase = CertificationPhase::ViewPrepay;
        let (checked, sources) = audit::<Checked>(
            &rows,
            &edges,
            &[None],
            0,
            0,
            &mut work,
            &mut bytes,
            &mut scratch,
            &mut false,
            &mut phase,
        )
        .unwrap();
        assert_eq!((raw.total, raw.source, raw.native), (count + 2, 0, 0));
        assert_eq!(
            (checked.total, checked.source, checked.native),
            (raw.total, raw.source, raw.native)
        );
        assert!(raw_sources.is_empty() && sources.is_empty());
        assert_eq!(raw_work, count + 3);
        assert_eq!(work, 12, "8 view + setup/row/backing/slot");
        let envelope = 2 * std::mem::size_of::<CheckedDraft<'_>>()
            + std::mem::size_of::<Draft<'_>>()
            + 8 * std::mem::size_of::<usize>();
        assert_eq!(scratch, raw_scratch + envelope);
        assert_eq!(
            bytes,
            raw_bytes + envelope + 2 * std::mem::size_of::<usize>()
        );
        assert_eq!(phase, CertificationPhase::SourceCount);
    }
}

#[path = "tests/bounds.rs"]
mod bounds;
#[path = "tests/phases.rs"]
mod phases;

#[test]
fn consuming_slot_count_matches_full_raw_facts_with_exact_positive_deltas() {
    for count in [0usize, 1, 17, 4096] {
        for some in [false, true] {
            let roots = usize::from(some);
            let rows = (0..count)
                .map(|_| Some(PublishedNode::Cell { contents: None }))
                .collect::<Vec<_>>();
            let mut slots = Vec::with_capacity(count + 33);
            slots.extend(
                (0..count)
                    .map(|n| (some && n % 2 == 0).then(|| PublishedRootSlot::new(0).unwrap())),
            );
            assert!(slots.capacity() > slots.len());
            let mut raw_work = 0;
            let mut raw_bytes = 0;
            let mut raw_scratch = 0;
            let (raw, raw_sources) = audit::<Raw>(
                &rows,
                &[],
                &slots,
                0,
                roots,
                &mut raw_work,
                &mut raw_bytes,
                &mut raw_scratch,
                &mut false,
                &mut CertificationPhase::ViewPrepay,
            )
            .unwrap();
            let mut work = 0;
            let mut bytes = 0;
            let mut scratch = 0;
            let (checked, sources) = audit::<Checked>(
                &rows,
                &[],
                &slots,
                0,
                roots,
                &mut work,
                &mut bytes,
                &mut scratch,
                &mut false,
                &mut CertificationPhase::ViewPrepay,
            )
            .unwrap();
            assert_eq!(
                (raw.total, raw.source, raw.native),
                (2 * count + roots, 0, 0)
            );
            assert_eq!(
                (checked.total, checked.source, checked.native),
                (raw.total, raw.source, raw.native)
            );
            assert!(raw_sources.is_empty() && sources.is_empty());
            assert_eq!(raw_work, 1 + 2 * count + roots);
            assert_eq!(work, 3 + count + roots);
            // Empty edge count adds1; slot count independently changes1-R.
            assert_eq!(work + count, raw_work + 2);
            assert_eq!(bytes, raw_bytes + 2 * std::mem::size_of::<usize>());
            assert_eq!(scratch, raw_scratch);
        }
    }
}
