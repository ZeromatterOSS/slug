use slug_core_v2::runtime::BzlmodCommandPolicyKey;
use slug_core_v2::runtime::BzlmodEnvironmentPolicyKey;
use slug_core_v2::runtime::LockfileMode;
use slug_core_v2::runtime::TargetPattern;
use slug_core_v2::runtime::TerminalOutput;

use super::super::requested_tests;
use super::super::tests::DEFS;
use super::tests::PublicationWorkspace;
use super::*;

#[test]
#[ignore = "requires supervised fresh verifying NativeLink and Linux publication"]
fn nativelink_target_and_exec_roots_coexist_through_cache_change_restore() {
    use slug_configuration_v2::SlugConfigurationKind;
    let workspace = PublicationWorkspace::new();
    let transport = super::tests::transport();
    let defs = std::fs::read_to_string(workspace.root.join("defs.bzl")).unwrap();
    let build = std::fs::read_to_string(workspace.root.join("BUILD.bazel")).unwrap();
    std::fs::write(workspace.root.join("BUILD.bazel"), format!("{build}\nload(':defs.bzl', 'exec_output', 'configured')\nexec_output(name='exec_output')\nconfigured(name='configured', dep='exec_output')\n")).unwrap();
    let mut original_paths = None;
    let mut original_digests = None;
    for (payload, hit) in [("a", false), ("a", true), ("b", false), ("a", true)] {
        // Authored callers use real FileWrite actions and an ordinary exec edge.
        // Both producers intentionally declare exactly the same relative name.
        let added = format!(
            r#"
def _exec_output(ctx):
    out = ctx.actions.declare_file('configured')
    ctx.actions.write(out, 'wp766 exec {payload}')
    return [DefaultInfo(files=depset([out]))]
exec_output = rule(implementation=_exec_output)
def _configured(ctx):
    out = ctx.actions.declare_file('configured')
    ctx.actions.write(out, 'wp766 target {payload}')
    return [DefaultInfo(files=depset([out], transitive=[ctx.attr.dep[DefaultInfo].files]))]
configured = rule(implementation=_configured, attrs={{'dep':attr.label(cfg='exec')}})
"#
        );
        std::fs::write(workspace.root.join("defs.bzl"), format!("{defs}{added}")).unwrap();
        let accepted = workspace
            .runtime
            .execute_and_publish_requested_actions_with_repository_environment(
                &[TargetPattern::parse("//:configured").unwrap()],
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
        drop(accepted.project(|accepted| {
            let accepted = accepted
                .as_ref()
                .expect("target and exec publication succeeds");
            let plan = accepted.inputs().plan().unwrap();
            let remote = accepted.output().unwrap();
            assert_eq!(plan.actions().len(), 2);
            assert_eq!(remote.results().len(), 2);
            assert_eq!(accepted.published_outputs().len(), 2);
            let mut paths = Vec::new();
            let mut digests = Vec::new();
            let mut roles = Vec::new();
            for group in accepted.published_outputs() {
                let index = group.action_index();
                let action = plan.actions()[index].action();
                let [output] = action.outputs() else {
                    panic!("one declared output per producer")
                };
                assert_eq!(output.root_relative_path(), "configured");
                let role = output.root().configuration().kind();
                let name = match role {
                    SlugConfigurationKind::Target => "target",
                    SlugConfigurationKind::Exec => "exec",
                    SlugConfigurationKind::HostLike => panic!("HostLike root is forbidden"),
                };
                roles.push(role);
                let result = remote.results()[index].remote().unwrap();
                let [file] = result.result.output_files() else {
                    panic!("one REAPI output per producer")
                };
                assert_eq!(file.path(), output.path());
                assert_eq!(
                    file.digest(),
                    &ReapiDigest::of_bytes(format!("wp766 {name} {payload}").as_bytes())
                );
                assert_eq!(result.evidence.ac_hits, u64::from(hit));
                assert_eq!(result.evidence.ac_misses, u64::from(!hit));
                assert_eq!(group.outputs().outputs(), std::slice::from_ref(output));
                assert_eq!(
                    group.outputs().root(),
                    workspace.root.join(output.root().path())
                );
                let physical = group.outputs().root().join(output.root_relative_path());
                assert_eq!(physical, workspace.root.join(output.path()));
                assert_eq!(
                    std::fs::read(&physical).unwrap(),
                    format!("wp766 {name} {payload}").as_bytes()
                );
                assert!(
                    !group.outputs().root().join(output.path()).exists(),
                    "root must not be applied twice"
                );
                paths.push(physical);
                digests.push(result.action_digest.clone());
            }
            assert!(roles.contains(&SlugConfigurationKind::Target));
            assert!(roles.contains(&SlugConfigurationKind::Exec));
            assert_ne!(paths[0], paths[1]);
            assert_ne!(digests[0], digests[1]);
            if let Some(original) = &original_paths {
                assert_eq!(original, &paths);
            } else {
                original_paths = Some(paths);
            }
            if let Some(original) = &original_digests {
                assert_eq!(original == &digests, payload == "a");
            } else {
                original_digests = Some(digests);
            }
            assert!(
                !workspace.root.join("configured").exists(),
                "source root must remain untouched"
            );
            TerminalOutput::new(0, String::new(), String::new())
        }));
    }
}

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
            ActionChainStepResult::Remote(RemoteExecutionResult {
                action_digest: ReapiDigest::of_bytes(b"synthetic action"),
                platform_properties: Default::default(),
                result: ActionResult::new(files).with_output_directories(directories),
                output_blobs: Default::default(),
                evidence: ExecutionEvidence::reapi("synthetic verified metadata"),
            })
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
    let selected_file = [ActionOutput::new(
        crate::test_roots::owner_root(&workspace.owner),
        "file",
        ActionOutputKind::File,
    )];
    let selected_tree = [ActionOutput::new(
        crate::test_roots::owner_root(&workspace.owner),
        "result_tree",
        ActionOutputKind::Directory,
    )];
    let selection = [(1, selected_file.as_slice()), (2, selected_tree.as_slice())];
    let projected = project_outputs(&session, selection).unwrap();
    assert_eq!(projected.iter().map(Vec::len).collect::<Vec<_>>(), [1, 1]);
    let SelectedOutput::File(output) = projected[0][0] else {
        panic!("first producer file");
    };
    assert_eq!(
        output.path(),
        crate::test_roots::owner_path(&workspace.owner, "file")
    );
    let SelectedOutput::Directory(output) = projected[1][0] else {
        panic!("second producer tree");
    };
    assert_eq!(
        output.path(),
        crate::test_roots::owner_path(&workspace.owner, "result_tree")
    );
    assert_eq!(
        session.results[1]
            .remote()
            .unwrap()
            .result
            .output_directories()
            .len(),
        2
    );
    assert_eq!(
        session.results[2].remote().unwrap().result.output_files()[0].path(),
        crate::test_roots::owner_path(&workspace.owner, "done")
    );

    // Every malformed result still contains the selected tree. Only unselected
    // cooutputs are missing, extra, duplicated or assigned the wrong kind.
    for bad in [
        ActionResult::new(vec![]).with_output_directories(vec![directory(
            &crate::test_roots::owner_path(&workspace.owner, "result_tree"),
        )]),
        ActionResult::new(vec![
            file(&crate::test_roots::owner_path(&workspace.owner, "done")),
            file(&crate::test_roots::owner_path(&workspace.owner, "extra")),
        ])
        .with_output_directories(vec![directory(&crate::test_roots::owner_path(
            &workspace.owner,
            "result_tree",
        ))]),
        ActionResult::new(vec![
            file(&crate::test_roots::owner_path(&workspace.owner, "done")),
            file(&crate::test_roots::owner_path(&workspace.owner, "done")),
        ])
        .with_output_directories(vec![directory(&crate::test_roots::owner_path(
            &workspace.owner,
            "result_tree",
        ))]),
        ActionResult::new(vec![]).with_output_directories(vec![
            directory(&crate::test_roots::owner_path(&workspace.owner, "done")),
            directory(&crate::test_roots::owner_path(
                &workspace.owner,
                "result_tree",
            )),
        ]),
    ] {
        let ActionChainStepResult::Remote(result) = &mut session.results[2] else {
            panic!("expected remote producer")
        };
        result.result = bad;
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
    let selected = [ActionOutput::new(
        crate::test_roots::owner_root(&workspace.owner),
        "file",
        ActionOutputKind::File,
    )];
    let wrong_kind = [ActionOutput::new(
        crate::test_roots::owner_root(&workspace.owner),
        "file",
        ActionOutputKind::Directory,
    )];
    let wrong_path = [ActionOutput::new(
        crate::test_roots::owner_root(&workspace.owner),
        "absent",
        ActionOutputKind::File,
    )];
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
        let accepted = accepted.as_ref().expect("requested build succeeds");
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
                .all(|result| result.remote().unwrap().output_blobs.is_empty())
        );
        assert_eq!(
            remote.results()[1]
                .remote()
                .unwrap()
                .result
                .output_directories()
                .len(),
            2
        );
        assert_eq!(
            remote.results()[2].remote().unwrap().result.output_files()[0].path(),
            crate::test_roots::owner_path(&workspace.owner, "done")
        );
        assert!(!remote.results()[1].remote().unwrap().result.output_files()[0].is_executable());
        let tree = &remote.results()[2]
            .remote()
            .unwrap()
            .result
            .output_directories()[0];
        assert_eq!(tree.directories(), ["", "empty", "nested"]);
        assert!(tree.files().iter().all(|file| !file.is_executable()));
        let groups = accepted.published_outputs();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].action_index(), 1);
        assert_eq!(
            groups[0].outputs().outputs(),
            [ActionOutput::new(
                crate::test_roots::owner_root(&workspace.owner),
                "file",
                ActionOutputKind::File
            )]
        );
        assert_eq!(groups[1].action_index(), 2);
        assert_eq!(
            groups[1].outputs().outputs(),
            [ActionOutput::new(
                crate::test_roots::owner_root(&workspace.owner),
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
