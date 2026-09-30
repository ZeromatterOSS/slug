//! Selected data prerequisites reuse native filegroup's configured/runfiles owners.
use super::*;

#[tokio::test]
async fn native_data_policies_preserve_files_order_alias_kinds_and_auxiliary_runfiles() {
    let workspace = Workspace::new();
    for observed in [false, true] {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        for external in [false, true] {
            let original = workspace
                .request_data(&dice, observed, external, "data_mixed", configuration(&[]))
                .await
                .unwrap();
            for src_data in [true, false] {
                for include in [true, false] {
                    let target = configuration(&[
                        (
                            NativeCommandOption::IncompatibleFilegroupRunfilesForData,
                            if src_data { "true" } else { "false" },
                        ),
                        (
                            NativeCommandOption::IncompatibleAlwaysIncludeFilesInData,
                            if include { "true" } else { "false" },
                        ),
                    ]);
                    let structural = target.slug_configuration().unwrap();
                    let selected_exec = exec(&target);
                    assert_eq!(exec(&selected_exec), selected_exec);
                    for config in [
                        target.clone(),
                        ConfigurationKey::from_slug(structural.to_exec().unwrap()),
                        selected_exec,
                    ] {
                        let result = workspace
                            .request_data(&dice, observed, external, "data_mixed", config)
                            .await
                            .unwrap();
                        assert!(result.actions().is_empty());
                        let default = info(&result);
                        assert_eq!(paths(default.files()), source_paths(external, &["a.txt"]));
                        assert_eq!(
                            default.executable.as_ref(),
                            default.file_artifacts().first()
                        );
                        assert_eq!(default.files_to_run.executable, default.executable);
                        let mut expected_default = vec!["c.txt"];
                        if include {
                            expected_default.push("b.txt");
                        }
                        expected_default.extend(["d.txt", "default.txt"]);
                        let mut expected_data = vec!["a.txt", "c.txt"];
                        if include {
                            expected_data.push("b.txt");
                        }
                        expected_data.push("d.txt");
                        if src_data {
                            expected_data.push("data.txt");
                        }
                        assert_eq!(
                            paths(&default.default_runfiles.files),
                            source_paths(external, &expected_default)
                        );
                        assert_eq!(
                            paths(&default.data_runfiles.files),
                            source_paths(external, &expected_data)
                        );
                        for runfiles in [&default.default_runfiles, &default.data_runfiles] {
                            assert_eq!(runfiles.files.order(), DepsetOrder::Postorder);
                            assert!(runfiles.empty_filenames.is_empty());
                            assert_eq!(
                                runfiles.conflict_policy,
                                slug_build_api_v2::RunfilesConflictPolicy::Error
                            );
                        }
                        for (runfiles, expected) in [
                            (&default.default_runfiles, vec!["d.txt", "default.txt"]),
                            (
                                &default.data_runfiles,
                                if src_data {
                                    vec!["d.txt", "data.txt"]
                                } else {
                                    vec!["d.txt"]
                                },
                            ),
                        ] {
                            for links in [&runfiles.symlinks, &runfiles.root_symlinks] {
                                assert_eq!(
                                    links
                                        .to_list()
                                        .iter()
                                        .map(|link| link.artifact.path().into_owned())
                                        .collect::<Vec<_>>(),
                                    source_paths(external, &expected)
                                );
                            }
                        }
                        assert_eq!(
                            group_paths(&result, "_hidden_top_level_INTERNAL_"),
                            source_paths(external, &expected_default)
                        );
                        assert_eq!(
                            group_paths(&result, "_validation"),
                            source_paths(external, &["validation.txt", "c.txt"])
                        );
                    }
                    let restored = workspace
                        .request_data(&dice, observed, external, "data_mixed", target)
                        .await
                        .unwrap();
                    if src_data && include {
                        assert_eq!(restored, original);
                    }
                }
            }
            assert_eq!(
                workspace
                    .request_data(&dice, observed, external, "data_mixed", configuration(&[]))
                    .await
                    .unwrap(),
                original
            );
            for include in [false, true] {
                let config = configuration(&[(
                    NativeCommandOption::IncompatibleAlwaysIncludeFilesInData,
                    if include { "true" } else { "false" },
                )]);
                for (name, expected) in [
                    ("data_source_only", vec!["a.txt"]),
                    ("data_file_alias_only", vec!["b.txt"]),
                    (
                        "data_rule_only",
                        if include { vec!["c.txt"] } else { vec![] },
                    ),
                    (
                        "data_rule_alias_only",
                        if include { vec!["c.txt"] } else { vec![] },
                    ),
                ] {
                    let result = workspace
                        .request_data(&dice, observed, external, name, config.clone())
                        .await
                        .unwrap();
                    let default = info(&result);
                    assert!(result.actions().is_empty());
                    assert!(default.files().is_empty());
                    assert!(default.executable.is_none());
                    assert!(default.files_to_run.executable.is_none());
                    assert_eq!(
                        paths(&default.default_runfiles.files),
                        source_paths(external, &expected)
                    );
                    assert_eq!(
                        paths(&default.data_runfiles.files),
                        source_paths(external, &expected)
                    );
                }
                let simple = workspace
                    .request_data(&dice, observed, external, "with_data", config.clone())
                    .await
                    .unwrap();
                assert_eq!(
                    paths(info(&simple).files()),
                    source_paths(external, &["a.txt"])
                );
                assert_eq!(
                    paths(&info(&simple).default_runfiles.files),
                    source_paths(external, &["data.txt"])
                );
                assert_eq!(
                    paths(&info(&simple).data_runfiles.files),
                    source_paths(external, &["a.txt", "data.txt"])
                );
                if !external {
                    for config in [config.clone(), exec(&config)] {
                        let result = workspace
                            .request_data(&dice, observed, false, "data_generated", config.clone())
                            .await
                            .unwrap();
                        let files = info(&result).default_runfiles.files.to_list();
                        assert_eq!(files.len(), 1);
                        let AnalysisArtifact::Derived { output, .. } = artifact(&files[0]) else {
                            panic!("generated data File")
                        };
                        assert_eq!(
                            output.root().configuration(),
                            config.slug_configuration().unwrap()
                        );
                        assert_eq!(output.root_relative_path(), "data_generated.txt");
                        assert!(info(&result).files().is_empty());
                        assert!(info(&result).executable.is_none());
                        assert_eq!(info(&result).data_runfiles.files.to_list(), files);
                        assert!(result.actions().is_empty());
                    }
                }
            }
        }
    }
}

#[tokio::test]
async fn native_selected_data_edges_keep_attribute_indices_and_restore_configuration() {
    let workspace = Workspace::new();
    for observed in [false, true] {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        for external in [false, true] {
            let base = configuration(&[]);
            let selected = workspace
                .request_data(&dice, observed, external, "data_selected", base.clone())
                .await
                .unwrap();
            assert_eq!(
                paths(&info(&selected).default_runfiles.files),
                source_paths(external, &["b.txt"])
            );
            assert!(
                !selected.edges().iter().any(|edge| edge
                    .target()
                    .label()
                    .to_string()
                    .ends_with(":missing.txt"))
            );
            let original = workspace
                .request_data(&dice, observed, external, "data_switch", base.clone())
                .await
                .unwrap();
            let concatenated = workspace
                .request_data(&dice, observed, external, "data_concat", base.clone())
                .await
                .unwrap();
            assert_eq!(
                paths(&info(&concatenated).default_runfiles.files),
                source_paths(external, &["a.txt", "b.txt"])
            );
            let attributes = original
                .edges()
                .iter()
                .filter_map(|edge| {
                    let ConfiguredEdgeKind::Attribute {
                        attribute, index, ..
                    } = edge.kind()
                    else {
                        return None;
                    };
                    Some((
                        attribute.as_str(),
                        *index,
                        edge.target().label().target().as_str(),
                    ))
                })
                .collect::<Vec<_>>();
            assert_eq!(
                attributes,
                [
                    ("srcs", 0, "a.txt"),
                    ("srcs", 1, "b.txt"),
                    ("data", 0, "b.txt"),
                    ("data", 1, "c.txt")
                ]
            );
            for (config, expected) in [
                (
                    configuration(&[(NativeCommandOption::CompilationMode, "opt")]),
                    vec!["d.txt"],
                ),
                (base, vec!["b.txt", "c.txt"]),
            ] {
                let result = workspace
                    .request_data(&dice, observed, external, "data_switch", config.clone())
                    .await
                    .unwrap();
                assert_eq!(
                    paths(info(&result).files()),
                    source_paths(external, &["a.txt", "b.txt"])
                );
                assert!(info(&result).executable.is_none());
                assert_eq!(
                    paths(&info(&result).default_runfiles.files),
                    source_paths(external, &expected)
                );
                if expected == ["d.txt"] {
                    assert_ne!(result, original);
                    let error = workspace
                        .request_data(&dice, observed, external, "data_selected", config.clone())
                        .await
                        .unwrap_err();
                    assert!(error.contains("missing.txt"), "{error}");
                    let error = workspace
                        .request_data(&dice, observed, external, "data_concat", config)
                        .await
                        .unwrap_err();
                    assert!(
                        error.contains("duplicate") && error.contains("data"),
                        "{error}"
                    );
                } else {
                    assert_eq!(result, original);
                    assert_eq!(
                        workspace
                            .request_data(&dice, observed, external, "data_concat", config)
                            .await
                            .unwrap(),
                        concatenated
                    );
                }
            }
        }
    }
}

#[tokio::test]
async fn native_data_provider_source_and_guard_errors_restore_in_one_dice() {
    let workspace = Workspace::new();
    for observed in [false, true] {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        for external in [false, true] {
            let package = if external {
                workspace.0.join("dep")
            } else {
                workspace.0.clone()
            };
            let build = package.join("BUILD.bazel");
            let original = fs::read_to_string(&build).unwrap();
            let config = configuration(&[(
                NativeCommandOption::IncompatibleAlwaysIncludeFilesInData,
                "false",
            )]);
            let first = workspace
                .request_data(&dice, observed, external, "data_mixed", config.clone())
                .await
                .unwrap();
            for (edited, expected) in [
                (
                    original.replace("data_files=['d.txt']", "data_files=['b.txt']"),
                    vec!["c.txt", "b.txt", "default.txt"],
                ),
                (
                    original.replace(
                        "data=['c.txt',':data_origin',':data_duplicate_source_alias',':data_origin_alias']",
                        "data=['a.txt',':data_origin']",
                    ),
                    vec!["a.txt", "d.txt", "default.txt"],
                ),
            ] {
                fs::write(&build, edited).unwrap();
                let changed = workspace
                    .request_data(&dice, observed, external, "data_mixed", config.clone())
                    .await
                    .unwrap();
                assert_ne!(changed, first);
                assert_eq!(
                    paths(&info(&changed).default_runfiles.files),
                    source_paths(external, &expected)
                );
                fs::write(&build, &original).unwrap();
                assert_eq!(
                    workspace
                        .request_data(&dice, observed, external, "data_mixed", config.clone())
                        .await
                        .unwrap(),
                    first
                );
            }
            fs::write(
                &build,
                original.replace(
                    "data=['c.txt',':data_origin',':data_duplicate_source_alias',':data_origin_alias']",
                    "data=['c.txt','c.txt']",
                ),
            )
            .unwrap();
            let error = workspace
                .request_data(&dice, observed, external, "data_mixed", config.clone())
                .await
                .unwrap_err();
            assert!(
                error.contains("duplicate") && error.contains("data"),
                "{error}"
            );
            fs::write(&build, &original).unwrap();
            assert_eq!(
                workspace
                    .request_data(&dice, observed, external, "data_mixed", config.clone())
                    .await
                    .unwrap(),
                first
            );
            let source = package.join("c.txt");
            let source_bytes = fs::read(&source).unwrap();
            fs::remove_file(&source).unwrap();
            assert!(
                workspace
                    .request_data(&dice, observed, external, "data_mixed", config.clone())
                    .await
                    .is_err()
            );
            fs::write(source, source_bytes).unwrap();
            assert_eq!(
                workspace
                    .request_data(&dice, observed, external, "data_mixed", config.clone())
                    .await
                    .unwrap(),
                first
            );
            for (name, expected) in [
                ("data_cycle_a", "cycle"),
                ("data_private_cross", "visibility"),
                ("data_package_group", "package group"),
                ("data_package_group_alias", "package group"),
                ("data_testonly_origin", "testonly"),
                ("data_hidden_testonly", "testonly"),
            ] {
                let error = workspace
                    .request_data(&dice, observed, external, name, config.clone())
                    .await
                    .unwrap_err();
                assert!(error.to_lowercase().contains(expected), "{name}: {error}");
            }
            let source_alias = workspace
                .request_data(
                    &dice,
                    observed,
                    external,
                    "data_testonly_alias",
                    config.clone(),
                )
                .await
                .unwrap();
            assert_eq!(
                paths(&info(&source_alias).data_runfiles.files),
                source_paths(external, &["a.txt"])
            );
            let public = workspace
                .request_data(
                    &dice,
                    observed,
                    external,
                    "data_public_cross",
                    config.clone(),
                )
                .await
                .unwrap();
            assert!(
                runfiles_package_names(&public)
                    .iter()
                    .any(|name| name.ends_with("//other"))
            );
            let unchecked = configuration(&[(NativeCommandOption::CheckVisibility, "false")]);
            assert!(
                workspace
                    .request_data(&dice, observed, external, "data_private_cross", unchecked)
                    .await
                    .is_ok()
            );
            if !external {
                let error = workspace
                    .request_data(
                        &dice,
                        observed,
                        false,
                        "data_testonly_generated",
                        config.clone(),
                    )
                    .await
                    .unwrap_err();
                assert!(
                    error.contains("testonly") && error.contains("unsupported"),
                    "{error}"
                );
            }
            assert_eq!(
                workspace
                    .request_data(&dice, observed, external, "data_mixed", config)
                    .await
                    .unwrap(),
                first
            );
        }
    }
}
