use slug_core_v2::runtime::BzlmodCommandPolicyKey;
use slug_core_v2::runtime::BzlmodEnvironmentPolicyKey;
use slug_core_v2::runtime::LockfileMode;
use slug_core_v2::runtime::TargetPattern;
use slug_core_v2::runtime::TerminalOutput;

use super::super::requested_tests;
use super::super::tests::DEFS;
use super::tests::PublicationWorkspace;
use super::*;

fn workspace() -> PublicationWorkspace {
    let workspace = PublicationWorkspace::new();
    std::fs::write(
        workspace.root.join("defs.bzl"),
        DEFS.replace("depset([done])", "depset([file, result_tree])"),
    )
    .unwrap();
    workspace
}

fn directory(path: &str) -> GeneratedDirectory {
    GeneratedDirectory {
        path: path.into(),
        tree_digest: ReapiDigest::of_bytes(b"synthetic tree"),
        root_digest: ReapiDigest::of_bytes(b"synthetic root"),
        directories: vec![String::new()],
        files: Vec::new(),
    }
}

fn file(path: &str) -> GeneratedOutput {
    GeneratedOutput::new(path, ReapiDigest::of_bytes(path.as_bytes()), false)
}

fn complete_session(workspace: &PublicationWorkspace) -> ActionChainReapiSession {
    let inputs = requested_tests::prepare(workspace, &["//:one"]);
    let mut session = requested_tests::disconnected_session(inputs);
    let plan = session.inputs.plan().unwrap();
    session.results = plan
        .actions()
        .iter()
        .map(|step| {
            let mut files = Vec::new();
            let mut directories = Vec::new();
            for output in step.action().outputs() {
                match output.kind() {
                    ActionOutputKind::File => files.push(file(output.path())),
                    ActionOutputKind::Directory => directories.push(directory(output.path())),
                    _ => unreachable!(),
                }
            }
            RemoteExecutionResult {
                action_digest: ReapiDigest::of_bytes(b"synthetic action"),
                platform_properties: Default::default(),
                result: ActionResult::new(files).with_output_directories(directories),
                output_blobs: Default::default(),
                evidence: ExecutionEvidence::reapi("synthetic verified metadata"),
            }
        })
        .collect();
    session
}

#[test]
fn requested_publication_projects_subsets_and_checks_unselected_cooutputs() {
    let workspace = workspace();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _entered = runtime.enter();
    let mut session = complete_session(&workspace);
    let selected_file = [ActionOutput::new("file", ActionOutputKind::File)];
    let selected_tree = [ActionOutput::new(
        "result_tree",
        ActionOutputKind::Directory,
    )];
    let selection = [(1, selected_file.as_slice()), (2, selected_tree.as_slice())];
    let projected = project_outputs(&session, selection).unwrap();
    assert_eq!(projected.iter().map(Vec::len).collect::<Vec<_>>(), [1, 1]);
    let SelectedOutput::File(output) = projected[0][0] else {
        panic!("first producer file");
    };
    assert_eq!(output.path(), "file");
    let SelectedOutput::Directory(output) = projected[1][0] else {
        panic!("second producer tree");
    };
    assert_eq!(output.path(), "result_tree");
    assert_eq!(session.results[1].result.output_directories().len(), 2);
    assert_eq!(session.results[2].result.output_files()[0].path(), "done");

    // Every malformed result still contains the selected tree. Only unselected
    // cooutputs are missing, extra, duplicated or assigned the wrong kind.
    for bad in [
        ActionResult::new(vec![]).with_output_directories(vec![directory("result_tree")]),
        ActionResult::new(vec![file("done"), file("extra")])
            .with_output_directories(vec![directory("result_tree")]),
        ActionResult::new(vec![file("done"), file("done")])
            .with_output_directories(vec![directory("result_tree")]),
        ActionResult::new(vec![])
            .with_output_directories(vec![directory("done"), directory("result_tree")]),
    ] {
        session.results[2].result = bad;
        assert!(
            project_outputs(&session, selection).is_err(),
            "unselected cooutput schema must be checked before transfer"
        );
    }
}

#[test]
fn requested_publication_rejects_incomplete_table_invalid_producer_and_subset() {
    let workspace = workspace();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _entered = runtime.enter();
    let mut session = complete_session(&workspace);
    let selected = [ActionOutput::new("file", ActionOutputKind::File)];
    let wrong_kind = [ActionOutput::new("file", ActionOutputKind::Directory)];
    let wrong_path = [ActionOutput::new("absent", ActionOutputKind::File)];
    let duplicate = [selected[0].clone(), selected[0].clone()];
    for bad in [
        wrong_kind.as_slice(),
        wrong_path.as_slice(),
        duplicate.as_slice(),
    ] {
        let error = project_outputs(&session, [(1, bad)]).err().unwrap();
        assert!(error.to_string().contains("subset differs"), "{error}");
    }
    let error = project_outputs(
        &session,
        [(1, selected.as_slice()), (3, selected.as_slice())],
    )
    .err()
    .unwrap();
    assert!(error.to_string().contains("producer index"), "{error}");
    let error = project_outputs(
        &session,
        [(1, selected.as_slice()), (1, selected.as_slice())],
    )
    .err()
    .unwrap();
    assert!(
        error
            .to_string()
            .contains("duplicate output staging producer"),
        "{error}"
    );
    session.results.pop();
    let error = project_outputs(&session, [(1, selected.as_slice())])
        .err()
        .unwrap();
    assert!(
        error.to_string().contains("completed action plan"),
        "{error}"
    );
}

#[test]
#[ignore = "requires supervised fresh verifying NativeLink and Linux publication"]
fn nativelink_requested_subsets_publish_file_and_tree_from_distinct_producers() {
    let workspace = workspace();
    let transport = super::tests::transport();
    let accepted = workspace
        .runtime
        .execute_and_publish_requested_actions_with_repository_environment(
            &[
                TargetPattern::parse("//:one").unwrap(),
                TargetPattern::parse("//:input").unwrap(),
                TargetPattern::parse("//:one").unwrap(),
            ],
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &[format!(
                "file://{}/empty-registry",
                workspace.root.display()
            )],
            Default::default(),
            Default::default(),
            &transport,
        )
        .unwrap();
    let mut root = None;
    drop(accepted.project(|accepted| {
        let plan = accepted.inputs().plan().unwrap();
        let requested = plan.requested().unwrap();
        assert_eq!(requested.selection().targets().len(), 3);
        assert_eq!(requested.artifact_producers(), [Some(1), Some(2), None]);
        let remote = accepted.output().unwrap();
        assert!(remote.selected().is_none());
        assert_eq!(remote.results().len(), 3);
        assert!(
            remote
                .results()
                .iter()
                .all(|result| result.output_blobs.is_empty())
        );
        assert_eq!(remote.results()[1].result.output_directories().len(), 2);
        assert_eq!(remote.results()[2].result.output_files()[0].path(), "done");
        assert!(!remote.results()[1].result.output_files()[0].is_executable());
        let tree = &remote.results()[2].result.output_directories()[0];
        assert_eq!(tree.directories(), ["", "empty", "nested"]);
        assert!(tree.files().iter().all(|file| !file.is_executable()));
        let groups = accepted.published_outputs();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].action_index(), 1);
        assert_eq!(
            groups[0].outputs().outputs(),
            [ActionOutput::new("file", ActionOutputKind::File)]
        );
        assert_eq!(groups[1].action_index(), 2);
        assert_eq!(
            groups[1].outputs().outputs(),
            [ActionOutput::new(
                "result_tree",
                ActionOutputKind::Directory
            )]
        );
        assert_eq!(groups[0].outputs().root(), groups[1].outputs().root());
        root = Some(groups[0].outputs().root().to_owned());
        TerminalOutput::new(0, String::new(), String::new())
    }));
    let root = root.unwrap();
    assert_eq!(std::fs::read(root.join("file")).unwrap(), b"aaa");
    assert_eq!(
        std::fs::read(root.join("result_tree/nested/value")).unwrap(),
        b"aaa"
    );
    assert!(root.join("result_tree/empty").is_dir());
    for unselected in ["seed", "tree", "emptytree", "done", "input"] {
        assert!(
            !root.join(unselected).exists(),
            "unexpected published {unselected}"
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in [
            "file",
            "result_tree",
            "result_tree/nested",
            "result_tree/nested/value",
            "result_tree/empty",
        ] {
            assert_eq!(
                std::fs::metadata(root.join(path))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o555,
                "{path}"
            );
        }
    }
}
