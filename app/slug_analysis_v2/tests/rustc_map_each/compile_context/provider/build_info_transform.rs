//! Installed rule construction and pure rendering from supplied template/status strings.
use slug_build_api_v2::ActionOutputMetadata;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::BuildInfoCallbackRecipe;
use slug_build_api_v2::BuildInfoTransformKind;
use slug_build_api_v2::DerivedArtifactRoot;
use slug_build_api_v2::SymlinkTarget;
use slug_build_api_v2::WorkspaceStatusArtifactKind;

use super::*;

const INSTALLED: &str = "@@bazel_tools//tools/build_defs/build_info:cc_build_info";
const TEMPLATE_PACKAGE: &str = "@@bazel_tools//tools/build_defs/build_info/templates";

fn fixture() -> PathBuf {
    let root = workspace();
    // Instantiate the unchanged loaded rule, not an authored replacement body.
    // All selected-repository fixture writes precede its first DICE request.
    for build in [
        root.join("BUILD.bazel"),
        root.join("rules_rust/BUILD.bazel"),
    ] {
        let original = fs::read_to_string(&build).unwrap();
        fs::write(build, format!("{original}\nload('@bazel_tools//tools/build_defs/build_info:bazel_cc_build_info.bzl','bazel_cc_build_info')\nbazel_cc_build_info(name='local_build_info')\n")).unwrap();
    }
    root
}

fn group(result: &ConfiguredNodeResult, name: &str) -> Vec<AnalysisArtifact> {
    result
        .providers()
        .output_group_info()
        .unwrap()
        .groups()
        .get(name)
        .unwrap()
        .to_list()
        .into_iter()
        .map(|value| {
            let AnalysisValueKind::Artifact(artifact) = value.kind() else {
                panic!("genuine output-group File")
            };
            artifact.clone()
        })
        .collect()
}

fn check_rendered_header(spec: &slug_build_api_v2::BuildInfoTransformSpec) {
    let status = slug_configuration_v2::CanonicalStringMap::from_pairs([
        ("BUILD_EMBED_LABEL", "release"),
        ("BUILD_HOST", "builder"),
        ("BUILD_USER", "user"),
        ("BUILD_SCM_REVISION", "abc123"),
        ("BUILD_SCM_STATUS", "clean"),
        ("BUILD_TIMESTAMP", "123456"),
    ]);
    // Supplied immutable catalog bodies, not runtime template-read authority.
    let (template, expected) = match spec.callback().kind() {
        BuildInfoTransformKind::Info => (
            include_str!(
                "../../../../../slug_bzlmod_v2/builtin/bazel_tools/tools/build_defs/build_info/templates/non_volatile_file.h.template"
            ),
            "#define BUILD_EMBED_LABEL \"release\"\n#define BUILD_HOST \"builder\"\n#define BUILD_USER \"user\"\n",
        ),
        BuildInfoTransformKind::Version => (
            include_str!(
                "../../../../../slug_bzlmod_v2/builtin/bazel_tools/tools/build_defs/build_info/templates/volatile_file.h.template"
            ),
            "#define BUILD_SCM_REVISION \"abc123\"\n#define BUILD_SCM_STATUS \"clean\"\n#define BUILD_TIMESTAMP 123456\n",
        ),
    };
    let before = spec.clone();
    for _ in 0..2 {
        assert_eq!(
            spec.callback().render_template(&status, template).unwrap(),
            expected
        );
        assert_eq!(spec, &before);
    }
    if spec.callback().kind() == BuildInfoTransformKind::Version {
        assert_eq!(
            spec.callback()
                .render_template(&Default::default(), template),
            Err(
                "timestamp information is missing from workspace status file, BUILD_TIMESTAMP key not found."
            )
        );
    }
}

fn check(result: &ConfiguredNodeResult, root: &std::path::Path, configuration: &ConfigurationKey) {
    let key = result.configured_target_key().unwrap();
    let owner = key.artifact_owner();
    let output_root =
        DerivedArtifactRoot::bin(configuration.slug_configuration().unwrap().clone()).unwrap();
    let actions = result.actions();
    assert_eq!(
        actions.len(),
        3,
        "unconditional two transforms plus redacted symlink"
    );
    let mut generated = Vec::new();
    for (index, kind, basename, template_name, role, metadata) in [
        (
            0,
            BuildInfoTransformKind::Version,
            "volatile_file.h",
            "volatile_file.h.template",
            WorkspaceStatusArtifactKind::Volatile,
            ActionOutputMetadata::Constant,
        ),
        (
            1,
            BuildInfoTransformKind::Info,
            "non_volatile_file.h",
            "non_volatile_file.h.template",
            WorkspaceStatusArtifactKind::Stable,
            ActionOutputMetadata::Ordinary,
        ),
    ] {
        let action = &actions[index];
        let spec = action
            .build_info_transform_spec()
            .expect("typed transform in source order");
        check_rendered_header(spec);
        assert_eq!(action.mnemonic(), "TranslateBuildInfo");
        assert_eq!(spec.callback().kind(), kind);
        assert_eq!(
            spec.callback().source().to_string(),
            BuildInfoCallbackRecipe::SOURCE_LABEL
        );
        assert_eq!(
            spec.callback().digest(),
            &BuildInfoCallbackRecipe::SOURCE_DIGEST
        );
        assert_eq!(spec.status().kind(), role);
        assert_eq!(spec.status().root().workspace().as_path(), root);
        assert_eq!(spec.status().root().path(), "bazel-out");
        assert_eq!(spec.status().is_constant_metadata(), index == 0);
        assert_eq!(spec.is_volatile(), index == 0);
        assert_eq!(spec.execute_unconditionally(), index == 0);
        let template = AnalysisArtifact::Source(
            CanonicalLabel::parse(&format!("{TEMPLATE_PACKAGE}:{template_name}")).unwrap(),
        );
        assert_eq!(spec.template(), &template);
        assert_eq!(
            spec.inputs(),
            &[
                AnalysisArtifact::WorkspaceStatus(spec.status().clone()),
                template
            ]
        );
        assert_eq!(action.outputs(), &[spec.output().clone()]);
        assert_eq!(spec.output().root(), &output_root);
        assert_eq!(spec.output().metadata(), metadata);
        let package = key.label().package().package().as_str();
        let relative = if package.is_empty() {
            basename.to_owned()
        } else {
            format!("{package}/{basename}")
        };
        let expected = slug_build_api_v2::repository_relative_output_path(
            key.label().package().repo(),
            &relative,
        );
        assert_eq!(spec.output().root_relative_path(), expected);
        assert_eq!(
            spec.output().path(),
            format!("{}/{expected}", output_root.path())
        );
        assert!(
            action.render_argv().is_err(),
            "no fabricated executable projection"
        );
        assert!(slug_build_api_v2::ReapiCommandProjection::from_action(action).is_err());
        generated.push(AnalysisArtifact::Derived {
            owner: owner.clone(),
            output: spec.output().clone(),
        });
    }
    let symlink = actions[2]
        .symlink_spec()
        .expect("redacted branch retains genuine Symlink");
    assert_eq!(actions[2].mnemonic(), "Symlink");
    assert_eq!(actions[2].outputs(), &[symlink.output().clone()]);
    let SymlinkTarget::Artifact {
        input,
        require_executable,
        ..
    } = symlink.target()
    else {
        panic!("redacted template artifact target")
    };
    assert_eq!(
        input,
        &AnalysisArtifact::Source(
            CanonicalLabel::parse(&format!("{TEMPLATE_PACKAGE}:redacted_file.h.template")).unwrap()
        )
    );
    assert!(!require_executable);
    assert_eq!(symlink.output().metadata(), ActionOutputMetadata::Ordinary);
    assert_eq!(symlink.output().root(), &output_root);
    let package = key.label().package().package().as_str();
    let relative = if package.is_empty() {
        "redacted_file.h".to_owned()
    } else {
        format!("{package}/redacted_file.h")
    };
    assert_eq!(
        symlink.output().root_relative_path(),
        slug_build_api_v2::repository_relative_output_path(key.label().package().repo(), &relative)
    );
    let redacted = AnalysisArtifact::Derived {
        owner,
        output: symlink.output().clone(),
    };
    assert_eq!(
        group(result, "non_redacted_build_info_files"),
        [generated[1].clone(), generated[0].clone()]
    );
    assert_eq!(group(result, "redacted_build_info_files"), [redacted]);
    assert!(
        !root.join("bazel-out").exists(),
        "analysis must not write output/status bytes"
    );
}

#[tokio::test]
async fn installed_build_info_and_unchanged_main_external_instances_register_real_actions() {
    let root = fixture();
    let dice = registry::dice(&root);
    let target = config(&[]);
    let exec = default_exec(&target);
    let mut shared_status = None;
    for label in [
        INSTALLED,
        "@@//:local_build_info",
        "@@rules_rust+//:local_build_info",
    ] {
        for configuration in [target.clone(), exec.clone()] {
            let result = request(&dice, &root, label, configuration.clone())
                .await
                .unwrap();
            check(&result, &root, &configuration);
            let status = result.actions()[0]
                .build_info_transform_spec()
                .unwrap()
                .status()
                .clone();
            if let Some(expected) = &shared_status {
                assert_eq!(&status, expected);
            }
            shared_status = Some(status);
        }
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn installed_build_info_restores_full_configuration_and_rejects_java_callbacks() {
    let root = fixture();
    let dice = registry::dice(&root);
    let a = config(&[]);
    let b = config(&[(NativeCommandOption::Copt, "-DBUILD_INFO_B")]);
    let first = request(&dice, &root, INSTALLED, a.clone()).await.unwrap();
    check(&first, &root, &a);
    let changed = request(&dice, &root, INSTALLED, b.clone()).await.unwrap();
    check(&changed, &root, &b);
    assert_ne!(first, changed);
    let left = first.actions()[0].build_info_transform_spec().unwrap();
    let right = changed.actions()[0].build_info_transform_spec().unwrap();
    assert_eq!(left.status(), right.status());
    assert_eq!(left.template(), right.template());
    assert_eq!(left.callback(), right.callback());
    assert_ne!(left.output().root(), right.output().root());
    let restored = request(&dice, &root, INSTALLED, a).await.unwrap();
    assert_eq!(first, restored);
    let error = request(
        &dice,
        &root,
        "@@bazel_tools//tools/build_defs/build_info:java_build_info",
        config(&[]),
    )
    .await
    .unwrap_err();
    assert!(error.contains("unchanged pinned source"), "{error}");
    assert!(!root.join("bazel-out").exists());
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn configured_bare_user_and_native_infos_are_admitted_but_structs_are_not() {
    let root = fixture();
    fs::write(
        root.join("bare_return.bzl"),
        r#"
Bare = provider(fields = ["value"])
def _impl(ctx):
    if ctx.attr.mode == "user":
        return Bare(value = "bare-user")
    if ctx.attr.mode == "native":
        return platform_common.TemplateVariableInfo({"BARE": "native"})
    if ctx.attr.mode == "struct":
        return struct(value = "not Info")
    return "not Info"
bare_return = rule(implementation = _impl, attrs = {"mode": attr.string()})
"#,
    )
    .unwrap();
    let build = root.join("BUILD.bazel");
    let original = fs::read_to_string(&build).unwrap();
    fs::write(
        build,
        format!(
            "{original}\nload(':bare_return.bzl','bare_return')\n{}",
            ["user", "native", "struct", "scalar"]
                .into_iter()
                .map(|mode| format!("bare_return(name='bare_{mode}',mode='{mode}')\n"))
                .collect::<String>()
        ),
    )
    .unwrap();
    let dice = registry::dice(&root);
    let user = request(&dice, &root, "@@//:bare_user", config(&[]))
        .await
        .unwrap();
    let info = user
        .providers()
        .occurrences()
        .find(|info| info.field("value").is_some())
        .expect("genuine user Info retained by configured result");
    assert_eq!(info.field("value").unwrap().as_str(), Some("bare-user"));
    assert!(user.actions().is_empty());
    let native = request(&dice, &root, "@@//:bare_native", config(&[]))
        .await
        .unwrap();
    assert_eq!(
        native
            .providers()
            .template_variable_info()
            .unwrap()
            .variables()
            .get("BARE")
            .unwrap()
            .as_str(),
        "native"
    );
    assert!(native.actions().is_empty());
    for mode in ["struct", "scalar"] {
        let error = request(&dice, &root, &format!("@@//:bare_{mode}"), config(&[]))
            .await
            .unwrap_err();
        assert!(error.contains("returned non-provider"), "{mode}: {error}");
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn installed_template_source_keeps_catalog_authority_and_refuses_host_resolution() {
    let root = fixture();
    let label =
        CanonicalLabel::parse(&format!("{TEMPLATE_PACKAGE}:volatile_file.h.template")).unwrap();
    // This guessed execution-path shadow exists before observations. It is not
    // an installed source owner and must never make runtime resolution legal.
    let shadow = root.join(
        "external/bazel_tools/tools/build_defs/build_info/templates/volatile_file.h.template",
    );
    fs::create_dir_all(shadow.parent().unwrap()).unwrap();
    fs::write(&shadow, "HOST SHADOW: not installed template bytes\n").unwrap();
    let dice = registry::dice(&root);
    let source = request(&dice, &root, &label.to_string(), config(&[]))
        .await
        .unwrap();
    assert!(source.actions().is_empty());
    assert_eq!(
        source.providers().default_info().unwrap().file_artifacts(),
        [AnalysisArtifact::Source(label.clone())]
    );
    let mut transaction = dice
        .updater_with_data(dice::UserComputationData {
            cycle_detector: Some(slug_analysis_v2::analysis_cycle_detector()),
            ..Default::default()
        })
        .commit()
        .await;
    let workspace = slug_workspace_v2::NormalizedAbsolutePath::new(root.clone()).unwrap();
    let outcome =
        slug_analysis_v2::resolve_source_input_observed(&mut transaction, &workspace, &label).await;
    let slug_loading_v2::LoadingPreparationOutcome::Complete(Ok(observed)) = outcome else {
        panic!(
            "builtin runtime path must fail closed, not request guessed path observations: {outcome:?}"
        )
    };
    let error = observed.result().as_ref().unwrap_err().to_string();
    assert!(
        error.contains("built-in bazel_tools source requires its immutable source owner"),
        "{error}"
    );
    drop(transaction);
    for absent in [
        "@@bazel_tools//tools/build_defs/build_info/templates:missing.h.template",
        "@@bazel_tools//tools/build_defs/build_info:templates",
    ] {
        // Declaration rejection may precede source-owner lookup; Bzlmod's
        // owner tests separately discriminate unknown and directory entries.
        assert!(
            request(&dice, &root, absent, config(&[])).await.is_err(),
            "non-file catalog target must reject: {absent}"
        );
    }
    assert_eq!(
        fs::read_to_string(&shadow).unwrap(),
        "HOST SHADOW: not installed template bytes\n"
    );
    assert!(!root.join("bazel-out").exists());
    fs::remove_dir_all(root).unwrap();
}
