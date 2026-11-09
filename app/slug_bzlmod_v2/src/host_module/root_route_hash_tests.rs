// Structural routing controls only; these fixtures grant no native authority.
// Loading separately exercises the genuine selected Registry producer/key/cycle.
use super::super::*;
use super::*;

#[derive(Default, Debug, PartialEq, Eq)]
struct Writes(Vec<(&'static str, Vec<u8>)>);
macro_rules! primitive_writes {
    ($($method:ident($ty:ty)),* $(,)?) => {
        $(fn $method(&mut self, value: $ty) {
            self.0.push((stringify!($method), value.to_ne_bytes().to_vec()));
        })*
    };
}
impl Hasher for Writes {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, value: &[u8]) {
        self.0.push(("write", value.to_vec()));
    }
    primitive_writes!(
        write_u8(u8),
        write_u16(u16),
        write_u32(u32),
        write_u64(u64),
        write_u128(u128),
        write_usize(usize),
        write_i8(i8),
        write_i16(i16),
        write_i32(i32),
        write_i64(i64),
        write_i128(i128),
        write_isize(isize),
    );
}
fn stream(value: &impl Hash) -> Writes {
    let mut out = Writes::default();
    value.hash(&mut out);
    out
}
fn reference(route: &RootRepositoryRoute, state: &mut impl Hasher) {
    // Independent literal pre-cache writer, including unchanged nested writers.
    route.data.workspace.hash(state);
    route.data.apparent_repo.hash(state);
    route.data.module_name.hash(state);
    route.data.canonical_repo.hash(state);
    route.data.source.hash(state);
}
fn check(route: &RootRepositoryRoute) {
    let mut expected = Writes::default();
    reference(route, &mut expected);
    assert_eq!(stream(route), expected);
    let mut hash = DefaultHasher::new();
    reference(route, &mut hash);
    assert_eq!(route.data.routing_hash, hash.finish());
}
fn local_spec(path: &str) -> RepoSpec {
    RepoSpec {
        rule_id: crate::RepoRuleId {
            bzl_file: CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:local.bzl")
                .unwrap(),
            rule_name: "local_repository".into(),
        },
        attributes: Arc::new(SmallMap::from_iter([(
            CompactString::new("path"),
            OverrideAttributeValue::String(path.into()),
        )])),
    }
}
fn local(count: usize) -> RootRepositoryRoute {
    let owned = (0..count)
        .map(|i| (format!("d{i}"), format!("d{i}+")))
        .collect::<Vec<_>>();
    let pairs = owned
        .iter()
        .map(|(a, c)| (a.as_str(), c.as_str()))
        .collect::<Vec<_>>();
    let source = crate::selected_repo_spec::testing_selected_command_local_source(
        local_spec("/dep"),
        crate::HostRepositoryLocalPathPolicy::CommandAbsolute,
        &pairs,
    );
    RootRepositoryRoute::for_selected_extension_definition(workspace(), &source).unwrap()
}
fn variants() -> Vec<RootRepositoryRoute> {
    let selected = local(64);
    let RootRepositorySource::SelectedLocal(local) = &selected.data.source else {
        unreachable!()
    };
    // Exercise the Registry Hash arm as a structural value, not an admitted
    // Registry request. The genuine Registry constructor is covered in Loading.
    let registry = RootRepositoryRoute::from_fields(
        workspace(),
        selected.data.apparent_repo.clone(),
        selected.data.module_name.clone(),
        selected.data.canonical_repo.clone(),
        RootRepositorySource::SelectedRegistry(SelectedRegistryRepositoryRoute {
            definition: local.definition.clone(),
            repo_spec: local_spec("/registry-shape"),
            local_path_policy: crate::HostRepositoryLocalPathPolicy::LocalUnsupported,
            mapping: local.mapping.clone(),
        }),
    );
    vec![
        RootRepositoryRoute::for_test(
            workspace(),
            ApparentRepoName::new("direct").unwrap(),
            "direct".into(),
            CanonicalRepoName::new("direct+").unwrap(),
            local_spec("direct"),
        ),
        RootRepositoryRoute::builtin_for_test(workspace()),
        RootRepositoryRoute::for_generated_repo_spec(
            workspace(),
            ApparentRepoName::new("generated").unwrap(),
            CanonicalRepoName::new("generated+").unwrap(),
            local_spec("generated"),
            crate::HostRepositoryLocalPathPolicy::LocalUnsupported,
            GeneratedRepositoryFileEffectPlan::build([]).unwrap(),
        )
        .unwrap(),
        selected,
        registry,
    ]
}

#[test]
fn original_full_stream_and_fixed_operational_work_cover_every_source_arm() {
    for route in variants().into_iter().chain([local(0), local(256)]) {
        check(&route);
        let source = crate::HostRepositorySourceRoute::root(route.clone());
        let mut work = Writes::default();
        for _ in 0..64 {
            source.hash(&mut work);
        }
        assert_eq!(work.0.len(), 64);
        assert!(
            work.0
                .iter()
                .all(|(method, bytes)| *method == "write_u64" && bytes.len() == 8)
        );
        assert_eq!(
            work.0.iter().map(|(_, bytes)| bytes.len()).sum::<usize>(),
            512
        );
        let clone = route.clone();
        assert_eq!(clone.data.routing_hash, route.data.routing_hash);
        assert_eq!(clone, route);
        check(&clone);
    }
    let canonical = Arc::new(crate::HostCanonicalRepositoryRoute::builtin(
        workspace(),
        crate::HostBuiltinBazelToolsRepositoryMapping::testing(),
    ));
    let input = crate::host_canonical_repository_source_input(canonical, None).unwrap();
    let source = crate::HostRepositorySourceRoute::canonical(input.clone());
    let mut expected = Writes::default();
    1_u8.hash(&mut expected);
    input.hash(&mut expected);
    assert_eq!(stream(&source), expected);
}

#[test]
fn full_equality_and_bucket_lookup_ignore_forced_cached_collisions() {
    let first = local(3);
    let mut changed = Vec::new();
    for index in 0..5 {
        let mut fields = (
            first.data.workspace.clone(),
            first.data.apparent_repo.clone(),
            first.data.module_name.clone(),
            first.data.canonical_repo.clone(),
            first.data.source.clone(),
        );
        match index {
            0 => fields.0 = NormalizedAbsolutePath::new("/elsewhere").unwrap(),
            1 => fields.1 = ApparentRepoName::new("other").unwrap(),
            2 => fields.2 = "other".into(),
            3 => fields.3 = CanonicalRepoName::new("other+").unwrap(),
            _ => fields.4 = RootRepositorySource::DirectLocal(local_spec("different")),
        }
        let value =
            RootRepositoryRoute::from_fields(fields.0, fields.1, fields.2, fields.3, fields.4);
        check(&value);
        assert_ne!(first, value);
        changed.push(value);
    }
    let mut cache_only = local(3);
    assert!(!Arc::ptr_eq(&first.data, &cache_only.data));
    Arc::get_mut(&mut cache_only.data).unwrap().routing_hash ^= u64::MAX;
    assert_eq!(first, cache_only);
    assert_eq!(stream(&first), stream(&cache_only));
    assert_eq!(format!("{first:?}"), format!("{cache_only:?}"));
    // All entries below have distinct full fields; artificially force ONE bucket.
    // Never mix equal full fields carrying different artificial cache values.
    let mut entries = vec![first];
    entries.extend(changed);
    let mut map = SmallMap::new();
    for (index, route) in entries.iter_mut().enumerate() {
        Arc::get_mut(&mut route.data).unwrap().routing_hash = 7;
        map.insert(crate::HostRepositorySourceRoute::root(route.clone()), index);
    }
    assert_eq!(map.len(), entries.len());
    for (index, route) in entries.iter().enumerate() {
        assert_eq!(
            map.get(&crate::HostRepositorySourceRoute::root(route.clone())),
            Some(&index)
        );
    }
    let equal = local(3);
    let foreign = local(3);
    assert!(!Arc::ptr_eq(&equal.data, &foreign.data));
    assert_eq!(equal, foreign);
    assert_eq!(equal.data.routing_hash, foreign.data.routing_hash);
}

#[tokio::test]
async fn natural_root_and_selected_local_factories_initialize_complete_routing_value() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    for (name, text) in [
        (
            "dep",
            "module(name='root')\nbazel_dep(name='dep',version='1')\nlocal_path_override(module_name='dep',path='dep')\n",
        ),
        ("bazel_tools", "module(name='root')\n"),
    ] {
        let value = observed_route(&dice, EpochBuilder::root(text, 1).build(), name).await;
        let SourcePreparationOutcome::Complete(value) = value else {
            panic!("complete root fixture")
        };
        check(value.as_ref().as_ref().unwrap());
    }
    check(&local(8));
    let source = HostSelectedExtensionDefinitionSource::BuiltinBazelTools {
        route_identity: BuiltinBazelToolsSnapshot::CURRENT.route_identity(),
    };
    check(&RootRepositoryRoute::for_selected_extension_definition(workspace(), &source).unwrap());
    assert!(
        RootRepositoryRoute::for_selected_extension_definition(
            workspace(),
            &HostSelectedExtensionDefinitionSource::Root
        )
        .is_none()
    );
    assert!(
        RootRepositoryRoute::for_generated_repo_spec(
            workspace(),
            ApparentRepoName::new("bad").unwrap(),
            CanonicalRepoName::new("bazel_tools").unwrap(),
            local_spec("x"),
            crate::HostRepositoryLocalPathPolicy::LocalUnsupported,
            GeneratedRepositoryFileEffectPlan::build([]).unwrap()
        )
        .is_none()
    );
}

#[derive(allocative::Allocative)]
struct Before {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
    module_name: CompactString,
    canonical_repo: CanonicalRepoName,
    source: RootRepositorySource,
}
#[derive(allocative::Allocative)]
#[allow(dead_code)]
enum BeforeSourceRoute {
    Root(Before),
    Canonical(crate::HostCanonicalRepositorySourceInput),
}
#[derive(allocative::Allocative)]
struct BeforeDirectKey {
    route: Before,
    repo_relative_path: PathBuf,
}
#[derive(allocative::Allocative)]
struct BeforeListingKey {
    route: BeforeSourceRoute,
    directory: slug_identity_v2::PackagePath,
}
fn before(route: &RootRepositoryRoute) -> Before {
    Before {
        workspace: route.data.workspace.clone(),
        apparent_repo: route.data.apparent_repo.clone(),
        module_name: route.data.module_name.clone(),
        canonical_repo: route.data.canonical_repo.clone(),
        source: route.data.source.clone(),
    }
}
fn retained(roots: &[&dyn allocative::Allocative]) -> usize {
    let mut visitor = allocative::FlameGraphBuilder::default();
    for root in roots {
        visitor.visit_root(*root);
    }
    visitor.finish().flamegraph().total_size()
}

#[test]
fn shared_data_layout_unique_overhead_clone_savings_and_last_drop_are_counted() {
    let route = local(64);
    let prior = before(&route);
    let old = std::mem::size_of::<Before>();
    let handle = std::mem::size_of::<RootRepositoryRoute>();
    let data = std::mem::size_of::<RootRepositoryRouteData>();
    let allocation = std::alloc::Layout::new::<[usize; 2]>()
        .extend(std::alloc::Layout::new::<RootRepositoryRouteData>())
        .unwrap()
        .0
        .pad_to_align()
        .size();
    assert_eq!(data, old + std::mem::size_of::<u64>());
    assert_eq!(handle, std::mem::size_of::<Arc<RootRepositoryRouteData>>());
    let unique_extra = handle + allocation - old;
    assert!(unique_extra > 0);
    assert_eq!(retained(&[&route]), retained(&[&prior]) + unique_extra);
    let old_source = std::mem::size_of::<BeforeSourceRoute>();
    let new_source = std::mem::size_of::<crate::HostRepositorySourceRoute>();
    assert!(new_source < old_source);
    assert!(new_source <= 256);
    eprintln!(
        "ROOT_ROUTE_SHARED_LAYOUT old={old} handle={handle} data={data} allocation={allocation} unique_extra={unique_extra} source_old={old_source} source_new={new_source}"
    );

    // One traversal per simultaneously live cluster counts the shared data and
    // its actual Arc header once. No sum of independently visited Arc owners.
    let clone = route.clone();
    assert!(Arc::ptr_eq(&route.data, &clone.data));
    let prior_clone = before(&route);
    let one_clone = retained(&[&route, &clone]);
    let old_one_clone = retained(&[&prior, &prior_clone]);
    assert!(one_clone < old_one_clone);
    let clones = (0..64).map(|_| route.clone()).collect::<Vec<_>>();
    let priors = (0..64).map(|_| before(&route)).collect::<Vec<_>>();
    assert!(
        clones
            .iter()
            .all(|clone| Arc::ptr_eq(&route.data, &clone.data))
    );
    let many = retained(&[&route, &clones]);
    let old_many = retained(&[&prior, &priors]);
    assert!(many < old_many);

    let directory = slug_identity_v2::PackagePath::parse("pkg").unwrap();
    let old_direct = BeforeDirectKey {
        route: before(&route),
        repo_relative_path: "leaf.bzl".into(),
    };
    let direct = crate::HostRepositorySourceFileKey::new(route.clone(), "leaf.bzl".into());
    let old_listing = BeforeListingKey {
        route: BeforeSourceRoute::Root(before(&route)),
        directory: directory.clone(),
    };
    let listing = crate::HostRepositoryDirectoryListingKey::new(route.clone(), directory);
    let old_source_value = BeforeSourceRoute::Root(before(&route));
    let source_value = crate::HostRepositorySourceRoute::root(route.clone());
    let cluster = retained(&[&route, &source_value, &direct, &listing]);
    let old_cluster = retained(&[&prior, &old_source_value, &old_direct, &old_listing]);
    assert!(cluster < old_cluster);
    assert_eq!(
        retained(&[&direct]),
        retained(&[&old_direct]) + unique_extra
    );
    // The source enum's smaller handle is included, as is the new shared owner.
    let old_listing_bytes = retained(&[&old_listing]);
    let listing_bytes = retained(&[&listing]);
    assert_eq!(
        listing_bytes + old_source,
        old_listing_bytes + new_source + allocation
    );
    eprintln!(
        "ROOT_ROUTE_SHARED_CLUSTERS clone_old={old_one_clone} clone_new={one_clone} many_old={old_many} many_new={many} holders_old={old_cluster} holders_new={cluster} direct_old={} direct_new={} listing_old={} listing_new={}",
        std::mem::size_of_val(&old_direct),
        std::mem::size_of_val(&direct),
        std::mem::size_of_val(&old_listing),
        std::mem::size_of_val(&listing)
    );

    let data_weak = Arc::downgrade(&route.data);
    let mapping = route.bzl_repository_mapping();
    let mapping_weak = Arc::downgrade(&mapping);
    let spec_weak = Arc::downgrade(&route.repo_spec().attributes);
    assert!(Arc::ptr_eq(&mapping, &clone.bzl_repository_mapping()));
    drop(mapping);
    drop(prior);
    drop(prior_clone);
    drop(priors);
    drop(old_direct);
    drop(old_listing);
    drop(old_source_value);
    drop(route);
    drop(clone);
    drop(clones);
    drop(source_value);
    drop(direct);
    assert!(data_weak.upgrade().is_some());
    assert!(mapping_weak.upgrade().is_some());
    assert!(spec_weak.upgrade().is_some());
    drop(listing);
    assert!(data_weak.upgrade().is_none());
    assert!(mapping_weak.upgrade().is_none());
    assert!(spec_weak.upgrade().is_none());
}
