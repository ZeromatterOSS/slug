use super::*;

#[test]
fn platforms_os_package_need_is_opt_in_exact_slot_two_and_captured_rows_only() {
    for relative in PLATFORMS_OS_METADATA {
        let operations: &[PathObservationOperation] = if PLATFORMS_OS_SOURCES.contains(&relative) {
            &[
                PathObservationOperation::Lstat,
                PathObservationOperation::FileBytes,
            ]
        } else {
            &[PathObservationOperation::Lstat]
        };
        let selected = format!("/platforms/{relative}");
        assert_exact_root_file_role(
            relative,
            operations,
            Bodies::with_platforms_os_package_sources,
            |b| b.platforms_os_package_sources,
            &[
                "/platforms/OS",
                "/platforms/os/build",
                "/platforms/os/BUILD.other",
                "/platforms/os/BUILD/child",
                "/platforms/os/BUILD.bazel/child",
                "/platforms/os/sub/BUILD",
                "/platforms-other/os/BUILD",
                "/other/os/BUILD",
                "/platforms/cpu/BUILD",
            ],
            &selected,
            Some("/platforms"),
        );
    }
    let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
    let make = || Bodies::new([path("/fixture"), path("/rules"), path("/platforms")]);
    let need =
        |namespace, spelling: &str, operation| {
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(namespace, path(spelling), operation),
            ))
        };
    let host = PathObservationNamespace::Host;
    let body = need(
        host,
        "/platforms/os/BUILD",
        PathObservationOperation::FileBytes,
    );
    for isolated in [
        make(),
        make()
            .with_platforms_host_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe),
        make().with_platforms_host_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        ),
        make().with_platforms_host_directory(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        ),
        make().with_platforms_cpu_package_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        ),
    ] {
        assert!(!isolated.platforms_os_package_sources);
        assert_eq!(isolated.admit(&body).unwrap_err().family, "path");
    }
    let wrong_slot = Bodies::new([path("/fixture"), path("/platforms"), path("/rules")])
        .with_platforms_os_package_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
    assert_eq!(wrong_slot.admit(&body).unwrap_err().family, "path");
    let bodies = Arc::new(make().with_platforms_os_package_sources(
        RegistryChoice::PreparedFile,
        CommandChoice::PublicProbe,
    ));
    // The new permission does not alter source-attempt profile selection.
    assert_eq!(
        bodies
            .source_limits(RegistryChoice::PreparedFile)
            .source_attempts,
        16
    );
    let mut allowed = body;
    for relative in PLATFORMS_OS_METADATA {
        allowed = allowed
            .try_union(&need(
                host,
                &format!("/platforms/{relative}"),
                PathObservationOperation::Lstat,
            ))
            .unwrap();
    }
    assert_eq!(allowed.path_observations().unwrap().demands().len(), 4);
    assert!(bodies.admit(&allowed).is_ok());
    let root = BuildCommandRootKey::new_with_overlay(
        path("/fixture"),
        &[TargetPattern::parse("//pkg:probe").unwrap()],
        build_test_configuration("os-whole-need"),
        nightly_overlay(),
    )
    .unwrap();
    let milestones = Arc::new(Mutex::new(Milestones::default()));
    let guarded = GuardedSingleton {
        inner: SingletonRootSingleBuildCommandKey::new(root).unwrap(),
        bodies: bodies.clone(),
        milestones: milestones.clone(),
    };
    assert!(guarded.check_need(&allowed).is_ok());
    let mut progress = 0;
    for (excluded, family) in [
        (
            need(host, "/platforms/os", PathObservationOperation::FileBytes),
            "path",
        ),
        // Unavailable marker remains Lstat-only, never a body substitution.
        (
            need(
                host,
                "/platforms/os/BUILD.bazel",
                PathObservationOperation::FileBytes,
            ),
            "path",
        ),
        (
            need(
                host,
                "/platforms/os/BUILD",
                PathObservationOperation::FileDigest,
            ),
            "path",
        ),
        (
            need(
                host,
                "/platforms/os/BUILD",
                PathObservationOperation::DirectoryEntries,
            ),
            "operation",
        ),
        (
            need(
                host,
                "/platforms/os/BUILD.bazel",
                PathObservationOperation::DirectoryEntries,
            ),
            "operation",
        ),
        (
            need(
                host,
                "/platforms/os/child",
                PathObservationOperation::DirectoryEntries,
            ),
            "operation",
        ),
        (
            need(
                host,
                "/platforms/os",
                PathObservationOperation::DirectoryEntries,
            ),
            "operation",
        ),
        (
            need(
                host,
                "/platforms/cpu/BUILD",
                PathObservationOperation::FileBytes,
            ),
            "path",
        ),
        (
            need(
                PathObservationNamespace::Materialization(
                    slug_workspace_v2::PathObservationInstanceId::new(1),
                ),
                "/platforms/os/BUILD",
                PathObservationOperation::FileBytes,
            ),
            "namespace",
        ),
    ] {
        for mixed in [
            allowed.try_union(&excluded).unwrap(),
            excluded.try_union(&allowed).unwrap(),
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
            progress += usize::from(guarded.check_need(&mixed).is_ok());
            let recorded = milestones.lock().unwrap();
            let denied = recorded.denied.as_ref().unwrap();
            assert_eq!(denied.family, family);
            assert_eq!(denied.operation, path_operation_code(&expected.operation()));
            assert_eq!(denied.path.as_ref(), Some(expected.path()));
            assert_eq!((recorded.calls, recorded.preparations), (0, 0));
        }
    }
    let local = SourcePreparationNeeds::repository(
        (*local_native_request(&bodies.roots[0], "unknown+", "/platforms")).clone(),
    );
    let original = local.repository_materializations().values().next().unwrap();
    let held = original.clone();
    let request_weak = Arc::downgrade(original);
    for mixed in [
        allowed.try_union(&local).unwrap(),
        local.try_union(&allowed).unwrap(),
    ] {
        let (first, reason) = bodies
            .first_denied_materialization_with_plan_matcher(
                &mixed,
                &generated_features_plan_matches,
            )
            .unwrap();
        assert!(std::ptr::eq(first, original.as_ref()));
        assert!(Arc::ptr_eq(
            original,
            mixed.repository_materializations().values().next().unwrap()
        ));
        assert_eq!(reason, "binding");
        assert_eq!(bodies.admit(&mixed).unwrap_err().family, "materialization");
        progress += usize::from(guarded.check_need(&mixed).is_ok());
    }
    assert_eq!(progress, 0);
    {
        let recorded = milestones.lock().unwrap();
        assert_eq!((recorded.calls, recorded.preparations), (0, 0));
    }
    drop(local);
    assert!(request_weak.upgrade().is_some());
    drop(held);
    assert!(request_weak.upgrade().is_none());
    let bodies_weak = Arc::downgrade(&bodies);
    drop(guarded);
    assert!(bodies_weak.upgrade().is_some());
    drop(bodies);
    assert!(bodies_weak.upgrade().is_none());
}
