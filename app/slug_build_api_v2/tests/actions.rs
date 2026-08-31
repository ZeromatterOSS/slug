/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;
use std::sync::Arc;

use slug_build_api_v2::ActionError;
use slug_build_api_v2::ActionInput;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisDepset;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::ArtifactInputSource;
use slug_build_api_v2::ArtifactInputs;
use slug_build_api_v2::CtxActions;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::ReapiCommandProjection;
use slug_build_api_v2::RetainedArtifactInputs;
use slug_build_api_v2::RetainedCommandLine;
use slug_build_api_v2::RetainedCommandLineSegment;
use slug_build_api_v2::RetainedScalarArg;
use slug_build_api_v2::RetainedScalarValue;
use slug_build_api_v2::SpawnExecutable;
use slug_build_api_v2::SpawnSpec;
use slug_build_api_v2::SymlinkSpec;
use slug_build_api_v2::SymlinkTarget;
use slug_configuration_v2::CanonicalStringMap;
use slug_configuration_v2::HostPathFlavor;
use slug_configuration_v2::NormalizedAbsoluteBazelPath;
use slug_configuration_v2::NormalizedBazelPath;
use slug_configuration_v2::RetainedActionEnvironment;
use slug_identity_v2::CanonicalLabel;

fn source_artifact(name: &str) -> AnalysisArtifact {
    AnalysisArtifact::Source(
        CanonicalLabel::parse(&format!("@@//pkg:{name}")).expect("source label"),
    )
}

fn artifact_depset(name: &str) -> AnalysisDepset {
    AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::artifact(source_artifact(name))],
        Vec::new(),
    )
    .unwrap()
}

fn spawn_action(inputs: AnalysisDepset, tools: AnalysisDepset) -> ActionSpec {
    let command_line = RetainedCommandLine::new(vec![
        RetainedCommandLineSegment::LiteralRun(Arc::from(["--literal".into()])),
        RetainedCommandLineSegment::ArgsSnapshot(Arc::from([
            RetainedScalarArg::new(
                Some("--count"),
                RetainedScalarValue::Integer("7".into()),
                None::<&str>,
            ),
            RetainedScalarArg::new(
                None::<&str>,
                RetainedScalarValue::Artifact(source_artifact("arg.txt")),
                Some("value=%s%%"),
            ),
        ])),
    ]);
    ActionSpec::spawn(SpawnSpec::new(
        SpawnExecutable::Path(
            NormalizedBazelPath::new(HostPathFlavor::Unix, "tools/runner").unwrap(),
        ),
        command_line,
        ArtifactInputs::new(vec![ArtifactInputSource::Depset(
            RetainedArtifactInputs::new(inputs).unwrap(),
        )]),
        ArtifactInputs::new(vec![ArtifactInputSource::Depset(
            RetainedArtifactInputs::new(tools).unwrap(),
        )]),
        vec![ActionOutput::new("pkg/out", ActionOutputKind::File)],
        RetainedActionEnvironment::default().for_action(false, [("K", "V")]),
        CanonicalStringMap::default(),
        "Compile",
        Some("building output"),
    ))
}

#[test]
fn ctx_actions_records_basic_action_ir() {
    let mut actions = CtxActions::new();
    let write_out = actions.declare_file("pkg/write.txt").unwrap();
    let json_out = actions.declare_file("pkg/write.json").unwrap();
    let run_out = actions.declare_file("pkg/run.txt").unwrap();
    let shell_out = actions.declare_file("pkg/shell.txt").unwrap();
    let link_out = actions.declare_symlink("pkg/link.txt").unwrap();
    let template_out = actions.declare_file("pkg/template.txt").unwrap();

    actions
        .write(write_out.clone(), "hello\n", false)
        .expect("write action");
    actions
        .write_json(json_out.clone(), "{\"ok\":true}\n")
        .expect("write json action");
    actions
        .run(
            run_out.clone(),
            "tools/echo",
            vec!["hello".to_owned()],
            vec![ActionInput::new("pkg/input.txt", Some("abc123".to_owned()))],
            vec![ActionInput::new("tools/echo", Some("tool123".to_owned()))],
        )
        .expect("run action");
    actions
        .run_shell(
            shell_out.clone(),
            "printf shell > $1",
            vec![shell_out.path().to_owned()],
            vec![],
        )
        .expect("run shell action");
    actions
        .symlink(link_out.clone(), "pkg/write.txt")
        .expect("symlink action");

    let mut substitutions = BTreeMap::new();
    substitutions.insert("{NAME}".to_owned(), "Slug".to_owned());
    actions
        .expand_template(
            template_out.clone(),
            ActionInput::new("pkg/template.in", Some("tmpl123".to_owned())),
            substitutions,
        )
        .expect("expand template action");

    let registry = actions.registry();
    assert_eq!(registry.actions().len(), 6);
    assert_eq!(registry.output_owner("pkg/run.txt"), Some(2));
    assert!(matches!(
        registry.actions()[0].kind(),
        ActionKind::Write {
            content,
            is_executable: false
        } if content == "hello\n"
    ));
    assert!(matches!(
        registry.actions()[1].kind(),
        ActionKind::WriteJson { content } if content == "{\"ok\":true}\n"
    ));
    assert_eq!(registry.actions()[2].argv(), &["tools/echo", "hello"]);
    assert_eq!(registry.actions()[3].mnemonic(), "Shell");
    assert!(matches!(
        registry.actions()[4].kind(),
        ActionKind::Symlink { target_path } if target_path == "pkg/write.txt"
    ));
    assert!(matches!(
        registry.actions()[5].kind(),
        ActionKind::ExpandTemplate { substitutions, .. } if substitutions["{NAME}"] == "Slug"
    ));
}

#[test]
fn retained_artifact_inputs_stream_ordered_unique_topology_to_sink() {
    let shared_artifact = source_artifact("shared.h");
    let shared = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::artifact(shared_artifact.clone())],
        Vec::new(),
    )
    .unwrap();
    let left_artifact = source_artifact("left.h");
    let left = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::artifact(left_artifact.clone())],
        vec![shared.clone()],
    )
    .unwrap();
    let right_artifact = source_artifact("right.h");
    let right = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::artifact(right_artifact.clone())],
        vec![shared],
    )
    .unwrap();
    let root_artifact = source_artifact("root.h");
    let root = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::artifact(root_artifact.clone())],
        vec![left, right],
    )
    .unwrap();

    let inputs = RetainedArtifactInputs::new(root).unwrap();
    let mut sink = Vec::new();
    inputs
        .visit(|artifact| sink.push(artifact.clone()))
        .unwrap();
    assert_eq!(
        sink,
        [
            shared_artifact,
            left_artifact,
            right_artifact,
            root_artifact
        ]
    );

    let strings = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::string("not-a-file")],
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        RetainedArtifactInputs::new(strings)
            .unwrap_err()
            .to_string(),
        "action inputs require a depset of File, got depset of string"
    );
}

#[test]
fn typed_spawn_retains_one_recipe_and_publication_equal_depsets() {
    let left = spawn_action(artifact_depset("input.h"), artifact_depset("tool.h"));
    let right = spawn_action(artifact_depset("input.h"), artifact_depset("tool.h"));

    assert_eq!(left, right);
    assert_eq!(left.kind(), &ActionKind::Spawn);
    assert_eq!(left.mnemonic(), "Compile");
    assert_eq!(left.progress_message(), Some("building output"));
    assert_eq!(
        left.render_argv(),
        [
            "tools/runner",
            "--literal",
            "--count",
            "7",
            "value=pkg/arg.txt%"
        ]
    );
    assert!(left.argv().is_empty());
    assert!(left.inputs().is_empty());
    assert!(left.tools().is_empty());
    assert!(left.env().is_empty());
    assert!(left.execution_requirements().is_empty());
    assert!(left.param_files().is_empty());
    let typed = left.spawn_spec().unwrap();
    assert_eq!(typed.inputs().sources().len(), 1);
    assert_eq!(typed.tools().sources().len(), 1);
    assert_eq!(typed.environment().fixed().get("K"), Some("V"));

    assert_ne!(
        left,
        spawn_action(artifact_depset("other.h"), artifact_depset("tool.h"))
    );
    assert!(ReapiCommandProjection::from_action(&right).is_err());
}

#[test]
fn spawn_publication_equality_covers_every_ordinary_field() {
    let make = |executable: &str,
                argument: &str,
                output: &str,
                environment: &str,
                requirement: &str,
                mnemonic: &str,
                progress: &str| {
        ActionSpec::spawn(SpawnSpec::new(
            SpawnExecutable::Path(
                NormalizedBazelPath::new(HostPathFlavor::Unix, executable).unwrap(),
            ),
            RetainedCommandLine::new(vec![RetainedCommandLineSegment::LiteralRun(Arc::from([
                argument.into(),
            ]))]),
            ArtifactInputs::new(Vec::new()),
            ArtifactInputs::new(Vec::new()),
            vec![ActionOutput::new(output, ActionOutputKind::File)],
            RetainedActionEnvironment::default().for_action(false, [("K", environment)]),
            CanonicalStringMap::from_pairs([("requirement", requirement)]),
            mnemonic,
            Some(progress),
        ))
    };
    let base = make("tool", "arg", "out", "env", "req", "Mnemonic", "progress");
    for changed in [
        make("other", "arg", "out", "env", "req", "Mnemonic", "progress"),
        make("tool", "other", "out", "env", "req", "Mnemonic", "progress"),
        make("tool", "arg", "other", "env", "req", "Mnemonic", "progress"),
        make("tool", "arg", "out", "other", "req", "Mnemonic", "progress"),
        make("tool", "arg", "out", "env", "other", "Mnemonic", "progress"),
        make("tool", "arg", "out", "env", "req", "Other", "progress"),
        make("tool", "arg", "out", "env", "req", "Mnemonic", "other"),
    ] {
        assert_ne!(base, changed);
    }
}

#[test]
fn spawn_publication_equality_preserves_alias_partitions_across_domains() {
    let shared = artifact_depset("shared.h");
    let aliased = spawn_action(shared.clone(), shared);
    let split = spawn_action(artifact_depset("shared.h"), artifact_depset("shared.h"));

    assert_ne!(aliased, split);
}

#[test]
fn typed_symlink_variants_are_structurally_distinct_and_fail_reapi_projection() {
    let output = ActionOutput::new("pkg/link", ActionOutputKind::File);
    let artifact = ActionSpec::symlink(SymlinkSpec::new(
        output.clone(),
        SymlinkTarget::Artifact {
            input: source_artifact("source"),
            require_executable: true,
            use_exec_root_for_source: false,
        },
        Some("artifact link"),
    ));
    let absolute = ActionSpec::symlink(SymlinkSpec::new(
        output,
        SymlinkTarget::AbsolutePath {
            target: NormalizedAbsoluteBazelPath::new(HostPathFlavor::Unix, "/pkg/source").unwrap(),
        },
        Some("absolute link"),
    ));

    assert_eq!(artifact.kind(), &ActionKind::ArtifactSymlink);
    assert_eq!(absolute.kind(), &ActionKind::AbsoluteSymlink);
    assert_ne!(artifact, absolute);
    assert!(artifact.argv().is_empty());
    assert!(absolute.inputs().is_empty());
    assert!(matches!(
        artifact.symlink_spec().unwrap().target(),
        SymlinkTarget::Artifact {
            require_executable: true,
            ..
        }
    ));
    assert!(ReapiCommandProjection::from_action(&artifact).is_err());
    assert!(ReapiCommandProjection::from_action(&absolute).is_err());
}

#[test]
fn run_shell_pads_empty_dollar_zero_when_arguments_are_present() {
    let mut actions = CtxActions::new();
    let out = actions.declare_file("pkg/out.txt").unwrap();
    actions
        .run_shell(
            out,
            "printf reapi > $1",
            vec!["pkg/out.txt".to_owned()],
            vec![],
        )
        .unwrap();
    let action = &actions.registry().actions()[0];
    // Bazel's ShellCommand inserts an empty $0 before user arguments so the
    // first argument is $1, not $0.
    assert_eq!(
        action.argv(),
        &["sh", "-c", "printf reapi > $1", "", "pkg/out.txt"]
    );
}

#[test]
fn run_shell_omits_pad_when_no_arguments() {
    let mut actions = CtxActions::new();
    let out = actions.declare_file("pkg/out.txt").unwrap();
    actions.run_shell(out, "echo hi", vec![], vec![]).unwrap();
    let action = &actions.registry().actions()[0];
    assert_eq!(action.argv(), &["sh", "-c", "echo hi"]);
}

#[test]
fn registry_rejects_conflicting_outputs() {
    let mut actions = CtxActions::new();
    let first = actions.declare_file("pkg/out.txt").unwrap();
    let second = actions.declare_file("pkg/out.txt").unwrap();
    actions.write(first, "first", false).unwrap();

    let err = actions.write(second, "second", false).unwrap_err();
    assert_eq!(
        err,
        ActionError::ConflictingOutput {
            path: "pkg/out.txt".to_owned()
        }
    );
}

#[test]
fn output_paths_are_package_relative() {
    let actions = CtxActions::new();
    for path in [
        "",
        "/abs/out",
        "pkg/../out",
        "pkg/./out",
        "pkg//out",
        "pkg\\out",
    ] {
        assert!(matches!(
            actions.declare_file(path),
            Err(ActionError::InvalidOutputPath { .. })
        ));
    }
    assert_eq!(
        actions.declare_directory("pkg/tree").unwrap().kind(),
        ActionOutputKind::Directory
    );
}

#[test]
fn run_actions_project_to_reapi_command_shape() {
    let output = CtxActions::new().declare_directory("pkg/tree").unwrap();
    let mut env = BTreeMap::new();
    env.insert("LANG".to_owned(), "C".to_owned());
    let mut exec_properties = BTreeMap::new();
    exec_properties.insert("container-image".to_owned(), "toolchain:v1".to_owned());

    let action = ActionSpec::new(ActionKind::Run, "Spawn", vec![output])
        .with_argv(vec!["tool".to_owned(), "--flag".to_owned()])
        .with_env(env.clone())
        .with_exec_properties(exec_properties.clone());
    let projection = ReapiCommandProjection::from_action(&action).unwrap();

    assert_eq!(
        projection.argv,
        vec!["tool".to_owned(), "--flag".to_owned()]
    );
    assert_eq!(projection.env, env);
    assert_eq!(projection.output_files, Vec::<String>::new());
    assert_eq!(projection.output_directories, vec!["pkg/tree".to_owned()]);
    assert_eq!(projection.platform_properties, exec_properties);
}
