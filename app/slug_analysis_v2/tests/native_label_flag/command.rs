use super::*;

async fn request(
    workspace: &Workspace,
    dice: &Arc<Dice>,
    target: &str,
    rows: Vec<CommandConfigurationOccurrence>,
    repositories: &[(&str, &str)],
) -> Result<Arc<ConfiguredNodeResult>, String> {
    command_configuration_request_result_with_inputs(
        dice,
        &workspace.0,
        target,
        rows.into(),
        Arc::new(RootActivationTracker::default()),
        repositories,
    )
    .await
}

fn row(label: &str, value: Option<&str>, negated: bool) -> CommandConfigurationOccurrence {
    CommandConfigurationOccurrence::starlark(label, value, negated)
}

#[tokio::test]
async fn command_label_flags_use_terminal_setting_identity_and_canonical_values() {
    let workspace = Workspace::new();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let repos = [("dep+", "dep")];
    let first = request(&workspace, &dice, "@@//:flag", vec![], &repos)
        .await
        .unwrap();
    assert_eq!(text(&first), "A");
    let changed = request(
        &workspace,
        &dice,
        "@@//:flag",
        vec![
            row("//:flag", Some(":a"), false),
            row("//:alias2", Some(":b"), false),
        ],
        &repos,
    )
    .await
    .unwrap();
    assert_eq!(text(&changed), "B");
    let own = CanonicalLabel::parse("@@//:flag").unwrap();
    let config = changed.configured_target_key().unwrap().configuration();
    assert_eq!(config.starlark_options().iter().len(), 1);
    assert_eq!(
        config.starlark_option(&own).unwrap().value(),
        &StarlarkOptionValue::Label(CanonicalLabel::parse("@@//:b").unwrap())
    );
    let restored = request(
        &workspace,
        &dice,
        "@@//:flag",
        vec![
            row("//:alias", Some("//:b"), false),
            row("//:flag", Some("//:a"), false),
        ],
        &repos,
    )
    .await
    .unwrap();
    assert_eq!(restored, first);
    assert!(
        restored
            .configured_target_key()
            .unwrap()
            .configuration()
            .starlark_option(&own)
            .is_none()
    );

    // The flag is external, but an unqualified CLI VALUE still belongs to root.
    let external = request(
        &workspace,
        &dice,
        "@@dep+//:flag",
        vec![row("@dep//:flag", Some(":b"), false)],
        &repos,
    )
    .await
    .unwrap();
    assert_eq!(external.actual_target().label().to_string(), "@@//:b");
    fs::create_dir(workspace.0.join("pkg")).unwrap();
    fs::write(
        workspace.0.join("pkg/BUILD.bazel"),
        "exports_files(['tool'])\n",
    )
    .unwrap();
    fs::write(workspace.0.join("pkg/tool"), "source\n").unwrap();
    let source = request(
        &workspace,
        &dice,
        "@@//:source",
        vec![row("//:source", Some("pkg:tool"), false)],
        &repos,
    )
    .await
    .unwrap();
    assert_eq!(source.actual_target().label().to_string(), "@@//pkg:tool");

    for (value, negated, message) in [
        (None, false, "expected a value"),
        (Some(":b"), true, "no-form"),
        (Some("//bad/...:x"), false, "..."),
    ] {
        let error = request(
            &workspace,
            &dice,
            "@@//:flag",
            vec![row("//:flag", value, negated)],
            &repos,
        )
        .await
        .unwrap_err();
        assert!(error.contains(message), "{error}");
    }
    let build = include_str!("BUILD.fixture");
    fs::write(workspace.0.join("BUILD.bazel"), format!("{build}\nalias(name='cycle_a', actual=':cycle_b')\nalias(name='cycle_b', actual=':cycle_a')\nalias(name='configured_alias',actual=select({{'//conditions:default':':flag'}}))\n")).unwrap();
    for (name, message) in [
        ("//:cycle_a", "cycle"),
        ("//:configured_alias", "configurable alias"),
    ] {
        let error = request(
            &workspace,
            &dice,
            "@@//:flag",
            vec![row(name, Some(":b"), false)],
            &repos,
        )
        .await
        .unwrap_err();
        assert!(error.contains(message), "{error}");
    }
}

#[tokio::test]
async fn command_label_value_only_mapping_changes_and_restores() {
    let workspace = Workspace::new();
    fs::create_dir(workspace.0.join("other")).unwrap();
    for (name, contents) in [
        ("MODULE.bazel", "module(name='other',version='1.0.0')\n"),
        ("REPO.bazel", ""),
        (".bazelignore", ""),
        ("BUILD.bazel", include_str!("BUILD.fixture")),
        ("defs.bzl", include_str!("defs.bzl")),
        ("implicit.txt", "source\n"),
    ] {
        fs::write(workspace.0.join("other").join(name), contents).unwrap();
    }
    let module = |swap| {
        format!(
            "module(name='root')\nbazel_dep(name='dep',version='1.0.0',repo_name='{}')\nbazel_dep(name='other',version='1.0.0',repo_name='{}')\nlocal_path_override(module_name='dep',path='dep')\nlocal_path_override(module_name='other',path='other')\n",
            if swap { "spare" } else { "visible" },
            if swap { "visible" } else { "spare" }
        )
    };
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let repos = [("dep+", "dep"), ("other+", "other")];
    let mut results = Vec::new();
    for swap in [false, true, false] {
        fs::write(workspace.0.join("MODULE.bazel"), module(swap)).unwrap();
        let result = request(
            &workspace,
            &dice,
            "@@//:flag",
            vec![row("//:flag", Some("@visible//:b"), false)],
            &repos,
        )
        .await
        .unwrap();
        assert_eq!(
            result.actual_target().label().to_string(),
            if swap { "@@other+//:b" } else { "@@dep+//:b" }
        );
        results.push(result);
    }
    assert_ne!(results[0], results[1]);
    assert_eq!(results[0], results[2]);
}

#[tokio::test]
async fn label_transition_inputs_and_outputs_reject_before_invocation() {
    let workspace = Workspace::new();
    let defs = format!(
        "{}\ndef _forbidden(settings, attr): fail('TRANSITION_RAN')\ninput_transition = transition(implementation=_forbidden, inputs=['//:flag'], outputs=[])\noutput_transition = transition(implementation=_forbidden, inputs=[], outputs=['//:flag'])\ndef _empty(ctx): return []\ninput_rule = rule(implementation=_empty, attrs={{'dep':attr.label(cfg=input_transition)}})\noutput_rule = rule(implementation=_empty, attrs={{'dep':attr.label(cfg=output_transition)}})\n",
        include_str!("defs.bzl")
    );
    fs::write(workspace.0.join("defs.bzl"), defs).unwrap();
    fs::write(workspace.0.join("BUILD.bazel"), format!("{}\nload(':defs.bzl','input_rule','output_rule')\ninput_rule(name='input',dep=':a')\noutput_rule(name='output',dep=':a')\n", include_str!("BUILD.fixture"))).unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    for name in ["input", "output"] {
        let error = workspace
            .request(&dice, false, name, configuration())
            .await
            .unwrap_err();
        assert!(
            error.contains("label-valued build-setting transitions are unsupported"),
            "{error}"
        );
        assert!(
            !error.contains("TRANSITION_RAN"),
            "implementation must not run: {error}"
        );
    }
}
