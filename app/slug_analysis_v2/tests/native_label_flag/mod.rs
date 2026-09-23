//! Source regressions: LabelBuildSettingTest + ConfigSetting normalized predicates.
use slug_configuration_v2::StarlarkOptionValue;

use super::*;

mod command;

#[tokio::test]
async fn label_flag_declaring_visibility_tracks_package_groups_and_restores() {
    let workspace = Workspace::new();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let original = include_str!("BUILD.fixture");
    fs::create_dir_all(workspace.0.join("access")).unwrap();
    let group_build = workspace.0.join("access/BUILD.bazel");
    let group = "package_group(name='allowed',packages=['//...'])\n";
    fs::write(&group_build, group).unwrap();
    fs::write(workspace.0.join("BUILD.bazel"), format!("{original}\nlabel_flag(name='visible_flag',build_setting_default=':test_leaf',visibility=['//access:allowed'])\n")).unwrap();
    let first = workspace
        .request(&dice, false, "visible_flag", configuration())
        .await
        .unwrap();
    assert_eq!(text(&first), "test");
    assert_eq!(first.edges().len(), 2);
    assert!(first.edges().iter().any(|edge| matches!(
        edge.kind(),
        slug_analysis_v2::ConfiguredEdgeKind::DeclaringVisibility
    )));
    assert!(
        first
            .runfiles_packages()
            .to_list()
            .iter()
            .any(|metadata| metadata.package().to_string() == "@@//access")
    );
    for replacement in ["", "filegroup(name='allowed')\n"] {
        fs::write(&group_build, replacement).unwrap();
        let error = workspace
            .request(&dice, false, "visible_flag", configuration())
            .await
            .err()
            .expect("declaring visibility fails closed");
        assert!(error.contains("allowed"), "{error}");
        if !replacement.is_empty() {
            assert!(error.contains("package group"), "{error}");
        }
        fs::write(&group_build, group).unwrap();
        let restored = workspace
            .request(&dice, false, "visible_flag", configuration())
            .await
            .unwrap();
        assert_eq!(text(&restored), "test");
        assert_eq!(first, restored);
    }
}

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
            fs::write(package.join("implicit.txt"), "source\n").unwrap();
        }
        Self(root)
    }
    async fn request(
        &self,
        dice: &Arc<Dice>,
        external: bool,
        name: &str,
        configuration: ConfigurationKey,
    ) -> Result<Arc<ConfiguredNodeResult>, String> {
        let repo = if external { "dep+" } else { "" };
        root_target_request_with_inputs(
            dice,
            &self.0,
            &format!("@@{repo}//:{name}"),
            configuration,
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

fn configuration() -> ConfigurationKey {
    super::cc_configure_features::config(&[])
}
fn exec(config: &ConfigurationKey) -> ConfigurationKey {
    let config = config.slug_configuration().unwrap();
    ConfigurationKey::from_slug(
        config
            .to_exec_for_platform(&config.host_platform_label().unwrap())
            .unwrap(),
    )
}
fn with_label(config: &ConfigurationKey, setting: &str, value: &str) -> ConfigurationKey {
    config.with_starlark_option(StarlarkOption::new(
        CanonicalLabel::parse(setting).unwrap(),
        StarlarkOptionValue::Label(CanonicalLabel::parse(value).unwrap()),
        slug_configuration_v2::StarlarkOptionScope::Default,
    ))
}
fn field<'a>(result: &'a ConfiguredNodeResult, name: &str) -> &'a slug_build_api_v2::AnalysisValue {
    result
        .providers()
        .occurrences()
        .find_map(|p| p.field(name))
        .unwrap()
}
fn text(result: &ConfiguredNodeResult) -> &str {
    let AnalysisValueKind::String(value) = field(result, "value").kind() else {
        panic!("string proof");
    };
    value
}

#[tokio::test]
async fn label_flags_forward_real_providers_files_and_exec_without_actions() {
    let workspace = Workspace::new();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let base = configuration();
    for external in [false, true] {
        let prefix = if external { "@@dep+" } else { "@@" };
        for config in [base.clone(), exec(&base)] {
            let flag = workspace
                .request(&dice, external, "flag", config.clone())
                .await
                .unwrap();
            assert!(flag.actions().is_empty());
            assert_eq!(flag.key().label().to_string(), format!("{prefix}//:flag"));
            assert_eq!(
                flag.actual_target().label().to_string(),
                format!("{prefix}//:a")
            );
            assert_eq!(text(&flag), "A");
            assert_eq!(flag.edges().len(), 1);
            let probe = workspace
                .request(&dice, external, "probe", config.clone())
                .await
                .unwrap();
            assert_eq!(text(&probe), "A");
            let AnalysisValueKind::Artifact(file) = field(&probe, "file").kind() else {
                panic!("real File");
            };
            assert_eq!(
                file,
                flag.providers()
                    .default_info()
                    .unwrap()
                    .executable
                    .as_ref()
                    .unwrap()
            );
            let changed = with_label(
                &config,
                &format!("{prefix}//:flag"),
                &format!("{prefix}//:b"),
            );
            let probe = workspace
                .request(&dice, external, "probe", changed)
                .await
                .unwrap();
            assert_eq!(text(&probe), "B");
            let source = workspace
                .request(&dice, external, "source", config.clone())
                .await
                .unwrap();
            assert!(matches!(source.actual_target(), ConfiguredNodeKey::Null(_)));
            assert!(source.actions().is_empty());
            assert_eq!(
                source
                    .providers()
                    .default_info()
                    .unwrap()
                    .file_artifacts()
                    .len(),
                1
            );
            let private = workspace
                .request(&dice, external, "private_test", config)
                .await
                .unwrap();
            assert_eq!(text(&private), "test");
        }
    }
    let tool = workspace.request(&dice, false, "tool", base).await.unwrap();
    let AnalysisValueKind::Artifact(slug_build_api_v2::AnalysisArtifact::Derived {
        output, ..
    }) = field(&tool, "file").kind()
    else {
        panic!("derived exec File");
    };
    assert_eq!(
        output.root().configuration().kind(),
        slug_configuration_v2::SlugConfigurationKind::Exec
    );
}

#[tokio::test]
async fn label_predicates_normalize_alias_keys_and_expected_labels_and_reject_duplicate_actuals() {
    let workspace = Workspace::new();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let base = configuration();
    for external in [false, true] {
        let selected = workspace
            .request(&dice, external, "selected", base.clone())
            .await
            .unwrap();
        assert_eq!(text(&selected), "specific");
        for (target, message) in [
            ("duplicate_probe", "referenced multiple times"),
            ("invalid_probe", "..."),
        ] {
            let error = workspace
                .request(&dice, external, target, base.clone())
                .await
                .err()
                .expect("rejected condition");
            assert!(error.contains(message), "{error}");
        }
    }
    let changed = with_label(&base, "@@//:flag", "@@//:value_alias");
    let error = workspace
        .request(&dice, false, "selected", changed.clone())
        .await
        .err()
        .expect("alias value is not ultimate target value");
    assert!(error.contains("match"), "{error}");
    assert_eq!(
        text(
            &workspace
                .request(&dice, false, "probe", changed)
                .await
                .unwrap()
        ),
        "A"
    );
    let selected = workspace
        .request(&dice, false, "selected", base)
        .await
        .unwrap();
    assert_eq!(text(&selected), "specific");
}

#[tokio::test]
async fn label_flag_same_dice_changes_missing_inputs_and_cycles_restore() {
    let workspace = Workspace::new();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let base = configuration();
    let first = workspace
        .request(&dice, false, "probe", base.clone())
        .await
        .unwrap();
    let original = include_str!("BUILD.fixture");
    fs::write(
        workspace.0.join("BUILD.bazel"),
        original.replace("value = \"A\"", "value = \"A changed\""),
    )
    .unwrap();
    let changed = workspace
        .request(&dice, false, "probe", base.clone())
        .await
        .unwrap();
    assert_eq!(text(&changed), "A changed");
    assert_ne!(first, changed);
    fs::write(workspace.0.join("BUILD.bazel"), original).unwrap();
    let restored = workspace
        .request(&dice, false, "probe", base.clone())
        .await
        .unwrap();
    assert_eq!(text(&restored), "A");
    assert_eq!(first, restored);
    let override_missing = with_label(&base, "@@//:unused_missing", "@@//:a");
    assert_eq!(
        text(
            &workspace
                .request(&dice, false, "unused_missing", override_missing)
                .await
                .unwrap()
        ),
        "A"
    );
    assert!(
        workspace
            .request(&dice, false, "unused_missing", base.clone())
            .await
            .is_err()
    );
    let source = workspace
        .request(&dice, false, "source", base.clone())
        .await
        .unwrap();
    fs::remove_file(workspace.0.join("implicit.txt")).unwrap();
    assert!(
        workspace
            .request(&dice, false, "source", base.clone())
            .await
            .is_err()
    );
    fs::write(workspace.0.join("implicit.txt"), "source\n").unwrap();
    assert_eq!(
        source,
        workspace
            .request(&dice, false, "source", base.clone())
            .await
            .unwrap()
    );
    let cycle = "\nlabel_flag(name='cycle_flag',build_setting_default=':cycle_alias')\nconfig_setting(name='cycle_condition',flag_values={':cycle_flag':':a'})\nalias(name='cycle_alias',actual=select({':cycle_condition':':a','//conditions:default':':a'}))\n";
    fs::write(
        workspace.0.join("BUILD.bazel"),
        format!("{original}{cycle}"),
    )
    .unwrap();
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        workspace.request(&dice, false, "cycle_flag", base.clone()),
    )
    .await
    .expect("condition cycle must terminate")
    .err()
    .expect("cycle rejected");
    assert!(error.contains("cycle"), "{error}");
    fs::write(
        workspace.0.join("BUILD.bazel"),
        format!("{original}\nlabel_flag(name='cycle_flag',build_setting_default=':a')\n"),
    )
    .unwrap();
    assert_eq!(
        text(
            &workspace
                .request(&dice, false, "cycle_flag", base)
                .await
                .unwrap()
        ),
        "A"
    );
}

#[tokio::test]
async fn same_truth_predicates_recompute_specialization_and_condition_mapping() {
    let workspace = Workspace::new();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let base = configuration();
    let original = include_str!("BUILD.fixture");
    let extra = r#"
label_flag(name="other_flag",build_setting_default=":a")
config_setting(name="other_specific",values={"compilation_mode":"fastbuild"},flag_values={":other_flag":":a"})
select_probe(name="same_truth",value=select({":relative":"base",":other_specific":"specific"}))
label_flag(name="mapped_flag",build_setting_default="@dep//:a")
config_setting(name="mapped_condition",flag_values={":mapped_flag":"@dep//:a"})
select_probe(name="mapped",value=select({":mapped_condition":"mapped"}))
"#;
    let first_build = format!("{original}{extra}");
    fs::write(workspace.0.join("BUILD.bazel"), &first_build).unwrap();
    let first = workspace
        .request(&dice, false, "same_truth", base.clone())
        .await
        .unwrap();
    assert_eq!(text(&first), "specific");
    assert_eq!(
        text(
            &workspace
                .request(&dice, false, "mapped", base.clone())
                .await
                .unwrap()
        ),
        "mapped"
    );
    // Both conditions still match, but now compare different actual labels.
    // Truth-only DICE cutoff must not retain the old refinement relation.
    let changed_extra = extra
        .replace(
            "build_setting_default=\":a\"",
            "build_setting_default=\":b\"",
        )
        .replace("{\":other_flag\":\":a\"}", "{\":other_flag\":\":b\"}");
    fs::write(
        workspace.0.join("BUILD.bazel"),
        format!("{original}{changed_extra}"),
    )
    .unwrap();
    let error = workspace
        .request(&dice, false, "same_truth", base.clone())
        .await
        .err()
        .expect("incomparable true predicates");
    assert!(error.contains("ambiguous"), "{error}");
    fs::write(workspace.0.join("BUILD.bazel"), &first_build).unwrap();
    let restored = workspace
        .request(&dice, false, "same_truth", base.clone())
        .await
        .unwrap();
    assert_eq!(text(&restored), "specific");
    assert_eq!(first, restored);

    fs::create_dir_all(workspace.0.join("condition")).unwrap();
    fs::write(
        workspace.0.join("condition/BUILD.bazel"),
        r#"
load("//:defs.bzl", "leaf", "select_probe")
leaf(name="a",value="condition A")
config_setting(name="local",flag_values={"//:flag":":a"})
select_probe(name="probe",value=select({":local":"local","//conditions:default":"not local"}))
"#,
    )
    .unwrap();
    let request = |config| {
        root_target_request_with_inputs(
            &dice,
            &workspace.0,
            "@@//condition:probe",
            config,
            Arc::new(RootActivationTracker::default()),
            root_epoch(&workspace.0),
            &[("dep+", "dep")],
        )
    };
    assert_eq!(text(&request(base.clone()).await.unwrap()), "not local");
    assert_eq!(
        text(
            &request(with_label(&base, "@@//:flag", "@@//condition:a"))
                .await
                .unwrap()
        ),
        "local"
    );
}

#[tokio::test]
async fn label_flag_rejects_unmodeled_attributes_and_literal_setting_alias_cycles() {
    let workspace = Workspace::new();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let original = include_str!("BUILD.fixture");
    for (attribute, value) in [
        ("tags", "['unmodeled']"),
        ("features", "['unmodeled']"),
        ("testonly", "True"),
    ] {
        fs::write(workspace.0.join("BUILD.bazel"), format!("{original}\nlabel_flag(name='bad',build_setting_default=':a',{attribute}={value})\n")).unwrap();
        let error = workspace
            .request(&dice, false, "bad", configuration())
            .await
            .err()
            .expect("nonneutral attr rejected");
        assert!(
            error.contains("unsupported") && error.contains(attribute),
            "{error}"
        );
    }
    let cycle = r#"
alias(name="setting_cycle_a",actual=":setting_cycle_b")
alias(name="setting_cycle_b",actual=":setting_cycle_a")
config_setting(name="bad_setting",flag_values={":setting_cycle_a":":a"})
select_probe(name="bad",value=select({":bad_setting":"bad"}))
"#;
    fs::write(
        workspace.0.join("BUILD.bazel"),
        format!("{original}{cycle}"),
    )
    .unwrap();
    let error = workspace
        .request(&dice, false, "bad", configuration())
        .await
        .err()
        .expect("literal alias cycle rejected");
    assert!(error.contains("alias cycle"), "{error}");
    let selected_alias = r#"
alias(name="selected_setting",actual=select({"//conditions:default":":flag"}))
config_setting(name="bad_setting",flag_values={":selected_setting":":a"})
select_probe(name="bad",value=select({":bad_setting":"bad"}))
"#;
    fs::write(
        workspace.0.join("BUILD.bazel"),
        format!("{original}{selected_alias}"),
    )
    .unwrap();
    let error = workspace
        .request(&dice, false, "bad", configuration())
        .await
        .err()
        .expect("selector alias to setting rejected");
    assert!(
        error.contains("configurable alias") && error.contains("unsupported"),
        "{error}"
    );
}
