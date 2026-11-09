//! Pure route identity/cache controls. Actual source production is covered by
//! Loading's registry-route DICE test; these fixtures mint no IO authority.
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use compact_str::CompactString;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use starlark_map::small_map::SmallMap;

use super::*;
use crate::HostSelectedExtensionDefinitionSource;
use crate::OverrideAttributeKey;
use crate::RepoRuleId;

#[derive(Default)]
struct HashWork {
    writes: usize,
    bytes: usize,
}
impl Hasher for HashWork {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, bytes: &[u8]) {
        self.writes += 1;
        self.bytes += bytes.len();
    }
}
fn digest(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
fn workspace() -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new("/canonical-route-cache").unwrap()
}
fn spec(reverse_attrs: bool, reverse_map: bool, reverse_list: bool) -> RepoSpec {
    let mut nested = vec![
        (
            OverrideAttributeKey::String("alpha".into()),
            OverrideAttributeValue::Int(1),
        ),
        (
            OverrideAttributeKey::String("beta".into()),
            OverrideAttributeValue::Int(2),
        ),
    ];
    if reverse_map {
        nested.reverse();
    }
    let mut list = vec![
        OverrideAttributeValue::Int(3),
        OverrideAttributeValue::Int(4),
    ];
    if reverse_list {
        list.reverse();
    }
    let mut attrs = vec![
        (
            CompactString::new("path"),
            OverrideAttributeValue::String("/local-route".into()),
        ),
        (
            CompactString::new("nested"),
            OverrideAttributeValue::Map(Arc::new(nested.into_iter().collect())),
        ),
        (
            CompactString::new("list"),
            OverrideAttributeValue::Iterable(list.into()),
        ),
    ];
    if reverse_attrs {
        attrs.reverse();
    }
    RepoSpec {
        rule_id: RepoRuleId {
            bzl_file: CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:local.bzl")
                .unwrap(),
            rule_name: "local_repository".into(),
        },
        attributes: Arc::new(attrs.into_iter().collect()),
    }
}
fn mapping(count: usize, reverse: bool) -> SmallMap<ApparentRepoName, CanonicalRepoName> {
    let mut pairs = (0..count)
        .map(|i| {
            (
                ApparentRepoName::new(format!("dep{i}")).unwrap(),
                CanonicalRepoName::new(format!("selected{i}+")).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    if reverse {
        pairs.reverse();
    }
    pairs.into_iter().collect()
}
fn generated(
    spec: RepoSpec,
    mapping: SmallMap<ApparentRepoName, CanonicalRepoName>,
) -> HostCanonicalRepositoryRoute {
    let canonical = CanonicalRepoName::new("generated+").unwrap();
    HostCanonicalRepositoryRoute::generated(
        workspace(),
        canonical.clone(),
        Arc::new(HostSelectedExtensionOwner::testing("+extension")),
        0,
        "generated",
        spec,
        canonical,
        mapping,
    )
    .unwrap()
}
fn selected(
    spec: RepoSpec,
    pairs: &[(&str, &str)],
    policy: HostRepositoryLocalPathPolicy,
) -> HostCanonicalRepositoryRoute {
    // Existing pure selected-owner fixture retains the complete definition,
    // including its explicit mapping order. It supplies no source certificate.
    let HostSelectedExtensionDefinitionSource::Selected { definition, .. } =
        crate::selected_repo_spec::testing_selected_command_local_source(spec, policy, pairs)
    else {
        panic!("selected owner fixture")
    };
    HostCanonicalRepositoryRoute::from_selected(workspace(), definition)
}
fn builtin() -> HostCanonicalRepositoryRoute {
    HostCanonicalRepositoryRoute::builtin(
        workspace(),
        HostBuiltinBazelToolsRepositoryMapping::testing(),
    )
}

#[test]
fn cached_route_hash_is_fixed_for_all_variants_after_construction() {
    let routes = [
        builtin(),
        selected(
            spec(false, false, false),
            &[("a", "a+"), ("b", "b+")],
            HostRepositoryLocalPathPolicy::CommandAbsolute,
        ),
        generated(spec(false, false, false), mapping(0, false)),
        generated(spec(false, false, false), mapping(64, false)),
    ];
    for route in routes {
        let mut full = StarlarkHasher::new();
        route.hash_fields(&mut full);
        assert_eq!(
            route.cached_hash,
            full.finish(),
            "every constructor caches its complete original field hash"
        );
        let cloned = route.clone();
        let mut structural = HashWork::default();
        route.hash_fields(&mut structural);
        assert!(structural.writes > 1);
        let mut repeated = HashWork::default();
        for _ in 0..64 {
            route.hash(&mut repeated);
            cloned.hash(&mut repeated);
        }
        assert_eq!(
            (repeated.writes, repeated.bytes),
            (128, 128 * std::mem::size_of::<u64>())
        );
        assert_eq!(route, cloned);
        assert_eq!(digest(&route), digest(&cloned));
    }
}

#[test]
fn cached_route_hash_preserves_unordered_membership_and_nested_publication_order() {
    let first = generated(spec(false, false, false), mapping(8, false));
    let reordered = generated(spec(true, false, false), mapping(8, true));
    assert_eq!(first, reordered);
    assert_eq!(digest(&first), digest(&reordered));
    let (
        HostCanonicalRepositoryRouteSource::Generated(left),
        HostCanonicalRepositoryRouteSource::Generated(right),
    ) = (&first.source, &reordered.source)
    else {
        unreachable!()
    };
    assert!(!Arc::ptr_eq(&left.mapping, &right.mapping));
    assert!(!Arc::ptr_eq(
        &left.repo_spec.attributes,
        &right.repo_spec.attributes
    ));
    for changed in [
        generated(spec(false, true, false), mapping(8, false)),
        generated(spec(false, false, true), mapping(8, false)),
    ] {
        // Nested publication Map order and Iterable order were already semantic.
        assert_ne!(first, changed);
        assert_ne!(digest(&first), digest(&changed));
    }
    let pairs = [("a", "a+"), ("b", "b+")];
    let a = selected(
        spec(false, false, false),
        &pairs,
        HostRepositoryLocalPathPolicy::CommandAbsolute,
    );
    let equal = selected(
        spec(true, false, false),
        &pairs,
        HostRepositoryLocalPathPolicy::CommandAbsolute,
    );
    assert_eq!(a, equal);
    assert_eq!(digest(&a), digest(&equal));
    let reordered = selected(
        spec(false, false, false),
        &[("b", "b+"), ("a", "a+")],
        HostRepositoryLocalPathPolicy::CommandAbsolute,
    );
    // Selected definitions retain an explicit ordered mapping owner: do not
    // silently replace that full equality with unordered hash equivalence.
    assert_ne!(a, reordered);
}

#[test]
fn cached_route_hash_collisions_keep_full_structural_equality_and_lookup() {
    let base = generated(spec(false, false, false), mapping(2, false));
    let mut variants = vec![
        base.clone(),
        builtin(),
        selected(
            spec(false, false, false),
            &[("a", "a+")],
            HostRepositoryLocalPathPolicy::CommandAbsolute,
        ),
    ];
    let mut other_workspace = base.clone();
    other_workspace.workspace = NormalizedAbsolutePath::new("/other-route").unwrap();
    variants.push(other_workspace);
    for field in 0..8 {
        let mut changed = base.clone();
        let HostCanonicalRepositoryRouteSource::Generated(g) = &mut changed.source else {
            unreachable!()
        };
        match field {
            0 => g.owner = Arc::new(HostSelectedExtensionOwner::testing("+other-extension")),
            1 => g.ordinal += 1,
            2 => g.internal_name = "other-internal".into(),
            3 => g.repo_spec.rule_id.rule_name = "other_rule".into(),
            4 => {
                Arc::make_mut(&mut g.repo_spec.attributes)
                    .insert("extra".into(), OverrideAttributeValue::Bool(true));
            }
            5 => {
                Arc::make_mut(&mut g.mapping).insert(
                    ApparentRepoName::new("dep0").unwrap(),
                    CanonicalRepoName::new("different+").unwrap(),
                );
            }
            // Private-field corruptions independently discriminate every Eq
            // field; neither change can be masked by another changed field.
            6 => changed.canonical_repo = CanonicalRepoName::new("other-generated+").unwrap(),
            7 => g.mapping_context = CanonicalRepoName::new("other-context+").unwrap(),
            _ => unreachable!(),
        }
        variants.push(changed);
    }
    variants.push(HostCanonicalRepositoryRoute::builtin(
        NormalizedAbsolutePath::new("/other-builtin").unwrap(),
        HostBuiltinBazelToolsRepositoryMapping::testing(),
    ));
    variants.push(selected(
        spec(false, true, false),
        &[("a", "a+")],
        HostRepositoryLocalPathPolicy::CommandAbsolute,
    ));
    variants.push(selected(
        spec(false, false, false),
        &[("a", "other+")],
        HostRepositoryLocalPathPolicy::CommandAbsolute,
    ));
    variants.push(selected(
        spec(false, false, false),
        &[("a", "a+")],
        HostRepositoryLocalPathPolicy::WorkspaceRelative,
    ));
    // Test-only corruption of the private derived cache creates deliberate
    // collisions. The actual semantic fields and complete Eq remain intact.
    for route in &mut variants {
        route.cached_hash = 7;
    }
    let mut indexed = SmallMap::new();
    for (index, route) in variants.iter().enumerate() {
        assert!(indexed.insert(route.clone(), index).is_none());
    }
    assert_eq!(indexed.len(), variants.len());
    for (index, route) in variants.iter().enumerate() {
        assert_eq!(indexed.get(route), Some(&index));
        for other in &variants[..index] {
            assert_eq!(digest(route), digest(other));
            assert_ne!(route, other);
        }
    }
}

#[test]
fn cached_route_clone_keeps_only_existing_payload_owners_and_positive_inline_layout() {
    #[allow(dead_code)]
    struct LegacyLayout {
        workspace: NormalizedAbsolutePath,
        canonical_repo: CanonicalRepoName,
        source: HostCanonicalRepositoryRouteSource,
    }
    let old = std::mem::size_of::<LegacyLayout>();
    let new = std::mem::size_of::<HostCanonicalRepositoryRoute>();
    let added = new
        .checked_sub(old)
        .expect("cached route cannot shrink original fields");
    println!("CANONICAL_ROUTE_LAYOUT legacy={old} cached={new} positive_delta={added}");
    assert!(added > 0 && added <= std::mem::size_of::<u64>());
    let route = generated(spec(false, false, false), mapping(2, false));
    let HostCanonicalRepositoryRouteSource::Generated(g) = &route.source else {
        unreachable!()
    };
    let owner = Arc::downgrade(&g.owner);
    let attrs = Arc::downgrade(&g.repo_spec.attributes);
    let mapping = Arc::downgrade(&g.mapping);
    assert_eq!(
        (
            owner.strong_count(),
            attrs.strong_count(),
            mapping.strong_count()
        ),
        (1, 1, 1)
    );
    let clone = route.clone();
    assert_eq!(route.cached_hash, clone.cached_hash);
    assert_eq!(
        (
            owner.strong_count(),
            attrs.strong_count(),
            mapping.strong_count()
        ),
        (2, 2, 2)
    );
    drop(route);
    assert!(owner.upgrade().is_some() && attrs.upgrade().is_some() && mapping.upgrade().is_some());
    drop(clone);
    assert!(owner.upgrade().is_none() && attrs.upgrade().is_none() && mapping.upgrade().is_none());
}
