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

use allocative::Allocative;
use dupe::Dupe;
use slug_build_api_v2::ActionError;
use slug_build_api_v2::ActionInput;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisDepset;
use slug_build_api_v2::AnalysisDepsetGraphInput;
use slug_build_api_v2::AnalysisDepsetGraphNode;
use slug_build_api_v2::AnalysisDepsetGraphRow;
use slug_build_api_v2::AnalysisDepsetOccurrence;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::ArgsWriteSpec;
use slug_build_api_v2::ArtifactInputSource;
use slug_build_api_v2::ArtifactInputs;
use slug_build_api_v2::CtxActions;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::ReapiCommandProjection;
use slug_build_api_v2::RetainedArgCall;
use slug_build_api_v2::RetainedArgsDepset;
use slug_build_api_v2::RetainedArgsRecipe;
use slug_build_api_v2::RetainedArtifactInputs;
use slug_build_api_v2::RetainedCommandLine;
use slug_build_api_v2::RetainedCommandLineSegment;
use slug_build_api_v2::RetainedParamFileFormat;
use slug_build_api_v2::RetainedScalarArg;
use slug_build_api_v2::RetainedScalarValue;
use slug_build_api_v2::RetainedSpawnArgsSnapshot;
use slug_build_api_v2::RetainedSpawnInvocation;
use slug_build_api_v2::RetainedSpawnParamFilePolicy;
use slug_build_api_v2::RetainedVectorArg;
use slug_build_api_v2::RetainedVectorOptions;
use slug_build_api_v2::RetainedVectorSource;
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

fn default_vector_options() -> RetainedVectorOptions {
    RetainedVectorOptions {
        arg_name: None,
        format_each: None,
        before_each: None,
        join_with: None,
        format_joined: None,
        omit_if_empty: true,
        uniquify: false,
        expand_directories: true,
        terminate_with: None,
    }
}

fn spawn_action(inputs: AnalysisDepset, tools: AnalysisDepset) -> ActionSpec {
    let recipe = RetainedArgsRecipe::new(
        vec![
            RetainedArgCall::Scalar(RetainedScalarArg::new(
                Some("--count"),
                RetainedScalarValue::Integer("7".into()),
                None::<&str>,
            )),
            RetainedArgCall::Scalar(RetainedScalarArg::new(
                None::<&str>,
                RetainedScalarValue::Artifact(source_artifact("arg.txt")),
                Some("value=%s%%"),
            )),
        ],
        RetainedParamFileFormat::Shell,
    );
    let command_line = RetainedCommandLine::new(vec![
        RetainedCommandLineSegment::LiteralRun(Arc::from(["--literal".into()])),
        RetainedCommandLineSegment::ArgsSnapshot(RetainedSpawnArgsSnapshot::new(recipe, None)),
    ]);
    ActionSpec::spawn(SpawnSpec::new(
        RetainedSpawnInvocation::Executable(SpawnExecutable::Path(
            NormalizedBazelPath::new(HostPathFlavor::Unix, "tools/runner").unwrap(),
        )),
        command_line,
        ArtifactInputs::new(vec![ArtifactInputSource::Depset(
            RetainedArtifactInputs::new(inputs).unwrap(),
        )]),
        ArtifactInputs::new(vec![ArtifactInputSource::Depset(
            RetainedArtifactInputs::new(tools).unwrap(),
        )]),
        vec![ActionOutput::new("pkg/out", ActionOutputKind::File)],
        None,
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
            RetainedSpawnInvocation::Executable(SpawnExecutable::Path(
                NormalizedBazelPath::new(HostPathFlavor::Unix, executable).unwrap(),
            )),
            RetainedCommandLine::new(vec![RetainedCommandLineSegment::LiteralRun(Arc::from([
                argument.into(),
            ]))]),
            ArtifactInputs::new(Vec::new()),
            ArtifactInputs::new(Vec::new()),
            vec![ActionOutput::new(output, ActionOutputKind::File)],
            None,
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

    let envelope = |invocation, unused_inputs_list| {
        ActionSpec::spawn(SpawnSpec::new(
            invocation,
            RetainedCommandLine::new(Vec::new()),
            ArtifactInputs::new(Vec::new()),
            ArtifactInputs::new(Vec::new()),
            vec![ActionOutput::new("out", ActionOutputKind::File)],
            unused_inputs_list,
            RetainedActionEnvironment::default(),
            CanonicalStringMap::default(),
            "Action",
            None::<&str>,
        ))
    };
    let executable = || {
        RetainedSpawnInvocation::Executable(SpawnExecutable::Path(
            NormalizedBazelPath::new(HostPathFlavor::Unix, "tool").unwrap(),
        ))
    };
    assert_ne!(
        envelope(executable(), None),
        envelope(
            RetainedSpawnInvocation::Shell {
                command: "tool".into(),
                pad_dollar_zero: false,
            },
            None,
        )
    );
    assert_ne!(
        envelope(executable(), None),
        envelope(executable(), Some(source_artifact("unused.txt")))
    );
    assert_ne!(
        envelope(
            RetainedSpawnInvocation::Shell {
                command: "command".into(),
                pad_dollar_zero: false,
            },
            None,
        ),
        envelope(
            RetainedSpawnInvocation::Shell {
                command: "command".into(),
                pad_dollar_zero: true,
            },
            None,
        )
    );
}

#[test]
fn spawn_publication_equality_preserves_alias_partitions_across_domains() {
    let shared = artifact_depset("shared.h");
    let aliased = spawn_action(shared.clone(), shared);
    let split = spawn_action(artifact_depset("shared.h"), artifact_depset("shared.h"));

    assert_ne!(aliased, split);
}

#[test]
fn vector_args_render_bazel_transform_order_and_empty_groups() {
    let add_all = RetainedVectorArg::new(
        RetainedVectorSource::Sequence(
            vec!["a", "b", "a"]
                .into_iter()
                .map(|value| RetainedScalarValue::String(value.into()))
                .collect::<Vec<_>>()
                .into(),
        ),
        RetainedVectorOptions {
            arg_name: Some("--all".into()),
            format_each: Some("item=%s%%".into()),
            before_each: Some("-B".into()),
            join_with: None,
            format_joined: None,
            omit_if_empty: true,
            uniquify: true,
            expand_directories: false,
            terminate_with: Some("--end".into()),
        },
    );
    let joined_empty = RetainedVectorArg::new(
        RetainedVectorSource::Sequence(Arc::from([])),
        RetainedVectorOptions {
            arg_name: Some("--joined".into()),
            format_each: None,
            before_each: None,
            join_with: Some(":".into()),
            format_joined: Some("[%s]".into()),
            omit_if_empty: false,
            uniquify: false,
            expand_directories: true,
            terminate_with: None,
        },
    );
    let mut empty_options = default_vector_options();
    empty_options.uniquify = true;
    let empty_strings = RetainedVectorArg::new(
        RetainedVectorSource::Sequence(vec![RetainedScalarValue::String("".into()); 2].into()),
        empty_options,
    );
    let recipe = RetainedArgsRecipe::new(
        vec![
            RetainedArgCall::AddAll(add_all),
            RetainedArgCall::AddJoined(joined_empty),
            RetainedArgCall::AddAll(empty_strings),
        ],
        RetainedParamFileFormat::Multiline,
    );

    assert_eq!(
        recipe.render(),
        [
            "--all", "-B", "item=a%", "-B", "item=b%", "--end", "--joined", "[]", "",
        ]
    );
    assert_eq!(
        recipe.render_write_content(),
        "--all\n-B\nitem=a%\n-B\nitem=b%\n--end\n--joined\n[]\n\n"
    );

    let integers = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::integer_from_magnitude(
            false,
            [1, 0, 0, 0, 0, 0, 0, 0, 0],
        )],
        Vec::new(),
    )
    .unwrap();
    let integer_recipe = RetainedArgsRecipe::new(
        vec![RetainedArgCall::AddAll(RetainedVectorArg::new(
            RetainedVectorSource::Depset(RetainedArgsDepset::new(integers).unwrap()),
            default_vector_options(),
        ))],
        RetainedParamFileFormat::Multiline,
    );
    assert_eq!(integer_recipe.render(), ["18446744073709551616"]);
}

#[test]
fn args_write_formats_and_ignores_spawn_only_param_policy() {
    let calls = vec![
        RetainedArgCall::Scalar(RetainedScalarArg::new(
            Some("--flag"),
            RetainedScalarValue::String("two words".into()),
            None::<&str>,
        )),
        RetainedArgCall::Scalar(RetainedScalarArg::new(
            None::<&str>,
            RetainedScalarValue::String("quote'd".into()),
            None::<&str>,
        )),
    ];
    let shell = RetainedArgsRecipe::new(calls.clone(), RetainedParamFileFormat::Shell);
    let multiline = RetainedArgsRecipe::new(calls.clone(), RetainedParamFileFormat::Multiline);
    let flag_per_line = RetainedArgsRecipe::new(calls, RetainedParamFileFormat::FlagPerLine);
    assert_eq!(
        shell.render_write_content(),
        "--flag\n'two words'\n'quote'\\''d'\n"
    );
    assert_eq!(
        multiline.render_write_content(),
        "--flag\ntwo words\nquote'd\n"
    );
    assert_eq!(
        flag_per_line.render_write_content(),
        "--flag=two words\nquote'd\n"
    );

    let output = ActionOutput::new("pkg/args.params", ActionOutputKind::File);
    let left = ActionSpec::args_write(ArgsWriteSpec::new(output.clone(), shell.clone(), false));
    let right = ActionSpec::args_write(ArgsWriteSpec::new(output, shell.clone(), false));
    assert_eq!(left, right);
    assert_eq!(left.kind(), &ActionKind::ArgsWrite);
    assert_eq!(left.mnemonic(), "FileWrite");
    let write = left.args_write_spec().unwrap();
    assert_eq!(write.output().path(), "pkg/args.params");
    assert!(!write.is_executable());
    assert_eq!(write.execution_requirements().iter().len(), 0);
    assert_eq!(write.render_content(), shell.render_write_content());
    assert!(ReapiCommandProjection::from_action(&left).is_err());

    let spawn_left = RetainedSpawnArgsSnapshot::new(
        shell.clone(),
        Some(RetainedSpawnParamFilePolicy::new("@%s", false)),
    );
    let spawn_right = RetainedSpawnArgsSnapshot::new(
        shell,
        Some(RetainedSpawnParamFilePolicy::new("--file=%s", true)),
    );
    assert_ne!(
        RetainedCommandLine::new(vec![RetainedCommandLineSegment::ArgsSnapshot(spawn_left)]),
        RetainedCommandLine::new(vec![RetainedCommandLineSegment::ArgsSnapshot(spawn_right)])
    );

    let mut actions = CtxActions::new();
    let existing = actions.declare_file("pkg/conflict.params").unwrap();
    actions.write(existing, "string content", false).unwrap();
    let conflicting = actions.declare_file("pkg/conflict.params").unwrap();
    let error = actions
        .register_args_write(ArgsWriteSpec::new(conflicting, multiline, true))
        .unwrap_err();
    assert_eq!(
        error,
        ActionError::ConflictingOutput {
            path: "pkg/conflict.params".to_owned()
        }
    );
    assert_eq!(actions.registry().actions().len(), 1);
    assert!(matches!(
        actions.registry().actions()[0].kind(),
        ActionKind::Write { .. }
    ));
}

#[test]
fn vector_depsets_share_publication_alias_state_with_spawn_inputs() {
    let make = |source: RetainedVectorSource,
                input: AnalysisDepset,
                policy: Option<RetainedSpawnParamFilePolicy>| {
        let vector = RetainedVectorArg::new(source, default_vector_options());
        ActionSpec::spawn(SpawnSpec::new(
            RetainedSpawnInvocation::Executable(SpawnExecutable::Path(
                NormalizedBazelPath::new(HostPathFlavor::Unix, "tool").unwrap(),
            )),
            RetainedCommandLine::new(vec![RetainedCommandLineSegment::ArgsSnapshot(
                RetainedSpawnArgsSnapshot::new(
                    RetainedArgsRecipe::new(
                        vec![RetainedArgCall::AddAll(vector)],
                        RetainedParamFileFormat::Shell,
                    ),
                    policy,
                ),
            )]),
            ArtifactInputs::new(vec![ArtifactInputSource::Depset(
                RetainedArtifactInputs::new(input).unwrap(),
            )]),
            ArtifactInputs::new(Vec::new()),
            vec![ActionOutput::new("pkg/out", ActionOutputKind::File)],
            None,
            RetainedActionEnvironment::default(),
            CanonicalStringMap::default(),
            "Action",
            None::<&str>,
        ))
    };
    let shared = artifact_depset("shared");
    let aliased = make(
        RetainedVectorSource::Depset(RetainedArgsDepset::new(shared.clone()).unwrap()),
        shared,
        None,
    );
    let split = make(
        RetainedVectorSource::Depset(RetainedArgsDepset::new(artifact_depset("shared")).unwrap()),
        artifact_depset("shared"),
        None,
    );
    assert_ne!(aliased, split);
    assert_eq!(
        make(
            RetainedVectorSource::Depset(
                RetainedArgsDepset::new(artifact_depset("shared")).unwrap(),
            ),
            artifact_depset("other"),
            None,
        ),
        make(
            RetainedVectorSource::Depset(
                RetainedArgsDepset::new(artifact_depset("shared")).unwrap(),
            ),
            artifact_depset("other"),
            None,
        )
    );

    let flat = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![
            AnalysisValue::artifact(source_artifact("shared")),
            AnalysisValue::artifact(source_artifact("other")),
        ],
        Vec::new(),
    )
    .unwrap();
    let branched = AnalysisDepset::from_local_graph(vec![
        AnalysisDepsetGraphNode::new(
            AnalysisDepsetOccurrence::new(),
            DepsetOrder::Default,
            1,
            AnalysisDepsetGraphRow::Successors(vec![AnalysisDepsetGraphInput::Direct(
                AnalysisValue::artifact(source_artifact("shared")),
            )]),
        ),
        AnalysisDepsetGraphNode::new(
            AnalysisDepsetOccurrence::new(),
            DepsetOrder::Default,
            1,
            AnalysisDepsetGraphRow::Successors(vec![AnalysisDepsetGraphInput::Direct(
                AnalysisValue::artifact(source_artifact("other")),
            )]),
        ),
        AnalysisDepsetGraphNode::new(
            AnalysisDepsetOccurrence::new(),
            DepsetOrder::Default,
            2,
            AnalysisDepsetGraphRow::Successors(vec![
                AnalysisDepsetGraphInput::Local(0),
                AnalysisDepsetGraphInput::Local(1),
            ]),
        ),
    ])
    .unwrap()
    .pop()
    .unwrap();
    assert_eq!(flat.to_list(), branched.to_list());
    assert_ne!(
        make(
            RetainedVectorSource::Depset(RetainedArgsDepset::new(flat).unwrap()),
            artifact_depset("input"),
            None,
        ),
        make(
            RetainedVectorSource::Depset(RetainedArgsDepset::new(branched).unwrap()),
            artifact_depset("input"),
            None,
        )
    );
    let topological = AnalysisDepset::new(
        DepsetOrder::Topological,
        vec![AnalysisValue::artifact(source_artifact("shared"))],
        Vec::new(),
    )
    .unwrap();
    let base = || {
        make(
            RetainedVectorSource::Depset(
                RetainedArgsDepset::new(artifact_depset("shared")).unwrap(),
            ),
            artifact_depset("input"),
            None,
        )
    };
    for changed in [
        make(
            RetainedVectorSource::Depset(RetainedArgsDepset::new(topological).unwrap()),
            artifact_depset("input"),
            None,
        ),
        make(
            RetainedVectorSource::Depset(
                RetainedArgsDepset::new(artifact_depset("value-change")).unwrap(),
            ),
            artifact_depset("input"),
            None,
        ),
        make(
            RetainedVectorSource::Sequence(
                vec![RetainedScalarValue::Artifact(source_artifact("shared"))].into(),
            ),
            artifact_depset("input"),
            None,
        ),
        make(
            RetainedVectorSource::Depset(
                RetainedArgsDepset::new(artifact_depset("shared")).unwrap(),
            ),
            artifact_depset("input"),
            Some(RetainedSpawnParamFilePolicy::new("@%s", false)),
        ),
    ] {
        assert_ne!(base(), changed);
    }
}

#[test]
fn retained_args_graph_values_are_allocative_and_cheap_to_clone() {
    fn assert_allocative<T: Allocative>() {}
    fn assert_dupe<T: Dupe>() {}
    assert_allocative::<RetainedArgsDepset>();
    assert_allocative::<RetainedArgsRecipe>();
    assert_allocative::<RetainedCommandLine>();
    assert_dupe::<RetainedArgsDepset>();
    assert_dupe::<RetainedArgsRecipe>();
    assert_dupe::<RetainedCommandLine>();
}

#[test]
fn retained_vector_depsets_reject_deferred_types_and_directories() {
    let labels = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::label(
            CanonicalLabel::parse("@@//pkg:value").unwrap(),
        )],
        Vec::new(),
    )
    .unwrap();
    assert!(RetainedArgsDepset::new(labels).is_err());
    let owner = slug_build_api_v2::AnalysisConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//pkg:owner").unwrap(),
        b"cfg".as_slice(),
    );
    let directory = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::artifact(AnalysisArtifact::Derived {
            owner,
            output: ActionOutput::new("pkg/tree", ActionOutputKind::Directory),
        })],
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        RetainedArgsDepset::new(directory).unwrap_err().to_string(),
        "Args vector directory expansion is not supported"
    );
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
