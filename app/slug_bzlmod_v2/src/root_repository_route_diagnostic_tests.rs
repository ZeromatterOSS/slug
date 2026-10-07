use super::*;

#[test]
fn selected_command_local_route_shares_mapping_and_classifies_source() {
    fn spec(path: &str) -> RepoSpec {
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
    fn retained(roots: &[&dyn Allocative]) -> usize {
        let mut builder = allocative::FlameGraphBuilder::default();
        for root in roots {
            builder.visit_root(*root);
        }
        builder.finish().flamegraph().total_size()
    }
    fn route_hash(route: &RootRepositoryRoute) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        route.hash(&mut hasher);
        hasher.finish()
    }
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    let pairs = [
        ("dep", "dep+1.0"),
        ("alpha", "alpha+1.0"),
        ("beta", "beta+1.0"),
        ("gamma", "gamma+1.0"),
        ("delta", "delta+1.0"),
        ("epsilon", "epsilon+1.0"),
        ("zeta", "zeta+1.0"),
        ("eta", "eta+1.0"),
    ];
    assert!(pairs.iter().all(|(a, c)| a.len() <= 64 && c.len() <= 64));
    let source = crate::selected_repo_spec::testing_selected_command_local_source(
        spec("/selected-dep"),
        crate::HostRepositoryLocalPathPolicy::CommandAbsolute,
        &pairs,
    );
    let HostSelectedExtensionDefinitionSource::Selected { definition, .. } = &source else {
        panic!("selected Local source");
    };
    let attrs_weak = Arc::downgrade(&definition.view().repo_spec().unwrap().attributes);
    let route =
        RootRepositoryRoute::for_selected_extension_definition(workspace.dupe(), &source).unwrap();
    let RootRepositorySource::SelectedLocal(local) = route.source() else {
        panic!("Local definition must retain its own checked route variant");
    };
    assert_eq!(route.apparent_repo().as_str(), "dep");
    assert_eq!(route.canonical_repo().as_str(), "dep+1.0");
    assert_eq!(route.associated_module_metadata(), Some(("dep", "0.2.17")));
    assert!(std::ptr::eq(
        route.repo_spec(),
        definition.view().repo_spec().unwrap()
    ));
    assert_eq!(local.mapping.len(), 8);
    for (actual, expected) in local.mapping.iter().zip(pairs) {
        assert_eq!((actual.0.as_str(), actual.1.as_str()), expected);
    }
    let first = route.bzl_repository_mapping();
    let second = route.bzl_repository_mapping();
    assert!(Arc::ptr_eq(&first, &second));
    assert!(Arc::ptr_eq(&first, &local.mapping));
    assert!(!crate::HostRepositorySourceRoute::root(route.clone()).is_immutable_materialization());
    let capability = route.source_capability();
    assert_eq!(
        capability.local_path_policy(),
        Some(crate::HostRepositoryLocalPathPolicy::CommandAbsolute)
    );
    assert!(Arc::ptr_eq(
        &capability.repo_spec().unwrap().attributes,
        &definition.view().repo_spec().unwrap().attributes
    ));
    let crate::HostRepositoryMaterializationDisposition::Request(request) =
        crate::host_repository_materialization_request(&capability).unwrap()
    else {
        panic!("ordinary Local materialization request");
    };
    assert_eq!(request.id.canonical_repo, *route.canonical_repo());
    assert!(
        matches!(&request.kind, crate::RepositoryMaterializationKind::Local { logical_root }
        if logical_root.as_path() == std::path::Path::new("/selected-dep"))
    );
    assert!(
        matches!(route.selected_bzl_load_route(&ApparentRepoName::new("dep").unwrap()),
        Some(RootRepositoryBzlLoadRoute::Canonical(name)) if name == *route.canonical_repo())
    );
    assert!(
        matches!(route.selected_bzl_load_route(&ApparentRepoName::new("alpha").unwrap()),
        Some(RootRepositoryBzlLoadRoute::Canonical(name)) if name.as_str() == "alpha+1.0")
    );
    assert!(
        route
            .selected_bzl_load_route(&ApparentRepoName::new("unknown").unwrap())
            .is_none()
    );

    let baseline = retained(&[&source]);
    let combined = retained(&[&source, &route]);
    let increment = combined.checked_sub(baseline).unwrap();
    assert!(
        increment <= 4096,
        "incremental retained Local route bytes: {increment}"
    );
    let with_capability = retained(&[&source, &route, &capability]);
    let with_request = retained(&[&source, &route, &capability, &request]);
    eprintln!(
        "selected Local retained increment: route={increment} capability={} request={}",
        with_capability.checked_sub(combined).unwrap(),
        with_request.checked_sub(with_capability).unwrap()
    );
    let clone = route.clone();
    let RootRepositorySource::SelectedLocal(cloned) = clone.source() else {
        unreachable!()
    };
    assert!(Arc::ptr_eq(&local.mapping, &cloned.mapping));
    assert!(std::ptr::eq(
        local.definition.view().repo_spec().unwrap(),
        cloned.definition.view().repo_spec().unwrap()
    ));
    assert!(std::ptr::eq(
        local.definition.view().mapping().next().unwrap().0,
        cloned.definition.view().mapping().next().unwrap().0
    ));
    assert_eq!(route, clone);
    assert_eq!(route_hash(&route), route_hash(&clone));
    let mapping_weak = Arc::downgrade(&local.mapping);
    drop(first);
    drop(second);
    drop(clone);
    drop(request);
    drop(capability);
    let route = Arc::new(route);
    let weak = Arc::downgrade(&route);
    let clones: Vec<_> = (0..64).map(|_| route.dupe()).collect();
    assert_eq!(Arc::strong_count(&route), 65);
    assert!(clones.iter().all(|clone| Arc::ptr_eq(&route, clone)));
    drop(route);
    assert!(weak.upgrade().is_some());
    drop(clones);
    assert!(weak.upgrade().is_none());
    assert!(mapping_weak.upgrade().is_none());
    assert!(attrs_weak.upgrade().is_some());
    drop(source);
    assert!(attrs_weak.upgrade().is_none());

    for policy in [
        crate::HostRepositoryLocalPathPolicy::WorkspaceRelative,
        crate::HostRepositoryLocalPathPolicy::LocalUnsupported,
    ] {
        let source = crate::selected_repo_spec::testing_selected_command_local_source(
            spec("/selected-dep"),
            policy,
            &pairs,
        );
        assert!(
            RootRepositoryRoute::for_selected_extension_definition(workspace.dupe(), &source)
                .is_none()
        );
    }
    let mut source = crate::selected_repo_spec::testing_selected_command_local_source(
        spec("/selected-dep"),
        crate::HostRepositoryLocalPathPolicy::CommandAbsolute,
        &pairs,
    );
    let HostSelectedExtensionDefinitionSource::Selected { apparent_repo, .. } = &mut source else {
        unreachable!()
    };
    *apparent_repo = ApparentRepoName::root();
    assert!(
        RootRepositoryRoute::for_selected_extension_definition(workspace.dupe(), &source).is_none()
    );
    let HostSelectedExtensionDefinitionSource::Selected { apparent_repo, .. } = &mut source else {
        unreachable!()
    };
    *apparent_repo = ApparentRepoName::new("caller_alias").unwrap();
    let caller_route =
        RootRepositoryRoute::for_selected_extension_definition(workspace.dupe(), &source).unwrap();
    assert_eq!(caller_route.apparent_repo().as_str(), "caller_alias");
    assert_eq!(caller_route.canonical_repo().as_str(), "dep+1.0");
    let crate::HostRepositoryMaterializationDisposition::Request(caller_request) =
        crate::host_repository_materialization_request(&caller_route.source_capability()).unwrap()
    else {
        panic!("caller alias keeps the normal Local request")
    };
    assert_eq!(
        caller_request.id.canonical_repo,
        *caller_route.canonical_repo()
    );
    assert!(
        matches!(&caller_request.kind, crate::RepositoryMaterializationKind::Local { logical_root }
        if logical_root.as_path() == std::path::Path::new("/selected-dep"))
    );
    for invalid in ["relative", "/selected-dep/../escape", ""] {
        let source = crate::selected_repo_spec::testing_selected_command_local_source(
            spec(invalid),
            crate::HostRepositoryLocalPathPolicy::CommandAbsolute,
            &pairs,
        );
        assert!(
            RootRepositoryRoute::for_selected_extension_definition(workspace.dupe(), &source)
                .is_none()
        );
    }
    let a = crate::selected_repo_spec::testing_selected_command_local_source(
        spec("/selected-dep"),
        crate::HostRepositoryLocalPathPolicy::CommandAbsolute,
        &pairs,
    );
    let b = crate::selected_repo_spec::testing_selected_command_local_source(
        spec("/changed-dep"),
        crate::HostRepositoryLocalPathPolicy::CommandAbsolute,
        &pairs,
    );
    let a = RootRepositoryRoute::for_selected_extension_definition(workspace.dupe(), &a).unwrap();
    let b = RootRepositoryRoute::for_selected_extension_definition(workspace, &b).unwrap();
    assert_ne!(a, b);
    assert_ne!(route_hash(&a), route_hash(&b));
}

#[test]
fn route_diagnostic_distinguishes_original_safe_and_opaque_stages() {
    let apparent_repo = ApparentRepoName::new("external_repo").unwrap();
    let cases = [
        (
            RootRepositoryRouteErrorKind::Unknown {
                apparent_repo: apparent_repo.clone(),
            },
            "Unknown @external_repo",
            true,
        ),
        (
            RootRepositoryRouteErrorKind::Unsupported {
                apparent_repo,
                module_name: CompactString::new("original_module"),
            },
            "Unsupported @external_repo module=original_module",
            true,
        ),
        (
            RootRepositoryRouteErrorKind::Root(HostRootModuleFileError::CommandPolicy {
                message: CompactString::new("OPAQUE_POISON"),
            }),
            "Root [diagnostic incomplete: Root]",
            false,
        ),
        (
            RootRepositoryRouteErrorKind::SelectedInfrastructure(Arc::from("ORIGINAL_CAUSE")),
            "SelectedInfrastructure: ORIGINAL_CAUSE",
            false,
        ),
    ];
    for (kind, expected, fallback) in cases {
        let owner = Arc::new(RootRepositoryRouteError { kind });
        let weak = Arc::downgrade(&owner);
        let count = Arc::strong_count(&owner);
        let mut text = String::new();
        owner.write_route_diagnostic(&mut text).unwrap();
        assert_eq!(text, expected);
        assert!(!text.contains("OPAQUE_POISON"));
        assert_eq!(owner.is_generated_route_fallback(), fallback);
        assert_eq!(Arc::strong_count(&owner), count);
        drop(owner);
        assert!(weak.upgrade().is_none());
    }
}

#[test]
fn route_diagnostic_borrows_message_and_propagates_writer_stop() {
    let message: Arc<str> = Arc::from("original \"cause\"\n\u{e9}\t");
    let weak = Arc::downgrade(&message);
    let owner = RootRepositoryRouteError {
        kind: RootRepositoryRouteErrorKind::SelectedInfrastructure(message),
    };
    struct BorrowWitness {
        pointer: *const u8,
        length: usize,
        seen: bool,
    }
    impl fmt::Write for BorrowWitness {
        fn write_str(&mut self, text: &str) -> fmt::Result {
            if text.len() == self.length && text.as_ptr() == self.pointer {
                self.seen = true;
            }
            Ok(())
        }
    }
    let mut witness = BorrowWitness {
        pointer: weak.as_ptr() as *const u8,
        length: "original \"cause\"\n\u{e9}\t".len(),
        seen: false,
    };
    owner.write_route_diagnostic(&mut witness).unwrap();
    assert!(witness.seen);
    struct Reject(usize);
    impl fmt::Write for Reject {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            self.0 += 1;
            Err(fmt::Error)
        }
    }
    let mut reject = Reject(0);
    assert!(owner.write_route_diagnostic(&mut reject).is_err());
    assert_eq!(reject.0, 1);
    let mut text = String::new();
    owner.write_route_diagnostic(&mut text).unwrap();
    assert_eq!(text, "SelectedInfrastructure: original \"cause\"\n\u{e9}\t");
    // Escaping and byte limits belong to the receiving Loading sink. The
    // natural owner streams the original message without copying its storage.
    assert!(weak.upgrade().is_some());
    drop(owner);
    assert!(weak.upgrade().is_none());
}
