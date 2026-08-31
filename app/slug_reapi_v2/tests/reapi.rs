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

use prost::Message;
use slug_analysis_v2::ConfigurationKey;
use slug_analysis_v2::ConfiguredActionAspectProvenance;
use slug_analysis_v2::ConfiguredActionExecGroup;
use slug_analysis_v2::ConfiguredActionOwnerContext;
use slug_analysis_v2::ConfiguredActionToolchainContext;
use slug_analysis_v2::ConfiguredNodeResult;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_analysis_v2::ConfiguredToolchainContextRow;
use slug_analysis_v2::ConfiguredToolchainSelection;
use slug_analysis_v2::PlatformSemanticFact;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::ArgsWriteSpec;
use slug_build_api_v2::ArtifactInputs;
use slug_build_api_v2::DefaultInfo;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::ProviderIdentity;
use slug_build_api_v2::ProviderOccurrence;
use slug_build_api_v2::ProviderValue;
use slug_build_api_v2::RetainedArgsRecipe;
use slug_build_api_v2::RetainedCommandLine;
use slug_build_api_v2::RetainedParamFileFormat;
use slug_build_api_v2::SpawnExecutable;
use slug_build_api_v2::SpawnSpec;
use slug_configuration_v2::CanonicalStringMap;
use slug_configuration_v2::NormalizedBazelPath;
use slug_configuration_v2::RetainedActionEnvironment;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;
use slug_identity_v2::CanonicalLabel;
use slug_reapi_v2::ExecutionEvidence;
use slug_reapi_v2::FileWriteReapiPlan;
use slug_reapi_v2::GeneratedOutput;
use slug_reapi_v2::ReapiActionIdentity;
use slug_reapi_v2::ReapiCommand;
use slug_reapi_v2::ReapiDigest;
use slug_reapi_v2::ReapiInputTree;
use slug_reapi_v2::RemoteConfig;
use slug_reapi_v2::RemoteExecutionResult;
use slug_reapi_v2::RemoteMode;

fn typed_spawn_action() -> ActionSpec {
    ActionSpec::spawn(SpawnSpec::new(
        SpawnExecutable::Path(
            NormalizedBazelPath::new(HostPathFlavor::Unix, "tools/runner").unwrap(),
        ),
        RetainedCommandLine::new(Vec::new()),
        ArtifactInputs::new(Vec::new()),
        ArtifactInputs::new(Vec::new()),
        vec![ActionOutput::new("pkg/out.txt", ActionOutputKind::File)],
        RetainedActionEnvironment::default(),
        CanonicalStringMap::default(),
        "Action",
        None::<&str>,
    ))
}

fn typed_args_write_action() -> ActionSpec {
    ActionSpec::args_write(ArgsWriteSpec::new(
        ActionOutput::new("pkg/args.params", ActionOutputKind::File),
        RetainedArgsRecipe::new(Vec::new(), RetainedParamFileFormat::Shell),
        false,
    ))
}

#[test]
fn bare_remote_executor_supplies_cache_endpoint() {
    let config = RemoteConfig::from_args(&[
        "--remote_executor=grpc://127.0.0.1:50051",
        "--remote_instance_name=main",
        "--remote_header=x-build=slug",
        "--remote_default_exec_properties=container-image=toolchain:v1,cpu=x86_64",
        "--remote_timeout=30",
        "--remote_retries=3",
    ])
    .unwrap();

    assert_eq!(config.mode(), RemoteMode::Execute);
    assert_eq!(config.executor.as_deref(), Some("grpc://127.0.0.1:50051"));
    assert_eq!(config.cache.as_deref(), Some("grpc://127.0.0.1:50051"));
    assert_eq!(config.instance_name.as_deref(), Some("main"));
    assert_eq!(config.headers["x-build"], "slug");
    assert_eq!(
        config.default_exec_properties["container-image"],
        "toolchain:v1"
    );
    assert_eq!(config.timeout_seconds, Some(30));
    assert_eq!(config.retry_attempts, Some(3));
}

#[test]
fn remote_cache_only_does_not_enable_execution() {
    let config = RemoteConfig::from_args(&["--remote_cache=grpc://cache:50051"]).unwrap();
    assert_eq!(config.mode(), RemoteMode::CacheOnly);
    assert_eq!(config.executor, None);
    assert_eq!(config.cache.as_deref(), Some("grpc://cache:50051"));
}

#[test]
fn action_ir_projects_to_reapi_command_and_identity() {
    let mut env = BTreeMap::new();
    env.insert("LANG".to_owned(), "C".to_owned());
    let mut props = BTreeMap::new();
    props.insert("container-image".to_owned(), "toolchain:v1".to_owned());
    let action = ActionSpec::new(
        ActionKind::Run,
        "Spawn",
        vec![ActionOutput::new("pkg/out.txt", ActionOutputKind::File)],
    )
    .with_argv(vec![
        "/bin/sh".to_owned(),
        "-c".to_owned(),
        "echo hi".to_owned(),
    ])
    .with_env(env.clone())
    .with_exec_properties(props.clone());

    let command = ReapiCommand::from_action(&action).unwrap();
    assert_eq!(command.argv[0], "/bin/sh");
    assert_eq!(command.env, env);
    assert_eq!(command.output_files, vec!["pkg/out.txt".to_owned()]);
    assert_eq!(command.platform_properties, props);

    let identity =
        ReapiActionIdentity::new(&command, ReapiDigest::of_bytes(b"input-root"), Some(30));
    assert_ne!(
        identity.command_digest.hash(),
        identity.input_root_digest.hash()
    );
    assert_ne!(
        identity.action_digest.hash(),
        identity.command_digest.hash()
    );
    let action = slug_reapi_v2::proto::Action::decode(identity.action_bytes()).unwrap();
    assert_eq!(
        action.command_digest.unwrap().hash,
        identity.command_digest.hash()
    );
    assert_eq!(
        action.input_root_digest.unwrap().hash,
        identity.input_root_digest.hash()
    );
    assert_eq!(action.timeout.unwrap().seconds, 30);
    assert_eq!(
        action.platform.unwrap().properties[0].name,
        "container-image"
    );
}

#[test]
fn declarative_write_action_rejects_the_raw_executor_projection() {
    let action = ActionSpec::new(
        ActionKind::Write {
            content: "hello from reapi\n".to_owned(),
            is_executable: false,
        },
        "FileWrite",
        vec![ActionOutput::new("pkg/out.txt", ActionOutputKind::File)],
    );

    assert_eq!(
        ReapiCommand::for_execution(&action).unwrap_err(),
        "raw FileWrite REAPI lowering is forbidden"
    );
}

#[test]
fn typed_actions_reject_command_input_tree_and_execution_projection() {
    for action in [typed_spawn_action(), typed_args_write_action()] {
        assert_eq!(
            ReapiCommand::from_action(&action).unwrap_err(),
            "typed Spawn/Symlink/ArgsWrite REAPI projection is not admitted"
        );
        assert_eq!(
            ReapiCommand::for_execution(&action).unwrap_err(),
            "typed Spawn/Symlink/ArgsWrite REAPI projection is not admitted"
        );
        assert_eq!(
            ReapiInputTree::from_action(&action)
                .unwrap_err()
                .to_string(),
            "typed Spawn/Symlink/ArgsWrite REAPI input trees are not admitted"
        );
    }
}

#[tokio::test]
async fn typed_action_execution_rejects_before_transport() {
    let config = RemoteConfig {
        executor: Some("grpc://127.0.0.1:1".to_owned()),
        cache: None,
        instance_name: None,
        headers: BTreeMap::new(),
        timeout_seconds: Some(30),
        retry_attempts: None,
        default_exec_properties: BTreeMap::new(),
    };
    for action in [typed_spawn_action(), typed_args_write_action()] {
        assert!(
            slug_reapi_v2::execute_action(&config, &action)
                .await
                .unwrap_err()
                .to_string()
                .contains("typed Spawn/Symlink/ArgsWrite REAPI projection is not admitted")
        );
    }
}

#[test]
fn configured_file_write_reapi_plan_reads_retained_platform_properties() {
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap();
    let target = ConfigurationKey::from_slug(SlugConfiguration::default_target(&host).unwrap());
    let exec = ConfigurationKey::from_slug(SlugConfiguration::default_exec(&host).unwrap());
    let label = |value| CanonicalLabel::parse(value).unwrap();
    let owner = ConfiguredTargetKey::new(label("@@//:write"), target.clone());
    let platform = ConfiguredTargetKey::new(label("@@//:platform"), exec.clone());
    let implementation = ConfiguredTargetKey::new(label("@@//:implementation"), exec);
    let selected = ConfiguredToolchainSelection::new(
        label("@@//:toolchain"),
        implementation.clone(),
        implementation,
        ProviderOccurrence::new(
            ProviderIdentity::builtin("ToolchainInfo"),
            [("marker", AnalysisValue::string("retained"))],
        ),
    );
    let toolchains = Arc::new(
        ConfiguredActionToolchainContext::new(
            platform.clone(),
            vec![ConfiguredToolchainContextRow::new(
                ConfiguredTargetKey::new(label("@@//:type"), target.clone()),
                ConfiguredTargetKey::new(label("@@//:type"), target),
                true,
                Some(selected),
            )],
        )
        .unwrap(),
    );
    let context = Arc::new(
        ConfiguredActionOwnerContext::new(
            owner.clone(),
            ConfiguredActionExecGroup::Default,
            platform,
            PlatformSemanticFact {
                exec_properties: Arc::from([
                    ("a".into(), "first".into()),
                    ("z".into(), "last".into()),
                ]),
            },
            &BTreeMap::new(),
            &BTreeMap::new(),
            Vec::new(),
            Some(toolchains),
            ConfiguredActionAspectProvenance::Absent,
        )
        .unwrap(),
    );
    let providers =
        ProviderCollection::new(vec![ProviderValue::DefaultInfo(DefaultInfo::empty())]).unwrap();
    let result = ConfiguredNodeResult::new_rule(owner, providers, None)
        .with_action_specs(
            vec![ActionSpec::new(
                ActionKind::Write {
                    content: "content".to_owned(),
                    is_executable: false,
                },
                "FileWrite",
                vec![ActionOutput::new("out.txt", ActionOutputKind::File)],
            )],
            vec![context.clone()],
        )
        .unwrap();
    let action = result
        .configured_file_write_actions()
        .unwrap()
        .next()
        .unwrap();
    let view = slug_core_v2::runtime::ResolvedFileWriteSemanticView::from_configured_action(action);
    let mut remote_defaults = BTreeMap::new();
    remote_defaults.insert("remote".to_owned(), "ignored".to_owned());
    let row = &view.action().toolchain().unwrap().rows()[0];
    assert_eq!(row.requested().label().to_string(), "@@//:type");
    assert_eq!(
        row.selected().unwrap().implementation().label().to_string(),
        "@@//:implementation"
    );
    assert_eq!(
        row.selected()
            .unwrap()
            .info()
            .field("marker")
            .unwrap()
            .as_str(),
        Some("retained")
    );
    let plan = FileWriteReapiPlan::from_resolved(&view, &remote_defaults).unwrap();
    assert_eq!(
        plan.command().platform_properties,
        BTreeMap::from([
            ("a".to_owned(), "first".to_owned()),
            ("z".to_owned(), "last".to_owned()),
        ])
    );
    assert!(Arc::ptr_eq(view.action().context(), &context));
}

#[test]
fn verified_remote_outputs_materialize_beneath_the_requested_root() {
    let root = std::env::temp_dir().join(format!("slug-reapi-test-{}", std::process::id()));
    let output = GeneratedOutput::new("pkg/out.txt", ReapiDigest::of_bytes(b"materialized"));
    let execution = RemoteExecutionResult {
        action_digest: ReapiDigest::of_bytes(b"action"),
        result: slug_reapi_v2::ActionResult::new(vec![output]),
        output_blobs: [("pkg/out.txt".to_owned(), b"materialized".to_vec())]
            .into_iter()
            .collect(),
        platform_properties: BTreeMap::new(),
        evidence: ExecutionEvidence::reapi("nativelink"),
    };

    slug_reapi_v2::materialize_outputs(&root, &execution).unwrap();
    assert_eq!(
        std::fs::read(root.join("pkg/out.txt")).unwrap(),
        b"materialized"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn raw_file_write_execution_rejects_before_transport() {
    let config = RemoteConfig {
        executor: Some("grpc://127.0.0.1:1".to_owned()),
        cache: None,
        instance_name: None,
        headers: BTreeMap::new(),
        timeout_seconds: Some(30),
        retry_attempts: None,
        default_exec_properties: BTreeMap::new(),
    };
    let action = ActionSpec::new(
        ActionKind::Write {
            content: "hello from NativeLink\n".to_owned(),
            is_executable: false,
        },
        "FileWrite",
        vec![ActionOutput::new("pkg/out.txt", ActionOutputKind::File)],
    );

    assert!(
        slug_reapi_v2::execute_action(&config, &action)
            .await
            .unwrap_err()
            .to_string()
            .contains("raw FileWrite REAPI lowering is forbidden")
    );
}
#[test]
fn evidence_rows_pin_reapi_boundary_and_zero_direct_local_actions() {
    let digest = ReapiDigest::of_bytes(b"output");
    let evidence = ExecutionEvidence::reapi("nativelink")
        .record_action()
        .record_ac_miss()
        .record_upload(ReapiDigest::of_bytes(b"input"))
        .record_materialized_output(digest.clone());

    assert_eq!(evidence.executor_boundary, "reapi");
    assert_eq!(evidence.backend, "nativelink");
    assert_eq!(evidence.reapi_actions, 1);
    assert_eq!(evidence.direct_local_actions, 0);
    assert_eq!(evidence.ac_misses, 1);
    assert_eq!(evidence.materialized_outputs, vec![digest]);
}
#[test]
fn paramfiles_are_part_of_reapi_input_tree() {
    use slug_build_api_v2::ActionInput;
    use slug_build_api_v2::ParamFile;
    use slug_build_api_v2::ParamFileFormat;
    use slug_reapi_v2::InputTreeEntryKind;
    use slug_reapi_v2::ReapiInputTree;

    let input_digest = ReapiDigest::of_bytes(b"input");
    let tool_digest = ReapiDigest::of_bytes(b"tool");
    let action = ActionSpec::new(
        ActionKind::Run,
        "Spawn",
        vec![ActionOutput::new("pkg/out.txt", ActionOutputKind::File)],
    )
    .with_inputs(vec![ActionInput::new(
        "pkg/input.txt",
        Some(input_digest.to_string()),
    )])
    .with_tools(vec![ActionInput::new(
        "tools/tool.sh",
        Some(tool_digest.to_string()),
    )])
    .with_param_files(vec![ParamFile::new(
        "pkg/out.params",
        vec!["--name".to_owned(), "Slug V2".to_owned()],
        ParamFileFormat::ShellQuoted,
    )]);

    let tree = ReapiInputTree::from_action(&action).unwrap();
    let paths = tree
        .entries()
        .iter()
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        vec!["pkg/input.txt", "pkg/out.params", "tools/tool.sh"]
    );
    assert!(
        tree.entries()
            .iter()
            .any(|entry| entry.kind() == InputTreeEntryKind::ParamFile)
    );
    assert_ne!(tree.root_digest(), &ReapiDigest::of_bytes(b""));
    assert_eq!(
        tree.directory_blobs().last().unwrap().digest(),
        tree.root_digest()
    );
    let root =
        slug_reapi_v2::proto::Directory::decode(tree.directory_blobs().last().unwrap().data())
            .unwrap();
    assert_eq!(
        root.directories
            .iter()
            .map(|directory| directory.name.as_str())
            .collect::<Vec<_>>(),
        vec!["pkg", "tools"]
    );
    assert_eq!(tree.inline_blobs().len(), 1);
}

#[test]
fn missing_input_digest_is_rejected_before_upload_planning() {
    use slug_build_api_v2::ActionInput;
    use slug_reapi_v2::InputTreeError;
    use slug_reapi_v2::ReapiInputTree;

    let action = ActionSpec::new(
        ActionKind::Run,
        "Spawn",
        vec![ActionOutput::new("pkg/out.txt", ActionOutputKind::File)],
    )
    .with_inputs(vec![ActionInput::new("pkg/input.txt", None)]);

    let err = ReapiInputTree::from_action(&action).unwrap_err();
    assert!(matches!(err, InputTreeError::MissingDigest { .. }));
}

#[test]
fn cas_upload_plan_is_digest_first_and_deduped() {
    use slug_build_api_v2::ActionInput;
    use slug_reapi_v2::CasUploadPlan;
    use slug_reapi_v2::ReapiInputTree;

    let digest = ReapiDigest::of_bytes(b"shared");
    let action = ActionSpec::new(
        ActionKind::Run,
        "Spawn",
        vec![ActionOutput::new("pkg/out.txt", ActionOutputKind::File)],
    )
    .with_inputs(vec![
        ActionInput::new("pkg/a.txt", Some(digest.to_string())),
        ActionInput::new("pkg/b.txt", Some(digest.to_string())),
    ]);
    let tree = ReapiInputTree::from_action(&action).unwrap();

    let plan = CasUploadPlan::from_missing(&tree, &[digest.clone(), tree.root_digest().clone()]);
    let mut expected_missing = vec![tree.root_digest().clone(), digest.clone()];
    expected_missing.sort();
    assert_eq!(plan.missing_blobs(), expected_missing);
    assert_eq!(
        plan.uploaded_bytes(),
        tree.root_digest().size_bytes() + digest.size_bytes()
    );
}

#[test]
fn generated_output_reupload_plan_selects_missing_outputs() {
    use slug_reapi_v2::GeneratedOutput;
    use slug_reapi_v2::GeneratedOutputReuploadPlan;

    let present = GeneratedOutput::new("pkg/present.txt", ReapiDigest::of_bytes(b"present"));
    let missing = GeneratedOutput::new("pkg/missing.txt", ReapiDigest::of_bytes(b"missing"));
    let plan = GeneratedOutputReuploadPlan::from_missing(
        &[present.clone(), missing.clone()],
        &[missing.digest().clone()],
    );

    assert_eq!(plan.missing_outputs(), &[missing]);
}
#[test]
fn action_cache_records_action_digest_to_action_result() {
    use slug_reapi_v2::ActionCacheStatus;
    use slug_reapi_v2::ActionCacheTable;
    use slug_reapi_v2::ActionResult;
    use slug_reapi_v2::GeneratedOutput;

    let action_digest = ReapiDigest::of_bytes(b"action");
    let output = GeneratedOutput::new("pkg/out.txt", ReapiDigest::of_bytes(b"out"));
    let result = ActionResult::new(vec![output.clone()])
        .with_stdout_digest(ReapiDigest::of_bytes(b"stdout"));
    let mut table = ActionCacheTable::new();
    table.insert(action_digest.clone(), result.clone());

    assert_eq!(table.status_for(&action_digest), ActionCacheStatus::Hit);
    let entry = table.lookup(&action_digest).unwrap();
    assert_eq!(entry.action_digest(), &action_digest);
    assert_eq!(entry.result(), &result);
    assert_eq!(
        entry.result().validate_local_outputs(&[output]),
        ActionCacheStatus::Hit
    );
}

#[test]
fn local_action_cache_detects_stale_materialized_outputs() {
    use slug_reapi_v2::ActionCacheStatus;
    use slug_reapi_v2::ActionResult;
    use slug_reapi_v2::GeneratedOutput;

    let expected = GeneratedOutput::new("pkg/out.txt", ReapiDigest::of_bytes(b"expected"));
    let corrupt = GeneratedOutput::new("pkg/out.txt", ReapiDigest::of_bytes(b"corrupt"));
    let result = ActionResult::new(vec![expected]);

    assert_eq!(
        result.validate_local_outputs(&[corrupt]),
        ActionCacheStatus::StaleLocal {
            missing_paths: vec!["pkg/out.txt".to_owned()]
        }
    );
}

#[test]
fn remote_action_cache_detects_orphaned_output_blobs() {
    use slug_reapi_v2::ActionCacheStatus;
    use slug_reapi_v2::ActionResult;
    use slug_reapi_v2::GeneratedOutput;

    let present = GeneratedOutput::new("pkg/present.txt", ReapiDigest::of_bytes(b"present"));
    let missing = GeneratedOutput::new("pkg/missing.txt", ReapiDigest::of_bytes(b"missing"));
    let result = ActionResult::new(vec![present.clone(), missing.clone()]);

    assert_eq!(
        result.validate_remote_cas(&[present.digest().clone()]),
        ActionCacheStatus::OrphanedRemote {
            missing_digests: vec![missing.digest().clone()]
        }
    );
}
