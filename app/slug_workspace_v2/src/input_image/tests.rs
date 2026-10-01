use dice::DetectCycles;
use dice::Dice;
use dice::UserComputationData;

use super::*;
use crate::PathDirectoryEntries;
use crate::PathDirectoryEntry;
use crate::PathDirectoryEntryKind;
use crate::PathDirectoryName;
use crate::PathObservationEpochKey;

fn path(value: &str, budget: &ImageDescriptorBudget) -> ImageChargedPath {
    ImageChargedPath::pin_existing(NormalizedAbsolutePath::new(value).unwrap(), budget).unwrap()
}

fn empty_epoch(
    key: &ObservedInputImageKey,
    budget: &ImageDescriptorBudget,
    freshness: i64,
) -> PathObservationEpoch {
    let mut rows = Vec::new();
    for (operation, data) in [
        (
            ImageObservationOperation::Lstat,
            ImageObservationData::Lstat(PathOperationResult::Present(PathLstat::new(
                PathNodeKind::Directory,
                0,
                freshness,
                freshness,
                freshness,
                0o1755,
            ))),
        ),
        (
            ImageObservationOperation::BoundedDirectory(DirectoryObservationLimits {
                max_entries: key.policy.limits.max_entries,
                max_name_bytes_total: key.policy.limits.max_descriptor_bytes,
                max_component_bytes: key.policy.limits.max_path_bytes,
            }),
            ImageObservationData::Directory(PathOperationResult::Present(
                PathDirectoryEntries::new([]),
            )),
        ),
    ] {
        let request = ImageObservationRequest::new(key.anchor_root.dupe(), operation);
        let demand = PathObservationDemand::input_image(
            key.namespace,
            key.source_root.dupe(),
            request.dupe(),
        )
        .unwrap();
        let result = ImageObservationResult::new(
            key.source_root.dupe(),
            request,
            data,
            budget.try_reserve(256).unwrap(),
        )
        .unwrap();
        rows.push((demand, PathObservationResult::InputImage(result)));
    }
    PathObservationEpoch::new(rows).unwrap()
}

fn user_data(budget: &ImageDescriptorBudget) -> UserComputationData {
    let mut data = UserComputationData::default();
    data.data.set(budget.dupe());
    data
}

fn image(value: ImageValue) -> ObservedInputImage {
    match value {
        PathOutcome::Complete(Ok(image)) => image,
        other => panic!("expected complete image, got {other:?}"),
    }
}

#[tokio::test]
async fn metadata_freshness_changes_wrapper_without_changing_content_and_restores() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let root = path("/image", &budget);
    let key = ObservedInputImageKey::from_charged(
        PathObservationNamespace::Host,
        root.dupe(),
        root,
        InputImagePolicy::plain(),
    )
    .unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(user_data(&budget));
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            empty_epoch(&key, &budget, 1),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    let a = image(tx.compute(&key).await.unwrap());
    let mut updater = tx.into_updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            empty_epoch(&key, &budget, 2),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    let b = image(tx.compute(&key).await.unwrap());
    assert_ne!(a, b, "fresh observation frontier must replace old wrapper");
    assert_eq!(
        a.content(),
        b.content(),
        "metadata freshness is not content identity"
    );
    let mut updater = tx.into_updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            empty_epoch(&key, &budget, 1),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    let restored = image(tx.compute(&key).await.unwrap());
    assert_eq!(a, restored);
}

#[tokio::test]
async fn historical_engine_ownership_keeps_credits_and_exhaustion_is_invalid() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let root = path("/image", &budget);
    let key = ObservedInputImageKey::from_charged(
        PathObservationNamespace::Host,
        root.dupe(),
        root,
        InputImagePolicy::plain(),
    )
    .unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(user_data(&budget));
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            empty_epoch(&key, &budget, 1),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    drop(image(tx.compute(&key).await.unwrap()));
    let mut updater = tx.into_updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::empty(),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    let historical_charge = budget.used_bytes();
    assert!(
        historical_charge > 0,
        "dropping a local image does not retire historical engine owners"
    );
    let pressure = budget
        .try_reserve(budget.limit_bytes() - historical_charge)
        .unwrap();
    let mut limits = InputImageLimits::default();
    limits.max_entries = 1;
    let different = ObservedInputImageKey::from_charged(
        key.namespace,
        key.anchor_root.dupe(),
        key.source_root.dupe(),
        InputImagePolicy::new(InputImageAliasPolicy::Deny, limits).unwrap(),
    )
    .unwrap();
    let exhausted = tx.compute(&different).await.unwrap();
    assert!(matches!(
        &exhausted,
        PathOutcome::Complete(Err(InputImageError::Budget(
            ImageDescriptorBudgetError::ResourceLimit
        )))
    ));
    assert!(!ObservedInputImageKey::validity(&exhausted));
    assert!(!ObservedInputImageKey::equality(&exhausted, &exhausted));
    drop(pressure);
    drop(exhausted);
    drop(different);
    drop(key);
    drop(tx);
    dice.wait_for_idle().await;
    assert!(
        budget.used_bytes() > 0,
        "engine remains a historical descriptor owner"
    );
    drop(dice);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while budget.used_bytes() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("all charged descriptors must retire after the actual last engine owner drops");
}

#[tokio::test]
async fn foreign_budget_cold_compute_and_warm_consumption_are_rejected() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let foreign = ImageDescriptorBudget::new(1 << 20);
    let root = path("/image", &budget);
    let key = ObservedInputImageKey::from_charged(
        PathObservationNamespace::Host,
        root.dupe(),
        root,
        InputImagePolicy::plain(),
    )
    .unwrap();
    let cold = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = cold.updater_with_data(user_data(&foreign)).commit().await;
    let invalid = tx.compute(&key).await.unwrap();
    assert!(matches!(
        &invalid,
        PathOutcome::Complete(Err(InputImageError::Budget(
            ImageDescriptorBudgetError::ForeignBudget
        )))
    ));
    assert!(!ObservedInputImageKey::validity(&invalid));
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(user_data(&budget));
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            empty_epoch(&key, &budget, 1),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    drop(image(tx.compute(&key).await.unwrap()));
    drop(tx);
    let mut tx = dice.updater_with_data(user_data(&foreign)).commit().await;
    // Per-request userdata is not key identity. A cached same-version value
    // can bypass compute's budget check; consumption must validate its owner.
    let warm = image(tx.compute(&key).await.unwrap());
    assert!(warm.same_budget(&budget));
    assert!(!warm.same_budget(&foreign));
}

#[test]
fn shared_descriptor_credit_retires_only_at_last_drop() {
    let budget = ImageDescriptorBudget::new(8192);
    let block = budget
        .try_reserve(100)
        .unwrap()
        .freeze(vec![1u8; 100])
        .unwrap();
    let clone = block.dupe();
    let charged = budget.used_bytes();
    assert!(charged >= 100);
    drop(block);
    assert_eq!(budget.used_bytes(), charged);
    drop(clone);
    assert_eq!(budget.used_bytes(), 0);
    assert!(matches!(
        budget.try_reserve(8193),
        Err(ImageDescriptorBudgetError::ResourceLimit)
    ));
    assert_eq!(budget.used_bytes(), 0);
}

#[test]
fn selector_pin_counts_retained_path_capacity() {
    let budget = ImageDescriptorBudget::new(8192);
    let mut owned = std::path::PathBuf::with_capacity(4096);
    owned.push("/image");
    let normalized = NormalizedAbsolutePath::from_normalized_owned(owned);
    let charged = ImageChargedPath::pin_existing(normalized, &budget).unwrap();
    assert!(
        budget.used_bytes() >= 4096,
        "retained excess capacity is owned storage"
    );
    drop(charged);
    assert_eq!(budget.used_bytes(), 0);
}

#[test]
fn returned_need_retains_full_path_charge_after_original_owner_drops() {
    let budget = ImageDescriptorBudget::new(16 << 10);
    let mut owned = std::path::PathBuf::with_capacity(4096);
    owned.push("/image/file");
    let original = ImageChargedPath::pin_existing(
        NormalizedAbsolutePath::from_normalized_owned(owned),
        &budget,
    )
    .unwrap();
    let anchor = path("/image", &budget);
    let request = ImageObservationRequest::new(anchor.dupe(), ImageObservationOperation::Lstat);
    let returned = original
        .retain_charge(budget.try_reserve(256).unwrap())
        .unwrap();
    let demand =
        PathObservationDemand::input_image(PathObservationNamespace::Host, returned, request)
            .unwrap();
    let need = NeedPathObservations::singleton(demand);
    drop(original);
    drop(anchor);
    assert!(
        budget.used_bytes() >= 4096 + 256,
        "Need owns both the path's full retained capacity and collector storage after original owners retire"
    );
    let last_owner = need.dupe();
    let retained = budget.used_bytes();
    drop(need);
    assert_eq!(budget.used_bytes(), retained);
    drop(last_owner);
    assert_eq!(budget.used_bytes(), 0);
}

#[test]
fn growth_is_geometric_and_charge_survives_need_retirement() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let mut reservation = budget.try_reserve(0).unwrap();
    let mut values = Vec::new();
    for value in 0..1000u64 {
        budget::reserve_vec_slot(&mut reservation, &mut values).unwrap();
        values.push(value);
    }
    assert!(reservation.charged_bytes() < 3 * values.capacity() * std::mem::size_of::<u64>());
    let original = path("/image", &budget);
    let charged = original.retain_charge(reservation).unwrap();
    let request = ImageObservationRequest::new(original.dupe(), ImageObservationOperation::Lstat);
    let demand =
        PathObservationDemand::input_image(PathObservationNamespace::Host, charged, request)
            .unwrap();
    let need = NeedPathObservations::singleton(demand);
    drop(original);
    assert!(budget.used_bytes() >= 1000 * std::mem::size_of::<u64>());
    drop(need);
    assert_eq!(budget.used_bytes(), 0);
}

#[test]
fn semantic_identity_ignores_credit_but_foreign_owner_is_rejected() {
    let first = ImageDescriptorBudget::new(8192);
    let second = ImageDescriptorBudget::new(8192);
    let a = path("/image", &first);
    let b = path("/image", &second);
    assert_eq!(a, b);
    assert!(matches!(
        ObservedInputImageKey::from_charged(
            PathObservationNamespace::Host,
            a,
            b,
            InputImagePolicy::plain()
        ),
        Err(InputImageError::Budget(
            ImageDescriptorBudgetError::ForeignBudget
        ))
    ));
    let invalid = PathOutcome::Complete(Err(InputImageError::Budget(
        ImageDescriptorBudgetError::ResourceLimit,
    )));
    assert!(!ObservedInputImageKey::validity(&invalid));
    assert!(!ObservedInputImageKey::equality(&invalid, &invalid));
}

#[test]
fn result_constructor_checks_scope_and_actual_bounds() {
    let budget = ImageDescriptorBudget::new(8192);
    let anchor = path("/image", &budget);
    let request = ImageObservationRequest::new(
        anchor.dupe(),
        ImageObservationOperation::BoundedReadLink {
            max_target_bytes: 2,
        },
    );
    assert!(
        ImageObservationResult::new(
            anchor.dupe(),
            request,
            ImageObservationData::ReadLink(PathOperationResult::Present(Arc::new("long".into()))),
            budget.try_reserve(100).unwrap()
        )
        .is_err()
    );
    let request = ImageObservationRequest::new(anchor, ImageObservationOperation::Lstat);
    assert!(
        ImageObservationResult::new(
            path("/outside", &budget),
            request,
            ImageObservationData::Lstat(PathOperationResult::Missing),
            budget.try_reserve(100).unwrap()
        )
        .is_err()
    );
}

#[test]
fn runtime_alias_profile_refuses_parent_components_before_normalization() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let root = path("/image", &budget);
    let key = ObservedInputImageKey::from_charged(
        PathObservationNamespace::Host,
        root.dupe(),
        root,
        InputImagePolicy::runtime_aliases(),
    )
    .unwrap();
    let link = path("/image/link", &budget);
    let request = ImageObservationRequest::new(
        key.anchor_root.dupe(),
        ImageObservationOperation::BoundedReadLink {
            max_target_bytes: 4096,
        },
    );
    let result = ImageObservationResult::new(
        link.dupe(),
        request,
        ImageObservationData::ReadLink(PathOperationResult::Present(Arc::new(
            "dir/x/../../file".into(),
        ))),
        budget.try_reserve(100).unwrap(),
    )
    .unwrap();
    let mut entries = vec![InputImageEntry {
        path: link,
        kind: InputImageEntryKind::Symlink,
        mode: 0o777,
        link: Some(result),
        resolved_alias: None,
    }];
    let mut reservation = budget.try_reserve(0).unwrap();
    assert_eq!(
        resolve_aliases(&mut entries, &key, &mut reservation),
        Err(InputImageError::InvalidAlias)
    );
}

fn alias_catalog(
    key: &ObservedInputImageKey,
    budget: &ImageDescriptorBudget,
    links: &[(&str, &str)],
) -> Vec<InputImageEntry> {
    let mut entries = vec![
        InputImageEntry {
            path: key.source_root.dupe(),
            kind: InputImageEntryKind::Directory,
            mode: 0o755,
            link: None,
            resolved_alias: None,
        },
        InputImageEntry {
            path: path("/image/file", budget),
            kind: InputImageEntryKind::RegularFile(FileContentDigest::new([0; 32], 0).unwrap()),
            mode: 0o644,
            link: None,
            resolved_alias: None,
        },
    ];
    for (name, literal) in links {
        let link = key
            .source_root
            .try_join_name(std::ffi::OsStr::new(name))
            .unwrap();
        let request = ImageObservationRequest::new(
            key.anchor_root.dupe(),
            ImageObservationOperation::BoundedReadLink {
                max_target_bytes: key.policy.limits.max_path_bytes,
            },
        );
        let result = ImageObservationResult::new(
            link.dupe(),
            request,
            ImageObservationData::ReadLink(PathOperationResult::Present(Arc::new(literal.into()))),
            budget.try_reserve(256).unwrap(),
        )
        .unwrap();
        entries.push(InputImageEntry {
            path: link,
            kind: InputImageEntryKind::Symlink,
            mode: 0o777,
            link: Some(result),
            resolved_alias: None,
        });
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    entries
}

#[test]
fn alias_hop_limit_counts_defining_alias_exactly() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let root = path("/image", &budget);
    let mut limits = InputImageLimits::default();
    limits.max_alias_hops = 1;
    let key = ObservedInputImageKey::from_charged(
        PathObservationNamespace::Host,
        root.dupe(),
        root,
        InputImagePolicy::new(InputImageAliasPolicy::RuntimeContained, limits).unwrap(),
    )
    .unwrap();
    let mut exact = alias_catalog(&key, &budget, &[("a", "file")]);
    resolve_aliases(&mut exact, &key, &mut budget.try_reserve(0).unwrap()).unwrap();
    let mut over = alias_catalog(&key, &budget, &[("a", "file"), ("b", "a")]);
    assert_eq!(
        resolve_aliases(&mut over, &key, &mut budget.try_reserve(0).unwrap()),
        Err(InputImageError::LimitExceeded(InputImageLimit::AliasHops))
    );
    limits.max_alias_hops = 2;
    let key = ObservedInputImageKey::from_charged(
        key.namespace,
        key.anchor_root.dupe(),
        key.source_root.dupe(),
        InputImagePolicy::new(InputImageAliasPolicy::RuntimeContained, limits).unwrap(),
    )
    .unwrap();
    resolve_aliases(&mut over, &key, &mut budget.try_reserve(0).unwrap()).unwrap();
    assert!(over.iter().filter(|entry| entry.kind() == InputImageEntryKind::Symlink).all(|entry| entry.resolved_alias_path().unwrap().as_path() == Path::new("/image/file")));
}

#[test]
fn absolute_alias_path_limit_uses_normalized_target_spelling() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let root = path("/image", &budget);
    let mut limits = InputImageLimits::default();
    limits.max_path_bytes = "/image/file".len();
    let key = ObservedInputImageKey::from_charged(
        PathObservationNamespace::Host,
        root.dupe(),
        root,
        InputImagePolicy::new(InputImageAliasPolicy::RuntimeContained, limits).unwrap(),
    )
    .unwrap();
    let mut entries = alias_catalog(&key, &budget, &[("a", "/image/file")]);
    resolve_aliases(&mut entries, &key, &mut budget.try_reserve(0).unwrap()).unwrap();
    // Both literal and resulting spelling are within the profile. The link
    // path plus literal would exceed it and must not be used as identity.
    assert_eq!(
        entries[1].resolved_alias_path().unwrap().as_path(),
        Path::new("/image/file")
    );
}

#[tokio::test]
async fn hundred_file_tree_uses_bounded_batched_needs_and_keeps_modes() {
    let budget = ImageDescriptorBudget::new(64 << 20);
    let root = path("/image", &budget);
    let key = ObservedInputImageKey::from_charged(
        PathObservationNamespace::Host,
        root.dupe(),
        root,
        InputImagePolicy::plain(),
    )
    .unwrap();
    let mut data = UserComputationData::default();
    data.data.set(budget.dupe());
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = dice.updater_with_data(data).commit().await;
    let mut observations = Vec::new();
    let mut rounds = 0;
    let image = loop {
        match transaction.compute(&key).await.unwrap() {
            PathOutcome::Complete(Ok(image)) => break image,
            PathOutcome::Complete(Err(error)) => panic!("image failed: {error}"),
            PathOutcome::Need(need) => {
                rounds += 1;
                assert!(rounds <= 4, "siblings must batch, got round {rounds}");
                assert!(!need.demands().is_empty() && need.demands().len() <= 256);
                for demand in need.demands() {
                    let PathObservationOperation::InputImage(request) = demand.operation() else {
                        panic!("unscoped demand")
                    };
                    let root = demand.path().as_path() == Path::new("/image");
                    let data = match request.operation() {
                        ImageObservationOperation::Lstat => ImageObservationData::Lstat(
                            PathOperationResult::Present(PathLstat::new(
                                if root {
                                    PathNodeKind::Directory
                                } else {
                                    PathNodeKind::RegularFile
                                },
                                0,
                                1,
                                1,
                                1,
                                if root { 0o1755 } else { 0o4755 },
                            )),
                        ),
                        ImageObservationOperation::BoundedDirectory(_) => {
                            ImageObservationData::Directory(PathOperationResult::Present(
                                PathDirectoryEntries::new((0..100).map(|index| {
                                    PathDirectoryEntry::new(
                                        PathDirectoryName::new(format!("file-{index:03}")).unwrap(),
                                        PathDirectoryEntryKind::File,
                                    )
                                })),
                            ))
                        }
                        ImageObservationOperation::BoundedDigest { .. } => {
                            ImageObservationData::Digest(PathOperationResult::Present(
                                FileContentDigest::new([0; 32], 0).unwrap(),
                            ))
                        }
                        other => panic!("unexpected operation {other:?}"),
                    };
                    let result = ImageObservationResult::new(
                        demand.image_path().unwrap().dupe(),
                        request,
                        data,
                        budget.try_reserve(8192).unwrap(),
                    )
                    .unwrap();
                    observations.push((demand.dupe(), PathObservationResult::InputImage(result)));
                }
                let mut updater = transaction.into_updater();
                updater
                    .changed_to(vec![(
                        PathObservationEpochKey,
                        PathObservationEpoch::new(observations.clone()).unwrap(),
                    )])
                    .unwrap();
                transaction = updater.commit().await;
            }
        }
    };
    assert_eq!(rounds, 4);
    assert_eq!(image.entries().len(), 101);
    assert_eq!(image.entries()[0].mode(), 0o1755);
    assert!(
        image.entries()[1..]
            .iter()
            .all(|entry| entry.mode() == 0o4755)
    );
    assert_eq!(image.total_file_bytes(), 0);
    assert!(image.same_budget(&budget));
}

#[tokio::test]
async fn discovered_child_descriptor_limit_precedes_path_allocation() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let root = path("/image", &budget);
    let mut limits = InputImageLimits::default();
    limits.max_descriptor_bytes = 10;
    let key = ObservedInputImageKey::from_charged(
        PathObservationNamespace::Host,
        root.dupe(),
        root,
        InputImagePolicy::new(InputImageAliasPolicy::Deny, limits).unwrap(),
    )
    .unwrap();
    let mut rows = Vec::new();
    for (operation, data) in [
        (
            ImageObservationOperation::Lstat,
            ImageObservationData::Lstat(PathOperationResult::Present(PathLstat::new(
                PathNodeKind::Directory,
                0,
                1,
                1,
                1,
                0o755,
            ))),
        ),
        (
            ImageObservationOperation::BoundedDirectory(DirectoryObservationLimits {
                max_entries: limits.max_entries,
                max_name_bytes_total: limits.max_descriptor_bytes,
                max_component_bytes: limits.max_path_bytes,
            }),
            ImageObservationData::Directory(PathOperationResult::Present(
                PathDirectoryEntries::new([PathDirectoryEntry::new(
                    PathDirectoryName::new("file").unwrap(),
                    PathDirectoryEntryKind::File,
                )]),
            )),
        ),
    ] {
        let request = ImageObservationRequest::new(key.anchor_root.dupe(), operation);
        let demand = PathObservationDemand::input_image(
            key.namespace,
            key.source_root.dupe(),
            request.dupe(),
        )
        .unwrap();
        let result = ImageObservationResult::new(
            key.source_root.dupe(),
            request,
            data,
            budget.try_reserve(512).unwrap(),
        )
        .unwrap();
        rows.push((demand, PathObservationResult::InputImage(result)));
    }
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(user_data(&budget));
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new(rows).unwrap(),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    let charged_before = budget.used_bytes();
    let value = tx.compute(&key).await.unwrap();
    assert!(matches!(
        value,
        PathOutcome::Complete(Err(InputImageError::LimitExceeded(
            InputImageLimit::DescriptorBytes
        )))
    ));
    assert_eq!(
        budget.used_bytes(),
        charged_before,
        "no child path or failed builder allocation may survive this semantic refusal"
    );
}

#[tokio::test]
async fn unfinished_siblings_remain_in_monotonic_discovery_limit() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let root = path("/image", &budget);
    let mut limits = InputImageLimits::default();
    limits.max_entries = 3;
    let key = ObservedInputImageKey::from_charged(
        PathObservationNamespace::Host,
        root.dupe(),
        root,
        InputImagePolicy::new(InputImageAliasPolicy::Deny, limits).unwrap(),
    )
    .unwrap();
    let directory_op = ImageObservationOperation::BoundedDirectory(DirectoryObservationLimits {
        max_entries: limits.max_entries,
        max_name_bytes_total: limits.max_descriptor_bytes,
        max_component_bytes: limits.max_path_bytes,
    });
    let stat = ImageObservationData::Lstat(PathOperationResult::Present(PathLstat::new(
        PathNodeKind::Directory,
        0,
        1,
        1,
        1,
        0o755,
    )));
    let listing = |names: &[&str]| {
        ImageObservationData::Directory(PathOperationResult::Present(PathDirectoryEntries::new(
            names.iter().map(|name| {
                PathDirectoryEntry::new(
                    PathDirectoryName::new(*name).unwrap(),
                    if *name == "b" {
                        PathDirectoryEntryKind::Directory
                    } else {
                        PathDirectoryEntryKind::File
                    },
                )
            }),
        )))
    };
    let mut observations = Vec::new();
    for (spelling, operation, data) in [
        ("/image", ImageObservationOperation::Lstat, stat.dupe()),
        ("/image", directory_op, listing(&["a", "b"])),
        ("/image/b", ImageObservationOperation::Lstat, stat),
        ("/image/b", directory_op, listing(&["c"])),
    ] {
        let charged = path(spelling, &budget);
        let request = ImageObservationRequest::new(key.anchor_root.dupe(), operation);
        let demand =
            PathObservationDemand::input_image(key.namespace, charged.dupe(), request.dupe())
                .unwrap();
        let result =
            ImageObservationResult::new(charged, request, data, budget.try_reserve(512).unwrap())
                .unwrap();
        observations.push((demand, PathObservationResult::InputImage(result)));
    }
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(user_data(&budget));
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new(observations).unwrap(),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    // `a` is popped into Need before known `b` discovers `c`. Root+a+b+c
    // proves four entries even though a's Lstat has not been fulfilled.
    let value = tx.compute(&key).await.unwrap();
    assert!(
        matches!(
            value,
            PathOutcome::Complete(Err(InputImageError::LimitExceeded(
                InputImageLimit::Entries
            )))
        ),
        "known membership must win over an incomplete sibling: {value:?}"
    );
}

#[tokio::test]
async fn ordinary_image_key_refuses_missing_kind_depth_and_aggregate_limits() {
    for (name, root_kind, children, depth, total, expected) in [
        (
            "missing",
            None,
            vec![],
            32,
            2 << 30,
            InputImageError::Missing,
        ),
        (
            "wrong_kind",
            Some(PathNodeKind::RegularFile),
            vec![],
            32,
            2 << 30,
            InputImageError::WrongKind,
        ),
        (
            "special_file",
            Some(PathNodeKind::Directory),
            vec![("special", PathNodeKind::SpecialFile)],
            32,
            2 << 30,
            InputImageError::WrongKind,
        ),
        (
            "depth",
            Some(PathNodeKind::Directory),
            vec![("file", PathNodeKind::RegularFile)],
            0,
            2 << 30,
            InputImageError::LimitExceeded(InputImageLimit::Depth),
        ),
        (
            "aggregate_bytes",
            Some(PathNodeKind::Directory),
            vec![
                ("a", PathNodeKind::RegularFile),
                ("b", PathNodeKind::RegularFile),
            ],
            32,
            1,
            InputImageError::LimitExceeded(InputImageLimit::TotalFileBytes),
        ),
    ] {
        let budget = ImageDescriptorBudget::new(1 << 20);
        let root = path("/image", &budget);
        let mut limits = InputImageLimits::default();
        limits.max_depth = depth;
        limits.max_total_file_bytes = total;
        let key = ObservedInputImageKey::from_charged(
            PathObservationNamespace::Host,
            root.dupe(),
            root,
            InputImagePolicy::new(InputImageAliasPolicy::Deny, limits).unwrap(),
        )
        .unwrap();
        let mut observations = Vec::new();
        let mut add = |charged: ImageChargedPath, operation, data| {
            let request = ImageObservationRequest::new(key.anchor_root.dupe(), operation);
            let demand =
                PathObservationDemand::input_image(key.namespace, charged.dupe(), request.dupe())
                    .unwrap();
            let result = ImageObservationResult::new(
                charged,
                request,
                data,
                budget.try_reserve(1024).unwrap(),
            )
            .unwrap();
            observations.push((demand, PathObservationResult::InputImage(result)));
        };
        let stat = |kind, size| PathLstat::new(kind, size, 1, 1, 1, 0o755);
        add(
            key.source_root.dupe(),
            ImageObservationOperation::Lstat,
            ImageObservationData::Lstat(match root_kind {
                Some(kind) => PathOperationResult::Present(stat(kind, 0)),
                None => PathOperationResult::Missing,
            }),
        );
        if root_kind == Some(PathNodeKind::Directory) {
            add(
                key.source_root.dupe(),
                ImageObservationOperation::BoundedDirectory(DirectoryObservationLimits {
                    max_entries: limits.max_entries,
                    max_name_bytes_total: limits.max_descriptor_bytes,
                    max_component_bytes: limits.max_path_bytes,
                }),
                ImageObservationData::Directory(PathOperationResult::Present(
                    PathDirectoryEntries::new(children.iter().map(|(child, _)| {
                        PathDirectoryEntry::new(
                            PathDirectoryName::new(*child).unwrap(),
                            PathDirectoryEntryKind::File,
                        )
                    })),
                )),
            );
        }
        for (child, kind) in children {
            let charged = key
                .source_root
                .try_join_name(std::ffi::OsStr::new(child))
                .unwrap();
            add(
                charged.dupe(),
                ImageObservationOperation::Lstat,
                ImageObservationData::Lstat(PathOperationResult::Present(stat(kind, 1))),
            );
            if kind == PathNodeKind::RegularFile {
                add(
                    charged,
                    ImageObservationOperation::BoundedDigest {
                        max_bytes: limits.max_file_bytes,
                    },
                    ImageObservationData::Digest(PathOperationResult::Present(
                        FileContentDigest::new([0; 32], 1).unwrap(),
                    )),
                );
            }
        }
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater_with_data(user_data(&budget));
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new(observations).unwrap(),
            )])
            .unwrap();
        let mut tx = updater.commit().await;
        let value = tx.compute(&key).await.unwrap();
        assert!(
            matches!(&value, PathOutcome::Complete(Err(actual)) if *actual == expected),
            "case {name}: expected {expected:?}, got {value:?}"
        );
        assert!(
            ObservedInputImageKey::validity(&value),
            "case {name}: observed deterministic refusal remains a valid fact"
        );
    }
}
