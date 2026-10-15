use super::*;

fn create(builder: &mut PublishedValueArenaBuilder, kind: usize, borrowed: bool) -> usize {
    match kind {
        0 => {
            let value = AnalysisValue::string("complete native scalar");
            if borrowed {
                builder
                    .native_atom_borrowed(&value)
                    .unwrap()
                    .retained_visits()
            } else {
                builder.native_atom(value).unwrap().retained_visits()
            }
        }
        1 => {
            let value =
                crate::ProviderValue::TemplateVariableInfo(crate::TemplateVariableInfo::new(
                    [
                        (compact_str::CompactString::from("first"), "one".into()),
                        (compact_str::CompactString::from("second"), "two".into()),
                    ]
                    .into_iter()
                    .collect(),
                ));
            if borrowed {
                builder
                    .provider_value_atom_borrowed(&value)
                    .unwrap()
                    .retained_visits()
            } else {
                builder
                    .provider_value_atom(value)
                    .unwrap()
                    .retained_visits()
            }
        }
        _ => {
            let value = crate::FilesToRunProvider::empty();
            if borrowed {
                builder
                    .files_to_run_atom_borrowed(&value)
                    .unwrap()
                    .retained_visits()
            } else {
                builder.files_to_run_atom(value).unwrap().retained_visits()
            }
        }
    }
}

#[test]
fn six_pure_factories_cache_complete_counts_and_restore_strict_mode() {
    let mut prior = PublishedValueArenaBuilder::new();
    atom(&mut prior, AnalysisValue::integer(9));
    let prior = seal(prior, 1);
    for kind in 0..3 {
        let mut plain = PublishedValueArenaBuilder::new();
        let expected = create(&mut plain, kind, false);
        assert!(expected > 0);
        for borrowed in [false, true] {
            let mut history = PublishedValueArenaBuilder::new();
            for _ in 0..41 {
                history.charge_visit().unwrap();
            }
            history.import(prior.clone()).unwrap();
            history.audit_owner_shape(&evaluation_owner()).unwrap();
            history.owner_audit = true;
            assert_eq!(create(&mut history, kind, borrowed), expected);
            assert!(history.owner_audit);
        }
    }
    let mut b = PublishedValueArenaBuilder::new();
    let id = atom(&mut b, AnalysisValue::none());
    let origin = seal(b, 1).value(id).unwrap();
    let value = AnalysisValue::with_original_origin(AnalysisValue::none(), origin).unwrap();
    for borrowed in [false, true] {
        let mut b = PublishedValueArenaBuilder::new();
        b.owner_audit = true;
        let result = if borrowed {
            b.native_atom_borrowed(&value)
        } else {
            b.native_atom(value.clone())
        };
        assert!(result.is_err());
        assert!(b.owner_audit);
        assert!(
            b.imports().is_empty(),
            "strict atom cannot smuggle an origin arena"
        );
        assert!(b.charge_bytes(0).is_err());
        let provider = crate::ProviderValue::Occurrence(crate::ProviderOccurrence::empty(
            crate::ProviderIdentity::builtin("Unadmitted"),
        ));
        let mut b = PublishedValueArenaBuilder::new();
        b.owner_audit = true;
        let result = if borrowed {
            b.provider_value_atom_borrowed(&provider)
        } else {
            b.provider_value_atom(provider)
        };
        assert!(result.is_err());
        assert!(b.owner_audit);
        assert!(b.charge_bytes(0).is_err());
        let mut b = PublishedValueArenaBuilder::new();
        b.owner_audit = true;
        b.charge_bytes(BYTE_LIMIT).unwrap();
        let value = crate::FilesToRunProvider::empty();
        assert!(
            if borrowed {
                b.files_to_run_atom_borrowed(&value)
            } else {
                b.files_to_run_atom(value)
            }
            .is_err()
        );
        assert!(b.owner_audit);
        assert!(b.charge_bytes(0).is_err());
    }
}

fn feature_model(count: usize) -> crate::cc_features::CcToolchainFeatures {
    use crate::cc_features::*;
    CcToolchainFeatures::new(
        (0..count)
            .map(|i| CcFeature {
                name: format!("feature{i}").into(),
                enabled: false,
                flag_sets: Box::new([]),
                env_sets: Box::new([]),
                requires: Box::new([]),
                implies: Box::new([]),
                provides: Box::new([]),
            })
            .collect(),
        vec![],
        vec![],
        "tools",
        CcExecOs::Linux,
    )
    .unwrap()
}

#[test]
fn full_native_payload_counts_survive_sealing_and_preclone_failure() {
    let small = feature_model(1);
    let large = feature_model(128);
    let counts = [small, large.clone()].map(|model| {
        let mut b = PublishedValueArenaBuilder::new();
        let value = AnalysisValue::cc_toolchain_features(model);
        let pure = b.native_atom_borrowed(&value).unwrap();
        let native = pure.retained_visits();
        let id = b.reserve().unwrap();
        b.set_node(id, PublishedNode::Native(pure)).unwrap();
        b.set_original_slot(id, PublishedRootSlot::new(0).unwrap())
            .unwrap();
        let arena = seal(b, 1);
        assert_eq!(arena.0.local_shape.nodes, 1);
        assert!(arena.0.local_shape.visits > native);
        (native, arena.0.local_shape.visits)
    });
    assert!(counts[1].0 > counts[0].0 + 200);
    assert_eq!(counts[1].1 - counts[0].1, counts[1].0 - counts[0].0);
    // Exactly fourteen producer admissions remain for one native row:
    // view setup8, source setup, row, certificate read, empty backing, slot, root.
    // Rewalking the large native model would exhaust this shared allowance.
    let mut cached = PublishedValueArenaBuilder::new();
    atom(
        &mut cached,
        AnalysisValue::cc_toolchain_features(large.clone()),
    );
    let padding = NODE_LIMIT - 14 - cached.charged_visits();
    for _ in 0..padding {
        cached.charge_visit().unwrap();
    }
    let cached = seal(cached, 1);
    assert_eq!(cached.0.local_shape.visits, counts[1].1);
    let refs = Arc::strong_count(large.model());
    let value = AnalysisValue::cc_toolchain_features(large.clone());
    let refs_with_value = Arc::strong_count(large.model());
    let mut b = PublishedValueArenaBuilder::new();
    b.charge_bytes(BYTE_LIMIT).unwrap();
    assert!(b.native_atom_borrowed(&value).is_err());
    assert_eq!(Arc::strong_count(large.model()), refs_with_value);
    drop(value);
    assert_eq!(Arc::strong_count(large.model()), refs);
}
