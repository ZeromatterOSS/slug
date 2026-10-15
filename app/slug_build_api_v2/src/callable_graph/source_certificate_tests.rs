use super::*;

fn data() -> CallableSourceAuthorityData {
    let identity = CallableModuleIdentity {
        label: CanonicalLabel::parse("@@//pkg:source.bzl").unwrap(),
        source: crate::ModuleSourceOrigin::Workspace("/workspace/pkg/source.bzl".into()),
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
        environment_bindings: Arc::from([]),
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
        .chain(
            data.environment_bindings
                .iter()
                .map(|binding| &binding.module),
        )
        .chain(data.reachable.iter().map(|row| &row.identity))
    {
        count += 1;
        for _ in identity.repository_mapping.iter() {
            count += 1;
        }
    }
    count + 2 * data.environment_bindings.len()
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
        byte_cost: original.byte_cost(),
    }));
    assert_eq!(original, forged);
    assert_eq!(forged.fact_count(), original.fact_count() + 1);
    for field in 0..7 {
        let mut changed = original.data().clone();
        match field {
            0 => changed.root.label = CanonicalLabel::parse("@@//other:source.bzl").unwrap(),
            1 => {
                changed.root.source =
                    crate::ModuleSourceOrigin::Workspace("/other/source.bzl".into())
            }
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
        + root.source.retained_text_bytes().unwrap();
    let allocation = std::mem::size_of::<CallableSourceAuthorityData>()
        + CallableSourceAuthority::storage_overhead_bytes();
    let arrays = 6 * std::mem::size_of::<usize>()
        + std::mem::size_of_val(value.direct_children.as_ref())
        + std::mem::size_of_val(value.environment_bindings.as_ref())
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
        allocation + 8 * std::mem::size_of::<usize>() + prefix_bytes
    );
}

#[test]
fn builtin_environment_bindings_preserve_full_facts_without_becoming_loads() {
    let mut value = data();
    let direct = value.direct_children.clone();
    let mut module = value.root.clone();
    module.source = ModuleSourceOrigin::builtin("bazel9.2", "common/exports.bzl").unwrap();
    value.environment_bindings = Arc::from([CallableEnvironmentBinding {
        global: "apple_common".into(),
        export: "apple_common_bazel".into(),
        module: module.clone(),
    }]);
    let mut sources = value.reachable.to_vec();
    sources.push(CallableModuleSource {
        identity: module,
        digest: [9; 32],
    });
    value.reachable = sources.into();
    let original = CallableSourceAuthority::new(value.clone()).unwrap();
    assert_eq!(original.data().direct_children, direct);
    assert_eq!(original.fact_count(), reference_count(&value));
    assert_eq!(original.fact_count(), 20);
    assert!(original.byte_cost() > CallableSourceAuthority::new(data()).unwrap().byte_cost());
    for change in 0..4 {
        let mut changed = value.clone();
        let mut bindings = changed.environment_bindings.to_vec();
        match change {
            0 => bindings[0].global = "other_global".into(),
            1 => bindings[0].export = "other_export".into(),
            2 => {
                bindings[0].module.source =
                    ModuleSourceOrigin::builtin("other", "common/exports.bzl").unwrap()
            }
            3 => {
                bindings[0].module.source =
                    ModuleSourceOrigin::Workspace("common/exports.bzl".into())
            }
            _ => unreachable!(),
        }
        changed.environment_bindings = bindings.into();
        assert_ne!(original, CallableSourceAuthority::new(changed).unwrap());
    }
    assert_eq!(original, CallableSourceAuthority::new(value).unwrap());
}

#[test]
fn new_builtin_and_environment_text_accounts_for_retained_capacity() {
    let mut value = data();
    let mut module = value.root.clone();
    module.source =
        ModuleSourceOrigin::builtin(&"c".repeat(25), "common/heap_sized_source.bzl").unwrap();
    let origin_bytes = module.source.retained_text_bytes().unwrap();
    assert!(origin_bytes > 25 + "common/heap_sized_source.bzl".len());
    let mut global = CompactString::with_capacity(4096);
    global.push_str("global");
    let mut export = CompactString::with_capacity(8192);
    export.push_str("export");
    let name_capacity = global.capacity() + export.capacity();
    assert!(name_capacity >= 12288);
    value.environment_bindings = Arc::from([CallableEnvironmentBinding {
        global,
        export,
        module,
    }]);
    let authority = CallableSourceAuthority::new(value).unwrap();
    assert!(
        authority.byte_cost()
            >= name_capacity + origin_bytes + std::mem::size_of::<CallableSourceAuthorityData>()
    );
    // The short logical names cannot stand in for their retained buffers.
    assert!(authority.byte_cost() > 12288);
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
