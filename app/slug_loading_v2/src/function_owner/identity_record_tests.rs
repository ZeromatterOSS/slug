use super::*;

fn identity(entries: usize, mapped_label: bool) -> BzlModuleIdentity {
    let mut label = slug_identity_v2::CanonicalLabel::parse("@@//pkg:defs.bzl").unwrap();
    if mapped_label {
        let mapping = slug_identity_v2::RepositoryMapping::new(
            slug_identity_v2::RepositoryMappingId::new("complete-origin-map").unwrap(),
        );
        label = slug_identity_v2::ApparentLabel::parse("//pkg:defs.bzl")
            .unwrap()
            .resolve(&mapping);
    }
    BzlModuleIdentity {
        label,
        source: slug_build_api_v2::ModuleSourceOrigin::Workspace(
            "/workspace/longer/path/defs.bzl".into(),
        ),
        repository_mapping: (0..entries)
            .map(|index| {
                (
                    ApparentRepoName::new(format!("apparent{index}")).unwrap(),
                    CanonicalRepoName::new(format!("selected-repository{index}+")).unwrap(),
                )
            })
            .collect::<Vec<_>>()
            .into(),
    }
}

// Literal pre-amendment field forecast protects storage/depth, not old visits.
fn previous_shape(identity: &BzlModuleIdentity) -> PublicationShape {
    let mut shape = fact(
        std::mem::size_of::<BzlModuleIdentity>() + 2 * std::mem::size_of::<usize>(),
        5,
        2,
    );
    for text in [
        identity.label.package().repo().as_str(),
        identity.label.package().package().as_str(),
        identity.label.target().as_str(),
    ] {
        add_shape(&mut shape, fact(text.len(), 0, 0), 2).unwrap();
    }
    if let Some(mapping) = identity.label.mapping_id() {
        add_shape(&mut shape, fact(mapping.as_str().len(), 1, 0), 2).unwrap();
    }
    add_shape(
        &mut shape,
        fact(
            identity.source.retained_text_bytes().unwrap_or(usize::MAX) * 2,
            0,
            0,
        ),
        1,
    )
    .unwrap();
    for (apparent, canonical) in &*identity.repository_mapping {
        add_shape(
            &mut shape,
            fact(
                std::mem::size_of::<(ApparentRepoName, CanonicalRepoName)>(),
                2,
                1,
            ),
            1,
        )
        .unwrap();
        add_shape(&mut shape, fact(apparent.as_str().len(), 0, 0), 3).unwrap();
        add_shape(&mut shape, fact(canonical.as_str().len(), 0, 0), 3).unwrap();
    }
    shape
}

#[test]
fn complete_identity_records_preserve_storage_edges_depth_and_mapping_provenance() {
    for entries in [0, 1, 12] {
        for mapped_label in [false, true] {
            let identity = identity(entries, mapped_label);
            let old = previous_shape(&identity);
            let actual = shape(&identity).unwrap();
            assert_eq!(
                (actual.nodes, actual.bytes, actual.edges, actual.depth),
                (old.nodes, old.bytes, old.edges, old.depth)
            );
            assert_eq!(old.visits, 5 + usize::from(mapped_label) + 3 * entries);
            assert_eq!(actual.visits, 1 + entries);
            let mut admitted = prefix(&identity).unwrap();
            assert_eq!(admitted.visits, 1);
            for (index, (apparent, canonical)) in identity.repository_mapping.iter().enumerate() {
                append_entry(&mut admitted, apparent, canonical).unwrap();
                assert_eq!(admitted.visits, index + 2);
                assert_eq!(admitted.depth, 3);
            }
            assert_eq!(
                (
                    admitted.nodes,
                    admitted.bytes,
                    admitted.edges,
                    admitted.depth,
                    admitted.visits
                ),
                (
                    actual.nodes,
                    actual.bytes,
                    actual.edges,
                    actual.depth,
                    actual.visits
                )
            );
        }
    }
}

#[derive(Default)]
struct Writes(usize);
impl Hasher for Writes {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, _: &[u8]) {
        self.0 += 1;
    }
}

#[test]
fn identity_record_limits_refuse_each_entry_and_precede_full_hash_or_clone() {
    let identity = identity(2, true);
    let refs = Arc::strong_count(&identity.repository_mapping);
    let mut admitted = prefix(&identity).unwrap();
    admitted.visits = 65535;
    let (first, last) = (
        &identity.repository_mapping[0],
        &identity.repository_mapping[1],
    );
    append_entry(&mut admitted, &first.0, &first.1).unwrap();
    assert_eq!(admitted.visits, 65536);
    assert!(append_entry(&mut admitted, &last.0, &last.1).is_err());
    assert_eq!(Arc::strong_count(&identity.repository_mapping), refs);

    let complete = shape(&identity).unwrap();
    for bytes in [false, true] {
        let mut work = zero_shape();
        if bytes {
            work.bytes = 64 * 1024 * 1024 - complete.bytes + 1;
        } else {
            work.visits = 65536 - complete.visits + 1;
        }
        let mut hash = Writes::default();
        assert!(
            write_candidate_index_hash(
                &identity,
                crate::bzl_environment::PreparedBzlEnvironment::uninjected(
                    crate::bzl_environment::BzlEnvironmentMode::BzlmodBootstrap
                )
                .unwrap()
                .inputs(),
                true,
                complete,
                &mut work,
                &mut hash,
            )
            .is_err()
        );
        assert_eq!(hash.0, 0);
        assert_eq!(Arc::strong_count(&identity.repository_mapping), refs);
    }
    // The prefix's complete storage is admitted independently of any entries.
    let mut work = zero_shape();
    work.bytes = 64 * 1024 * 1024;
    assert!(add_shape(&mut work, prefix(&identity).unwrap(), 0).is_err());
    assert_eq!(Arc::strong_count(&identity.repository_mapping), refs);
}
