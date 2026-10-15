use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;

use super::*;

fn identity(raw: &str, path: &str) -> BzlModuleIdentity {
    BzlModuleIdentity {
        label: CanonicalLabel::parse(raw).unwrap(),
        source: slug_build_api_v2::ModuleSourceOrigin::Workspace(path.into()),
        repository_mapping: Arc::from([]),
    }
}

fn frozen(
    identity: BzlModuleIdentity,
    marker: &str,
    children: &[FrozenBzlModule],
) -> FrozenBzlModule {
    let module = Module::new();
    module.set("MARKER", module.heap().alloc_str(marker).to_value());
    let children: Vec<_> = children
        .iter()
        .cloned()
        .map(|child| (child.manifest.root.label.to_string(), child))
        .collect();
    FrozenBzlModule {
        semantic_inputs: None,
        module: module.freeze().unwrap(),
        path: identity
            .source
            .workspace_path()
            .expect("workspace fixture source")
            .to_path_buf(),
        loads: children.iter().map(|(name, _)| name.clone()).collect(),
        manifest: BzlLoadManifest::new(identity, digest(marker), children.iter().map(|(_, m)| m)),
        bzl_load_visibility: BzlLoadVisibility::Public,
        retained_bzl_modules: retained_module_closure(&children),
    }
}

#[test]
fn manifest_helpers_preserve_first_seen_diamonds_and_owned_results() {
    let long = format!("@@dep+//{}:é雪.bzl", "package_".repeat(1024));
    let a = frozen(identity(&long, "/manifest/a.bzl"), "a-first", &[]);
    let duplicate_a = frozen(identity(&long, "/manifest/a-later.bzl"), "a-later", &[]);
    assert_eq!(a.manifest.root.label, duplicate_a.manifest.root.label);
    assert_ne!(
        a.manifest.root.label.package().package().as_str().as_ptr(),
        duplicate_a
            .manifest
            .root
            .label
            .package()
            .package()
            .as_str()
            .as_ptr()
    );
    let b = frozen(identity("@@dep+//:b.bzl", "/manifest/b.bzl"), "b", &[]);
    let left = frozen(
        identity("@@//:left.bzl", "/manifest/left.bzl"),
        "left",
        &[a.clone()],
    );
    let right = frozen(
        identity("@@//:right.bzl", "/manifest/right.bzl"),
        "right",
        &[duplicate_a.clone(), b.clone()],
    );
    let duplicate_left = frozen(
        identity("@@//:left.bzl", "/manifest/left-later.bzl"),
        "left-later",
        &[],
    );
    let root = identity("@@//:root.bzl", "/manifest/root.bzl");
    let repeated_root = frozen(root.clone(), "root-later", &[]);
    let loaded = vec![
        ("left".to_owned(), left.clone()),
        ("right".to_owned(), right.clone()),
        ("duplicate".to_owned(), duplicate_left.clone()),
        ("root".to_owned(), repeated_root.clone()),
    ];
    let roots = first_seen_direct_roots(&loaded);
    let expected_roots = [
        left.manifest.root.clone(),
        right.manifest.root.clone(),
        root.clone(),
    ];
    assert_eq!(roots, expected_roots);
    let manifest = BzlLoadManifest::new(
        root.clone(),
        digest("root-first"),
        loaded.iter().map(|(_, m)| m),
    );
    assert_eq!(manifest.direct_children.as_ref(), expected_roots);
    assert_eq!(
        manifest.reachable.as_ref(),
        [
            BzlModuleSourceProvenance::new(root.clone(), digest("root-first")),
            left.manifest.reachable[0].clone(),
            a.manifest.reachable[0].clone(),
            right.manifest.reachable[0].clone(),
            b.manifest.reachable[0].clone(),
        ]
    );
    assert_eq!(
        manifest.fingerprint,
        manifest_fingerprint(digest("root-first"), [&left, &right, &repeated_root])
    );
    let unique = vec![loaded[0].clone(), loaded[1].clone(), loaded[3].clone()];
    assert_eq!(
        package_load_fingerprint(&loaded),
        package_load_fingerprint(&unique)
    );
    let mut reversed = unique.clone();
    reversed.swap(0, 1);
    assert_ne!(
        package_load_fingerprint(&loaded),
        package_load_fingerprint(&reversed)
    );
    let sources = package_bzl_call_sources(&loaded).unwrap();
    let expected_sources = [
        left.manifest.reachable[0].clone(),
        a.manifest.reachable[0].clone(),
        right.manifest.reachable[0].clone(),
        b.manifest.reachable[0].clone(),
        repeated_root.manifest.reachable[0].clone(),
    ];
    assert_eq!(
        sources.iter().map(|(_, p)| p.clone()).collect::<Vec<_>>(),
        expected_sources
    );
    for (source, provenance) in sources.iter() {
        assert_eq!(
            source.as_str(),
            provenance
                .identity
                .source
                .workspace_path()
                .expect("workspace fixture source")
                .to_str()
                .unwrap()
        );
    }
    let lifetimes = flattened_lifetime_closure(&loaded);
    assert_eq!(
        lifetimes
            .iter()
            .map(|entry| entry.identity.clone())
            .collect::<Vec<_>>(),
        expected_sources
            .iter()
            .map(|p| p.identity.clone())
            .collect::<Vec<_>>()
    );
    drop((
        loaded,
        unique,
        reversed,
        left,
        right,
        a,
        duplicate_a,
        b,
        duplicate_left,
        repeated_root,
        root,
    ));
    assert_eq!(manifest.direct_children.as_ref(), roots);
    assert_eq!(
        sources.iter().map(|(_, p)| p.clone()).collect::<Vec<_>>(),
        expected_sources
    );
    let retained: Vec<_> = lifetimes
        .iter()
        .map(|entry| entry.module.get("MARKER").unwrap())
        .collect();
    drop(lifetimes);
    assert_eq!(
        retained
            .iter()
            .map(|v| v.unpack_str().unwrap())
            .collect::<Vec<_>>(),
        ["left", "a-first", "right", "b", "root-later"]
    );
}

#[test]
fn mapping_identity_and_existing_fingerprint_projection_stay_distinct() {
    let apparent = ApparentLabel::parse("@dep//pkg:item.bzl").unwrap();
    let mapped = |id: &str| {
        let mut mapping = RepositoryMapping::new(RepositoryMappingId::new(id).unwrap());
        mapping.insert(
            ApparentRepoName::new("dep").unwrap(),
            CanonicalRepoName::new("dep+").unwrap(),
        );
        apparent.resolve(&mapping)
    };
    let mut first = identity("@@dep+//pkg:item.bzl", "/manifest/mapped.bzl");
    first.label = mapped("one");
    let mut second = first.clone();
    second.label = mapped("two");
    assert_eq!(first.label.to_string(), second.label.to_string());
    assert_ne!(first.label, second.label);
    let a = frozen(first.clone(), "same", &[]);
    let b = frozen(second.clone(), "same", &[]);
    // Mapping ID participates in deduplication, but current fingerprint bytes
    // project label spelling and mapping entries, not the separate mapping ID.
    let singles = |m: &FrozenBzlModule| vec![("load".to_owned(), m.clone())];
    assert_eq!(
        package_load_fingerprint(&singles(&a)),
        package_load_fingerprint(&singles(&b))
    );
    // Pinned pre-origin Workspace encoding at aaac0fc70.
    assert_eq!(
        package_load_fingerprint(&singles(&a)),
        [
            171, 221, 150, 96, 211, 109, 235, 124, 231, 216, 208, 45, 241, 58, 49, 67, 145, 182,
            210, 5, 27, 171, 242, 209, 209, 203, 246, 253, 221, 116, 14, 199
        ]
    );
    let loaded = vec![("a".to_owned(), a.clone()), ("b".to_owned(), b.clone())];
    assert_eq!(first_seen_direct_roots(&loaded), [first.clone(), second]);
    assert_eq!(package_bzl_call_sources(&loaded).unwrap().len(), 2);
    assert_eq!(flattened_lifetime_closure(&loaded).len(), 2);
    let manifest = BzlLoadManifest::new(
        identity("@@//:root.bzl", "/manifest/root.bzl"),
        digest("root"),
        [&a, &b],
    );
    assert_eq!(manifest.direct_children.len(), 2);
    assert_eq!(manifest.reachable.len(), 3);
    assert_ne!(
        package_load_fingerprint(&loaded),
        package_load_fingerprint(&singles(&a))
    );
    for change in 0..4 {
        let mut changed = first.clone();
        if change == 0 {
            changed.source =
                slug_build_api_v2::ModuleSourceOrigin::Workspace("/manifest/other.bzl".into());
        }
        if change == 1 {
            changed.repository_mapping = Arc::from([(
                ApparentRepoName::new("alias").unwrap(),
                CanonicalRepoName::new("other+").unwrap(),
            )]);
        }
        let mut changed = frozen(
            changed,
            if change == 2 {
                "changed source"
            } else {
                "same"
            },
            &[],
        );
        if change == 3 {
            changed.manifest.fingerprint[0] ^= 1;
        }
        assert_ne!(
            package_load_fingerprint(&singles(&a)),
            package_load_fingerprint(&singles(&changed))
        );
    }
    assert_ne!(
        manifest_fingerprint(digest("root"), [&a]),
        manifest_fingerprint(digest("other root"), [&a])
    );
}

#[test]
fn empty_manifest_helpers_preserve_root_and_absent_sources() {
    let root = identity("@@//:root.bzl", "/manifest/root.bzl");
    let manifest = BzlLoadManifest::new(
        root.clone(),
        digest("root"),
        std::iter::empty::<&FrozenBzlModule>(),
    );
    assert_eq!(manifest.root, root);
    assert!(manifest.direct_children.is_empty());
    assert_eq!(
        manifest.reachable.as_ref(),
        [BzlModuleSourceProvenance::new(root, digest("root"))]
    );
    assert!(first_seen_direct_roots(&[]).is_empty());
    assert!(package_bzl_call_sources(&[]).is_none());
    assert!(flattened_lifetime_closure(&[]).is_empty());
    assert_eq!(
        package_load_fingerprint(&[]),
        package_load_fingerprint(&Vec::new())
    );
}
