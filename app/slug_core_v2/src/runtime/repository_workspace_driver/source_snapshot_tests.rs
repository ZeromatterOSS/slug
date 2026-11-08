//! Real original repository rules consume genuine source families through the
//! same command/runner. Family separation preserves the existing backing bank.
use std::os::unix::fs::PermissionsExt;

use base64::Engine as _;
use sha2::Digest;
use slug_bzlmod_v2::RepositorySourceSnapshotKind;

use super::*;

#[derive(Clone, Copy)]
enum Family {
    Builtin,
    Immutable,
    Local,
    Workspace,
    Mixed,
}

fn original(family: Family, host_input: &std::path::Path) -> String {
    let label = match family {
        Family::Builtin => "@@bazel_tools//src/conditions:BUILD",
        Family::Immutable => "@archive_tool//:tool.sh",
        Family::Local | Family::Mixed => "@local_tool//:tool.sh",
        Family::Workspace => "//:workspace_tool.sh",
    };
    let host_input = serde_json::to_string(host_input.to_str().unwrap()).unwrap();
    let mut body = format!(
        r#"
def _repository(ctx):
    ctx.file("host-copy", ctx.read({host_input}), executable=False)
    if ctx.execute(["/bin/true"]).return_code != 0: fail("warmup")
    source = Label("{label}")
    path = ctx.path(source)
    if not ctx.read(source): fail("empty captured source")
    ctx.template("source-copy", source, executable=False)
    ctx.file("source-display", str(path), executable=False)
"#
    );
    match family {
        Family::Builtin=>body.push_str(r#"    result = ctx.execute(["/bin/sh", "-c", "test -s \"$1\" && test \"$1\" = \"$2\" && printf 'builtin\\n' && printf 'builtin\\n' >out", "proof", source, str(path)], quiet=False)
"#),
        Family::Immutable=>body.push_str(r#"    if ctx.read(Label("@archive_tool//:MODULE.bazel")) != "module(name='archive_tool',version='1.0')\n": fail("verified registry MODULE")
    if ctx.read(Label("@archive_tool//:overlay.txt")) != "overlay-verified\n": fail("verified overlay")
    result = ctx.execute([source, str(path), "literal"], quiet=False)
"#),
        Family::Local|Family::Workspace=>body.push_str("    result = ctx.execute([source, str(path), 'literal'], quiet=False)\n"),
        Family::Mixed=>body.push_str(r#"    builtin = Label("@@bazel_tools//src/conditions:BUILD")
    main = Label("//:workspace_tool.sh")
    builtin_path = ctx.path(builtin)
    main_path = ctx.path(main)
    if not ctx.read(builtin) or not ctx.read(main): fail("selected source reads")
    result = ctx.execute([source, str(path), "literal", builtin, main], quiet=False)
    if ctx.read("workspace-stdout") != "workspace:A\n": fail("workspace child output")
"#),
    }
    body.push_str(
        r#"    if result.return_code != 0: fail("source process failed")
    if not ctx.path("out").exists: fail("actual process output missing")
    ctx.file("BUILD.bazel", "exports_files(['out'])\n", executable=False)
repository = repository_rule(implementation=_repository)
def _extension(ctx):
    repository(name="proof")
ext = module_extension(implementation=_extension)
"#,
    );
    // Repository-rule definition loads currently have an empty apparent-name
    // mapping to avoid the preexisting selected-graph cycle. Canonical Labels
    // are ordinary Bazel syntax and still reach the genuine selected routes.
    body.replace(
        "@local_tool//",
        &format!(
            "@@{}//",
            slug_bzlmod_v2::bazel_canonical_module_repo_name("local_tool")
        ),
    )
    .replace(
        "@archive_tool//",
        &format!(
            "@@{}//",
            slug_bzlmod_v2::bazel_canonical_module_repo_name("archive_tool")
        ),
    )
}

fn script_contents(family: &str, generation: &str) -> String {
    format!(
        r#"#!/bin/sh
test "$1" = "$0" || exit 51
test "$2" = literal || exit 52
printf '{family}:{generation}\n'
printf '{family}:{generation}\n' >out
if test "$#" -ge 4; then
    test -s "$3" || exit 53
    "$4" "$4" literal >workspace-stdout || exit 54
fi
"#
    )
}
fn script(path: &std::path::Path, family: &str, generation: &str) {
    std::fs::write(path, script_contents(family, generation)).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

struct Fixture {
    workspace: tempfile::TempDir,
    policy: BzlmodCommandPolicyKey,
    registries: Vec<String>,
    distdirs: crate::runtime::ArchiveDistdirPolicy,
    _archives: Option<tempfile::TempDir>,
    host_input: tempfile::TempDir,
}
fn fixture(family: Family) -> Fixture {
    let host_input = tempfile::tempdir().unwrap();
    std::fs::write(host_input.path().join("input.txt"), b"host:A\n").unwrap();
    let (workspace, policy) = original_workspace(
        &original(family, &host_input.path().join("input.txt")),
        "out",
    );
    if matches!(family, Family::Local | Family::Mixed) {
        let directory = workspace.path().join("local_tool");
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(
            directory.join("MODULE.bazel"),
            "module(name='local_tool',version='1.0')\n",
        )
        .unwrap();
        std::fs::write(
            directory.join("BUILD.bazel"),
            "exports_files(['tool.sh'])\n",
        )
        .unwrap();
        let mut module = std::fs::read_to_string(workspace.path().join("MODULE.bazel")).unwrap();
        module.push_str("\nbazel_dep(name='local_tool',version='1.0')\nlocal_path_override(module_name='local_tool',path='local_tool')\n");
        std::fs::write(workspace.path().join("MODULE.bazel"), module).unwrap();
        script(&directory.join("tool.sh"), "local", "A");
    }
    if matches!(family, Family::Workspace | Family::Mixed) {
        script(
            &workspace.path().join("workspace_tool.sh"),
            "workspace",
            "A",
        );
    }
    let mut fixture = Fixture {
        workspace,
        policy,
        registries: Vec::new(),
        distdirs: crate::runtime::ArchiveDistdirPolicy::default(),
        _archives: None,
        host_input,
    };
    if matches!(family, Family::Immutable) {
        initialize_archive(&mut fixture);
    }
    fixture
}

fn initialize_archive(fixture: &mut Fixture) {
    // Same genuine registry protocol as generated_owner_path_tests: source.json
    // creates the actual selected Spec. Only the archive capture comes from a
    // verified distdir; registry metadata/MODULE/patch/overlay use real files.
    let archives = tempfile::tempdir().unwrap();
    let registry = fixture.workspace.path().join("registry");
    let metadata = registry.join("modules/archive_tool/1.0");
    std::fs::create_dir_all(metadata.join("patches")).unwrap();
    std::fs::create_dir_all(metadata.join("overlay")).unwrap();
    std::fs::write(registry.join("bazel_registry.json"), "{}").unwrap();
    std::fs::write(
        metadata.join("MODULE.bazel"),
        "module(name='archive_tool',version='1.0')\n",
    )
    .unwrap();
    let original = script_contents("archive", "pre");
    let mut tar = tar::Builder::new(
        flate2::GzBuilder::new()
            .mtime(0)
            .write(Vec::new(), flate2::Compression::default()),
    );
    for (name, body, mode) in [
        (
            "BUILD.bazel",
            b"exports_files(['tool.sh','overlay.txt'])\n".as_slice(),
            0o644,
        ),
        (
            "MODULE.bazel",
            b"module(name='unverified_archive_module')\n".as_slice(),
            0o644,
        ),
        ("tool.sh", original.as_bytes(), 0o755),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(mode);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_cksum();
        tar.append_data(&mut header, name, body).unwrap();
    }
    let archive = tar.into_inner().unwrap().finish().unwrap();
    std::fs::write(archives.path().join("selected-tool.tar.gz"), &archive).unwrap();
    let patch = br#"--- a/tool.sh
+++ b/tool.sh
@@ -4,2 +4,2 @@
-printf 'archive:pre\n'
-printf 'archive:pre\n' >out
+printf 'archive:A\n'
+printf 'archive:A\n' >out
"#;
    let overlay = b"overlay-verified\n";
    std::fs::write(metadata.join("patches/tool.patch"), patch).unwrap();
    std::fs::write(metadata.join("overlay/overlay.txt"), overlay).unwrap();
    let sri = |bytes: &[u8]| {
        format!(
            "sha256-{}",
            base64::engine::general_purpose::STANDARD.encode(sha2::Sha256::digest(bytes))
        )
    };
    std::fs::write(metadata.join("source.json"),format!(r#"{{"url":"https://archive.invalid/selected-tool.tar.gz","integrity":"{}","archive_type":"tar.gz","strip_prefix":"","patches":{{"tool.patch":"{}"}},"overlay":{{"overlay.txt":"{}"}},"patch_strip":1}}"#,sri(&archive),sri(patch),sri(overlay))).unwrap();
    let mut module =
        std::fs::read_to_string(fixture.workspace.path().join("MODULE.bazel")).unwrap();
    module.push_str("\nbazel_dep(name='archive_tool',version='1.0')\n");
    std::fs::write(fixture.workspace.path().join("MODULE.bazel"), module).unwrap();
    fixture.registries.push(
        url::Url::from_directory_path(&registry)
            .unwrap()
            .to_string(),
    );
    fixture.distdirs = crate::runtime::ArchiveDistdirPolicy::from_request(
        fixture.workspace.path(),
        &[archives.path().to_str().unwrap().to_owned()],
    )
    .unwrap();
    fixture._archives = Some(archives);
}

fn run(family: Family, commands: usize) {
    let fixture = fixture(family);
    let workspace = &fixture.workspace;
    let policy = &fixture.policy;
    let mut runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    runtime
        .enable_general_repository_workspaces_for_test(resources.clone(), limits())
        .unwrap();
    let mut previous = None;
    for ordinal in 0..commands {
        let generation = if ordinal == 0 { "A" } else { "B" };
        if ordinal != 0 {
            std::fs::write(
                fixture.host_input.path().join("input.txt"),
                format!("host:{generation}\n"),
            )
            .unwrap();
            if matches!(family, Family::Local | Family::Mixed) {
                script(
                    &workspace.path().join("local_tool/tool.sh"),
                    "local",
                    generation,
                );
            }
            if matches!(family, Family::Workspace | Family::Mixed) {
                script(
                    &workspace.path().join("workspace_tool.sh"),
                    "workspace",
                    generation,
                );
            }
        }
        let accepted = runtime
            .cquery_command_with_repository_environment_and_distdirs(
                "//:use",
                true,
                true,
                policy.clone(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &fixture.registries,
                slug_bzlmod_v2::RepositoryEnvironmentSnapshot::empty(),
                CommandConfigurationOverlay::new(Arc::from([])),
                fixture.distdirs.clone(),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "source command {ordinal} failed at {}: {error}; bank={:?}",
                    runtime.native_demand_sessions.workspace_stage(),
                    resources.snapshot()
                )
            });
        assert!(
            accepted.terminal_for_test().as_ref().is_ok(),
            "{:#?}",
            accepted.terminal_for_test()
        );
        let expected = match family {
            Family::Builtin => "builtin\n".to_owned(),
            Family::Immutable => "archive:A\n".to_owned(),
            Family::Local | Family::Mixed => format!("local:{generation}\n"),
            Family::Workspace => format!("workspace:{generation}\n"),
        };
        let output = if matches!(family, Family::Mixed) {
            "workspace:A\n"
        } else {
            expected.as_str()
        };
        let (bindings, display, lease) = {
            let state = runtime.native_demand_sessions.state.lock().unwrap();
            assert!(matches!(state.phase, NativeDemandLeasePhase::Idle));
            let request = state
                .accepted
                .selected
                .repository_requests()
                .iter()
                .find(|r| matches!(r.kind, RepositoryMaterializationKind::GeneralWorkspace(_)))
                .unwrap();
            let RepositoryMaterializationKind::GeneralWorkspace(realization) = &request.kind else {
                unreachable!()
            };
            let command_paths = state.accepted.selected.command_paths();
            assert!(
                !command_paths.is_empty(),
                "current source observations must be retained as command facts"
            );
            for path in command_paths {
                assert!(
                    state.accepted.path_observations.get(path).is_some(),
                    "command facts stay in full accepted observation epoch"
                );
                assert!(
                    !state.accepted.selected.unscoped_paths().contains(path),
                    "source image is not a future unscoped IO demand"
                );
                assert!(
                    state
                        .accepted
                        .selected
                        .repository_validations()
                        .iter()
                        .all(|validation| !validation.paths().contains(path)),
                    "source image is not a future repository validation IO demand"
                );
            }
            let host = slug_workspace_v2::PathObservationDemand::new(
                slug_workspace_v2::PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(fixture.host_input.path().join("input.txt")).unwrap(),
                slug_workspace_v2::PathObservationOperation::FileBytes,
            );
            let host_value = state
                .accepted
                .path_observations
                .get(&host)
                .expect("ordinary Host input observation retained");
            let slug_workspace_v2::PathObservationResult::FileBytes(
                slug_workspace_v2::PathOperationResult::Present(host_bytes),
            ) = host_value.as_ref()
            else {
                panic!("Host input exact bytes")
            };
            assert_eq!(
                host_bytes.as_ref(),
                format!("host:{generation}\n").as_bytes()
            );
            assert!(!command_paths.contains(&host));
            assert!(
                state.accepted.selected.unscoped_paths().contains(&host)
                    || state
                        .accepted
                        .selected
                        .repository_validations()
                        .iter()
                        .any(|validation| validation.paths().contains(&host)),
                "Host freshness remains in reusable observation channels"
            );
            let slug_bzlmod_v2::RepositoryWorkspaceNode::File(host_copy) =
                realization.final_state().entry("host-copy").unwrap().node()
            else {
                panic!("Host-derived output")
            };
            assert_eq!(
                host_copy.as_ref(),
                format!("host:{generation}\n").as_bytes()
            );
            let records = realization
                .transcript()
                .records()
                .iter()
                .filter(|record| {
                    matches!(
                        record.request().operation(),
                        slug_bzlmod_v2::RepositoryWorkspaceOperation::Execute(_)
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(records.len(), 2);
            let slug_bzlmod_v2::RepositoryWorkspaceOperation::Execute(first) =
                records[0].request().operation()
            else {
                unreachable!()
            };
            assert!(
                first.read_views().views().is_empty(),
                "later prepared sources must not enter first prefix"
            );
            let slug_bzlmod_v2::RepositoryWorkspaceResult::Execute(first_result) =
                records[0].result()
            else {
                unreachable!()
            };
            assert_eq!(first_result.return_code(), 0);
            let slug_bzlmod_v2::RepositoryWorkspaceOperation::Execute(last) =
                records[1].request().operation()
            else {
                unreachable!()
            };
            let views = last.read_views().views();
            assert_eq!(
                views.len(),
                if matches!(family, Family::Mixed) {
                    3
                } else {
                    1
                }
            );
            for path in command_paths {
                assert!(
                    views.iter().any(|view| path.namespace()
                        == slug_workspace_v2::PathObservationNamespace::Materialization(
                            view.instance()
                        )
                        && path
                            .path()
                            .as_path()
                            .starts_with(view.display_root().as_path())),
                    "only this command's actual Source instances are partitioned"
                );
            }
            let mut bindings = views
                .iter()
                .map(|view| {
                    assert!(view.workspace_state().is_none());
                    let source = view.source_state().unwrap();
                    let reservation = source.reservation();
                    assert_eq!(reservation.lease(), realization.reservation().lease());
                    (
                        reservation.request().kind(),
                        reservation.repository().clone(),
                        reservation.display_root().clone(),
                        reservation.instance(),
                        reservation.lease(),
                    )
                })
                .collect::<Vec<_>>();
            bindings.sort_unstable_by(|a, b| a.1.cmp(&b.1));
            let kinds = bindings.iter().map(|r| r.0).collect::<Vec<_>>();
            assert_eq!(
                kinds,
                match family {
                    Family::Builtin => vec![RepositorySourceSnapshotKind::Builtin],
                    Family::Immutable => vec![RepositorySourceSnapshotKind::Immutable],
                    Family::Local => vec![RepositorySourceSnapshotKind::Local],
                    Family::Workspace => vec![RepositorySourceSnapshotKind::Workspace],
                    Family::Mixed => vec![
                        RepositorySourceSnapshotKind::Workspace,
                        RepositorySourceSnapshotKind::Builtin,
                        RepositorySourceSnapshotKind::Local
                    ],
                }
            );
            let slug_bzlmod_v2::RepositoryWorkspaceResult::Execute(last_result) =
                records[1].result()
            else {
                unreachable!()
            };
            assert_eq!(last_result.return_code(), 0);
            assert_eq!(last_result.stdout(), expected.as_bytes());
            let slug_bzlmod_v2::RepositoryWorkspaceNode::File(body) =
                realization.final_state().entry("out").unwrap().node()
            else {
                panic!("actual regular process output")
            };
            assert_eq!(body.as_ref(), output.as_bytes());
            let slug_bzlmod_v2::RepositoryWorkspaceNode::File(copied) = realization
                .final_state()
                .entry("source-copy")
                .unwrap()
                .node()
            else {
                panic!("template actual source")
            };
            assert!(!copied.is_empty());
            if matches!(family, Family::Immutable) {
                assert_eq!(copied.as_ref(), script_contents("archive", "A").as_bytes());
                let source = views[0].source_state().unwrap();
                let actual = source
                    .reservation()
                    .request()
                    .materialization_request()
                    .unwrap();
                assert!(matches!(
                    actual.kind,
                    RepositoryMaterializationKind::Immutable
                ));
                assert_eq!(actual.repo_spec.rule_id.rule_name.as_str(), "http_archive");
                assert_eq!(
                    source.reservation().request().selected_identity(),
                    Some(("archive_tool", "1.0"))
                );
                for (path, bytes) in [
                    ("tool.sh", script_contents("archive", "A").into_bytes()),
                    (
                        "MODULE.bazel",
                        b"module(name='archive_tool',version='1.0')\n".to_vec(),
                    ),
                    ("overlay.txt", b"overlay-verified\n".to_vec()),
                ] {
                    assert_eq!(
                        source.entry(path).unwrap().node(),
                        &slug_bzlmod_v2::RepositorySourceSnapshotNode::File(
                            slug_workspace_v2::FileContentDigest::new(
                                sha2::Sha256::digest(&bytes).into(),
                                bytes.len() as u64
                            )
                            .unwrap()
                        )
                    );
                }
            }
            (
                bindings,
                realization.reservation().display_root().clone(),
                realization.reservation().lease(),
            )
        };
        for (_, _, path, _, _) in &bindings {
            assert!(
                !path.as_path().exists(),
                "source display has no Host binding"
            );
        }
        assert!(!display.as_path().exists());
        if let Some(prior) = &previous {
            let prior: &Vec<(
                RepositorySourceSnapshotKind,
                slug_identity_v2::CanonicalRepoName,
                NormalizedAbsolutePath,
                slug_workspace_v2::PathObservationInstanceId,
                u64,
            )> = prior;
            assert_eq!(prior.len(), bindings.len());
            for (a, b) in prior.iter().zip(&bindings) {
                assert_eq!((&a.0, &a.1, &a.2), (&b.0, &b.1, &b.2));
                assert_ne!(a.3, b.3);
                assert_ne!(a.4, b.4);
            }
        }
        previous = Some(bindings);
        let forwarded = accepted
            .batches_for_test()
            .iter()
            .flat_map(|b| b.events())
            .filter_map(|event| match event {
                slug_events_v2::EvaluationEvent::RepositoryExecuteOutput {
                    command_lease,
                    stdout,
                    stderr,
                    ..
                } => Some((*command_lease, stdout.as_ref(), stderr.as_ref())),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(forwarded, [(lease, expected.as_bytes(), b"".as_slice())]);
        drop(accepted);
    }
    drop(runtime);
    assert_eq!(resources.snapshot().native_fds, 0);
    assert_eq!(resources.snapshot().backing_bytes, 0);
    assert_eq!(resources.snapshot().retained_bytes, baseline);
}

#[test]
#[ignore = "actual compiled CLI, source catalog and finite native supervisor required"]
fn native_original_builtin_source_is_fresh_across_commands() {
    run(Family::Builtin, 2)
}
#[test]
#[ignore = "actual compiled CLI, Local capture and finite native supervisor required"]
fn native_original_local_source_is_recaptured_across_commands() {
    run(Family::Local, 2)
}
#[test]
#[ignore = "actual compiled CLI, Workspace capture and finite native supervisor required"]
fn native_original_workspace_source_is_recaptured_across_commands() {
    run(Family::Workspace, 2)
}
#[test]
#[ignore = "actual compiled CLI, complete mixed source set and finite native supervisor required"]
fn native_original_mixed_source_set_is_visible_to_one_process() {
    run(Family::Mixed, 1)
}
#[test]
#[ignore = "actual compiled CLI, genuine selected registry archive and finite native supervisor required"]
fn native_original_immutable_archive_source_is_verified_and_fresh_across_commands() {
    run(Family::Immutable, 2)
}
