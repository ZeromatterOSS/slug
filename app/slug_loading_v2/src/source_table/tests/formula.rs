use super::*;
#[test]
fn full_certificate_matches_independent_typed_fields_and_actual_raw_order() {
    let raw = rows();
    let (intrinsic, full, union) = super::oracle::shapes(&raw);
    let mut work = Budget::default();
    let table = certify(raw.clone(), &mut work);
    assert_eq!(
        size_of::<CertifiedSourceTable>(),
        size_of::<Arc<SourceIdentityTable>>() + size_of::<SharedPublicationInput>()
    );
    let prior = table.descriptor_strong_count();
    let captured = table.capture(&mut Budget::default()).unwrap();
    assert_eq!(table.descriptor_strong_count(), prior + 2);
    drop(captured);
    assert_eq!(table.descriptor_strong_count(), prior);
    let root = &table.declarations()[0];
    assert_eq!(
        super::oracle::fields(root.intrinsic_shape()),
        super::oracle::fields(intrinsic)
    );
    assert_eq!(
        super::oracle::fields(root.retained_shape()),
        super::oracle::fields(full)
    );
    let mut audit = slug_build_api_v2::published_value::PublishedValueArenaBuilder::new();
    let before = audit.local_shape();
    audit.audit_shared_input_at_depth(root, 1).unwrap();
    let actual = audit.local_shape();
    assert_eq!(
        actual.visits - before.visits,
        union.visits + 2 * (1 + raw.len())
    );
    assert!(actual.bytes >= union.bytes);
    assert_eq!(actual.depth, 6);
    assert!(Arc::ptr_eq(table.raw(), &raw));
    assert_eq!(table.raw()[0].1.source_digest, [7; 32]);
    assert_eq!(
        table.raw()[0].1.identity.repository_mapping[1].0.as_str(),
        "second_long"
    );
    // Independent operation count: old stream plus new bank, positive local
    // certificate, complete leaf and cached-fold/read/transfer work.
    let rows = raw.len();
    let mappings = raw
        .iter()
        .map(|(_, p)| p.identity.repository_mapping.len())
        .sum::<usize>();
    // Per unique pair: init2 + positive scalar facts2 + scalar transfers8.
    // First-entry preparation adds4 per mapping; the raw general path is full.
    assert_eq!(work.records, 45 + 42 * rows + 34 + 14 * mappings);
}
#[test]
fn quota_exact_and_plus_one_refuse_before_any_new_guard_clone() {
    use crate::bzl_module::semantic_evaluation::budget::BYTE_LIMIT;
    use crate::bzl_module::semantic_evaluation::budget::RECORD_LIMIT;
    let raw = rows();
    let baseline = Arc::strong_count(&raw);
    let mut measured = Budget::default();
    drop(certify(raw.clone(), &mut measured));
    assert_eq!(Arc::strong_count(&raw), baseline);
    for (r, b, success) in [
        (RECORD_LIMIT - measured.records, 0, true),
        (RECORD_LIMIT - measured.records + 1, 0, false),
        (0, BYTE_LIMIT - measured.bytes, true),
        (0, BYTE_LIMIT - measured.bytes + 1, false),
    ] {
        let mut work = Budget {
            records: r,
            bytes: b,
            depth: 0,
        };
        let prepared = SourceTableCapture::from_raw(raw.clone(), &mut work);
        assert_eq!(matches!(prepared, SourceTableCapture::Complete(_)), success);
        drop(prepared);
        assert_eq!(Arc::strong_count(&raw), baseline);
    }
    let mut poisoned = Budget {
        records: RECORD_LIMIT + 1,
        bytes: 0,
        depth: 0,
    };
    let result = SourceTableCapture::from_raw(raw.clone(), &mut poisoned);
    assert!(matches!(result, SourceTableCapture::Refused { .. }));
    let before = poisoned.records;
    assert!(result.certified(&mut poisoned).is_err());
    assert!(poisoned.records > before);
    assert!(Arc::ptr_eq(result.raw(), &raw));
}

#[test]
fn spare_filename_capacity_is_positive_full_bytes_without_changing_semantic_rows() {
    let plain = rows();
    let mut spare = CompactString::with_capacity(8192);
    spare.push_str(&plain[0].0);
    assert!(spare.capacity() > spare.len() + 1024);
    let raw: Rows = Arc::from([(spare, plain[0].1.clone())]);
    let a = certify(plain.clone(), &mut Budget::default());
    let b = certify(raw.clone(), &mut Budget::default());
    assert_eq!(a, b);
    let ashape = a.declarations()[0].retained_shape();
    let bshape = b.declarations()[0].retained_shape();
    assert_eq!(
        (ashape.visits, ashape.edges, ashape.nodes, ashape.depth),
        (bshape.visits, bshape.edges, bshape.nodes, bshape.depth)
    );
    assert_eq!(
        bshape.bytes - ashape.bytes,
        raw[0].0.capacity() - plain[0].0.capacity()
    );
}

#[test]
fn owned_identity_mapping_and_workspace_spare_capacity_remains_in_complete_shape() {
    let a = rows();
    let mut p = a[0].1.clone();
    let mut path = std::path::PathBuf::with_capacity(8192);
    path.push("/actual/defs.bzl");
    p.identity.source = slug_build_api_v2::ModuleSourceOrigin::Workspace(path);
    let mut apparent = String::with_capacity(4096);
    apparent.push_str("first");
    let mut canonical = String::with_capacity(4096);
    canonical.push_str("one+");
    p.identity.repository_mapping = Arc::from([
        (
            ApparentRepoName::new(apparent).unwrap(),
            CanonicalRepoName::new(canonical).unwrap(),
        ),
        a[0].1.identity.repository_mapping[1].clone(),
    ]);
    let b: Rows = Arc::from([(a[0].0.clone(), p)]);
    let left = certify(a, &mut Budget::default());
    let right = certify(b, &mut Budget::default());
    assert_eq!(left, right);
    let extra = right.raw()[0].1.identity.repository_mapping[0]
        .0
        .retained_capacity_bytes()
        - "first".len()
        + right.raw()[0].1.identity.repository_mapping[0]
            .1
            .retained_capacity_bytes()
        - "one+".len()
        + right.raw()[0]
            .1
            .identity
            .label
            .target()
            .retained_capacity_bytes()
        - "defs.bzl".len()
        + match &right.raw()[0].1.identity.source {
            slug_build_api_v2::ModuleSourceOrigin::Workspace(p) => {
                p.capacity() - p.as_os_str().len()
            }
            _ => unreachable!(),
        };
    assert_eq!(
        right.declarations()[0].retained_shape().bytes
            - left.declarations()[0].retained_shape().bytes,
        extra
    );
}
