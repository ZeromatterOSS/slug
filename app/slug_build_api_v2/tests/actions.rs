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

use slug_build_api_v2::ActionError;
use slug_build_api_v2::ActionInput;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::CtxActions;
use slug_build_api_v2::ReapiCommandProjection;

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
    let projection = ReapiCommandProjection::from_action(&action);

    assert_eq!(
        projection.argv,
        vec!["tool".to_owned(), "--flag".to_owned()]
    );
    assert_eq!(projection.env, env);
    assert_eq!(projection.output_files, Vec::<String>::new());
    assert_eq!(projection.output_directories, vec!["pkg/tree".to_owned()]);
    assert_eq!(projection.platform_properties, exec_properties);
}
