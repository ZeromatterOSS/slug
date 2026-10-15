use super::*;

fn data() -> CallableSourceAuthorityData {
    let mapping: Mapping = (0..8)
        .map(|n| {
            (
                ApparentRepoName::new(format!("alias{n}")).unwrap(),
                CanonicalRepoName::new(format!("selected{n}+")).unwrap(),
            )
        })
        .collect::<Vec<_>>()
        .into();
    let module = CallableModuleIdentity {
        label: CanonicalLabel::parse("@@//pkg:source.bzl").unwrap(),
        source: ModuleSourceOrigin::Workspace("/workspace/pkg/source.bzl".into()),
        repository_mapping: mapping,
    };
    CallableSourceAuthorityData {
        root: module.clone(),
        direct_children: Arc::from([module.clone()]),
        environment_bindings: Arc::from([]),
        reachable: Arc::from([CallableModuleSource {
            identity: module,
            digest: [4; 32],
        }]),
        fingerprint: [5; 32],
    }
}
fn audit(data: CallableSourceAuthorityData) -> (CallableSourceAuthority, usize) {
    let count = identities(&data).count();
    let mut work = 0;
    let prepared = prepare(data, count, || {
        work += 1;
        Ok(())
    })
    .unwrap();
    (prepared.finish(), work)
}

#[test]
fn actual_mapping_reuse_keeps_full_count_bytes_and_final_fields() {
    let shared = data();
    let reference_bytes = CallableSourceAuthority::data_byte_cost(&shared);
    let (authority, work) = audit(shared.clone());
    assert_eq!(work, 24); // 3 + 4*3 + 8 pairs + 1 insertion
    assert_eq!(authority.fact_count(), 27);
    assert_eq!(authority.byte_cost(), reference_bytes);
    assert_eq!(
        authority,
        CallableSourceAuthority::new(shared.clone()).unwrap()
    );

    let mut distinct = shared.clone();
    let mut child = distinct.direct_children[0].clone();
    child.repository_mapping = child.repository_mapping.to_vec().into();
    distinct.direct_children = Arc::from([child]);
    let mut source = distinct.reachable[0].clone();
    source.identity.repository_mapping = source.identity.repository_mapping.to_vec().into();
    distinct.reachable = Arc::from([source]);
    let (foreign, foreign_work) = audit(distinct);
    assert_eq!(foreign_work, 42); // same M/F/Q, three genuine allocations
    assert_eq!(foreign, authority);
    assert_eq!(foreign.fact_count(), authority.fact_count());
    assert_eq!(foreign.byte_cost(), authority.byte_cost());

    let mut rows = shared.root.repository_mapping.to_vec();
    let old = rows[7].1.as_str().len();
    rows[7].1 = CanonicalRepoName::new("changed_last_repository+").unwrap();
    let delta = rows[7].1.as_str().len() - old;
    let mapping: Mapping = rows.into();
    let mut changed = shared;
    changed.root.repository_mapping = mapping.clone();
    let mut child = changed.direct_children[0].clone();
    child.repository_mapping = mapping.clone();
    changed.direct_children = Arc::from([child]);
    let mut source = changed.reachable[0].clone();
    source.identity.repository_mapping = mapping;
    changed.reachable = Arc::from([source]);
    let (changed, changed_work) = audit(changed);
    assert_eq!(changed_work, work);
    assert_eq!(changed.fact_count(), authority.fact_count());
    assert_eq!(changed.byte_cost() - authority.byte_cost(), 3 * delta);
    assert_ne!(changed, authority);
}

#[test]
fn binding_fields_and_complete_private_layout_match_ordinary_constructor() {
    let mut value = data();
    let mut global = CompactString::with_capacity(4096);
    global.push_str("effective_global");
    let mut export = CompactString::with_capacity(8192);
    export.push_str("effective_export");
    value.environment_bindings = Arc::from([CallableEnvironmentBinding {
        global,
        export,
        module: value.root.clone(),
    }]);
    let expected = CallableSourceAuthority::new(value.clone()).unwrap();
    let (actual, work) = audit(value);
    assert_eq!(work, 30); // 3 + 4*4 + 2 fields + 8 pairs + 1 insertion
    assert_eq!(actual.fact_count(), 38);
    assert_eq!(actual.byte_cost(), expected.byte_cost());
    assert!(actual.byte_cost() >= 12288 + CallableSourceAuthority::storage_overhead_bytes());
    assert_eq!(actual, expected);
}

#[test]
fn count_and_work_refusals_precede_their_operations_and_fact_limit_precedes_bytes() {
    assert!(scratch_bytes(0).is_err());
    assert!(scratch_bytes(CALLABLE_GRAPH_NODE_LIMIT + 1).is_err());
    assert!(scratch_bytes(usize::MAX).is_err());
    assert!(scratch_bytes(1).unwrap() >= size_of::<Memo<'_>>());
    assert!(scratch_bytes(5).unwrap() > scratch_bytes(4).unwrap());
    let value = data();
    let references = Arc::strong_count(&value.root.repository_mapping);
    let mut work = 0;
    let error = prepare(value.clone(), 2, || {
        work += 1;
        Ok(())
    })
    .err()
    .unwrap();
    assert_eq!(
        error,
        PublishedValueError::Invalid("source authority identity bound")
    );
    assert_eq!(work, 1);
    assert_eq!(
        Arc::strong_count(&value.root.repository_mapping),
        references
    );
    for allowance in [23, 24] {
        let mut paid = 0;
        let result = prepare(value.clone(), 3, || {
            if paid == allowance {
                return Err(PublishedValueError::Limit("visits"));
            }
            paid += 1;
            Ok(())
        });
        assert_eq!(result.is_ok(), allowance == 24);
        assert_eq!(paid, allowance);
    }
    let mut invalid = value;
    invalid.direct_children = Arc::from([]);
    invalid.reachable = Arc::from([]);
    invalid.root.repository_mapping = vec![
        (
            ApparentRepoName::new("alias").unwrap(),
            CanonicalRepoName::new("selected+").unwrap()
        );
        CALLABLE_GRAPH_NODE_LIMIT
    ]
    .into();
    let mut paid = 0;
    let error = prepare(invalid, 1, || {
        paid += 1;
        Ok(())
    })
    .err()
    .unwrap();
    assert_eq!(error, PublishedValueError::Limit("source facts"));
    assert_eq!(paid, 3, "no mapping row/byte pass after full-F refusal");
}

#[test]
fn full_source_byte_limit_is_not_discounted_by_actual_mapping_reuse() {
    let mut value = data();
    let mapping: Mapping = (0..64)
        .map(|n| {
            (
                ApparentRepoName::new(format!("{}_{n}", "a".repeat(2048))).unwrap(),
                CanonicalRepoName::new("b".repeat(2048)).unwrap(),
            )
        })
        .collect::<Vec<_>>()
        .into();
    value.root.repository_mapping = mapping;
    value.direct_children = vec![value.root.clone(); 256].into();
    value.reachable = Arc::from([]);
    assert_eq!(CallableSourceAuthority::data_fact_count(&value), 16705);
    assert!(CallableSourceAuthority::data_byte_cost(&value) > CALLABLE_GRAPH_BYTE_LIMIT);
    assert_eq!(
        CallableSourceAuthority::new(value.clone()).unwrap_err(),
        CallableGraphError::Limit("source bytes")
    );
    let mut work = 0;
    let error = prepare(value, 257, || {
        work += 1;
        Ok(())
    })
    .err()
    .unwrap();
    assert_eq!(error, PublishedValueError::Limit("source bytes"));
    assert_eq!(work, 1096); // 3 + 4*257 + 64 actual pairs + 1 insertion
}
