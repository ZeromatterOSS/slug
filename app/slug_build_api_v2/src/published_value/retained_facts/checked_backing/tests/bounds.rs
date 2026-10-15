use super::*;

#[test]
fn view_prepay_and_full_count_boundaries_poison_before_reads() {
    assert_eq!(
        std::mem::size_of::<CheckedDraft<'_>>(),
        6 * std::mem::size_of::<usize>()
    );
    assert_eq!(
        std::mem::size_of::<Draft<'_>>(),
        std::mem::size_of::<CheckedDraft<'_>>()
    );
    let envelope = 3 * std::mem::size_of::<CheckedDraft<'_>>() + 8 * std::mem::size_of::<usize>();
    for remaining in [8, 7] {
        let mut work = NODE_LIMIT - remaining;
        let mut bytes = 0;
        let mut scratch = 0;
        let mut failed = false;
        let result = prepare_checked_draft(&mut work, &mut bytes, &mut scratch, &mut failed);
        assert_eq!(result.is_ok(), remaining == 8);
        assert_eq!(work, NODE_LIMIT);
        assert_eq!(bytes, if remaining == 8 { envelope } else { 0 });
        assert_eq!(scratch, bytes);
        assert_eq!(failed, remaining != 8);
    }
    for available in [envelope, envelope - 1] {
        let mut work = 0;
        let mut bytes = BYTE_LIMIT - available;
        let before = bytes;
        let mut scratch = 0;
        let mut failed = false;
        let result = prepare_checked_draft(&mut work, &mut bytes, &mut scratch, &mut failed);
        assert_eq!(result.is_ok(), available == envelope);
        assert_eq!(work, 8);
        assert_eq!(
            bytes,
            if available == envelope {
                BYTE_LIMIT
            } else {
                before
            }
        );
        assert_eq!(scratch, if available == envelope { envelope } else { 0 });
        assert_eq!(failed, available != envelope);
    }
    for remaining in [3, 2] {
        let mut work = 0;
        let mut bytes = 0;
        let mut scratch = 0;
        let mut failed = false;
        let mut meter = Meter {
            work: &mut work,
            bytes: &mut bytes,
            scratch: &mut scratch,
            failed: &mut failed,
            retained: RETAINED_FACT_LIMIT - remaining,
            source: 0,
            native: 0,
        };
        let before = meter.retained;
        assert_eq!(meter.count_records(&[(); 3]).is_ok(), remaining == 3);
        assert_eq!(
            meter.retained,
            if remaining == 3 {
                RETAINED_FACT_LIMIT
            } else {
                before
            }
        );
        assert_eq!(*meter.work, 1);
        assert_eq!(*meter.bytes, std::mem::size_of::<usize>());
        assert_eq!(*meter.failed, remaining != 3);
        if remaining != 3 {
            assert!(matches!(
                meter.count_records(&[] as &[()]),
                Err(PublishedValueError::Limit("retained certificate work"))
            ));
            assert_eq!(*meter.work, 1);
        }
    }
}

#[test]
fn slot_scalar_work_bytes_and_full_facts_admit_exactly_before_length_add() {
    let slots = [None; 17];
    let word = std::mem::size_of::<usize>();
    // Isolated scalar primitive, not a certificate or a genuine quota witness.
    // Every allowance is installed before its one actual work/bytes/full add.
    for remaining in [1, 0] {
        let mut work = NODE_LIMIT - remaining;
        let mut bytes = 0;
        let mut scratch = 0;
        let mut failed = false;
        let mut meter = Meter {
            work: &mut work,
            bytes: &mut bytes,
            scratch: &mut scratch,
            failed: &mut failed,
            retained: 0,
            source: 0,
            native: 0,
        };
        let result = Checked::slots(&slots, 0, &mut meter);
        assert_eq!(result.is_ok(), remaining == 1);
        assert_eq!(*meter.work, NODE_LIMIT);
        assert_eq!(*meter.bytes, if remaining == 1 { word } else { 0 });
        assert_eq!(meter.retained, if remaining == 1 { slots.len() } else { 0 });
        assert_eq!(*meter.scratch, 0);
        assert_eq!(*meter.failed, remaining == 0);
    }
    for available in [word, word - 1] {
        let mut work = 0;
        let mut bytes = BYTE_LIMIT - available;
        let before = bytes;
        let mut scratch = 0;
        let mut failed = false;
        let mut meter = Meter {
            work: &mut work,
            bytes: &mut bytes,
            scratch: &mut scratch,
            failed: &mut failed,
            retained: 0,
            source: 0,
            native: 0,
        };
        let result = Checked::slots(&slots, 0, &mut meter);
        assert_eq!(result.is_ok(), available == word);
        assert_eq!(*meter.work, 1);
        assert_eq!(
            *meter.bytes,
            if available == word {
                BYTE_LIMIT
            } else {
                before
            }
        );
        assert_eq!(
            meter.retained,
            if available == word { slots.len() } else { 0 }
        );
        assert_eq!(*meter.failed, available != word);
    }
    for before in [
        RETAINED_FACT_LIMIT - 17,
        RETAINED_FACT_LIMIT - 16,
        usize::MAX - 16,
    ] {
        let mut work = 0;
        let mut bytes = 0;
        let mut scratch = 0;
        let mut failed = false;
        let mut meter = Meter {
            work: &mut work,
            bytes: &mut bytes,
            scratch: &mut scratch,
            failed: &mut failed,
            retained: before,
            source: 0,
            native: 0,
        };
        let result = Checked::slots(&slots, 0, &mut meter);
        let ok = before == RETAINED_FACT_LIMIT - 17;
        assert_eq!(result.is_ok(), ok);
        if !ok {
            assert!(matches!(
                result,
                Err(PublishedValueError::Limit("retained facts"))
            ));
        }
        assert_eq!(
            meter.retained,
            if ok { RETAINED_FACT_LIMIT } else { before }
        );
        assert_eq!((*meter.work, *meter.bytes, *meter.scratch), (1, word, 0));
        assert_eq!(*meter.failed, !ok);
        if !ok {
            assert!(matches!(
                Checked::slots(&[], 0, &mut meter),
                Err(PublishedValueError::Limit("retained certificate work"))
            ));
            assert_eq!(
                (*meter.work, *meter.bytes, meter.retained),
                (1, word, before)
            );
        }
    }
    let mut work = 0;
    let mut bytes = usize::MAX;
    let mut scratch = 0;
    let mut failed = false;
    let mut meter = Meter {
        work: &mut work,
        bytes: &mut bytes,
        scratch: &mut scratch,
        failed: &mut failed,
        retained: 7,
        source: 0,
        native: 0,
    };
    assert!(matches!(
        Checked::slots(&slots, 0, &mut meter),
        Err(PublishedValueError::Limit("retained certificate bytes"))
    ));
    assert_eq!(
        (*meter.work, *meter.bytes, meter.retained),
        (1, usize::MAX, 7)
    );
    assert!(*meter.failed);
    let (mut work, mut bytes, mut scratch, mut failed) = (0, 0, 0, false);
    let mut meter = Meter {
        work: &mut work,
        bytes: &mut bytes,
        scratch: &mut scratch,
        failed: &mut failed,
        retained: 0,
        source: 0,
        native: 0,
    };
    Checked::slots(&[], 0, &mut meter).unwrap();
    assert_eq!((*meter.work, *meter.bytes, meter.retained), (1, word, 0));
    assert!(
        !*meter.failed,
        "empty slots pay positive scalar work and retained word"
    );
}
