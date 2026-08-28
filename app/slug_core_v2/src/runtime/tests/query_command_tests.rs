use super::*;

    #[test]
    fn real_query_command_drives_typed_results_and_cold_events_without_warm_replay() {
        let target_parent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/slug-query-real-command");
        fs::create_dir_all(&target_parent).unwrap();
        let workspace = tempfile::tempdir_in(target_parent).unwrap();
        fs::write(
            workspace.path().join("MODULE.bazel"),
            "print(\"MODULE_EVENT\")\nmodule(name = \"driver\")\n",
        )
        .unwrap();
        fs::create_dir(workspace.path().join("pkg")).unwrap();
        fs::write(
            workspace.path().join("pkg/defs.bzl"),
            "print(\"BZL_EVENT\")\nNAME = \"probe\"\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("pkg/BUILD.bazel"),
            "load(\":defs.bzl\", \"NAME\")\nprint(\"BUILD_EVENT\")\nfilegroup(name = NAME)\n",
        )
        .unwrap();
        let runtime = test_runtime(workspace.path()).unwrap();
        let query = |runtime: &WorkspaceRuntime, expression: &str| {
            runtime.query_command_with_policy_and_bzlmod_inputs_and_output_completion(
                expression,
                QueryOrder::Auto,
                QueryPolicy::default(),
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                QueryOutputCompletion::Standard,
            )
        };

        let accepted = query(&runtime, "deps(//pkg:probe)").unwrap();
        assert_eq!(
            accepted
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "//pkg:probe\n"
        );
        assert_eq!(
            accepted_output_text(&accepted),
            ["MODULE_EVENT", "BZL_EVENT", "BUILD_EVENT"]
        );

        let warm = query(&runtime, "deps(//pkg:probe)").unwrap();
        assert!(warm.terminal_for_test().as_ref().is_ok());
        assert!(
            accepted_output_text(&warm).is_empty(),
            "{:?}",
            accepted_output_text(&warm)
        );

        let empty = query(&runtime, "set()").unwrap();
        assert_eq!(
            empty
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            ""
        );

        let missing_runtime = test_runtime(workspace.path()).unwrap();
        let missing = query(&missing_runtime, "//pkg:missing").unwrap();
        let error = missing.terminal_for_test().as_ref().as_ref().unwrap_err();
        assert_eq!(
            error.to_string(),
            "no such target '//pkg:missing': target 'missing' not declared in package 'pkg'"
        );
        assert_eq!(
            accepted_output_text(&missing),
            ["MODULE_EVENT", "BZL_EVENT", "BUILD_EVENT"]
        );
    }

    #[test]
    fn direct_external_query_uses_host_route_native_materialization_and_apparent_output() {
        let target_parent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/slug-query-external-command");
        fs::create_dir_all(&target_parent).unwrap();
        let workspace = tempfile::tempdir_in(target_parent).unwrap();
        fs::write(
            workspace.path().join("MODULE.bazel"),
            "print(\"MODULE_EVENT\")\nmodule(name = \"driver\")\nbazel_dep(name = \"dep\", version = \"1.0.0\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n",
        )
        .unwrap();
        fs::create_dir(workspace.path().join("dep")).unwrap();
        fs::write(
            workspace.path().join("dep/MODULE.bazel"),
            "module(name = \"dep\", version = \"1.0.0\")\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("dep/BUILD.bazel"),
            "print(\"EXTERNAL_BUILD_EVENT\")\nexports_files([\"target.txt\"])\nfilegroup(name = \"files\", srcs = [\"target.txt\", \"missing_input.txt\"])\nalias(name = \"files_alias\", actual = \":files\")\nconfig_setting(name = \"is_k8\", values = {\"cpu\": \"k8\"})\ntest_suite(name = \"suite_omitted\")\ntest_suite(name = \"suite_empty\", tests = [], tags = [\"manual\", \"a\"])\ntest_suite(name = \"suite_parent\", tests = [\":suite_empty\"])\ntest_suite(name = \"suite_cycle_a\", tests = [\":suite_cycle_b\"])\ntest_suite(name = \"suite_cycle_b\", tests = [\":suite_cycle_a\"])\npackage_group(name = \"pg_empty\")\npackage_group(name = \"pg_nonempty\", packages = [\"//pkg\", \"//tree/...\", \"-//blocked\", \"-//blocked_tree/...\", \"public\", \"private\"])\npackage_group(name = \"pg_leaf\", packages = [\"//leaf\"])\npackage_group(name = \"pg_parent\", includes = [\":pg_leaf\"])\npackage_group(name = \"pg_cycle_a\", includes = [\":pg_cycle_b\"])\npackage_group(name = \"pg_cycle_b\", includes = [\":pg_cycle_a\"])\n",
        )
        .unwrap();
        fs::write(workspace.path().join("dep/target.txt"), "target").unwrap();

        let activation_audit = Arc::new(ExternalQueryActivationAudit::default());
        let runtime = test_runtime(workspace.path())
            .unwrap()
            .with_activation_audit(activation_audit.clone());
        let query = |expression: &str| {
            runtime.query_command_with_policy_and_bzlmod_inputs_and_output_completion(
                expression,
                QueryOrder::Auto,
                QueryPolicy::default(),
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                QueryOutputCompletion::Standard,
            )
        };
        let query_label_kind = |expression: &str| {
            runtime.query_command_with_policy_and_bzlmod_inputs_and_output_completion(
                expression,
                QueryOrder::Auto,
                QueryPolicy::default(),
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                QueryOutputCompletion::LabelKind,
            )
        };

        let phase = activation_audit.checkpoint();
        let first = query("@dep//:target.txt").unwrap();
        activation_audit.assert_phase_clean(phase, 2);
        assert_eq!(
            first
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:target.txt\n"
        );
        assert_eq!(
            accepted_output_text(&first),
            ["MODULE_EVENT", "EXTERNAL_BUILD_EVENT"]
        );

        let files = query("@dep//:files").unwrap();
        assert_eq!(
            files
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:files\n"
        );
        assert_eq!(
            query("labels(srcs, @dep//:files)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:missing_input.txt\n@dep//:target.txt\n"
        );
        assert_eq!(
            query("deps(@dep//:files)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:files\n@dep//:missing_input.txt\n@dep//:target.txt\n"
        );
        assert_eq!(
            query("@dep//:files_alias")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:files_alias\n"
        );
        assert_eq!(
            query("labels(actual, @dep//:files_alias)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:files\n"
        );
        assert_eq!(
            query("deps(@dep//:files_alias)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:files\n@dep//:files_alias\n@dep//:missing_input.txt\n@dep//:target.txt\n"
        );
        assert_eq!(
            query("@dep//:is_k8")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:is_k8\n"
        );
        assert_eq!(
            query("deps(@dep//:is_k8)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:is_k8\n"
        );
        assert_eq!(
            query_label_kind("@dep//:is_k8")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .label_kind_stdout(),
            "config_setting rule @dep//:is_k8\n"
        );
        assert!(
            query("labels(visibility, @dep//:files)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout()
                .is_empty()
        );
        assert_eq!(
            query("@dep//:suite_omitted")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:suite_omitted\n"
        );
        assert_eq!(
            query("@dep//:suite_empty")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:suite_empty\n"
        );
        assert_eq!(
            query_label_kind("@dep//:suite_parent")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .label_kind_stdout(),
            "test_suite rule @dep//:suite_parent\n"
        );
        assert!(
            query("labels($implicit_tests, @dep//:suite_omitted)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout()
                .is_empty()
        );
        assert_eq!(
            query("labels(tests, @dep//:suite_parent)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:suite_empty\n"
        );
        assert_eq!(
            query("deps(@dep//:suite_parent)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:suite_empty\n@dep//:suite_parent\n"
        );
        assert!(
            query("tests(@dep//:suite_parent)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout()
                .is_empty()
        );
        assert_eq!(
            query("deps(@dep//:suite_cycle_a)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:suite_cycle_a\n@dep//:suite_cycle_b\n"
        );
        assert!(
            query("tests(@dep//:suite_cycle_a)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout()
                .is_empty()
        );
        assert_eq!(
            query("@dep//:pg_parent")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:pg_parent\n"
        );
        assert_eq!(
            query_label_kind("@dep//:pg_parent")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .label_kind_stdout(),
            "package group @dep//:pg_parent\n"
        );
        assert_eq!(
            query("deps(@dep//:pg_parent)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:pg_leaf\n@dep//:pg_parent\n"
        );
        assert_eq!(
            query("deps(@dep//:pg_cycle_a)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:pg_cycle_a\n@dep//:pg_cycle_b\n"
        );
        assert!(
            query("labels(visibility, @dep//:pg_parent)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout()
                .is_empty()
        );

        // An attribute-created external source is semantic loading state, not
        // a source-file observation. It remains addressable while absent.
        fs::write(workspace.path().join("dep/missing_input.txt"), "present").unwrap();
        let created = query("deps(@dep//:files)").unwrap();
        assert!(accepted_output_text(&created).is_empty());
        let suite_after_source_create = query("@dep//:suite_parent").unwrap();
        assert_eq!(
            suite_after_source_create
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:suite_parent\n"
        );
        assert!(accepted_output_text(&suite_after_source_create).is_empty());
        let group_after_source_create = query("@dep//:pg_parent").unwrap();
        assert_eq!(
            group_after_source_create
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:pg_parent\n"
        );
        assert!(accepted_output_text(&group_after_source_create).is_empty());
        let setting_after_source_create = query("@dep//:is_k8").unwrap();
        assert!(accepted_output_text(&setting_after_source_create).is_empty());
        fs::write(workspace.path().join("dep/missing_input.txt"), "edited").unwrap();
        let edited_source = query("deps(@dep//:files)").unwrap();
        assert!(accepted_output_text(&edited_source).is_empty());
        fs::remove_file(workspace.path().join("dep/missing_input.txt")).unwrap();
        let deleted_source = query("deps(@dep//:files)").unwrap();
        assert!(accepted_output_text(&deleted_source).is_empty());
        fs::write(workspace.path().join("dep/missing_input.txt"), "recreated").unwrap();
        let recreated_source = query("deps(@dep//:files)").unwrap();
        assert!(accepted_output_text(&recreated_source).is_empty());
        fs::remove_file(workspace.path().join("dep/missing_input.txt")).unwrap();

        for (build, expected) in [
            (
                "filegroup(name = \"member\")\ntest_suite(name = \"other\", tests = [\":member\"])\n",
                "external repository test_suite non-suite member is deferred",
            ),
            (
                "package_group(name = \"group\", includes = [\":missing\"])\n",
                "external repository package_group missing include is deferred",
            ),
            (
                "filegroup(name = \"member\")\npackage_group(name = \"group\", includes = [\":member\"])\n",
                "external repository package_group non-package-group include is deferred",
            ),
            (
                "exports_files([\"target.txt\"])\nalias(name = \"member\", actual = \":target.txt\")\npackage_group(name = \"group\", includes = [\":member\"])\n",
                "external repository package_group alias include is deferred",
            ),
            (
                "package_group(name = \"group\", includes = [\"//other:member\"])\n",
                "external repository package_group cross-package include is deferred",
            ),
            (
                "filegroup(name = \"files\", srcs = [\"//other:item\"])\n",
                "external repository filegroup cross-package srcs are deferred",
            ),
            (
                "filegroup(name = \"group\")\nfilegroup(name = \"files\", visibility = [\":group\"])\n",
                "external repository visibility wrong-kind group is deferred",
            ),
            (
                "filegroup(name = \"group\")\nfilegroup(name = \"files\")\nalias(name = \"files_alias\", actual = \":files\", visibility = [\":group\"])\n",
                "external repository Restricted visibility is deferred for non-filegroup",
            ),
            (
                "filegroup(name = \"group\")\nconfig_setting(name = \"is_k8\", values = {\"cpu\": \"k8\"}, visibility = [\":group\"])\n",
                "external repository Restricted visibility is deferred for non-filegroup",
            ),
            (
                "filegroup(name = \"group\")\ntest_suite(name = \"suite\", visibility = [\":group\"])\n",
                "external repository Restricted visibility is deferred for non-filegroup",
            ),
            (
                "filegroup(name = \"BUILD.bazel\")\n",
                "collides with active BUILD file",
            ),
            (
                "config_setting(name = \"BUILD.bazel\", values = {\"cpu\": \"k8\"})\n",
                "collides with active BUILD file",
            ),
            (
                "filegroup(name = \"files\")\nalias(name = \"first\", actual = \":second\")\nalias(name = \"second\", actual = \":files\")\n",
                "external repository alias chains are deferred",
            ),
            (
                "alias(name = \"to_build\", actual = \":BUILD.bazel\")\n",
                "external repository alias actual destination is deferred",
            ),
            (
                "alias(name = \"cross\", actual = \"//other:item\")\n",
                "external repository alias cross-package actual is deferred",
            ),
        ] {
            fs::write(workspace.path().join("dep/BUILD.bazel"), build).unwrap();
            let stopped = test_runtime(workspace.path()).unwrap();
            let error = stopped
                .query_command_with_policy_and_bzlmod_inputs_and_output_completion(
                    "@dep//:files",
                    QueryOrder::Auto,
                    QueryPolicy::default(),
                    BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                    BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                    LockfileMode::Update,
                    &[],
                    QueryOutputCompletion::Standard,
                )
                .unwrap();
            let failure = error
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap_err()
                .to_string();
            assert!(
                failure.contains(expected),
                "expected {expected:?}: {failure}"
            );
        }
        fs::write(
            workspace.path().join("dep/BUILD.bazel"),
            "print(\"EXTERNAL_BUILD_EVENT\")\nexports_files([\"target.txt\"])\nfilegroup(name = \"files\", srcs = [\"target.txt\", \"missing_input.txt\"])\nalias(name = \"files_alias\", actual = \":files\")\nconfig_setting(name = \"is_k8\", values = {\"cpu\": \"k8\"})\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("dep/BUILD.bazel"),
            "alias(name = \"files_alias\", actual = \"@other//:item\")\n",
        )
        .unwrap();
        let stopped = test_runtime(workspace.path()).unwrap();
        let named_repository = stopped
            .query_command_with_policy_and_bzlmod_inputs_and_output_completion(
                "@dep//:files",
                QueryOrder::Auto,
                QueryPolicy::default(),
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                QueryOutputCompletion::Standard,
            )
            .unwrap();
        assert!(
            named_repository
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap_err()
                .to_string()
                .contains("no repository visible as '@other'"),
            "{named_repository:?}"
        );
        fs::write(
            workspace.path().join("dep/BUILD.bazel"),
            "print(\"EXTERNAL_BUILD_EVENT\")\nexports_files([\"target.txt\"])\n",
        )
        .unwrap();
        let restored_after_stop_gates = query("@dep//:target.txt").unwrap();
        assert_eq!(
            accepted_output_text(&restored_after_stop_gates),
            ["EXTERNAL_BUILD_EVENT"]
        );
        let phase = activation_audit.checkpoint();
        let warm = query("@dep//:target.txt").unwrap();
        activation_audit.assert_phase_clean(phase, 1);
        assert!(accepted_output_text(&warm).is_empty());

        fs::rename(
            workspace.path().join("dep/BUILD.bazel"),
            workspace.path().join("dep/BUILD"),
        )
        .unwrap();
        let fallback = query("@dep//:target.txt").unwrap();
        assert_eq!(
            fallback
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:target.txt\n"
        );
        assert_eq!(accepted_output_text(&fallback), ["EXTERNAL_BUILD_EVENT"]);

        fs::write(
            workspace.path().join("dep/BUILD"),
            "print(\"EXTERNAL_BUILD_EDITED\")\nexports_files([\"edited.txt\"])\n",
        )
        .unwrap();
        fs::write(workspace.path().join("dep/edited.txt"), "edited").unwrap();
        let edited = query("@dep//:edited.txt").unwrap();
        assert_eq!(
            edited
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:edited.txt\n"
        );
        assert_eq!(accepted_output_text(&edited), ["EXTERNAL_BUILD_EDITED"]);

        fs::remove_file(workspace.path().join("dep/BUILD")).unwrap();
        let phase = activation_audit.checkpoint();
        let deleted = query("@dep//:edited.txt").unwrap();
        activation_audit.assert_phase_clean(phase, 2);
        assert!(
            deleted
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap_err()
                .to_string()
                .contains("BUILD file not found")
        );

        fs::write(
            workspace.path().join("dep/BUILD.bazel"),
            "print(\"EXTERNAL_BUILD_EVENT\")\nexports_files([\"target.txt\"])\n",
        )
        .unwrap();
        let phase = activation_audit.checkpoint();
        let restored = query("@dep//:target.txt").unwrap();
        activation_audit.assert_phase_clean(phase, 2);
        assert_eq!(
            restored
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:target.txt\n"
        );
        assert_eq!(accepted_output_text(&restored), ["EXTERNAL_BUILD_EVENT"]);

        let phase = activation_audit.checkpoint();
        let missing = query("@dep//:missing").unwrap();
        activation_audit.assert_phase_clean(phase, 1);
        let error = missing.terminal_for_test().as_ref().as_ref().unwrap_err();
        assert_eq!(
            error.to_string(),
            "no such target '@@dep+//:missing': target 'missing' not declared in package '' defined by <output_base>/external/dep+/BUILD.bazel"
        );
        assert_eq!(error.exit_code, 7);

        let missing_package = query("@dep//nope:missing").unwrap();
        let error = missing_package
            .terminal_for_test()
            .as_ref()
            .as_ref()
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "no such package '@@dep+//nope': BUILD file not found in directory 'nope' of external repository @@dep+. Add a BUILD file to a directory to mark it as a package."
        );
        assert_eq!(error.exit_code, 7);

        let phase = activation_audit.checkpoint();
        let unknown = query("@missing//:target.txt").unwrap();
        activation_audit.assert_phase_clean(phase, 1);
        let error = unknown.terminal_for_test().as_ref().as_ref().unwrap_err();
        assert_eq!(
            error.to_string(),
            "no such package '@@[unknown repo 'missing' requested from @@]//': The repository '@@[unknown repo 'missing' requested from @@]' could not be resolved: No repository visible as '@missing' from main repository"
        );
        assert_eq!(error.exit_code, 7);

        for pattern in ["@dep//:all", "@dep//:*", "@dep//..."] {
            let pattern_error = query(pattern).unwrap();
            assert_eq!(
                pattern_error
                    .terminal_for_test()
                    .as_ref()
                    .as_ref()
                    .unwrap_err()
                    .to_string(),
                format!("external repository query patterns are deferred: {pattern}")
            );
        }

        fs::write(
            workspace.path().join("dep/BUILD.bazel"),
            "load(\":defs.bzl\", \"defs\")\nexports_files([\"target.txt\"])\n",
        )
        .unwrap();
        let load = query("@dep//:target.txt").unwrap();
        assert!(
            load.terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap_err()
                .to_string()
                .contains("external repository BUILD loads are deferred")
        );

        fs::write(
            workspace.path().join("dep/BUILD.bazel"),
            "exports_files(glob([\"*.txt\"]))\n",
        )
        .unwrap();
        let glob = query("@dep//:target.txt").unwrap();
        assert!(
            glob.terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap_err()
                .to_string()
                .contains("external repository BUILD globs are deferred")
        );

        fs::write(workspace.path().join("dep/BUILD.bazel"), [0xff]).unwrap();
        let invalid_utf8 = query("@dep//:target.txt").unwrap();
        assert!(
            invalid_utf8
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap_err()
                .to_string()
                .contains("external repository BUILD file is not UTF-8")
        );
    }

    #[test]
    fn observed_query_publication_preserves_terminal_and_selected_epoch_arcs() {
        let target_parent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/slug-query-selected-arcs");
        fs::create_dir_all(&target_parent).unwrap();
        let workspace = tempfile::tempdir_in(target_parent).unwrap();
        fs::write(
            workspace.path().join("MODULE.bazel"),
            "module(name = \"driver\")\n",
        )
        .unwrap();
        fs::create_dir(workspace.path().join("pkg")).unwrap();
        fs::write(
            workspace.path().join("pkg/BUILD.bazel"),
            "filegroup(name = \"probe\")\n",
        )
        .unwrap();
        let runtime = test_runtime(workspace.path()).unwrap();
        let query = || {
            runtime.query_command_with_policy_and_bzlmod_inputs_and_output_completion(
                "//pkg:probe",
                QueryOrder::Auto,
                QueryPolicy::default(),
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                QueryOutputCompletion::Standard,
            )
        };

        let cold = query().unwrap();
        let cold_snapshot = accepted_native_snapshot(&runtime);
        assert!(!cold_snapshot.path_observations.observations().is_empty());
        assert!(cold_snapshot.selected.repository_requests().is_empty());
        assert!(cold_snapshot.selected.repository_validations().is_empty());
        let warm = query().unwrap();
        let warm_snapshot = accepted_native_snapshot(&runtime);

        assert!(Arc::ptr_eq(
            cold.terminal_for_test(),
            warm.terminal_for_test()
        ));
        assert_eq!(
            cold_snapshot.path_observations.observations().len(),
            warm_snapshot.path_observations.observations().len()
        );
        for ((cold_demand, cold_result), (warm_demand, warm_result)) in cold_snapshot
            .path_observations
            .observations()
            .iter()
            .zip(warm_snapshot.path_observations.observations().iter())
        {
            assert_eq!(cold_demand, warm_demand);
            assert_eq!(cold_result, warm_result);
            assert!(Arc::ptr_eq(cold_result, warm_result));
        }
        assert!(accepted_output_text(&warm).is_empty());
    }

    #[test]
    fn observed_external_query_accepts_closure_selected_repository_sidecars() {
        let target_parent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/slug-query-repository-selection");
        fs::create_dir_all(&target_parent).unwrap();
        let workspace = tempfile::tempdir_in(target_parent).unwrap();
        fs::write(
            workspace.path().join("MODULE.bazel"),
            concat!(
                "module(name = \"driver\")\n",
                "bazel_dep(name = \"dep\", version = \"1.0.0\")\n",
                "local_path_override(module_name = \"dep\", path = \"dep\")\n",
            ),
        )
        .unwrap();
        fs::create_dir(workspace.path().join("dep")).unwrap();
        fs::write(
            workspace.path().join("dep/MODULE.bazel"),
            "module(name = \"dep\", version = \"1.0.0\")\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("dep/BUILD.bazel"),
            "exports_files([\"target.txt\"])\n",
        )
        .unwrap();
        fs::write(workspace.path().join("dep/target.txt"), "target\n").unwrap();
        let runtime = test_runtime(workspace.path()).unwrap();
        let query = || {
            runtime.query_command_with_policy_and_bzlmod_inputs_and_output_completion(
                "@dep//:target.txt",
                QueryOrder::Auto,
                QueryPolicy::default(),
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                QueryOutputCompletion::Standard,
            )
        };

        let cold = query().unwrap();
        let cold_snapshot = accepted_native_snapshot(&runtime);
        assert!(!cold_snapshot.selected.repository_requests().is_empty());
        assert!(!cold_snapshot.selected.repository_validations().is_empty());
        assert!(!cold_snapshot.path_observations.observations().is_empty());
        assert_eq!(
            cold.terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:target.txt\n"
        );

        let warm = query().unwrap();
        let warm_snapshot = accepted_native_snapshot(&runtime);
        assert!(Arc::ptr_eq(
            cold.terminal_for_test(),
            warm.terminal_for_test()
        ));
        for ((cold_demand, cold_result), (warm_demand, warm_result)) in cold_snapshot
            .path_observations
            .observations()
            .iter()
            .zip(warm_snapshot.path_observations.observations().iter())
        {
            assert_eq!(cold_demand, warm_demand);
            assert_eq!(cold_result, warm_result);
            assert!(Arc::ptr_eq(cold_result, warm_result));
        }
        assert!(accepted_output_text(&warm).is_empty());
    }
