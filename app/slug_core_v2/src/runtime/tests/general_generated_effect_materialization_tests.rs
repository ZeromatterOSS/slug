use std::time::Duration;
use std::time::Instant;

use super::*;
use crate::runtime::dice::source_service::SourceServiceLimits;

fn named_spec(bzl_file: &str, rule_name: &str) -> RepoSpec {
    RepoSpec {
        rule_id: RepoRuleId {
            bzl_file: CanonicalLabel::parse(bzl_file).unwrap(),
            rule_name: rule_name.into(),
        },
        // The typed effects are authoritative. Missing native transport attrs
        // make accidental name-based transport dispatch fail this test.
        attributes: Arc::default(),
    }
}

#[cfg(unix)]
#[test]
fn generated_file_effects_ignore_rule_name_in_runtime_with_and_without_scope() {
    use std::os::unix::fs::PermissionsExt;

    let runtime = tokio::runtime::Runtime::new().unwrap();
    for scoped in [false, true] {
        for with_distdirs in [false, true] {
            let workspace_root = tempfile::tempdir().unwrap();
            let workspace =
                NormalizedAbsolutePath::new(workspace_root.path().to_path_buf()).unwrap();
            let scope = scoped.then(|| {
                let deadline = Instant::now() + Duration::from_secs(5);
                SourceServiceScope::new_for_test(
                    workspace.clone(),
                    deadline,
                    deadline + Duration::from_secs(1),
                    SourceServiceLimits::default(),
                )
                .unwrap()
            });
            let materializer = match &scope {
                Some(scope) => {
                    RepositoryMaterializer::new_scoped(workspace.clone(), scope.clone()).unwrap()
                }
                None => RepositoryMaterializer::new(workspace.clone()),
            };
            let token = begin_empty(&materializer);
            let mut roots = Vec::new();
            for (bzl_file, name) in [
                (
                    "@@bazel_tools//tools/build_defs/repo:local.bzl",
                    "local_repository",
                ),
                (
                    "@@bazel_tools//tools/build_defs/repo:http.bzl",
                    "http_archive",
                ),
                (
                    "@@bazel_tools//tools/build_defs/repo:git.bzl",
                    "git_repository",
                ),
            ] {
                let request = native_request(
                    &workspace,
                    name,
                    named_spec(bzl_file, name),
                    RepositoryMaterializationKind::GeneratedFileEffects(generated_plan([
                        ("tool", b"first" as &[u8], false),
                        ("tool", b"#!/bin/sh\nexit 0\n" as &[u8], true),
                        ("BUILD.bazel", b"exports_files(['tool'])\n" as &[u8], false),
                    ])),
                );
                if with_distdirs {
                    materializer.materialize_native_with_runtime_and_distdirs(
                        token,
                        request,
                        RepositoryMaterializationGeneration(1),
                        &runtime,
                        scope.as_ref(),
                        &crate::runtime::ArchiveDistdirPolicy::default(),
                    )
                } else {
                    materializer.materialize_native_with_runtime(
                        token,
                        request,
                        RepositoryMaterializationGeneration(1),
                        &runtime,
                        scope.as_ref(),
                    )
                }
                .unwrap();
                let (root, _) = immutable_result(&materializer, name);
                assert_eq!(
                    std::fs::read(root.join("tool")).unwrap(),
                    b"#!/bin/sh\nexit 0\n"
                );
                assert_eq!(
                    std::fs::read(root.join("BUILD.bazel")).unwrap(),
                    b"exports_files(['tool'])\n"
                );
                assert_eq!(
                    std::fs::metadata(root.join("tool"))
                        .unwrap()
                        .permissions()
                        .mode()
                        & 0o777,
                    0o755
                );
                assert_eq!(
                    std::fs::metadata(root.join("BUILD.bazel"))
                        .unwrap()
                        .permissions()
                        .mode()
                        & 0o777,
                    0o644
                );
                roots.push(root);
            }
            if let Some(scope) = &scope {
                assert!(scope.snapshot().written_bytes > 0);
                assert!(scope.check().is_ok());
            }
            materializer.discard(token).unwrap();
            assert!(roots.iter().all(|root| !root.exists()));
        }
    }
}

#[test]
fn generated_effect_runtime_keeps_workspace_and_bare_archive_guards() {
    let workspace_root = tempfile::tempdir().unwrap();
    let workspace = NormalizedAbsolutePath::new(workspace_root.path().to_path_buf()).unwrap();
    let materializer = RepositoryMaterializer::new(workspace.clone());
    let token = begin_empty(&materializer);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let empty_epoch = materializer.epoch(token).unwrap();
    let other_root = tempfile::tempdir().unwrap();
    let foreign = NormalizedAbsolutePath::new(other_root.path().to_path_buf()).unwrap();
    let spec = || {
        named_spec(
            "@@bazel_tools//tools/build_defs/repo:http.bzl",
            "http_archive",
        )
    };
    let wrong_workspace = native_request(
        &foreign,
        "foreign",
        spec(),
        RepositoryMaterializationKind::GeneratedFileEffects(generated_plan([(
            "BUILD.bazel",
            b"must not write" as &[u8],
            false,
        )])),
    );
    assert!(matches!(
        materializer.materialize_native_with_runtime(
            token,
            wrong_workspace,
            RepositoryMaterializationGeneration(1),
            &runtime,
            None
        ),
        Err(RepositorySessionError::WrongWorkspace)
    ));
    assert_eq!(materializer.epoch(token).unwrap(), empty_epoch);
    let mut plan = GeneratedRepositoryFileEffectPlan::builder();
    plan.push_archive(
        Arc::from([CompactString::new("file:///nonexistent/guard.tar.xz")]),
        [0; 32],
        "".into(),
    )
    .unwrap();
    let request = native_request(
        &workspace,
        "archive",
        spec(),
        RepositoryMaterializationKind::GeneratedFileEffects(plan.finish()),
    );
    materializer
        .materialize_native(token, request, RepositoryMaterializationGeneration(1))
        .unwrap();
    let RepositoryMaterializationResult::SpecError(message) =
        active_result(&materializer, "archive")
    else {
        panic!("bare API must refuse archive effects before transport or publication");
    };
    assert!(message.contains("archive"));
    assert!(
        materializer
            .state
            .lock()
            .unwrap()
            .active
            .as_ref()
            .unwrap()
            .provisional_roots
            .is_empty()
    );
    materializer.discard(token).unwrap();
}
