use super::*;

#[test]
fn platforms_host_directory_need_is_opt_in_exact_slot_two_only() {
    assert_exact_root_file_role(
        "host",
        &[PathObservationOperation::DirectoryEntries],
        Bodies::with_platforms_host_directory,
        |b| b.platforms_host_directory,
        &[
            "/platforms/HOST",
            "/platforms/Host",
            "/platforms/host.other",
            "/platforms/host/child",
            "/platforms/host/BUILD",
            "/platforms/host/constraints.bzl",
            "/other/host",
            "/platforms-other/host",
            "/platforms/sub/host",
        ],
        "/platforms/host",
        Some("/platforms"),
    );
    let path = |s| NormalizedAbsolutePath::new(s).unwrap();
    let make = || Bodies::new([path("/fixture"), path("/rules"), path("/platforms")]);
    let need =
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Host,
            path("/platforms/host"),
            PathObservationOperation::DirectoryEntries,
        )));
    for isolated in [
        make(),
        make()
            .with_platforms_host_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe),
        make().with_platforms_host_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        ),
    ] {
        assert!(!isolated.platforms_host_directory);
        let denied = isolated.admit(&need).unwrap_err();
        assert_eq!(
            (denied.family, denied.operation),
            ("operation", "directory_entries")
        );
    }
    let wrong_slot = Bodies::new([path("/platforms"), path("/rules"), path("/fixture")])
        .with_platforms_host_directory(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    assert_eq!(wrong_slot.admit(&need).unwrap_err().family, "operation");
}

#[test]
fn platforms_host_directory_mixed_need_preserves_first_offender_before_progress() {
    let path = |s| NormalizedAbsolutePath::new(s).unwrap();
    let bodies = Arc::new(
        Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
            .with_platforms_host_directory(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            ),
    );
    let root = BuildCommandRootKey::new_with_overlay(
        path("/fixture"),
        &[TargetPattern::parse("//pkg:probe").unwrap()],
        build_test_configuration("directory-whole-need"),
        nightly_overlay(),
    )
    .unwrap();
    let milestones = Arc::new(Mutex::new(Milestones::default()));
    let guarded = GuardedSingleton {
        inner: SingletonRootSingleBuildCommandKey::new(root).unwrap(),
        bodies: bodies.clone(),
        milestones: milestones.clone(),
    };
    let need =
        |namespace, spelling, operation| {
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(namespace, path(spelling), operation),
            ))
        };
    let host = PathObservationNamespace::Host;
    let listing = need(
        host,
        "/platforms/host",
        PathObservationOperation::DirectoryEntries,
    );
    assert!(guarded.check_need(&listing).is_ok());
    let mut progress = 0;
    for (excluded, family) in [
        (
            need(host, "/platforms/host", PathObservationOperation::FileBytes),
            "path",
        ),
        (
            need(host, "/platforms/host", PathObservationOperation::ReadLink),
            "operation",
        ),
        (
            need(
                host,
                "/platforms/host/child",
                PathObservationOperation::DirectoryEntries,
            ),
            "operation",
        ),
        (
            need(
                PathObservationNamespace::Materialization(
                    slug_workspace_v2::PathObservationInstanceId::new(1),
                ),
                "/platforms/host",
                PathObservationOperation::DirectoryEntries,
            ),
            "namespace",
        ),
    ] {
        for mixed in [
            listing.try_union(&excluded).unwrap(),
            excluded.try_union(&listing).unwrap(),
        ] {
            let expected = mixed
                .path_observations()
                .unwrap()
                .demands()
                .iter()
                .find(|d| **d == excluded.path_observations().unwrap().demands()[0])
                .unwrap();
            let (first, actual_family) = bodies
                .first_denied_path_with_namespace_matcher(&mixed, &|_| false)
                .unwrap();
            assert!(std::ptr::eq(first, expected));
            assert_eq!(actual_family, family);
            let admitted = guarded.check_need(&mixed).is_ok();
            progress += usize::from(admitted);
            assert!(!admitted);
            let recorded = milestones.lock().unwrap();
            let denied = recorded.denied.as_ref().unwrap();
            assert_eq!(denied.family, family);
            assert_eq!(denied.operation, path_operation_code(&expected.operation()));
            assert_eq!(denied.path.as_ref(), Some(expected.path()));
            assert_eq!((recorded.calls, recorded.preparations), (0, 0));
        }
    }
    let bytes = need(host, "/platforms/host", PathObservationOperation::FileBytes);
    let child = need(
        host,
        "/platforms/host/child",
        PathObservationOperation::DirectoryEntries,
    );
    for mixed in [
        listing
            .try_union(&bytes)
            .unwrap()
            .try_union(&child)
            .unwrap(),
        child
            .try_union(&bytes)
            .unwrap()
            .try_union(&listing)
            .unwrap(),
    ] {
        let allowed = &listing.path_observations().unwrap().demands()[0];
        let expected = mixed
            .path_observations()
            .unwrap()
            .demands()
            .iter()
            .find(|d| *d != allowed)
            .unwrap();
        let (first, _) = bodies
            .first_denied_path_with_namespace_matcher(&mixed, &|_| false)
            .unwrap();
        assert!(std::ptr::eq(first, expected));
        assert!(guarded.check_need(&mixed).is_err());
    }
    let bootstrap =
        SourcePreparationNeeds::root_module_bootstrap(slug_bzlmod_v2::RootModuleBootstrapRequest {
            workspace: path("/foreign"),
        });
    let local = SourcePreparationNeeds::repository(
        (*local_native_request(&bodies.roots[0], "unknown+", "/platforms")).clone(),
    );
    let original = local.repository_materializations().values().next().unwrap();
    let weak = Arc::downgrade(original);
    for (excluded, family) in [(&bootstrap, "bootstrap"), (&local, "materialization")] {
        for mixed in [
            listing.try_union(excluded).unwrap(),
            excluded.try_union(&listing).unwrap(),
        ] {
            assert_eq!(bodies.admit(&mixed).unwrap_err().family, family);
            assert!(guarded.check_need(&mixed).is_err());
            assert_eq!(
                milestones.lock().unwrap().denied.as_ref().unwrap().family,
                family
            );
            if family == "materialization" {
                assert!(Arc::ptr_eq(
                    original,
                    mixed.repository_materializations().values().next().unwrap()
                ));
                let (first, _) = bodies
                    .first_denied_materialization_with_plan_matcher(
                        &mixed,
                        &generated_features_plan_matches,
                    )
                    .unwrap();
                assert!(std::ptr::eq(first, original.as_ref()));
            }
        }
    }
    assert_eq!(progress, 0);
    let recorded = milestones.lock().unwrap();
    assert_eq!((recorded.calls, recorded.preparations), (0, 0));
    drop(recorded);
    drop(local);
    assert!(weak.upgrade().is_none());
}
