//! Native observer boundaries. Bridge tests separately drive the normal key.

use std::cell::Cell;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::path::Path;

use slug_workspace_v2::DirectoryObservationLimits;
use slug_workspace_v2::ImageChargedPath;
use slug_workspace_v2::ImageObservationData;
use slug_workspace_v2::ImageObservationOperation;
use slug_workspace_v2::ImageObservationRequest;
use slug_workspace_v2::PathOperationResult;

use super::*;

fn source(root: &Path) -> ScopedImageSource {
    let anchor = NormalizedAbsolutePath::new(root).unwrap();
    ScopedImageSource::new(
        PathObservationNamespace::Host,
        anchor.clone(),
        open_image_anchor(&anchor).unwrap(),
        None,
    )
    .unwrap()
}

fn demand(
    root: &Path,
    relative: &str,
    operation: ImageObservationOperation,
    budget: &ImageDescriptorBudget,
) -> PathObservationDemand {
    let anchor =
        ImageChargedPath::pin_existing(NormalizedAbsolutePath::new(root).unwrap(), budget).unwrap();
    let path = ImageChargedPath::pin_existing(
        NormalizedAbsolutePath::new(root.join(relative)).unwrap(),
        budget,
    )
    .unwrap();
    PathObservationDemand::input_image(
        PathObservationNamespace::Host,
        path,
        ImageObservationRequest::new(anchor, operation),
    )
    .unwrap()
}

#[test]
fn scoped_observer_rejects_foreign_budget_and_scope_before_io() {
    let temporary = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let approved = source(temporary.path());
    let budget = ImageDescriptorBudget::new(1024 * 1024);
    let foreign = ImageDescriptorBudget::new(1024 * 1024);
    let query = demand(
        temporary.path(),
        "absent",
        ImageObservationOperation::Lstat,
        &foreign,
    );
    assert!(matches!(
        approved.observe(&query, &budget, &|| true),
        Err(InputImageCaptureError::Unauthorized)
    ));
    let query = demand(
        other.path(),
        "absent",
        ImageObservationOperation::Lstat,
        &budget,
    );
    assert!(matches!(
        approved.observe(&query, &budget, &|| true),
        Err(InputImageCaptureError::Unauthorized)
    ));
    let query = demand(
        temporary.path(),
        "absent",
        ImageObservationOperation::Lstat,
        &budget,
    );
    assert!(matches!(
        approved.observe(&query, &budget, &|| false),
        Err(InputImageCaptureError::Cancelled)
    ));
    assert!(
        matches!(approved.observe(&query, &budget, &|| true).unwrap(), PathObservationResult::InputImage(result) if matches!(result.data(), ImageObservationData::Lstat(PathOperationResult::Missing)))
    );
}

#[test]
fn nofollow_walk_rejects_linked_anchor_and_linked_digest_ancestor() {
    let temporary = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("value"), "outside").unwrap();
    symlink(outside.path(), temporary.path().join("alias")).unwrap();
    let anchor = NormalizedAbsolutePath::new(temporary.path().join("alias")).unwrap();
    assert!(open_image_anchor(&anchor).is_err());
    let approved = source(temporary.path());
    let budget = ImageDescriptorBudget::new(1024 * 1024);
    let query = demand(
        temporary.path(),
        "alias/value",
        ImageObservationOperation::BoundedDigest { max_bytes: 1024 },
        &budget,
    );
    match approved.observe(&query, &budget, &|| true).unwrap() {
        PathObservationResult::InputImage(result) => assert!(matches!(
            result.data(),
            ImageObservationData::Digest(PathOperationResult::Error(_))
        )),
        _ => panic!("scoped image result expected"),
    }
}

#[test]
fn raced_ancestor_redirect_is_denied_before_first_bounded_digest_read() {
    let temporary = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir(temporary.path().join("nested")).unwrap();
    std::fs::write(temporary.path().join("nested/value"), b"inside").unwrap();
    std::fs::write(
        outside.path().join("value"),
        b"outside must not be observed",
    )
    .unwrap();
    let approved = source(temporary.path());
    let budget = ImageDescriptorBudget::new(1024 * 1024);
    let query = demand(
        temporary.path(),
        "nested/value",
        ImageObservationOperation::BoundedDigest { max_bytes: 1024 },
        &budget,
    );
    let checks = Cell::new(0);
    let redirected = Cell::new(false);
    let active = || {
        checks.set(checks.get() + 1);
        // The common observer checks scope/credits and the current anchor
        // first; its backend checks activity immediately before digest I/O.
        if checks.get() == 3 {
            std::fs::rename(
                temporary.path().join("nested"),
                temporary.path().join("retired"),
            )
            .unwrap();
            symlink(outside.path(), temporary.path().join("nested")).unwrap();
            redirected.set(true);
        }
        true
    };
    let result = approved.observe(&query, &budget, &active).unwrap();
    assert!(redirected.get());
    assert!(
        matches!(result, PathObservationResult::InputImage(result) if matches!(result.data(), ImageObservationData::Digest(PathOperationResult::Error(_))))
    );
    assert_eq!(
        std::fs::read(temporary.path().join("retired/value")).unwrap(),
        b"inside"
    );
}

#[test]
fn bounded_observers_limit_before_retaining_payload_and_preserve_modes() {
    let temporary = tempfile::tempdir().unwrap();
    std::fs::write(temporary.path().join("value"), vec![7u8; 65537]).unwrap();
    std::fs::set_permissions(
        temporary.path().join("value"),
        std::fs::Permissions::from_mode(0o1751),
    )
    .unwrap();
    symlink("value", temporary.path().join("link")).unwrap();
    let approved = source(temporary.path());
    let budget = ImageDescriptorBudget::new(1024 * 1024);
    for (relative, operation) in [
        (
            "value",
            ImageObservationOperation::BoundedDigest { max_bytes: 65536 },
        ),
        (
            "link",
            ImageObservationOperation::BoundedReadLink {
                max_target_bytes: 4,
            },
        ),
        (
            "",
            ImageObservationOperation::BoundedDirectory(DirectoryObservationLimits {
                max_entries: 1,
                max_name_bytes_total: 1024,
                max_component_bytes: 256,
            }),
        ),
    ] {
        let query = demand(temporary.path(), relative, operation, &budget);
        assert!(
            matches!(approved.observe(&query, &budget, &|| true).unwrap(), PathObservationResult::InputImage(result) if matches!(result.data(), ImageObservationData::LimitExceeded))
        );
    }
    let query = demand(
        temporary.path(),
        "value",
        ImageObservationOperation::Lstat,
        &budget,
    );
    assert!(
        matches!(approved.observe(&query, &budget, &|| true).unwrap(), PathObservationResult::InputImage(result) if matches!(result.data(), ImageObservationData::Lstat(PathOperationResult::Present(stat)) if stat.permissions() == 0o1751))
    );
}

#[test]
fn bounded_directory_component_and_cumulative_name_bytes_fail_independently() {
    let temporary = tempfile::tempdir().unwrap();
    std::fs::write(temporary.path().join("aaaa"), b"one").unwrap();
    std::fs::write(temporary.path().join("bbbb"), b"two").unwrap();
    let approved = source(temporary.path());
    let budget = ImageDescriptorBudget::new(1024 * 1024);
    for bounds in [
        DirectoryObservationLimits {
            max_entries: 8,
            max_name_bytes_total: 1024,
            max_component_bytes: 3,
        },
        DirectoryObservationLimits {
            max_entries: 8,
            max_name_bytes_total: 7,
            max_component_bytes: 8,
        },
    ] {
        let query = demand(
            temporary.path(),
            "",
            ImageObservationOperation::BoundedDirectory(bounds),
            &budget,
        );
        assert!(
            matches!(approved.observe(&query, &budget, &|| true).unwrap(), PathObservationResult::InputImage(result) if matches!(result.data(), ImageObservationData::LimitExceeded))
        );
    }
    // The exact inclusive limits succeed, independently of native row order.
    let query = demand(
        temporary.path(),
        "",
        ImageObservationOperation::BoundedDirectory(DirectoryObservationLimits {
            max_entries: 8,
            max_name_bytes_total: 8,
            max_component_bytes: 4,
        }),
        &budget,
    );
    assert!(
        matches!(approved.observe(&query, &budget, &|| true).unwrap(), PathObservationResult::InputImage(result) if matches!(result.data(), ImageObservationData::Directory(PathOperationResult::Present(rows)) if rows.entries().len() == 2))
    );
}

#[test]
fn digest_cancellation_returns_typed_error_and_releases_transient_credit() {
    let temporary = tempfile::tempdir().unwrap();
    std::fs::write(temporary.path().join("value"), vec![9u8; 3 * 65536]).unwrap();
    let approved = source(temporary.path());
    let budget = ImageDescriptorBudget::new(1024 * 1024);
    let query = demand(
        temporary.path(),
        "value",
        ImageObservationOperation::BoundedDigest {
            max_bytes: 3 * 65536,
        },
        &budget,
    );
    let baseline = budget.used_bytes();
    let checks = Cell::new(0);
    let active = || {
        checks.set(checks.get() + 1);
        checks.get() < 5
    };
    assert!(matches!(
        approved.observe(&query, &budget, &active),
        Err(InputImageCaptureError::Cancelled)
    ));
    assert_eq!(budget.used_bytes(), baseline);
}

#[test]
fn logical_credits_are_shared_until_last_reservation_drop() {
    let resources = WorkspaceImageResources::new(ImageDescriptorBudget::new(1024), 10);
    let same_workspace = resources.clone();
    let first = resources.reserve_logical(7).unwrap();
    assert!(matches!(
        same_workspace.reserve_logical(4),
        Err(InputImageCaptureError::LogicalByteBudget)
    ));
    let second = same_workspace.reserve_logical(3).unwrap();
    assert_eq!(resources.logical_bytes_used(), 10);
    drop(first);
    assert_eq!(same_workspace.logical_bytes_used(), 3);
    drop(second);
    assert_eq!(resources.logical_bytes_used(), 0);
}

#[test]
fn interrupted_syscall_is_attempted_once_and_cannot_complete_an_observation() {
    let calls = Cell::new(0);
    let result = linux::fs::single_call(|| {
        calls.set(calls.get() + 1);
        Err::<slug_workspace_v2::PathLstat, _>(io::Error::from_raw_os_error(libc::EINTR))
    });
    let data = ImageObservationData::Lstat(linux::observed(result));
    assert_eq!(calls.get(), 1);
    assert!(
        matches!(linux::reject_interrupted(&data), Err(InputImageCaptureError::Io(error)) if error.kind() == io::ErrorKind::Interrupted)
    );
}

async fn observe_certificate(
    root: &Path,
    resources: &WorkspaceImageResources,
    policy: slug_workspace_v2::InputImagePolicy,
) -> slug_workspace_v2::ObservedInputImage {
    observe_certificate_subtree(root, Path::new("input"), resources, policy).await
}

async fn observe_certificate_subtree(
    root: &Path,
    subtree: &Path,
    resources: &WorkspaceImageResources,
    policy: slug_workspace_v2::InputImagePolicy,
) -> slug_workspace_v2::ObservedInputImage {
    observe_certificate_shape(root, subtree, resources, policy, false).await
}

async fn observe_certificate_shape(
    root: &Path,
    subtree: &Path,
    resources: &WorkspaceImageResources,
    policy: slug_workspace_v2::InputImagePolicy,
    regular_root: bool,
) -> slug_workspace_v2::ObservedInputImage {
    use dice::DetectCycles;
    use dice::Dice;
    use dice::UserComputationData;
    use slug_workspace_v2::ObservedInputImageKey;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathOutcome;
    let approved = source(root);
    let budget = resources.descriptor_budget();
    let key = if regular_root {
        ObservedInputImageKey::regular_file_from_charged(
            PathObservationNamespace::Host,
            ImageChargedPath::pin_existing(NormalizedAbsolutePath::new(root).unwrap(), budget)
                .unwrap(),
            ImageChargedPath::pin_existing(
                NormalizedAbsolutePath::new(root.join(subtree)).unwrap(),
                budget,
            )
            .unwrap(),
            policy,
        )
        .unwrap()
    } else {
        ObservedInputImageKey::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(root).unwrap(),
            NormalizedAbsolutePath::new(root.join(subtree)).unwrap(),
            policy,
            budget,
        )
        .unwrap()
    };
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut observations = Vec::new();
    let mut saw_need = false;
    for _ in 0..64 {
        let mut data = UserComputationData::default();
        data.data.set(budget.clone());
        let mut updater = dice.updater_with_data(data);
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new(observations.clone()).unwrap(),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let outcome = transaction.compute(&key).await.unwrap();
        drop(transaction);
        dice.wait_for_idle().await;
        match outcome {
            PathOutcome::Need(need) => {
                saw_need = true;
                for demand in need.demands() {
                    let result = approved.observe(demand, budget, &|| true).unwrap();
                    assert!(
                        observations.iter().all(|(existing, _)| existing != demand),
                        "Need must make progress"
                    );
                    observations.push((demand.clone(), result));
                }
            }
            PathOutcome::Complete(result) => {
                assert!(
                    saw_need,
                    "certificate must be produced by actual native Need fulfillment"
                );
                return result.unwrap();
            }
        }
    }
    panic!("small fixture exceeded bounded image Need attempts");
}

#[test]
fn anchor_directory_reobservation_uses_independent_directory_cursors() {
    let temporary = tempfile::tempdir().unwrap();
    std::fs::write(temporary.path().join("value"), b"owned").unwrap();
    let approved = source(temporary.path());
    let budget = ImageDescriptorBudget::new(1024 * 1024);
    let query = demand(
        temporary.path(),
        "",
        ImageObservationOperation::BoundedDirectory(DirectoryObservationLimits {
            max_entries: 16,
            max_name_bytes_total: 1024,
            max_component_bytes: 256,
        }),
        &budget,
    );
    let first = approved.observe(&query, &budget, &|| true).unwrap();
    let second = approved.observe(&query, &budget, &|| true).unwrap();
    assert_eq!(first, second);
    assert!(
        matches!(second, PathObservationResult::InputImage(result) if matches!(result.data(), ImageObservationData::Directory(PathOperationResult::Present(rows)) if rows.entries().len() == 1))
    );
}

#[tokio::test]
async fn empty_subtree_root_capture_reobserves_nonempty_anchor_membership() {
    let temporary = tempfile::tempdir().unwrap();
    std::fs::write(temporary.path().join("value"), b"owned").unwrap();
    let resources =
        WorkspaceImageResources::new(ImageDescriptorBudget::new(64 * 1024 * 1024), 1024);
    let certificate = observe_certificate_subtree(
        temporary.path(),
        Path::new(""),
        &resources,
        slug_workspace_v2::InputImagePolicy::plain(),
    )
    .await;
    let image = capture_input_image(
        AuthorizedObservedInputImage::new(source(temporary.path()), certificate),
        &resources,
        &|| true,
    )
    .unwrap();
    assert_eq!(
        std::fs::read(image.test_path().join("value")).unwrap(),
        b"owned"
    );
}

#[tokio::test]
async fn capture_rejects_complete_frontier_content_mode_and_membership_changes() {
    for mutation in 0..3 {
        let temporary = tempfile::tempdir().unwrap();
        let input = temporary.path().join("input");
        std::fs::create_dir(&input).unwrap();
        std::fs::write(input.join("value"), b"alpha").unwrap();
        let resources =
            WorkspaceImageResources::new(ImageDescriptorBudget::new(64 * 1024 * 1024), 1024);
        let certificate = observe_certificate(
            temporary.path(),
            &resources,
            slug_workspace_v2::InputImagePolicy::plain(),
        )
        .await;
        match mutation {
            0 => std::fs::write(input.join("value"), b"bravo").unwrap(),
            1 => std::fs::set_permissions(
                input.join("value"),
                std::fs::Permissions::from_mode(0o700),
            )
            .unwrap(),
            _ => std::fs::write(input.join("added"), b"new").unwrap(),
        }
        let authorized = AuthorizedObservedInputImage::new(source(temporary.path()), certificate);
        assert!(matches!(
            capture_input_image(authorized, &resources, &|| true),
            Err(InputImageCaptureError::StaleSource)
        ));
        assert_eq!(resources.logical_bytes_used(), 0);
    }
}

#[tokio::test]
async fn post_copy_frontier_rejects_new_membership_before_promotion() {
    let temporary = tempfile::tempdir().unwrap();
    let input = temporary.path().join("input");
    std::fs::create_dir(&input).unwrap();
    std::fs::write(input.join("value"), vec![7u8; 3 * 65536]).unwrap();
    let resources =
        WorkspaceImageResources::new(ImageDescriptorBudget::new(64 * 1024 * 1024), 1024 * 1024);
    let certificate = observe_certificate(
        temporary.path(),
        &resources,
        slug_workspace_v2::InputImagePolicy::plain(),
    )
    .await;
    let changed = Cell::new(false);
    let active = || {
        if resources.logical_bytes_used() != 0 && !changed.replace(true) {
            std::fs::write(input.join("added"), b"not in certified membership").unwrap();
        }
        true
    };
    let authorized = AuthorizedObservedInputImage::new(source(temporary.path()), certificate);
    assert!(matches!(
        capture_input_image(authorized, &resources, &active),
        Err(InputImageCaptureError::StaleSource)
    ));
    assert!(
        changed.get(),
        "mutation happened after stage credit acquisition"
    );
    assert_eq!(resources.logical_bytes_used(), 0);
}

#[tokio::test]
async fn revoked_during_streaming_copy_returns_no_lease_and_releases_stage_credit() {
    let temporary = tempfile::tempdir().unwrap();
    let input = temporary.path().join("input");
    std::fs::create_dir(&input).unwrap();
    std::fs::write(input.join("value"), vec![7u8; 3 * 65536]).unwrap();
    let resources =
        WorkspaceImageResources::new(ImageDescriptorBudget::new(64 * 1024 * 1024), 1024 * 1024);
    let certificate = observe_certificate(
        temporary.path(),
        &resources,
        slug_workspace_v2::InputImagePolicy::plain(),
    )
    .await;
    let stage_checks = Cell::new(0);
    let active = || {
        if resources.logical_bytes_used() == 0 {
            return true;
        }
        stage_checks.set(stage_checks.get() + 1);
        stage_checks.get() <= 3
    };
    let authorized = AuthorizedObservedInputImage::new(source(temporary.path()), certificate);
    assert!(matches!(
        capture_input_image(authorized, &resources, &active),
        Err(InputImageCaptureError::Cancelled)
    ));
    assert!(stage_checks.get() > 3);
    assert_eq!(resources.logical_bytes_used(), 0);
}

#[tokio::test]
async fn retained_image_last_drop_restores_mode_zero_directories_for_cleanup() {
    let temporary = tempfile::tempdir().unwrap();
    let input = temporary.path().join("input");
    std::fs::create_dir(&input).unwrap();
    std::fs::create_dir(input.join("closed")).unwrap();
    std::fs::write(input.join("closed/value"), b"owned").unwrap();
    std::fs::set_permissions(input.join("closed"), std::fs::Permissions::from_mode(0o751)).unwrap();
    let resources =
        WorkspaceImageResources::new(ImageDescriptorBudget::new(64 * 1024 * 1024), 1024);
    let certificate = observe_certificate(
        temporary.path(),
        &resources,
        slug_workspace_v2::InputImagePolicy::plain(),
    )
    .await;
    let image = capture_input_image(
        AuthorizedObservedInputImage::new(source(temporary.path()), certificate),
        &resources,
        &|| true,
    )
    .unwrap();
    let path = image.test_path().to_owned();
    assert_eq!(
        std::fs::metadata(path.join("closed"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o751
    );
    // Test-only interference exercises cleanup of an inaccessible retained
    // stage even for a nonprivileged owner; production exposes no stage path.
    std::fs::set_permissions(path.join("closed"), std::fs::Permissions::from_mode(0o000)).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    let shared = image.clone();
    drop(image);
    assert_eq!(resources.logical_bytes_used(), 5);
    drop(shared);
    assert!(!path.exists());
    assert_eq!(resources.logical_bytes_used(), 0);
}

#[tokio::test]
async fn contained_alias_capture_preserves_literal_without_following_host_paths() {
    let temporary = tempfile::tempdir().unwrap();
    let input = temporary.path().join("input");
    std::fs::create_dir(&input).unwrap();
    std::fs::write(input.join("value"), b"owned").unwrap();
    symlink("value", input.join("relative")).unwrap();
    symlink(input.join("value"), input.join("absolute")).unwrap();
    let resources =
        WorkspaceImageResources::new(ImageDescriptorBudget::new(64 * 1024 * 1024), 1024);
    let certificate = observe_certificate(
        temporary.path(),
        &resources,
        slug_workspace_v2::InputImagePolicy::runtime_aliases(),
    )
    .await;
    let image = capture_input_image(
        AuthorizedObservedInputImage::new(source(temporary.path()), certificate),
        &resources,
        &|| true,
    )
    .unwrap();
    assert_eq!(
        std::fs::read_link(image.test_path().join("relative")).unwrap(),
        Path::new("value")
    );
    assert_eq!(
        std::fs::read_link(image.test_path().join("absolute")).unwrap(),
        input.join("value")
    );
    assert_eq!(
        resources.logical_bytes_used(),
        5,
        "aliases share certified target bytes"
    );
}

#[tokio::test]
async fn retaining_capture_pre_copy_and_promotion_refusals_keep_stage_and_credit() {
    for regular_root in [false, true] {
        let temporary = tempfile::tempdir().unwrap();
        std::fs::create_dir(temporary.path().join("input")).unwrap();
        std::fs::write(temporary.path().join("input/value"), b"owned").unwrap();
        let budget = ImageDescriptorBudget::new(64 * 1024 * 1024);
        let resources = WorkspaceImageResources::new(budget.clone(), 1024);
        let mut first = prepare_input_image_stage(&resources).unwrap();
        let certificate = observe_certificate_shape(
            temporary.path(),
            if regular_root {
                Path::new("input/value")
            } else {
                Path::new("input")
            },
            &resources,
            slug_workspace_v2::InputImagePolicy::plain(),
            regular_root,
        )
        .await;
        let capture = |authorized,
                       resources: &WorkspaceImageResources,
                       active: &dyn Fn() -> bool,
                       stage: &mut PreparedInputImageStage| {
            if regular_root {
                capture_prepared_input_file_retaining(authorized, resources, active, stage)
                    .map(drop)
            } else {
                capture_prepared_input_image_retaining(authorized, resources, active, stage)
                    .map(drop)
            }
        };
        let calls = Cell::new(0usize);
        capture(
            AuthorizedObservedInputImage::new(source(temporary.path()), certificate.dupe()),
            &resources,
            &|| {
                calls.set(calls.get() + 1);
                true
            },
            &mut first,
        )
        .unwrap();
        assert!(calls.get() > 0);
        let completed_calls = calls.get();
        drop(first);
        assert_eq!(resources.logical_bytes_used(), 0);
        let baseline = budget.used_bytes();
        let mut stage = prepare_input_image_stage(&resources).unwrap();
        let path = stage.test_path().to_owned();
        let ready_charge = budget.used_bytes();
        let denied_calls = Cell::new(0);
        let foreign = WorkspaceImageResources::new(ImageDescriptorBudget::new(1024 * 1024), 1024);
        assert!(matches!(
            capture(
                AuthorizedObservedInputImage::new(source(temporary.path()), certificate.dupe()),
                &foreign,
                &|| {
                    denied_calls.set(denied_calls.get() + 1);
                    true
                },
                &mut stage,
            ),
            Err(InputImageCaptureError::Unauthorized)
        ));
        assert_eq!(denied_calls.get(), 0);
        assert!(!stage.test_is_partial());
        assert!(path.exists());
        assert_eq!(budget.used_bytes(), ready_charge);
        let calls = Cell::new(0usize);
        let pressure = std::cell::RefCell::new(None);
        let result = capture(
            AuthorizedObservedInputImage::new(source(temporary.path()), certificate.dupe()),
            &resources,
            &|| {
                calls.set(calls.get() + 1);
                if calls.get() == completed_calls {
                    *pressure.borrow_mut() = Some(
                        budget
                            .try_reserve(budget.limit_bytes() - budget.used_bytes())
                            .unwrap(),
                    );
                }
                true
            },
            &mut stage,
        );
        assert!(matches!(
            result,
            Err(InputImageCaptureError::DescriptorBudget(_))
        ));
        assert!(
            pressure.borrow().is_some(),
            "pressure must reach the last pre-promotion activity check"
        );
        assert!(stage.test_is_partial());
        assert!(
            path.join(if regular_root { "leaf" } else { "value" })
                .exists()
        );
        assert_eq!(
            std::fs::read(path.join(if regular_root { "leaf" } else { "value" })).unwrap(),
            b"owned"
        );
        assert_eq!(resources.logical_bytes_used(), 5);
        drop(pressure);
        let held_charge = budget.used_bytes();
        assert!(held_charge > baseline);
        let retry_calls = Cell::new(0);
        assert!(matches!(
            capture(
                AuthorizedObservedInputImage::new(source(temporary.path()), certificate),
                &resources,
                &|| {
                    retry_calls.set(retry_calls.get() + 1);
                    true
                },
                &mut stage,
            ),
            Err(InputImageCaptureError::Unauthorized)
        ));
        assert_eq!(retry_calls.get(), 0);
        assert_eq!(budget.used_bytes(), held_charge);
        drop(stage);
        assert!(!path.exists());
        assert_eq!(resources.logical_bytes_used(), 0);
        // The consumed certificate has released its descriptors as well.
        assert!(budget.used_bytes() <= baseline);
    }
}
