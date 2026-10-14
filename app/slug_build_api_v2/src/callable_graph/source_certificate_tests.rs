use super::*;

fn data() -> CallableSourceAuthorityData {
    let identity = CallableModuleIdentity {
        label: CanonicalLabel::parse("@@//pkg:source.bzl").unwrap(),
        workspace_path: "/workspace/pkg/source.bzl".into(),
        repository_mapping: Arc::from([
            (
                ApparentRepoName::new("first").unwrap(),
                CanonicalRepoName::new("one+").unwrap(),
            ),
            (
                ApparentRepoName::new("second").unwrap(),
                CanonicalRepoName::new("two+").unwrap(),
            ),
        ]),
    };
    CallableSourceAuthorityData {
        root: identity.clone(),
        direct_children: Arc::from([identity.clone(), identity.clone()]),
        reachable: Arc::from([CallableModuleSource {
            identity,
            digest: [3; 32],
        }]),
        fingerprint: [4; 32],
    }
}
fn reference_count(data: &CallableSourceAuthorityData) -> usize {
    let mut count = 0;
    for identity in std::iter::once(&data.root)
        .chain(data.direct_children.iter())
        .chain(data.reachable.iter().map(|row| &row.identity))
    {
        count += 1;
        for _ in identity.repository_mapping.iter() {
            count += 1;
        }
    }
    count
}

#[test]
fn constructor_certifies_complete_count_and_full_data_equality() {
    let original = CallableSourceAuthority::new(data()).unwrap();
    assert_eq!(original.fact_count(), reference_count(original.data()));
    assert_eq!(original.fact_count(), 12);
    let separate = CallableSourceAuthority::new(original.data().clone()).unwrap();
    assert_eq!(original, separate);
    assert_ne!(original.storage_identity(), separate.storage_identity());
    // Only this private test can forge a certificate. It is not semantic data.
    let forged = CallableSourceAuthority(Arc::new(CallableSourceAuthorityStorage {
        data: original.data().clone(),
        fact_count: original.fact_count() + 1,
    }));
    assert_eq!(original, forged);
    assert_eq!(forged.fact_count(), original.fact_count() + 1);
    for field in 0..7 {
        let mut changed = original.data().clone();
        match field {
            0 => changed.root.label = CanonicalLabel::parse("@@//other:source.bzl").unwrap(),
            1 => changed.root.workspace_path = "/other/source.bzl".into(),
            2 => {
                let mut pairs = changed.root.repository_mapping.to_vec();
                pairs.reverse();
                changed.root.repository_mapping = pairs.into();
            }
            3 => changed.direct_children = Arc::from([changed.root.clone()]),
            4 => {
                let mut rows = changed.reachable.to_vec();
                rows[0].digest[0] ^= 1;
                changed.reachable = rows.into();
            }
            5 => changed.fingerprint[0] ^= 1,
            _ => {
                let mut rows = changed.reachable.to_vec();
                rows[0].identity.repository_mapping = Arc::from([]);
                changed.reachable = rows.into();
            }
        }
        let changed = CallableSourceAuthority::new(changed).unwrap();
        assert_ne!(original, changed, "semantic field {field}");
        assert_eq!(changed.fact_count(), reference_count(changed.data()));
    }
}

#[test]
fn envelope_layout_is_exact_and_oversized_count_refuses_before_retention() {
    #[repr(C)]
    struct Allocation {
        strong: usize,
        weak: usize,
        value: CallableSourceAuthorityStorage,
    }
    assert_eq!(
        std::mem::size_of::<CallableSourceAuthorityData>()
            + CallableSourceAuthority::storage_overhead_bytes(),
        std::mem::size_of::<Allocation>(),
    );
    assert!(CallableSourceAuthority::storage_overhead_bytes() >= 3 * std::mem::size_of::<usize>());
    let mapping: Arc<[(ApparentRepoName, CanonicalRepoName)]> = vec![
        (
            ApparentRepoName::new("alias").unwrap(),
            CanonicalRepoName::new("selected+").unwrap()
        );
        256
    ]
    .into();
    let mut value = data();
    value.root.repository_mapping = mapping.clone();
    value.reachable = Arc::from([]);
    value.direct_children = vec![value.root.clone(); 255].into();
    assert_eq!(reference_count(&value), 256 * 257);
    assert!(matches!(
        CallableSourceAuthority::new(value),
        Err(CallableGraphError::Limit("source facts"))
    ));
    assert_eq!(
        Arc::strong_count(&mapping),
        1,
        "no failed authority retains an allocation guard"
    );
}

#[test]
fn native_retention_covers_full_envelope_and_nested_arc_allocations() {
    let value = data();
    let root = &value.root;
    let mapping = &root.repository_mapping;
    let mapping_bytes = 2 * std::mem::size_of::<usize>()
        + std::mem::size_of_val(mapping.as_ref())
        + mapping
            .iter()
            .map(|(a, c)| a.as_str().len() + c.as_str().len())
            .sum::<usize>();
    let prefix_bytes = root.label.package().repo().as_str().len()
        + root.label.package().package().as_str().len()
        + root.label.target().as_str().len()
        + root.label.mapping_id().map_or(0, |id| id.as_str().len())
        + root.workspace_path.as_os_str().len();
    let allocation = std::mem::size_of::<CallableSourceAuthorityData>()
        + CallableSourceAuthority::storage_overhead_bytes();
    let arrays = 4 * std::mem::size_of::<usize>()
        + std::mem::size_of_val(value.direct_children.as_ref())
        + std::mem::size_of_val(value.reachable.as_ref());
    let authority = CallableSourceAuthority::new(value).unwrap();
    // Four identity occurrences, one actual shared map; recounting complete
    // immutable mapping allocations conservatively is intentional.
    assert_eq!(
        authority.byte_cost(),
        allocation + arrays + 4 * (prefix_bytes + mapping_bytes)
    );
    assert!(authority.byte_cost() >= allocation + arrays + 4 * prefix_bytes + mapping_bytes);
    let mut empty = data();
    empty.root.repository_mapping = Arc::from([]);
    empty.direct_children = Arc::from([]);
    empty.reachable = Arc::from([]);
    let empty = CallableSourceAuthority::new(empty).unwrap();
    assert_eq!(
        empty.byte_cost(),
        allocation + 6 * std::mem::size_of::<usize>() + prefix_bytes
    );
}

#[test]
fn source_authority_keeps_its_independent_64_mib_limit() {
    assert_eq!(CALLABLE_GRAPH_BYTE_LIMIT, 64 * 1024 * 1024);
    let mapping: Arc<[(ApparentRepoName, CanonicalRepoName)]> = (0..64)
        .map(|i| {
            (
                ApparentRepoName::new(format!("{}_{i}", "a".repeat(2048))).unwrap(),
                CanonicalRepoName::new("b".repeat(2048)).unwrap(),
            )
        })
        .collect::<Vec<_>>()
        .into();
    let mut value = data();
    value.root.repository_mapping = mapping.clone();
    value.direct_children = vec![value.root.clone(); 256].into();
    value.reachable = Arc::from([]);
    // One small shared mapping allocation is fully counted per identity. This
    // exercises the native source cap without allocating a 64MiB fixture.
    assert!(CallableSourceAuthority::data_fact_count(&value) <= CALLABLE_GRAPH_NODE_LIMIT);
    let bytes = CallableSourceAuthority::data_byte_cost(&value);
    assert!(bytes > CALLABLE_GRAPH_BYTE_LIMIT);
    assert!(bytes <= crate::published_value::BYTE_LIMIT);
    assert!(matches!(
        CallableSourceAuthority::new(value),
        Err(CallableGraphError::Limit("source bytes"))
    ));
    assert_eq!(
        Arc::strong_count(&mapping),
        1,
        "failed source retains no guard"
    );
}
