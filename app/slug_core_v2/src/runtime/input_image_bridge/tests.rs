use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;

use slug_workspace_v2::InputImageAliasPolicy;
use slug_workspace_v2::InputImageLimits;

use super::*;
use crate::runtime::ProcessHostOwner;

fn runtime(path: &Path) -> WorkspaceRuntime {
    WorkspaceRuntime::new(path, ProcessHostOwner::native()).unwrap()
}
fn capture(runtime: &WorkspaceRuntime) -> CapturedInputImageLease {
    runtime
        .capture_declared_input_image(
            DeclaredImageSource::WorkspaceSubtree("input".into()),
            InputImagePolicy::plain(),
        )
        .unwrap()
}

#[test]
fn declared_image_normal_need_capture_warm_a_b_a_and_last_drop() {
    let workspace = tempfile::tempdir().unwrap();
    let input = workspace.path().join("input");
    fs::create_dir(&input).unwrap();
    fs::write(input.join("value"), b"alpha").unwrap();
    fs::set_permissions(input.join("value"), fs::Permissions::from_mode(0o751)).unwrap();
    let runtime = runtime(workspace.path());
    let first = capture(&runtime);
    let first_path = first.test_path().to_owned();
    let shared = first.clone();
    assert_eq!(fs::read(first.test_path().join("value")).unwrap(), b"alpha");
    assert_eq!(
        fs::metadata(first.test_path().join("value"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o751
    );
    fs::write(input.join("value"), b"bravo").unwrap();
    fs::write(input.join("added"), b"new").unwrap();
    let second = capture(&runtime);
    assert_eq!(
        fs::read(second.test_path().join("value")).unwrap(),
        b"bravo"
    );
    assert!(second.test_path().join("added").is_file());
    assert_eq!(fs::read(first.test_path().join("value")).unwrap(), b"alpha");
    fs::remove_file(input.join("added")).unwrap();
    fs::write(input.join("value"), b"alpha").unwrap();
    let restored = capture(&runtime);
    assert_eq!(
        first.test_certificate().content(),
        restored.test_certificate().content()
    );
    assert!(!restored.test_path().join("added").exists());
    drop(first);
    assert!(first_path.exists());
    drop(shared);
    assert!(!first_path.exists());
    let preflight = runtime
        .begin_native_demand_command()
        .expect("image wrapper closed its source session");
    NativeDemandAbortGuard::new(preflight.into_command())
        .discard()
        .unwrap();
}

#[test]
fn declared_image_streams_sparse_input_and_live_disk_credit_retires() {
    let workspace = tempfile::tempdir().unwrap();
    let input = workspace.path().join("input");
    fs::create_dir(&input).unwrap();
    fs::File::create(input.join("large"))
        .unwrap()
        .set_len(8 * 1024 * 1024)
        .unwrap();
    let mut runtime = runtime(workspace.path());
    runtime.image_resources = input_image::WorkspaceImageResources::new(
        slug_workspace_v2::ImageDescriptorBudget::new(64 * 1024 * 1024),
        8 * 1024 * 1024,
    );
    let first = capture(&runtime);
    assert_eq!(
        fs::metadata(first.test_path().join("large")).unwrap().len(),
        8 * 1024 * 1024
    );
    assert!(
        runtime.image_resources.descriptor_budget().used_bytes() < 1024 * 1024,
        "regular bytes were not retained descriptors"
    );
    assert_eq!(
        runtime.image_resources.logical_bytes_used(),
        8 * 1024 * 1024
    );
    assert!(
        runtime
            .capture_declared_input_image(
                DeclaredImageSource::WorkspaceSubtree("input".into()),
                InputImagePolicy::plain()
            )
            .is_err()
    );
    let shared = first.clone();
    drop(first);
    assert_eq!(
        runtime.image_resources.logical_bytes_used(),
        8 * 1024 * 1024
    );
    drop(shared);
    assert_eq!(runtime.image_resources.logical_bytes_used(), 0);
    let recovered = capture(&runtime);
    drop(recovered);
    assert_eq!(runtime.image_resources.logical_bytes_used(), 0);
}

#[test]
fn declared_image_refuses_scope_escape_alias_and_keyed_limits() {
    let workspace = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret"), b"outside").unwrap();
    let input = workspace.path().join("input");
    fs::create_dir(&input).unwrap();
    symlink(outside.path(), input.join("escape")).unwrap();
    let runtime = runtime(workspace.path());
    assert!(
        runtime
            .capture_declared_input_image(
                DeclaredImageSource::WorkspaceSubtree("../".into()),
                InputImagePolicy::plain()
            )
            .is_err()
    );
    assert!(
        runtime
            .capture_declared_input_image(
                DeclaredImageSource::WorkspaceSubtree("input/escape".into()),
                InputImagePolicy::plain()
            )
            .is_err()
    );
    assert!(
        runtime
            .capture_declared_input_image(
                DeclaredImageSource::WorkspaceSubtree("input".into()),
                InputImagePolicy::runtime_aliases()
            )
            .is_err()
    );
    fs::remove_file(input.join("escape")).unwrap();
    fs::write(input.join("one"), b"1234").unwrap();
    let limits = InputImageLimits {
        max_file_bytes: 3,
        ..InputImageLimits::default()
    };
    let policy = InputImagePolicy::new(InputImageAliasPolicy::Deny, limits).unwrap();
    assert!(
        runtime
            .capture_declared_input_image(
                DeclaredImageSource::WorkspaceSubtree("input".into()),
                policy
            )
            .is_err()
    );
    fs::remove_file(input.join("one")).unwrap();
    fs::write(input.join("two"), b"new").unwrap();
    assert_eq!(
        fs::read(capture(&runtime).test_path().join("two")).unwrap(),
        b"new"
    );
}

#[test]
fn declared_image_materialization_uses_normal_request_and_actual_writer() {
    let workspace = tempfile::tempdir().unwrap();
    let runtime = runtime(workspace.path());
    let (request, plan) = generated_request(&runtime);
    let image = runtime
        .capture_declared_input_image(
            DeclaredImageSource::ImmutableRepository {
                request: request.clone(),
                subtree: PathBuf::new(),
            },
            InputImagePolicy::plain(),
        )
        .unwrap();
    for effect in plan.effects() {
        assert_eq!(
            fs::read(image.test_path().join(effect.path())).unwrap(),
            effect.content()
        );
    }
    assert!(matches!(
        image.test_certificate().namespace(),
        PathObservationNamespace::Materialization(_)
    ));
    let preflight = runtime.begin_native_demand_command().unwrap();
    assert!(preflight.path_observations().observations().is_empty());
    let mut guard = NativeDemandAbortGuard::new(preflight.into_command());
    guard.discard().unwrap();
    assert_eq!(
        fs::read(image.test_path().join("tool")).unwrap(),
        plan.effects()[0].content()
    );
}

pub(super) fn generated_request(
    runtime: &WorkspaceRuntime,
) -> (
    Arc<RepositoryMaterializationRequest>,
    slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan,
) {
    use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan;
    use slug_bzlmod_v2::RepoRuleId;
    use slug_bzlmod_v2::RepoSpec;
    use slug_bzlmod_v2::RepositoryMaterializationKind;
    use slug_identity_v2::CanonicalLabel;
    use slug_identity_v2::CanonicalRepoName;
    let plan = GeneratedRepositoryFileEffectPlan::build([(
        "tool".into(),
        Arc::<[u8]>::from(&b"selected bytes"[..]),
        true,
    )])
    .unwrap();
    let request = Arc::new(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: runtime.repository_materializer.workspace().clone(),
            canonical_repo: CanonicalRepoName::new("selected+").unwrap(),
        },
        repo_spec: RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@selected+//:defs.bzl").unwrap(),
                rule_name: "generated_repository".into(),
            },
            attributes: Arc::default(),
        },
        kind: RepositoryMaterializationKind::GeneratedFileEffects(plan.clone()),
    });
    (request, plan)
}

#[test]
fn declared_image_small_tree_under_deep_prefix_has_sufficient_attempts() {
    let workspace = tempfile::tempdir().unwrap();
    let subtree = PathBuf::from("a/b/c/d/e/f/g/h/i/j/k");
    fs::create_dir_all(workspace.path().join(&subtree)).unwrap();
    let runtime = runtime(workspace.path());
    let policy = InputImagePolicy::new(
        InputImageAliasPolicy::Deny,
        InputImageLimits {
            max_entries: 1,
            max_depth: 0,
            ..InputImageLimits::default()
        },
    )
    .unwrap();
    let image = runtime
        .capture_declared_input_image(DeclaredImageSource::WorkspaceSubtree(subtree), policy)
        .unwrap();
    assert_eq!(fs::read_dir(image.test_path()).unwrap().count(), 0);
}

#[test]
fn declared_image_factory_rejects_foreign_materialization_and_session_before_io() {
    use slug_bzlmod_v2::RepositoryMaterialization;
    use slug_workspace_v2::PathObservationInstanceId;

    use crate::runtime::repository_io::RepositorySessionError;
    let workspace = tempfile::tempdir().unwrap();
    let runtime = runtime(workspace.path());
    let (request, _) = generated_request(&runtime);
    let mut guard = NativeDemandAbortGuard::new(
        runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command(),
    );
    guard
        .progress(&slug_bzlmod_v2::SourcePreparationNeeds::repository(
            request.as_ref().clone(),
        ))
        .unwrap();
    guard.begin_attempt().unwrap();
    let expected = runtime.runtime.block_on(async {
        let mut updater = runtime
            .dice
            .updater_with_data(guard.attempt_user_computation_data().unwrap());
        guard.inject_attempt(&mut updater).unwrap();
        let mut tx = runtime.request_revision.commit(updater).await;
        match slug_bzlmod_v2::source_preparation::compute_repository_materialization_result(
            &mut tx,
            request.clone(),
        )
        .await
        .unwrap()
        {
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(result) => {
                result.as_ref().clone().unwrap()
            }
            _ => panic!("actual materialization must be complete"),
        }
    });
    runtime.runtime.block_on(runtime.dice.wait_for_idle());
    let token = guard.command().repository_session;
    let RepositoryMaterialization::Immutable {
        generation_root,
        observation_instance,
        ..
    } = &expected
    else {
        panic!("actual immutable writer result")
    };
    let anchor = NormalizedAbsolutePath::new(generation_root.clone()).unwrap();
    let namespace = PathObservationNamespace::Materialization(*observation_instance);
    let factory = |ns,
                   root: &NormalizedAbsolutePath,
                   selected: Option<(
        &Arc<RepositoryMaterializationRequest>,
        &RepositoryMaterialization,
    )>| {
        runtime
            .repository_materializer
            .image_source(token, ns, root, selected)
    };
    let source = factory(namespace, &anchor, Some((&request, &expected))).unwrap();
    assert!(matches!(
        factory(namespace, &anchor, None).err(),
        Some(RepositorySessionError::InvalidImageAuthority)
    ));
    assert!(matches!(
        factory(
            PathObservationNamespace::Host,
            &anchor,
            Some((&request, &expected))
        )
        .err(),
        Some(RepositorySessionError::InvalidImageAuthority)
    ));
    let foreign =
        NormalizedAbsolutePath::new(workspace.path().join("not-an-authorized-root")).unwrap();
    assert!(matches!(
        factory(PathObservationNamespace::Host, &foreign, None).err(),
        Some(RepositorySessionError::InvalidImageAuthority)
    ));
    let wrong_instance = PathObservationInstanceId::new(u64::MAX);
    assert!(matches!(
        factory(
            PathObservationNamespace::Materialization(wrong_instance),
            &anchor,
            Some((&request, &expected))
        )
        .err(),
        Some(RepositorySessionError::InvalidImageAuthority)
    ));
    let mut changed_request = request.as_ref().clone();
    changed_request.repo_spec.rule_id.rule_name = "foreign_rule".into();
    let changed_request = Arc::new(changed_request);
    assert!(matches!(
        factory(namespace, &anchor, Some((&changed_request, &expected))).err(),
        Some(RepositorySessionError::InvalidImageAuthority)
    ));
    for field in 0..5 {
        let mut changed = expected.clone();
        let RepositoryMaterialization::Immutable {
            source_identity,
            generation_root,
            observation_instance,
            canonical_repo,
            repo_spec,
            ..
        } = &mut changed
        else {
            unreachable!()
        };
        match field {
            0 => *source_identity = Arc::from("foreign source"),
            1 => *generation_root = foreign.as_path().to_owned(),
            2 => *observation_instance = wrong_instance,
            3 => *canonical_repo = slug_identity_v2::CanonicalRepoName::new("foreign+").unwrap(),
            4 => repo_spec.rule_id.rule_name = "foreign_rule".into(),
            _ => unreachable!(),
        }
        assert!(
            matches!(
                factory(namespace, &anchor, Some((&request, &changed))).err(),
                Some(RepositorySessionError::InvalidImageAuthority)
            ),
            "result field {field}"
        );
    }
    guard.discard().unwrap();
    assert!(!runtime.repository_materializer.session_is_active(token));
    assert!(matches!(
        runtime
            .repository_materializer
            .image_source(token, namespace, &anchor, Some((&request, &expected)))
            .err(),
        Some(RepositorySessionError::StaleToken { .. })
    ));
    drop(source);
    let pending = runtime.repository_materializer.begin().unwrap();
    assert!(matches!(
        runtime
            .repository_materializer
            .image_source(
                pending,
                PathObservationNamespace::Host,
                runtime.repository_materializer.workspace(),
                None
            )
            .err(),
        Some(RepositorySessionError::ValidationIncomplete)
    ));
    runtime.repository_materializer.discard(pending).unwrap();
}

#[test]
fn declared_images_parallel_workspace_owners_remain_isolated() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    for (workspace, content) in [
        (first.path(), b"first".as_slice()),
        (second.path(), b"second".as_slice()),
    ] {
        fs::create_dir(workspace.join("input")).unwrap();
        fs::write(workspace.join("input/value"), content).unwrap();
    }
    let started = std::sync::Barrier::new(2);
    let retained = std::sync::Barrier::new(2);
    let roots = std::thread::scope(|scope| {
        let capture_owner = |workspace: &Path, expected: &[u8]| {
            let runtime = runtime(workspace);
            started.wait();
            let image = capture(&runtime);
            let path = image.test_path().to_owned();
            retained.wait();
            assert_eq!(fs::read(path.join("value")).unwrap(), expected);
            assert_eq!(
                runtime.image_resources.logical_bytes_used(),
                expected.len() as u64
            );
            let shared = image.clone();
            drop(image);
            assert!(path.exists());
            drop(shared);
            assert!(!path.exists());
            assert_eq!(runtime.image_resources.logical_bytes_used(), 0);
            path
        };
        let a = scope.spawn(move || capture_owner(first.path(), b"first"));
        let b = scope.spawn(move || capture_owner(second.path(), b"second"));
        (a.join().unwrap(), b.join().unwrap())
    });
    assert_ne!(roots.0, roots.1);
}
