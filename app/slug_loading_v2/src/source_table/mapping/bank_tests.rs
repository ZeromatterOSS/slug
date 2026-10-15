use super::*;
#[test]
fn scratch_envelope_covers_actual_map_entry_ordinal_and_transfer_arrays() {
    assert!(
        std::mem::size_of::<(Entry<'_>, bool)>() >= 4 * std::mem::size_of::<Option<MappingPair>>()
    );
    for count in [0, 1, 2, 3, 4, 17, 65] {
        let mut work = Budget::default();
        let mut bank = Bank::new(count, count, &mut work).unwrap();
        assert_eq!(work.records, 8);
        let map = crate::bzl_environment::references::descriptor_memo_scratch_bytes(count).unwrap();
        let slots = std::mem::size_of::<(Entry<'_>, bool)>() * count;
        let ordinals = std::mem::size_of::<usize>() * count;
        let pairs = std::mem::size_of::<Option<MappingPair>>() * count;
        let transfer = if count == 0 {
            0
        } else {
            (count + 2 * count.max(4)) * std::mem::size_of::<Option<MappingPair>>()
        };
        assert!(
            2 * slots + 5 * pairs + 12 * std::mem::size_of::<usize>()
                >= transfer
                    + 3 * std::mem::size_of::<Vec<Option<MappingPair>>>()
                    + 2 * std::mem::size_of::<usize>()
        );
        assert_eq!(
            work.bytes,
            std::mem::size_of::<Bank<'_>>()
                + map
                + 2 * slots
                + 2 * ordinals
                + 5 * pairs
                + 12 * std::mem::size_of::<usize>()
        );
        let raws = (0..count)
            .map(|_| {
                let raw: Raw = Arc::from([]);
                raw
            })
            .collect::<Vec<_>>();
        for raw in &raws {
            bank.select(raw, &mut work).unwrap();
        }
        assert!(
            map >= std::mem::size_of_val(&bank.index)
                + 2 * bank.index.retained_storage_bytes_upper_bound().unwrap()
        );
        assert!(slots >= bank.entries.capacity() * std::mem::size_of::<(Entry<'_>, bool)>());
        assert!(ordinals >= bank.uses.capacity() * std::mem::size_of::<usize>());
        assert_eq!(bank.used, count);
    }
    assert!(Bank::new(usize::MAX, usize::MAX, &mut Budget::default()).is_err());
}

fn complete_copy_pair(raw: &Raw) -> MappingPair {
    let mut work = Budget::default();
    let mut bank = Bank::new(1, 1, &mut work).unwrap();
    bank.select(raw, &mut work).unwrap();
    for (a, c) in &**raw {
        bank.entry(a, c, &mut work).unwrap();
    }
    bank.prepay_finish(PublicationShape::default(), &mut work)
        .unwrap();
    let (pairs, _) = bank.finish().unwrap();
    pairs[0].clone()
}
fn legacy_copy_volume(raw: &Raw) -> (usize, usize) {
    (
        raw.len(),
        raw.iter()
            .map(|(a, c)| {
                std::mem::size_of::<(ApparentRepoName, CanonicalRepoName)>()
                    + a.as_str().len()
                    + c.as_str().len()
            })
            .sum(),
    )
}
fn copy_lookup_bytes() -> usize {
    std::mem::size_of::<(
        &Raw,
        usize,
        Option<&usize>,
        &(Entry<'_>, bool),
        Option<(usize, usize)>,
    )>()
}
#[test]
fn completed_copy_volume_is_positive_legacy_volume_and_only_actual_inherited_authority() {
    // Names preserve their real typed grammar; no forged Unicode repo names.
    assert!(ApparentRepoName::new("dép").is_err());
    assert!(CanonicalRepoName::new("dép+").is_err());
    let wide: Raw = (0..17)
        .map(|i| {
            (
                ApparentRepoName::new(format!("a{i}")).unwrap(),
                CanonicalRepoName::new(format!("c{i}+")).unwrap(),
            )
        })
        .collect::<Vec<_>>()
        .into();
    let mut a = String::with_capacity(4096);
    a.push_str("first");
    let mut c = String::with_capacity(8192);
    c.push_str("selected+");
    let spare: Raw = Arc::from([(
        ApparentRepoName::new(a).unwrap(),
        CanonicalRepoName::new(c).unwrap(),
    )]);
    for raw in [Arc::from([]), wide, spare] {
        assert_eq!(
            std::mem::size_of::<MappingPair>(),
            std::mem::size_of::<Raw>()
                + std::mem::size_of::<SharedPublicationInput>()
                + 2 * std::mem::size_of::<usize>()
        );
        let mut precursor = Budget::default();
        let mut prepared = Prepared::new(raw.len(), &mut precursor).unwrap();
        assert_eq!(precursor.records, 10);
        assert_eq!(
            precursor.bytes,
            std::mem::size_of::<Prepared>()
                + std::mem::size_of::<MappingPair>()
                + header_bytes()
                + 2 * std::mem::size_of::<usize>()
        );
        for (a, c) in &*raw {
            let before = precursor;
            prepared.entry(a, c, &mut precursor).unwrap();
            assert_eq!(precursor.records, before.records + 8);
            assert_eq!(
                precursor.bytes,
                before.bytes + 3 * std::mem::size_of::<usize>()
            );
        }
        drop(prepared);
        let pair = complete_copy_pair(&raw);
        let foreign: Raw = raw.iter().cloned().collect::<Vec<_>>().into();
        let foreign_pair = complete_copy_pair(&foreign);
        assert!(!Arc::ptr_eq(&raw, &foreign));
        let expected = legacy_copy_volume(&raw);
        let guards = Arc::strong_count(&raw);
        let mut work = Budget::default();
        let mut bank = Bank::new(2, 1, &mut work).unwrap();
        bank.inherit(&pair, &mut work).unwrap();
        let before = work;
        assert_eq!(bank.copy_volume(&raw, &mut work).unwrap(), Some(expected));
        assert_eq!(work.records, before.records + 8);
        assert_eq!(work.bytes, before.bytes + copy_lookup_bytes());
        assert_eq!(
            bank.copy_volume(&foreign, &mut work).unwrap(),
            None,
            "equal foreign cannot borrow sibling proof"
        );
        bank.inherit(&foreign_pair, &mut work).unwrap();
        assert_eq!(
            bank.copy_volume(&foreign, &mut work).unwrap(),
            Some(legacy_copy_volume(&foreign))
        );
        assert_eq!(Arc::strong_count(&raw), guards);
        let mut fresh = Bank::new(1, 1, &mut work).unwrap();
        assert_eq!(fresh.copy_volume(&raw, &mut work).unwrap(), None);
        fresh.select(&raw, &mut work).unwrap();
        for (a, c) in &*raw {
            fresh.entry(a, c, &mut work).unwrap();
        }
        // New remains unsealed authority even after every body entry succeeded,
        // including the empty case whose remaining was zero at construction.
        assert_eq!(fresh.copy_volume(&raw, &mut work).unwrap(), None);
        assert_eq!(Arc::strong_count(&raw), guards);
        if let Some((a, c)) = raw.first() {
            // One cumulative primitive allowance: init10 fits, entry8 refuses
            // at N+1 before decrement/native/cache commit. No pair can escape.
            let mut refused = Budget {
                records: crate::bzl_module::semantic_evaluation::budget::RECORD_LIMIT - 18 + 1,
                ..Budget::default()
            };
            let mut prepared = Prepared::new(raw.len(), &mut refused).unwrap();
            assert!(prepared.entry(a, c, &mut refused).is_err());
            assert_eq!(
                refused.records,
                crate::bzl_module::semantic_evaluation::budget::RECORD_LIMIT + 1
            );
            assert!(prepared.finish(&raw).is_err());
            assert_eq!(Arc::strong_count(&raw), guards);
        }
    }
}
#[test]
fn inherited_copy_lookup_uses_one_cumulative_preparation_ledger_at_exact_limits() {
    use crate::bzl_module::semantic_evaluation::budget::BYTE_LIMIT;
    use crate::bzl_module::semantic_evaluation::budget::RECORD_LIMIT;
    let raw: Raw = Arc::from([(
        ApparentRepoName::new("a").unwrap(),
        CanonicalRepoName::new("a+").unwrap(),
    )]);
    let pair = complete_copy_pair(&raw);
    let guards = Arc::strong_count(&raw);
    let mut measured = Budget::default();
    let mut bank = Bank::new(1, 1, &mut measured).unwrap();
    bank.inherit(&pair, &mut measured).unwrap();
    let prefix = measured;
    assert_eq!(
        bank.copy_volume(&raw, &mut measured).unwrap(),
        Some(legacy_copy_volume(&raw))
    );
    assert_eq!(measured.records, prefix.records + 8);
    assert_eq!(measured.bytes, prefix.bytes + copy_lookup_bytes());
    drop(bank);
    for bytes in [false, true] {
        for extra in [0, 1] {
            let mut work = if bytes {
                Budget {
                    bytes: BYTE_LIMIT - measured.bytes + extra,
                    ..Budget::default()
                }
            } else {
                Budget {
                    records: RECORD_LIMIT - measured.records + extra,
                    ..Budget::default()
                }
            };
            let mut bank = Bank::new(1, 1, &mut work).unwrap();
            bank.inherit(&pair, &mut work).unwrap();
            let result = bank.copy_volume(&raw, &mut work);
            assert_eq!(result.is_ok(), extra == 0);
            if extra == 0 {
                assert_eq!(result.unwrap(), Some(legacy_copy_volume(&raw)));
            }
            assert_eq!(
                work.records,
                if bytes {
                    measured.records
                } else {
                    RECORD_LIMIT + extra
                }
            );
            assert_eq!(
                work.bytes,
                if bytes {
                    BYTE_LIMIT + extra
                } else {
                    measured.bytes
                }
            );
            assert_eq!(Arc::strong_count(&raw), guards);
        }
    }
}
