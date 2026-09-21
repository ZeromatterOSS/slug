use compact_str::CompactString;

fn feature_strings(values: &[&str]) -> Arc<[CompactString]> {
    values.iter().map(|value| (*value).into()).collect()
}

fn root_feature_epoch(repo: Option<&str>, build: &str) -> PathObservationEpoch {
    let mut epoch = EpochBuilder::workspace_sources("", build, &[("leaf.bzl", "VALUE=1\n")], 975);
    if let Some(repo) = repo {
        epoch.file("/workspace/REPO.bazel", repo, 975);
    }
    epoch.build()
}

#[tokio::test]
async fn root_repo_feature_only_changes_restore_package_and_leave_bzl_payload_unrelated() {
    use slug_bzlmod_v2::RootPackageBzlTarget;
    use slug_bzlmod_v2::RootPackageSourceObservationKey;
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let key = observed_package_key();
    let bzl_key = RootPackageSourceObservationKey::for_bzl(
        workspace(),
        PackagePath::parse("pkg").unwrap(),
        RootPackageBzlTarget::parse("leaf.bzl").unwrap(),
    );
    const BUILD: &str = "package(features=['package', '-off'])\nfilegroup(name='same')\n";
    let mut first = None;
    let mut first_bzl = None;
    for (repo, expected) in [
        (
            Some("repo(features=['a','off'])"),
            Some(vec!["a", "package"]),
        ),
        (
            Some("repo(features=['b','off'])"),
            Some(vec!["b", "package"]),
        ),
        (None, Some(vec!["package"])),
        (Some("repo(features=[1])"), None),
        (
            Some("repo(features=['a','off'])"),
            Some(vec!["a", "package"]),
        ),
    ] {
        let mut tx = transaction(&dice, root_feature_epoch(repo, BUILD), false, None).await;
        let outcome = tx.compute(&key).await.unwrap();
        let result = observed_package(&outcome).result();
        if let Some(expected) = expected {
            let loaded = result.as_ref().as_ref().unwrap();
            assert_eq!(
                loaded.feature_defaults.resolve(&[], &[]).enabled(),
                feature_strings(&expected).as_ref()
            );
            assert_eq!(
                loaded.feature_defaults.resolve(&[], &[]).disabled(),
                ["off"]
            );
            if loaded
                .feature_defaults
                .repository()
                .first()
                .is_some_and(|v| v == "a")
            {
                if let Some(first) = &first {
                    assert_eq!(first, result);
                } else {
                    first = Some(result.clone());
                }
            } else if let Some(first) = &first {
                assert_ne!(first, result);
            }
            let bzl = tx.compute(&bzl_key).await.unwrap();
            let LoadingPreparationOutcome::Complete(Ok(bzl)) = bzl else {
                panic!("{bzl:?}")
            };
            assert!(
                bzl.result()
                    .as_ref()
                    .as_ref()
                    .unwrap()
                    .repository_features()
                    .is_empty()
            );
            if let Some(first) = &first_bzl {
                assert_eq!(first, bzl.result());
            } else {
                first_bzl = Some(bzl.result().clone());
            }
        } else {
            assert!(result.is_err());
        }
    }
}

#[tokio::test]
async fn external_repo_features_follow_selected_route_and_restore_without_build_edits() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut first = None;
    for (repo, expected) in [
        (Some("repo(features=['a'])"), Some("a")),
        (Some("repo(features=['b'])"), Some("b")),
        (None, Some("")),
        (Some("repo(features=None)"), None),
        (Some("repo(features=['a'])"), Some("a")),
    ] {
        let mut epoch = EpochBuilder::external_sources(
            &[(
                "BUILD.bazel",
                b"package(features=['package'])\nfilegroup(name='same')",
            )],
            976,
        );
        epoch.file("/workspace/REPO.bazel", "repo(features=['root_only'])", 976);
        if let Some(repo) = repo {
            epoch.file("/workspace/dep/REPO.bazel", repo, 976);
        }
        let mut tx = transaction(&dice, epoch.build(), false, None).await;
        let route = external_route(&mut tx).await;
        let key = RepositoryPackageLoadObservationKey::new(route, PackagePath::root());
        let outcome = tx.compute(&key).await.unwrap();
        let result = observed_repository_package(&outcome).result();
        if let Some(expected) = expected {
            let loaded = result.as_ref().as_ref().unwrap();
            let expected = if expected.is_empty() {
                vec!["package"]
            } else {
                vec![expected, "package"]
            };
            assert_eq!(
                loaded.feature_defaults.resolve(&[], &[]).enabled(),
                feature_strings(&expected).as_ref()
            );
            if expected[0] == "a" {
                if let Some(first) = &first {
                    assert_eq!(first, result);
                } else {
                    first = Some(result.clone());
                }
            } else if let Some(first) = &first {
                assert_ne!(first, result);
            }
        } else {
            assert!(result.is_err());
        }
    }
}

fn canonical_feature_epoch(
    base: &PathObservationEpoch,
    instance: PathObservationInstanceId,
    repo: Option<&str>,
) -> PathObservationEpoch {
    let mut epoch = EpochBuilder {
        entries: base
            .observations()
            .iter()
            .map(|(demand, value)| (demand.clone(), value.as_ref().clone()))
            .collect(),
    };
    if let Some(repo) = repo {
        epoch.materialized_file(instance, "/registry-dep/REPO.bazel", repo, 977);
    } else {
        epoch.materialized_missing(instance, "/registry-dep/REPO.bazel");
    }
    epoch.file("/workspace/REPO.bazel", "repo(features=['root_only'])", 977);
    epoch.build()
}

#[tokio::test]
async fn canonical_materialized_repo_features_restore_through_complete_observed_inventory() {
    let (base, instance) = selected_registry_root_package_epoch();
    let (mut tx, _) = selected_registry_visibility_transaction(
        canonical_feature_epoch(&base, instance, Some("repo(features=['a'])")),
        instance,
        None,
    )
    .await;
    let route = tx
        .compute(&HostCanonicalRepositoryLoadRouteObservationKey::new(
            workspace(),
            CanonicalRepoName::new("dep+").unwrap(),
        ))
        .await
        .unwrap();
    let LoadingPreparationOutcome::Complete(Ok(route)) = route else {
        panic!("{route:?}")
    };
    let input = route.result().as_ref().as_ref().unwrap().input().clone();
    let key = RepositoryPackageLoadObservationKey::new_canonical(input, PackagePath::root());
    let mut first = None;
    for (repo, expected) in [
        (Some("repo(features=['a'])"), Some(vec!["a"])),
        (Some("repo(features=['b'])"), Some(vec!["b"])),
        (None, Some(vec![])),
        (Some("repo(features=[False])"), None),
        (Some("repo(features=['a'])"), Some(vec!["a"])),
    ] {
        let mut updater = tx.into_updater();
        let epoch = canonical_feature_epoch(&base, instance, repo);
        updater.changed_to(path_observation_shards(&epoch)).unwrap();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        tx = updater.commit().await;
        let outcome = tx.compute(&key).await.unwrap();
        let observed = observed_repository_package(&outcome);
        let result = observed.result();
        if let Some(expected) = expected {
            let loaded = result.as_ref().as_ref().unwrap();
            assert_eq!(
                loaded.feature_defaults.resolve(&[], &[]).enabled(),
                feature_strings(&expected).as_ref()
            );
            if expected == vec!["a"] {
                if let Some(first) = &first {
                    assert_eq!(first, result);
                } else {
                    first = Some(result.clone());
                }
            } else if let Some(first) = &first {
                assert_ne!(first, result);
            }
        } else {
            assert!(result.is_err());
        }
        assert!(observed.observations().observations().keys().any(
            |demand| demand.namespace() == PathObservationNamespace::Materialization(instance)
        ));
    }
}

#[tokio::test]
async fn package_feature_calls_copy_values_and_validate_full_string_list_surface() {
    const BUILD: &str =
        "v=['a']\npackage(features=v)\nv.append('not_captured')\nfilegroup(name='same')\n";
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = transaction(
        &dice,
        root_feature_epoch(Some("repo(features=['repo'])"), BUILD),
        false,
        None,
    )
    .await;
    let result = tx.compute(&observed_package_key()).await.unwrap();
    let loaded = observed_package(&result)
        .result()
        .as_ref()
        .as_ref()
        .unwrap();
    assert_eq!(
        loaded.feature_defaults.package(),
        Some(feature_strings(&["a"]).as_ref())
    );
    assert_eq!(
        loaded.feature_defaults.resolve(&[], &[]).enabled(),
        feature_strings(&["a", "repo"]).as_ref()
    );
    for (value, expected) in [
        ("('tuple',)", vec!["tuple"]),
        ("range(0)", vec![]),
        ("depset(['depset'])", vec!["depset"]),
        ("{'key':False}", vec!["key"]),
        ("set(['set'])", vec!["set"]),
    ] {
        let mut tx = transaction(
            &dice,
            root_feature_epoch(None, &format!("package(features={value})\n")),
            false,
            None,
        )
        .await;
        let result = tx.compute(&observed_package_key()).await.unwrap();
        let loaded = observed_package(&result)
            .result()
            .as_ref()
            .as_ref()
            .unwrap();
        assert_eq!(
            loaded.feature_defaults.resolve(&[], &[]).enabled(),
            feature_strings(&expected).as_ref()
        );
    }
    for invalid in ["None", "'text'", "[1]", "{1:True}", "range(1)"] {
        let mut tx = transaction(
            &dice,
            root_feature_epoch(None, &format!("package(features={invalid})\n")),
            false,
            None,
        )
        .await;
        let result = tx.compute(&observed_package_key()).await.unwrap();
        assert!(observed_package(&result).result().is_err(), "{invalid}");
    }
}

#[tokio::test]
async fn package_feature_boundary_rejects_second_call_and_no_arguments_for_both_spellings() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    for first in ["package", "native.package"] {
        for second in ["package", "native.package"] {
            for argument in ["default_testonly=True", "features=['a']"] {
                let source = format!("{first}({argument})\n{second}(features=['b'])\n");
                let mut tx =
                    transaction(&dice, root_feature_epoch(None, &source), false, None).await;
                let outcome = tx.compute(&observed_package_key()).await.unwrap();
                let error = observed_package(&outcome)
                    .result()
                    .as_ref()
                    .as_ref()
                    .unwrap_err();
                assert!(
                    error.to_string().contains("can only be used once"),
                    "{source}: {error}"
                );
            }
        }
        let mut tx = transaction(
            &dice,
            root_feature_epoch(None, &format!("{first}()\n")),
            false,
            None,
        )
        .await;
        let outcome = tx.compute(&observed_package_key()).await.unwrap();
        assert!(
            observed_package(&outcome)
                .result()
                .as_ref()
                .as_ref()
                .unwrap_err()
                .to_string()
                .contains("at least one argument")
        );
    }
}
