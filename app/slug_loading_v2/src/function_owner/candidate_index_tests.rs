//! Candidate cache affects only ephemeral indexing; semantic hashing stays full.
use super::*;

#[derive(Default)]
struct CountHasher {
    bytes: usize,
    writes: usize,
}
impl Hasher for CountHasher {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, bytes: &[u8]) {
        self.bytes += bytes.len();
        self.writes += 1;
    }
}
fn row(name: &str, source: &str, mappings: usize) -> Arc<LocalBzlInput> {
    let identity = BzlModuleIdentity {
        label: slug_identity_v2::CanonicalLabel::parse(&format!("@@source+//pkg:{name}.bzl"))
            .unwrap(),
        source: slug_build_api_v2::ModuleSourceOrigin::Workspace(
            format!("/source/{name}.bzl").into(),
        ),
        repository_mapping: (0..mappings)
            .map(|index| {
                (
                    slug_identity_v2::ApparentRepoName::new(format!("dep{index}")).unwrap(),
                    slug_identity_v2::CanonicalRepoName::new(format!("selected{index}+")).unwrap(),
                )
            })
            .collect::<Vec<_>>()
            .into(),
    };
    let manifest = BzlLoadManifest {
        root: identity.clone(),
        direct_children: Arc::from([]),
        reachable: Arc::from([crate::BzlModuleSourceProvenance::new(identity, [0; 32])]),
        fingerprint: [0; 32],
    };
    let environment = crate::bzl_environment::PreparedBzlEnvironment::uninjected(
        crate::bzl_environment::BzlEnvironmentMode::BzlmodBootstrap,
    )
    .unwrap();
    let context = crate::bzl_environment::BzlModuleSourceContext::new(
        &manifest,
        &environment,
        std::iter::empty(),
    )
    .unwrap();
    let owner = bzl_evaluation_owner(
        crate::starlark_label::PreparedDefiningModuleMetadata::RawManifest {
            manifest: &manifest,
            transient_context: &context,
        },
        CodeMap::new("index.bzl".into(), source.into()),
        &environment,
        true,
        [],
    )
    .unwrap();
    owner
        .input_validity()
        .as_any()
        .downcast_ref::<BzlInputValidity>()
        .unwrap()
        .rows[0]
        .clone()
}
fn cached_variant(row: &Arc<LocalBzlInput>, hash: u64) -> Arc<LocalBzlInput> {
    let key = Arc::new(BzlStableKey {
        identity: row.key.identity.clone(),
        environment: row.key.environment.clone(),
        bazel_internal_strings: row.key.bazel_internal_strings,
        candidate_index_hash: hash,
        shape: row.key.shape,
        comparison: row.key.comparison,
        publication: row.key.publication.clone(),
    });
    let publication = row.publication.with_test_children(
        std::iter::once(SharedPublicationInput::new(key.clone()))
            .chain(
                row.children
                    .iter()
                    .map(|child| SharedPublicationInput::new(child.clone())),
            )
            .collect(),
    );
    Arc::new(LocalBzlInput {
        key,
        source_digest: row.source_digest,
        fingerprint: row.fingerprint,
        source: row.source.clone(),
        original_metadata: row.original_metadata,
        children: row.children.clone(),
        shape: row.shape,
        comparison: row.comparison,
        publication,
    })
}
fn semantic_hash(key: &BzlStableKey) -> u64 {
    let mut hash = DefaultHasher::new();
    key.hash(&mut hash);
    hash.finish()
}
#[derive(Default)]
struct Union {
    rows: Vec<Arc<LocalBzlInput>>,
    index: BTreeMap<u64, Vec<usize>>,
    occurrences: BTreeSet<usize>,
    retained: PublicationShape,
    intrinsic: PublicationShape,
    work: PublicationShape,
}
impl Union {
    fn insert(&mut self, row: &Arc<LocalBzlInput>) -> anyhow::Result<()> {
        insert_local_row(
            row,
            &mut self.rows,
            &mut self.index,
            &mut self.occurrences,
            &mut self.retained,
            &mut self.intrinsic,
            &mut self.work,
        )
    }
}
#[test]
fn candidate_index_cache_preserves_complete_semantic_hash_and_equality() {
    let original = row("same", "full source", 12);
    let altered = cached_variant(&original, original.key.candidate_index_hash ^ u64::MAX);
    assert_eq!(original.key, altered.key);
    assert_eq!(original, altered);
    assert_eq!(semantic_hash(&original.key), semantic_hash(&altered.key));
    let mut expected = DefaultHasher::new();
    original.key.identity.hash(&mut expected);
    original.key.environment.hash(&mut expected);
    original.key.bazel_internal_strings.hash(&mut expected);
    assert_eq!(semantic_hash(&original.key), expected.finish());
    assert_eq!(original.key.candidate_index_hash, expected.finish());
    let mut actual = CountHasher::default();
    original.key.hash(&mut actual);
    let mut old_fields = CountHasher::default();
    original.key.identity.hash(&mut old_fields);
    original.key.environment.hash(&mut old_fields);
    original.key.bazel_internal_strings.hash(&mut old_fields);
    assert_eq!(
        (actual.writes, actual.bytes),
        (old_fields.writes, old_fields.bytes)
    );
    assert_ne!(original.key, row("other", "full source", 12).key);
}
#[test]
fn candidate_index_collisions_compare_complete_keys_and_source_facts() {
    let a = cached_variant(&row("first", "same source", 3), 17);
    let b = cached_variant(&row("other", "same source", 3), 17);
    let conflict = cached_variant(&row("first", "changed source", 3), 17);
    let mut union = Union::default();
    union.insert(&a).unwrap();
    union.insert(&b).unwrap();
    assert_eq!(union.rows.len(), 2);
    assert_eq!(union.index[&17], [0, 1]);
    let before = (union.rows.len(), union.index.len(), union.occurrences.len());
    // Identical integrity digests cannot hide differing complete source bytes.
    assert_eq!(a.source_digest, conflict.source_digest);
    assert!(
        union
            .insert(&conflict)
            .unwrap_err()
            .to_string()
            .contains("conflicting facts")
    );
    assert_eq!(
        (union.rows.len(), union.index.len(), union.occurrences.len()),
        before
    );

    let mut limited = Union::default();
    limited.insert(&a).unwrap();
    limited.work.visits = 65536 - 2; // occurrence+cache read fit; full comparison does not.
    assert!(
        limited
            .insert(&b)
            .unwrap_err()
            .to_string()
            .contains("candidate-key comparison")
    );
    assert_eq!(limited.rows.len(), 1);
    assert_eq!(limited.index[&17], [0]);
}
fn shape_fields(shape: PublicationShape) -> (usize, usize, usize, usize, usize) {
    (
        shape.nodes,
        shape.visits,
        shape.edges,
        shape.bytes,
        shape.depth,
    )
}
#[test]
fn candidate_index_full_hash_is_paid_once_and_imports_pay_fixed_reads() {
    let narrow = row("narrow", "code", 0);
    let wide = row("wide", "code", 40);
    let mut hash = CountHasher::default();
    let mut construction = zero_shape();
    write_candidate_index_hash(
        &wide.key.identity,
        &wide.key.environment,
        wide.key.bazel_internal_strings,
        wide.key.shape,
        &mut construction,
        &mut hash,
    )
    .unwrap();
    assert_eq!(construction.visits, wide.key.shape.visits);
    let writes_once = (hash.writes, hash.bytes);
    assert!(hash.writes > 40);
    let mut import_work = None;
    for _ in 0..64 {
        let mut a = Union::default();
        let mut b = Union::default();
        a.insert(&narrow).unwrap();
        b.insert(&wide).unwrap();
        // Imported candidate lookup has fixed work/bytes, irrespective of the
        // full mapping length already hashed by its actual immutable producer.
        assert_eq!(shape_fields(a.work), shape_fields(b.work));
        assert!(b.work.visits < wide.key.shape.visits);
        assert_eq!((hash.writes, hash.bytes), writes_once);
        if let Some(previous) = import_work {
            assert_eq!(previous, shape_fields(b.work));
        }
        import_work = Some(shape_fields(b.work));
    }
}
#[test]
fn candidate_index_limit_refuses_before_hash_writes_key_clones_and_index_growth() {
    let original = row("limited", "source", 8);
    for bytes in [false, true] {
        let mut work = zero_shape();
        if bytes {
            work.bytes = 64 * 1024 * 1024 - original.key.shape.bytes + 1;
        } else {
            work.visits = 65536 - original.key.shape.visits + 1;
        }
        let mapping_refs = Arc::strong_count(&original.key.identity.repository_mapping);
        let key_refs = Arc::strong_count(&original.key);
        let mut hash = CountHasher::default();
        assert!(
            write_candidate_index_hash(
                &original.key.identity,
                &original.key.environment,
                original.key.bazel_internal_strings,
                original.key.shape,
                &mut work,
                &mut hash
            )
            .is_err()
        );
        assert_eq!((hash.writes, hash.bytes), (0, 0));
        assert_eq!(
            Arc::strong_count(&original.key.identity.repository_mapping),
            mapping_refs
        );
        assert_eq!(Arc::strong_count(&original.key), key_refs);
    }
    let mut limited = Union::default();
    limited.work.visits = 65536 - 1;
    let refs = Arc::strong_count(&original);
    assert!(
        limited
            .insert(&original)
            .unwrap_err()
            .to_string()
            .contains("candidate-index hash read")
    );
    assert_eq!(Arc::strong_count(&original), refs);
    assert!(limited.rows.is_empty() && limited.index.is_empty() && limited.occurrences.is_empty());
}
