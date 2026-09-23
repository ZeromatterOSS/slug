//! Pinned native Filegroup.java forwarding over real configured prerequisites.
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::DefaultInfo;
use slug_build_api_v2::DepsetOrder;
use slug_configuration_v2::NativeCommandOption;

use super::*;

mod lifecycle;
mod rejection;

struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let root = scratch();
        fs::write(root.join("MODULE.bazel"), "module(name='root')\nbazel_dep(name='dep',version='1.0.0')\nlocal_path_override(module_name='dep',path='dep')\n").unwrap();
        fs::create_dir_all(root.join("dep")).unwrap();
        fs::write(
            root.join("dep/MODULE.bazel"),
            "module(name='dep',version='1.0.0')\n",
        )
        .unwrap();
        fs::write(root.join("dep/REPO.bazel"), "").unwrap();
        fs::write(root.join("dep/.bazelignore"), "").unwrap();
        for package in [&root, &root.join("dep")] {
            fs::write(package.join("defs.bzl"), include_str!("defs.bzl")).unwrap();
            fs::write(package.join("BUILD.bazel"), include_str!("BUILD.fixture")).unwrap();
            for file in [
                "a.txt",
                "b.txt",
                "c.txt",
                "d.txt",
                "default.txt",
                "data.txt",
                "validation.txt",
                "implicit.txt",
            ] {
                fs::write(package.join(file), "authored source; not executable\n").unwrap();
            }
            fs::create_dir_all(package.join("other")).unwrap();
            fs::write(package.join("other/BUILD.bazel"), "filegroup(name='private')\nalias(name='public_alias',actual=':private',visibility=['//visibility:public'])\n").unwrap();
        }
        Self(root)
    }

    async fn request(
        &self,
        dice: &Arc<Dice>,
        external: bool,
        name: &str,
        config: ConfigurationKey,
    ) -> Result<Arc<ConfiguredNodeResult>, String> {
        let repo = if external { "dep+" } else { "" };
        root_target_request_with_inputs(
            dice,
            &self.0,
            &format!("@@{repo}//:{name}"),
            config,
            Arc::new(RootActivationTracker::default()),
            root_epoch(&self.0),
            &[("dep+", "dep")],
        )
        .await
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn configuration(options: &[(NativeCommandOption, &str)]) -> ConfigurationKey {
    super::cc_configure_features::config(options)
}
fn exec(configuration: &ConfigurationKey) -> ConfigurationKey {
    let configuration = configuration.slug_configuration().unwrap();
    ConfigurationKey::from_slug(
        configuration
            .to_exec_for_platform(&configuration.host_platform_label().unwrap())
            .unwrap(),
    )
}
fn info(result: &ConfiguredNodeResult) -> &DefaultInfo {
    result.providers().default_info().unwrap()
}
fn paths(values: &slug_build_api_v2::AnalysisDepset) -> Vec<String> {
    values
        .to_list()
        .iter()
        .map(|value| {
            let AnalysisValueKind::Artifact(file) = value.kind() else {
                panic!("File depset")
            };
            file.path().into_owned()
        })
        .collect()
}
fn source_paths(external: bool, names: &[&str]) -> Vec<String> {
    names
        .iter()
        .map(|name| {
            if external {
                format!("external/dep+/{name}")
            } else {
                (*name).to_owned()
            }
        })
        .collect()
}
fn field<'a>(result: &'a ConfiguredNodeResult, name: &str) -> &'a AnalysisValue {
    result
        .providers()
        .occurrences()
        .find_map(|p| p.field(name))
        .unwrap()
}
fn artifact(value: &AnalysisValue) -> &AnalysisArtifact {
    let AnalysisValueKind::Artifact(value) = value.kind() else {
        panic!("File")
    };
    value
}
fn group_paths(result: &ConfiguredNodeResult, name: &str) -> Vec<String> {
    paths(
        result
            .providers()
            .output_group_info()
            .unwrap()
            .groups()
            .get(name)
            .unwrap(),
    )
}

#[tokio::test]
async fn native_singletons_nested_aliases_and_consumers_preserve_target_exec_files() {
    let workspace = Workspace::new();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let base = configuration(&[]);
    for external in [false, true] {
        for config in [base.clone(), exec(&base)] {
            for (name, expected) in [
                ("empty", vec![]),
                ("single", vec!["a.txt"]),
                ("implicit", vec!["implicit.txt"]),
                ("duplicates", vec!["a.txt"]),
                ("alias", vec!["a.txt"]),
                ("diamond", vec!["a.txt", "b.txt", "c.txt"]),
                ("mixed", vec!["b.txt", "a.txt", "c.txt"]),
            ] {
                let result = workspace
                    .request(&dice, external, name, config.clone())
                    .await
                    .unwrap();
                assert!(result.actions().is_empty());
                let files = info(&result).file_artifacts();
                assert_eq!(
                    files
                        .iter()
                        .map(|f| f.path().into_owned())
                        .collect::<Vec<_>>(),
                    source_paths(external, &expected)
                );
                assert_eq!(
                    info(&result).executable.as_ref(),
                    if files.len() == 1 {
                        files.first()
                    } else {
                        None
                    }
                );
                assert!(info(&result).default_runfiles.files.is_empty());
                assert_eq!(
                    paths(&info(&result).data_runfiles.files),
                    source_paths(external, &expected)
                );
                assert_eq!(
                    info(&result).files_to_run.executable,
                    info(&result).executable
                );
                assert!(info(&result).files_to_run.support.is_none());
                let instrumented = result
                    .providers()
                    .occurrences()
                    .find(|p| {
                        p.identity()
                            == &slug_build_api_v2::ProviderIdentity::builtin(
                                "InstrumentedFilesInfo",
                            )
                    })
                    .unwrap();
                assert_eq!(instrumented.fields().len(), 2);
                for field in ["instrumented_files", "metadata_files"] {
                    assert!(
                        matches!(instrumented.field(field).unwrap().kind(), AnalysisValueKind::Depset(value) if value.is_empty())
                    );
                }
            }
            for name in ["source_consumer", "generated_consumer"] {
                let result = workspace
                    .request(&dice, external, name, config.clone())
                    .await
                    .unwrap();
                assert!(result.actions().is_empty());
                let file = artifact(field(&result, "file"));
                let exec_file = artifact(field(&result, "exec_file"));
                assert_eq!(file, artifact(field(&result, "executable")));
                assert_eq!(exec_file, artifact(field(&result, "exec_executable")));
                if name == "generated_consumer" {
                    let AnalysisArtifact::Derived { output, .. } = file else {
                        panic!("generated File")
                    };
                    assert_eq!(
                        output.root().configuration(),
                        config.slug_configuration().unwrap()
                    );
                    let AnalysisArtifact::Derived { output, .. } = exec_file else {
                        panic!("exec generated File")
                    };
                    assert_eq!(
                        output.root().configuration(),
                        exec(&config).slug_configuration().unwrap()
                    );
                } else {
                    assert_eq!(file, exec_file);
                }
            }
        }
    }
}

#[tokio::test]
async fn native_runfiles_policies_keep_distinct_contents_order_and_output_groups() {
    let workspace = Workspace::new();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    for enabled in [true, false, true] {
        let config = configuration(&[(
            NativeCommandOption::IncompatibleFilegroupRunfilesForData,
            if enabled { "true" } else { "false" },
        )]);
        for external in [false, true] {
            for name in ["runfiles", "nested_runfiles"] {
                let result = workspace
                    .request(&dice, external, name, config.clone())
                    .await
                    .unwrap();
                assert!(result.actions().is_empty());
                let info = info(&result);
                assert_eq!(
                    info.file_artifacts()
                        .iter()
                        .map(|f| f.path().into_owned())
                        .collect::<Vec<_>>(),
                    source_paths(external, &["a.txt"])
                );
                assert_eq!(
                    paths(&info.default_runfiles.files),
                    source_paths(external, &["default.txt"])
                );
                assert_eq!(
                    paths(&info.data_runfiles.files),
                    source_paths(
                        external,
                        if enabled {
                            &["a.txt", "data.txt"]
                        } else {
                            &["a.txt"]
                        }
                    )
                );
                assert_eq!(info.default_runfiles.files.order(), DepsetOrder::Postorder);
                assert_eq!(info.data_runfiles.files.order(), DepsetOrder::Postorder);
                assert_eq!(info.default_runfiles.symlinks.to_list().len(), 1);
                assert_eq!(info.default_runfiles.root_symlinks.to_list().len(), 1);
                assert_eq!(
                    info.data_runfiles.symlinks.to_list().len(),
                    usize::from(enabled)
                );
                assert_eq!(
                    info.data_runfiles.root_symlinks.to_list().len(),
                    usize::from(enabled)
                );
                assert!(info.default_runfiles.empty_filenames.is_empty());
                assert_eq!(
                    group_paths(&result, "_hidden_top_level_INTERNAL_"),
                    source_paths(external, &["default.txt"])
                );
                assert_eq!(
                    group_paths(&result, "_validation"),
                    source_paths(external, &["validation.txt"])
                );
            }
        }
    }
}

#[tokio::test]
async fn native_select_tracks_only_selected_files_and_rejects_condition_errors() {
    let workspace = Workspace::new();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let base = configuration(&[]);
    for external in [false, true] {
        let result = workspace
            .request(&dice, external, "selected", base.clone())
            .await
            .unwrap();
        assert_eq!(info(&result).file_artifacts().len(), 1);
        let edges = result
            .edges()
            .iter()
            .filter(|edge| matches!(edge.kind(), ConfiguredEdgeKind::Attribute { attribute, .. } if attribute == "srcs"))
            .collect::<Vec<_>>();
        assert_eq!(edges.len(), 1, "{edges:?}");
        let repo = if external { "dep+" } else { "" };
        assert_eq!(
            edges[0].target(),
            &ConfiguredNodeKey::Null(CanonicalLabel::parse(&format!("@@{repo}//:a.txt")).unwrap())
        );
        assert!(
            !edges
                .iter()
                .any(|edge| edge.target().label().to_string().ends_with(":missing.txt")),
            "{edges:?}"
        );
        for name in ["no_match", "bad_condition"] {
            assert!(
                workspace
                    .request(&dice, external, name, base.clone())
                    .await
                    .is_err(),
                "{name}"
            );
        }
        let opt = configuration(&[(NativeCommandOption::CompilationMode, "opt")]);
        assert!(
            workspace
                .request(&dice, external, "selected", opt)
                .await
                .is_err()
        );
        for (config, expected) in [
            (base.clone(), "a.txt"),
            (
                configuration(&[(NativeCommandOption::CompilationMode, "opt")]),
                "b.txt",
            ),
            (base.clone(), "a.txt"),
        ] {
            let result = workspace
                .request(&dice, external, "switch", config)
                .await
                .unwrap();
            assert_eq!(
                info(&result).file_artifacts()[0].path().as_ref(),
                source_paths(external, &[expected])[0]
            );
        }
    }
}

#[tokio::test]
async fn native_visibility_cycles_cardinality_and_unsupported_modes_fail_closed() {
    let workspace = Workspace::new();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let base = configuration(&[]);
    for external in [false, true] {
        for (name, expected) in [
            ("cycle_a", "cycle"),
            ("private_cross", "visibility"),
            ("macro_private", "visibility"),
            ("zero_tool", "executable"),
            ("multi_tool", "executable"),
            ("with_data", "data"),
            ("with_group", "output_group"),
        ] {
            let error = workspace
                .request(&dice, external, name, base.clone())
                .await
                .unwrap_err();
            assert!(error.to_lowercase().contains(expected), "{name}: {error}");
            if matches!(name, "with_data" | "with_group") {
                assert!(error.contains("unsupported"), "{name}: {error}");
            }
        }
        assert!(
            workspace
                .request(&dice, external, "public_cross", base.clone())
                .await
                .is_ok()
        );
        let unchecked = configuration(&[(NativeCommandOption::CheckVisibility, "false")]);
        for name in ["private_cross", "macro_private"] {
            assert!(
                workspace
                    .request(&dice, external, name, unchecked.clone())
                    .await
                    .is_ok(),
                "{name}"
            );
        }
        let covered = configuration(&[(NativeCommandOption::CollectCodeCoverage, "true")]);
        let error = workspace
            .request(&dice, external, "single", covered)
            .await
            .unwrap_err();
        assert!(error.contains("coverage"), "{error}");
        assert!(
            workspace
                .request(&dice, external, "single", base.clone())
                .await
                .is_ok()
        );
    }
}
