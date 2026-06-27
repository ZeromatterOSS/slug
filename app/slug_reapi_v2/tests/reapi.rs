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

use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::ActionSpec;
use slug_reapi_v2::ExecutionEvidence;
use slug_reapi_v2::ReapiActionIdentity;
use slug_reapi_v2::ReapiCommand;
use slug_reapi_v2::ReapiDigest;
use slug_reapi_v2::RemoteConfig;
use slug_reapi_v2::RemoteMode;

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

    let command = ReapiCommand::from_action(&action);
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
    assert!(identity.stable_serialize().contains("timeout=Some(30)"));
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
    assert_eq!(
        plan.missing_blobs(),
        &[tree.root_digest().clone(), digest.clone()]
    );
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
