use super::*;

struct BuiltinWorkspace(PathBuf);

impl BuiltinWorkspace {
    fn new() -> Self {
        let root = scratch();
        let mut root_module = "module(name='root')\n".to_owned();
        for &(name, version) in MODULES {
            fs::create_dir_all(root.join(name)).unwrap();
            fs::write(
                root.join(name).join("MODULE.bazel"),
                format!("module(name='{name}',version='{version}')\n"),
            )
            .unwrap();
            root_module.push_str(&format!(
                "local_path_override(module_name='{name}',path='{name}')\n"
            ));
            if matches!(
                name,
                "bazel_features" | "rules_apple" | "rules_swift" | "abseil-cpp"
            ) {
                root_module.push_str(&format!("bazel_dep(name='{name}',version='{version}')\n"));
            }
        }
        fs::write(root.join("MODULE.bazel"), root_module).unwrap();
        fs::write(
            root.join("BUILD.bazel"),
            "platform(name='linux_platform',constraint_values=['@@platforms//os:linux'])\nplatform(name='windows_platform',constraint_values=['@@platforms//os:windows'])\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("platforms/os")).unwrap();
        fs::write(root.join("platforms/REPO.bazel"), "").unwrap();
        fs::write(root.join("platforms/.bazelignore"), "").unwrap();
        fs::write(
            root.join("platforms/os/BUILD.bazel"),
            "constraint_setting(name='os')\nconstraint_value(name='linux',constraint_setting=':os')\nconstraint_value(name='windows',constraint_setting=':os')\n",
        )
        .unwrap();
        Self(root)
    }

    fn configuration(&self, platform: &str) -> ConfigurationKey {
        let platform = CanonicalLabel::parse(&format!("@@//:{platform}_platform")).unwrap();
        ConfigurationKey::from_slug(
            test_configuration()
                .slug_configuration()
                .unwrap()
                .with_transition_target_platform(Some(&platform))
                .unwrap(),
        )
    }

    async fn request(
        &self,
        dice: &Arc<Dice>,
        target: &str,
        configuration: ConfigurationKey,
    ) -> Result<Arc<ConfiguredNodeResult>, String> {
        let mut updater = dice.updater_with_data(UserComputationData {
            activation_tracker: Some(Arc::new(RootActivationTracker::default())),
            cycle_detector: Some(analysis_cycle_detector()),
            ..Default::default()
        });
        // The ordinary helper installs a local bazel_tools override. This
        // request must instead use the immutable Bazel 9.2 built-in route.
        inject_root_target_inputs_with_policy(
            &mut updater,
            &self.0,
            root_epoch(&self.0),
            REPOSITORIES,
            false,
        );
        let mut transaction = updater.commit().await;
        let key = ConfiguredNodeAnalysisObservationKey::new(
            NormalizedAbsolutePath::new(self.0.clone()).unwrap(),
            ConfiguredTargetKey::new(CanonicalLabel::parse(target).unwrap(), configuration),
        )
        .unwrap();
        let outcome = transaction
            .compute(&key)
            .await
            .map_err(|error| error.to_string())?;
        let AnalysisPreparationOutcome::Complete(Ok(result)) = outcome else {
            return Err(format!(
                "observed built-in filegroup did not complete: {outcome:#?}"
            ));
        };
        result
            .as_ref()
            .as_ref()
            .cloned()
            .map_err(|error| error.to_string())
    }
}

const MODULES: &[(&str, &str)] = &[
    ("rules_license", "1.0.0"),
    ("buildozer", "8.5.1"),
    ("platforms", "1.0.0"),
    ("zlib", "1.3.1.bcr.5"),
    ("bazel_features", "1.42.1"),
    ("protobuf", "33.4"),
    ("rules_java", "9.1.0"),
    ("rules_cc", "0.2.17"),
    ("rules_python", "1.7.0"),
    ("rules_shell", "0.6.1"),
    ("apple_support", "1.24.2"),
    ("rules_apple", "4.1.0"),
    ("rules_swift", "3.1.2"),
    ("abseil-cpp", "20250814.1"),
];

const REPOSITORIES: &[(&str, &str)] = &[
    ("rules_license+", "rules_license"),
    ("buildozer+", "buildozer"),
    ("platforms", "platforms"),
    ("platforms+", "platforms"),
    ("zlib+", "zlib"),
    ("bazel_features+", "bazel_features"),
    ("protobuf+", "protobuf"),
    ("rules_java+", "rules_java"),
    ("rules_cc+", "rules_cc"),
    ("rules_python+", "rules_python"),
    ("rules_shell+", "rules_shell"),
    ("apple_support+", "apple_support"),
    ("rules_apple+", "rules_apple"),
    ("rules_swift+", "rules_swift"),
    ("abseil-cpp+", "abseil-cpp"),
];

impl Drop for BuiltinWorkspace {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[tokio::test]
async fn authentic_builtin_module_tool_filegroups_select_linux_executables() {
    let workspace = BuiltinWorkspace::new();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    for (group, tool) in [
        ("aggregate-ddi-bin", "aggregate-ddi"),
        ("generate-modmap-bin", "generate-modmap"),
    ] {
        let group_label = format!("@@bazel_tools//tools/cpp/modules_tools:{group}");
        let source =
            CanonicalLabel::parse(&format!("@@bazel_tools//tools/cpp/modules_tools:{tool}"))
                .unwrap();
        let expected_file = AnalysisArtifact::Source(source.clone());
        let result = workspace
            .request(&dice, &group_label, workspace.configuration("linux"))
            .await
            .unwrap();
        assert!(result.actions().is_empty());
        let src_edges = result
            .edges()
            .iter()
            .filter(|edge| {
                matches!(edge.kind(), ConfiguredEdgeKind::Attribute { attribute, .. } if attribute == "srcs")
            })
            .collect::<Vec<_>>();
        assert_eq!(src_edges.len(), 1, "{group}: {src_edges:?}");
        assert!(matches!(
            src_edges[0].kind(),
            ConfiguredEdgeKind::Attribute {
                attribute,
                index: 0,
                hidden: false,
                dependency: ConfiguredAttributeDependency::Target,
            } if attribute == "srcs"
        ));
        assert_eq!(src_edges[0].target(), &ConfiguredNodeKey::Null(source));

        let default = info(&result);
        assert_eq!(default.file_artifacts(), [expected_file.clone()]);
        assert_eq!(default.executable.as_ref(), Some(&expected_file));
        assert_eq!(
            default.files_to_run.executable.as_ref(),
            Some(&expected_file)
        );
        assert_eq!(
            default.files_to_run.files().to_list(),
            [AnalysisValue::artifact(expected_file.clone())]
        );
        assert!(default.files_to_run.support.is_none());
        assert_eq!(
            expected_file.short_path().as_ref(),
            format!("../bazel_tools/tools/cpp/modules_tools/{tool}")
        );

        let error = workspace
            .request(&dice, &group_label, workspace.configuration("windows"))
            .await
            .unwrap_err();
        assert!(error.contains(&format!("{tool}.exe")), "{group}: {error}");
        assert!(
            error.contains("outside the Bazel 9.2 catalog"),
            "{group}: {error}"
        );
        let restored = workspace
            .request(&dice, &group_label, workspace.configuration("linux"))
            .await
            .unwrap();
        assert!(restored.actions().is_empty());
        assert_eq!(info(&restored).file_artifacts(), [expected_file.clone()]);
        assert_eq!(info(&restored).executable.as_ref(), Some(&expected_file));
    }
}
