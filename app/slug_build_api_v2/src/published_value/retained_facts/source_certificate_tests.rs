use super::*;

#[test]
fn one_paid_source_certificate_read_keeps_full_retained_count() {
    let authority = source(2048);
    let mut work = 0;
    let mut bytes = 0;
    let mut failed = false;
    let mut meter = Meter {
        work: &mut work,
        bytes: &mut bytes,
        scratch: &mut 0,
        failed: &mut failed,
        retained: 0,
    };
    let mut sources = sources::LocalSources::new(1, &mut meter).unwrap();
    let table = 2 * (4 * (std::mem::size_of::<usize>() + 1) + 32);
    assert_eq!(*meter.scratch, table);
    assert_eq!(
        *meter.bytes - *meter.scratch,
        std::mem::size_of::<fxhash::FxHashSet<usize>>()
            + 3 * std::mem::size_of::<CallableSourceAuthority>(),
        "fixed header and all source handles remain retained"
    );
    // Exactly two remaining operations: incoming lookup and certificate read.
    *meter.work = NODE_LIMIT - 2;
    let scratch_bytes = *meter.bytes;
    sources.observe(&authority, &mut meter).unwrap();
    assert_eq!(*meter.work, NODE_LIMIT);
    assert_eq!(*meter.bytes - scratch_bytes, std::mem::size_of::<usize>());
    assert_eq!(
        *meter.scratch, table,
        "certificate and payload facts are not scratch"
    );
    assert_eq!(
        meter.retained,
        2 * (2048 + 1),
        "full original source payload, not the scalar work count"
    );
    assert_eq!(
        sources.values()[0].storage_identity(),
        authority.storage_identity()
    );
    assert!(!*meter.failed);
}

#[test]
fn source_read_refusal_precedes_guard_clone_and_poisons_reuse() {
    let authority = source(4);
    // Independent work and byte refusal, after source-table scratch preflight.
    for bytes_limited in [false, true] {
        let mut work = 0;
        let mut bytes = 0;
        let mut failed = false;
        let mut meter = Meter {
            work: &mut work,
            bytes: &mut bytes,
            scratch: &mut 0,
            failed: &mut failed,
            retained: 0,
        };
        let mut sources = sources::LocalSources::new(1, &mut meter).unwrap();
        if bytes_limited {
            *meter.bytes = BYTE_LIMIT - std::mem::size_of::<usize>() + 1;
        } else {
            *meter.work = NODE_LIMIT - 1;
        }
        assert!(sources.observe(&authority, &mut meter).is_err());
        assert!(*meter.failed);
        assert_eq!(meter.retained, 0);
        assert!(
            sources.values().is_empty(),
            "failed read cannot retain a new source guard"
        );
        assert!(sources.observe(&authority, &mut meter).is_err());
        assert!(sources.values().is_empty());
    }
}

#[test]
fn sealed_source_count_survives_scalar_reuse_with_independent_import_work() {
    let authority = source(2048);
    let mut builder = PublishedValueArenaBuilder::new();
    code(&mut builder, authority.clone());
    let prior = seal(builder, 0);
    assert_eq!(prior.sources().len(), 1);
    assert_eq!(prior.sources()[0], authority);
    let full_facts = prior.0.local_shape.visits;
    assert_eq!(full_facts, authority.fact_count() + 4);
    for remaining in [3, 2] {
        let mut imported = PublishedValueArenaBuilder::new();
        for _ in 0..NODE_LIMIT - remaining {
            imported.charge_visit().unwrap();
        }
        let result = imported.import(prior.clone());
        assert_eq!(result.is_ok(), remaining == 3);
        assert_eq!(imported.charged_visits(), NODE_LIMIT);
        assert_eq!(imported.import_visits, full_facts);
        assert_eq!(imported.sources[0], authority);
        if remaining == 3 {
            assert_eq!(
                imported.complete_shape().unwrap().visits,
                NODE_LIMIT + full_facts
            );
        } else {
            assert!(imported.imports.is_empty());
            assert!(imported.charge_bytes(0).is_err());
        }
    }
}

#[test]
fn complete_large_source_seals_with_exact_scalar_read_work_allowance() {
    for (remaining, succeeds) in [(7, true), (6, false)] {
        let authority = source(2048);
        let mut builder = PublishedValueArenaBuilder::new();
        code(&mut builder, authority.clone());
        while builder.charged_visits() < NODE_LIMIT - remaining {
            builder.charge_visit().unwrap();
        }
        let result = builder.seal(Arc::new(Originals {
            count: 0,
            imports: vec![],
        }));
        assert_eq!(result.is_ok(), succeeds, "remaining seal work {remaining}");
        if let Ok(arena) = result {
            assert_eq!(arena.sources()[0], authority);
            assert_eq!(arena.0.local_shape.visits, authority.fact_count() + 4);
            assert_eq!(arena.shape().visits, arena.0.local_shape.visits);
        }
    }
}
