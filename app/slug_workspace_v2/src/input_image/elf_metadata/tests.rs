use dice::DetectCycles;
use dice::Dice;
use dice::UserComputationData;

use super::*;
use crate::FileContentDigest;
use crate::ImageObservationResult;
use crate::InputImagePolicy;
use crate::NormalizedAbsolutePath;
use crate::PathDirectoryEntries;
use crate::PathDirectoryEntry;
use crate::PathDirectoryEntryKind;
use crate::PathDirectoryName;
use crate::PathLstat;
use crate::PathNodeKind;
use crate::PathObservationEpochKey;
use crate::PathObservationNamespace;

fn path(value: &str, budget: &ImageDescriptorBudget) -> ImageChargedPath {
    ImageChargedPath::pin_existing(NormalizedAbsolutePath::new(value).unwrap(), budget).unwrap()
}
fn key(budget: &ImageDescriptorBudget) -> ObservedElfMetadataKey {
    let root = path("/image", budget);
    ObservedElfMetadataKey::from_charged(
        ObservedInputImageKey::from_charged(
            PathObservationNamespace::Host,
            root.dupe(),
            root,
            InputImagePolicy::plain(),
        )
        .unwrap(),
        path("/image/tool", budget),
        ElfMetadataPolicy::default(),
    )
    .unwrap()
}
fn user_data(budget: &ImageDescriptorBudget) -> UserComputationData {
    let mut data = UserComputationData::default();
    data.data.set(budget.dupe());
    data
}
fn facts() -> ElfMetadataFacts {
    ElfMetadataFacts {
        header: ElfHeaderFacts {
            object_type: 3,
            machine: 62,
            os_abi: 0,
            abi_version: 0,
            flags: 0,
            entry: 0,
        },
        program_headers: Vec::new(),
        interpreter: None,
        dynamic_entries: vec![
            ElfDynamicEntry { tag: 1, value: 0 },
            ElfDynamicEntry { tag: 1, value: 2 },
            ElfDynamicEntry { tag: 29, value: 4 },
            ElfDynamicEntry { tag: 0, value: 0 },
        ],
        string_refs: vec![
            ElfStringReference {
                dynamic_index: 0,
                tag: 1,
                bytes: b"a".to_vec(),
            },
            ElfStringReference {
                dynamic_index: 1,
                tag: 1,
                bytes: b"a".to_vec(),
            },
            ElfStringReference {
                dynamic_index: 2,
                tag: 29,
                bytes: Vec::new(),
            },
        ],
    }
}
fn digest(revision: u8) -> FileContentDigest {
    FileContentDigest::new([revision; 32], 64).unwrap()
}
fn verified(
    key: &ObservedElfMetadataKey,
    budget: &ImageDescriptorBudget,
    revision: u8,
    outcome: ElfMetadataOutcome,
) -> VerifiedElfMetadata {
    VerifiedElfMetadata::new(
        ElfSourceDigestBinding::from_digest(digest(revision)),
        key.policy(),
        outcome,
        budget.try_reserve(4096).unwrap(),
    )
    .unwrap()
}
fn row(
    key: &ObservedElfMetadataKey,
    path: ImageChargedPath,
    operation: ImageObservationOperation,
    data: ImageObservationData,
    budget: &ImageDescriptorBudget,
) -> (PathObservationDemand, PathObservationResult) {
    let request = ImageObservationRequest::new(key.image.anchor_root.dupe(), operation);
    let demand =
        PathObservationDemand::input_image(key.image.namespace(), path.dupe(), request.dupe())
            .unwrap();
    let result =
        ImageObservationResult::new(path, request, data, budget.try_reserve(1024).unwrap())
            .unwrap();
    (demand, PathObservationResult::InputImage(result))
}
fn source_rows(
    key: &ObservedElfMetadataKey,
    budget: &ImageDescriptorBudget,
    freshness: i64,
    revision: u8,
    mode: u32,
) -> Vec<(PathObservationDemand, PathObservationResult)> {
    let limits = key.image.policy().limits();
    vec![
        row(
            key,
            key.image.source_root.dupe(),
            ImageObservationOperation::Lstat,
            ImageObservationData::Lstat(PathOperationResult::Present(PathLstat::new(
                PathNodeKind::Directory,
                0,
                freshness,
                freshness,
                freshness,
                0o755,
            ))),
            budget,
        ),
        row(
            key,
            key.image.source_root.dupe(),
            ImageObservationOperation::BoundedDirectory(crate::DirectoryObservationLimits {
                max_entries: limits.max_entries,
                max_name_bytes_total: limits.max_descriptor_bytes,
                max_component_bytes: limits.max_path_bytes,
            }),
            ImageObservationData::Directory(PathOperationResult::Present(
                PathDirectoryEntries::new([PathDirectoryEntry::new(
                    PathDirectoryName::new("tool").unwrap(),
                    PathDirectoryEntryKind::File,
                )]),
            )),
            budget,
        ),
        row(
            key,
            key.file.dupe(),
            ImageObservationOperation::Lstat,
            ImageObservationData::Lstat(PathOperationResult::Present(PathLstat::new(
                PathNodeKind::RegularFile,
                64,
                freshness,
                freshness,
                freshness,
                mode as i32,
            ))),
            budget,
        ),
        row(
            key,
            key.file.dupe(),
            ImageObservationOperation::BoundedDigest {
                max_bytes: limits.max_file_bytes,
            },
            ImageObservationData::Digest(PathOperationResult::Present(digest(revision))),
            budget,
        ),
    ]
}
fn metadata_row(
    key: &ObservedElfMetadataKey,
    budget: &ImageDescriptorBudget,
    revision: u8,
    outcome: ElfMetadataOutcome,
) -> (PathObservationDemand, PathObservationResult) {
    row(
        key,
        key.file.dupe(),
        ImageObservationOperation::BoundedElfMetadata {
            expected_digest: ElfSourceDigestBinding::from_digest(digest(revision)),
            policy: key.policy(),
        },
        ImageObservationData::ElfMetadata(PathOperationResult::Present(verified(
            key, budget, revision, outcome,
        ))),
        budget,
    )
}
fn epoch(
    key: &ObservedElfMetadataKey,
    budget: &ImageDescriptorBudget,
    freshness: i64,
    revision: u8,
    mode: u32,
) -> PathObservationEpoch {
    let mut rows = source_rows(key, budget, freshness, revision, mode);
    rows.push(metadata_row(
        key,
        budget,
        revision,
        ElfMetadataOutcome::Parsed(facts()),
    ));
    PathObservationEpoch::new(rows).unwrap()
}
fn complete(value: ElfValue) -> ObservedElfMetadata {
    match value {
        PathOutcome::Complete(Ok(value)) => value,
        other => panic!("expected completed ELF metadata: {other:?}"),
    }
}

#[tokio::test]
async fn normal_image_need_then_exact_metadata_need_replays_complete_frontier() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let key = key(&budget);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = dice.updater_with_data(user_data(&budget)).commit().await;
    let first = tx.compute(&key).await.unwrap();
    assert!(matches!(&first, PathOutcome::Need(_)));
    assert!(!ObservedElfMetadataKey::validity(&first));
    let mut updater = tx.into_updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new(source_rows(&key, &budget, 1, 1, 0o755)).unwrap(),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    let need = tx.compute(&key).await.unwrap();
    let PathOutcome::Need(need) = need else {
        panic!("expected scoped metadata Need")
    };
    assert_eq!(need.demands().len(), 1);
    let demand = &need.demands()[0];
    assert_eq!(demand.path(), key.file.normalized_ref());
    assert_eq!(
        demand.operation(),
        crate::PathObservationOperation::InputImage(ImageObservationRequest::new(
            key.image.anchor_root.dupe(),
            ImageObservationOperation::BoundedElfMetadata {
                expected_digest: ElfSourceDigestBinding::from_digest(digest(1)),
                policy: key.policy()
            }
        ))
    );
    assert!(demand.image_path().unwrap().same_budget(&budget));
    let mut updater = tx.into_updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            epoch(&key, &budget, 1, 1, 0o755),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    let value = complete(tx.compute(&key).await.unwrap());
    assert_eq!(value.observations().observations().len(), 5);
    assert!(value.same_budget(&budget));
    let ElfMetadataOutcome::Parsed(facts) = value.verified().outcome() else {
        panic!()
    };
    assert_eq!(
        facts.string_refs().len(),
        3,
        "duplicates and empty strings are exact facts"
    );
    assert_eq!(facts.string_refs()[2].bytes(), b"");
}

#[tokio::test]
async fn freshness_content_mode_digest_and_restoration_are_distinct() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let key = key(&budget);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(user_data(&budget));
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            epoch(&key, &budget, 1, 1, 0o755),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    let a = complete(tx.compute(&key).await.unwrap());
    let mut updater = tx.into_updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            epoch(&key, &budget, 2, 1, 0o755),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    let fresh = complete(tx.compute(&key).await.unwrap());
    assert_ne!(a, fresh);
    assert_eq!(a.content(), fresh.content());
    let mut updater = tx.into_updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            epoch(&key, &budget, 3, 2, 0o4755),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    let changed = complete(tx.compute(&key).await.unwrap());
    assert_ne!(a.content(), changed.content());
    let mut updater = tx.into_updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            epoch(&key, &budget, 1, 1, 0o755),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    assert_eq!(a, complete(tx.compute(&key).await.unwrap()));
}

#[tokio::test]
async fn selection_and_operational_failure_have_precise_validity() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let key = key(&budget);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(user_data(&budget));
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            epoch(&key, &budget, 1, 1, 0o755),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    for (file, error) in [
        (
            path("/image/missing", &budget),
            ElfMetadataError::MissingMember,
        ),
        (
            key.image.source_root.dupe(),
            ElfMetadataError::NonRegularMember,
        ),
    ] {
        let selected =
            ObservedElfMetadataKey::from_charged(key.image.dupe(), file, key.policy()).unwrap();
        let value = tx.compute(&selected).await.unwrap();
        assert!(
            matches!(&value, PathOutcome::Complete(Err(actual)) if *actual == error),
            "{value:?}"
        );
        assert!(ObservedElfMetadataKey::validity(&value));
    }
    let mut rows = source_rows(&key, &budget, 1, 1, 0o755);
    rows.push(row(
        &key,
        key.file.dupe(),
        ImageObservationOperation::BoundedElfMetadata {
            expected_digest: ElfSourceDigestBinding::from_digest(digest(1)),
            policy: key.policy(),
        },
        ImageObservationData::ElfMetadata(PathOperationResult::Missing),
        &budget,
    ));
    let mut updater = tx.into_updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new(rows).unwrap(),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    let stale = tx.compute(&key).await.unwrap();
    assert!(
        matches!(
            &stale,
            PathOutcome::Complete(Err(ElfMetadataError::StaleSource))
        ),
        "{stale:?}"
    );
    assert!(!ObservedElfMetadataKey::validity(&stale));
    assert!(!ObservedElfMetadataKey::equality(&stale, &stale));
}

#[tokio::test]
async fn authenticated_rejection_is_a_valid_source_bound_fact() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let key = key(&budget);
    let mut rows = source_rows(&key, &budget, 1, 1, 0o755);
    let rejection = ElfMetadataRejection::Malformed(ElfMalformedReason::Header);
    rows.push(metadata_row(
        &key,
        &budget,
        1,
        ElfMetadataOutcome::Rejected(rejection),
    ));
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(user_data(&budget));
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new(rows).unwrap(),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    let value = tx.compute(&key).await.unwrap();
    assert!(ObservedElfMetadataKey::validity(&value));
    assert!(
        matches!(complete(value).verified().outcome(), ElfMetadataOutcome::Rejected(value) if *value == rejection)
    );
}

#[test]
fn wire_bounds_and_order_are_semantic_but_capacity_is_physical() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let key = key(&budget);
    let base = verified(&key, &budget, 1, ElfMetadataOutcome::Parsed(facts()));
    let mut reordered = facts();
    reordered.dynamic_entries[1].value = 3;
    assert_ne!(
        base,
        verified(&key, &budget, 1, ElfMetadataOutcome::Parsed(reordered))
    );
    let mut roomy = facts();
    roomy.string_refs[0].bytes.reserve(10_000);
    let before = budget.used_bytes();
    let roomy = verified(&key, &budget, 1, ElfMetadataOutcome::Parsed(roomy));
    assert_eq!(base, roomy, "spare capacity never changes semantic facts");
    assert!(
        budget.used_bytes() - before > 10_000,
        "physical pin charges capacity"
    );
    for mutate in 0..5 {
        let mut facts = facts();
        match mutate {
            0 => facts.string_refs[0].bytes = vec![b'x'; 4096], // +NUL is over
            1 => facts.string_refs[0].bytes = vec![0],
            2 => facts.string_refs[1].dynamic_index = 0,
            3 => facts.string_refs[0].tag = 29,
            _ => facts.header.machine = 183,
        }
        assert!(
            VerifiedElfMetadata::new(
                ElfSourceDigestBinding::from_digest(digest(1)),
                key.policy(),
                ElfMetadataOutcome::Parsed(facts),
                budget.try_reserve(0).unwrap()
            )
            .is_err()
        );
    }
    let mut exact = facts();
    exact.string_refs[0].bytes = vec![b'x'; 4095];
    assert!(
        VerifiedElfMetadata::new(
            ElfSourceDigestBinding::from_digest(digest(1)),
            key.policy(),
            ElfMetadataOutcome::Parsed(exact),
            budget.try_reserve(0).unwrap()
        )
        .is_ok()
    );
    for (programs, dynamic) in [(1, 8192), (1024, 1)] {
        let mut limits = ElfMetadataLimits::default();
        limits.max_program_headers = programs;
        limits.max_dynamic_entries = dynamic;
        let policy = ElfMetadataPolicy::new(key.policy().profile(), limits).unwrap();
        let mut over = facts();
        if programs == 1 {
            over.program_headers = vec![
                ElfProgramHeader {
                    segment_type: 1,
                    flags: 0,
                    offset: 0,
                    virtual_address: 0,
                    physical_address: 0,
                    file_size: 0,
                    memory_size: 0,
                    alignment: 1
                };
                2
            ];
        }
        assert!(
            VerifiedElfMetadata::new(
                ElfSourceDigestBinding::from_digest(digest(1)),
                policy,
                ElfMetadataOutcome::Parsed(over),
                budget.try_reserve(0).unwrap()
            )
            .is_err()
        );
    }
    let mut limits = ElfMetadataLimits::default();
    limits.max_fact_bytes = facts().logical_bytes().unwrap();
    let exact = ElfMetadataPolicy::new(key.policy().profile(), limits).unwrap();
    assert!(
        VerifiedElfMetadata::new(
            ElfSourceDigestBinding::from_digest(digest(1)),
            exact,
            ElfMetadataOutcome::Parsed(facts()),
            budget.try_reserve(0).unwrap()
        )
        .is_ok()
    );
    limits.max_fact_bytes -= 1;
    let bounded = ElfMetadataPolicy::new(key.policy().profile(), limits).unwrap();
    assert!(
        VerifiedElfMetadata::new(
            ElfSourceDigestBinding::from_digest(digest(1)),
            bounded,
            ElfMetadataOutcome::Parsed(facts()),
            budget.try_reserve(0).unwrap()
        )
        .is_err()
    );
}

#[test]
fn request_binding_policy_budget_and_issued_path_cannot_be_substituted() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let foreign = ImageDescriptorBudget::new(1 << 20);
    let key = key(&budget);
    let operation = ImageObservationOperation::BoundedElfMetadata {
        expected_digest: ElfSourceDigestBinding::from_digest(digest(1)),
        policy: key.policy(),
    };
    let request = ImageObservationRequest::new(key.image.anchor_root.dupe(), operation);
    for value in [
        verified(&key, &budget, 2, ElfMetadataOutcome::Parsed(facts())),
        verified(&key, &foreign, 1, ElfMetadataOutcome::Parsed(facts())),
    ] {
        assert!(
            ImageObservationResult::new(
                key.file.dupe(),
                request.dupe(),
                ImageObservationData::ElfMetadata(PathOperationResult::Present(value)),
                budget.try_reserve(0).unwrap()
            )
            .is_err()
        );
    }
    let result = ImageObservationResult::new(
        path("/image/other", &budget),
        request.dupe(),
        ImageObservationData::ElfMetadata(PathOperationResult::Present(verified(
            &key,
            &budget,
            1,
            ElfMetadataOutcome::Parsed(facts()),
        ))),
        budget.try_reserve(0).unwrap(),
    )
    .unwrap();
    let demand =
        PathObservationDemand::input_image(key.image.namespace(), key.file.dupe(), request)
            .unwrap();
    assert!(
        PathObservationEpoch::new([(demand, PathObservationResult::InputImage(result))]).is_err()
    );
    assert!(
        ObservedElfMetadataKey::from_charged(
            key.image.dupe(),
            path("/other/tool", &budget),
            key.policy()
        )
        .is_err()
    );
    assert!(
        ObservedElfMetadataKey::from_charged(
            key.image.dupe(),
            path("/image/tool", &foreign),
            key.policy()
        )
        .is_err()
    );
    let mut limits = ElfMetadataLimits::default();
    limits.max_proof_spans += 1;
    assert_eq!(
        ElfMetadataPolicy::new(key.policy().profile(), limits),
        Err(ElfMetadataError::InvalidPolicy)
    );
}

#[tokio::test]
async fn foreign_cold_and_warm_result_consumption_are_guarded() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let foreign = ImageDescriptorBudget::new(1 << 20);
    let key = key(&budget);
    let cold = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = cold.updater_with_data(user_data(&foreign)).commit().await;
    let value = tx.compute(&key).await.unwrap();
    assert!(
        matches!(
            &value,
            PathOutcome::Complete(Err(ElfMetadataError::Budget(
                ImageDescriptorBudgetError::ForeignBudget
            )))
        ),
        "{value:?}"
    );
    assert!(!ObservedElfMetadataKey::validity(&value));
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(user_data(&budget));
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            epoch(&key, &budget, 1, 1, 0o755),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    drop(complete(tx.compute(&key).await.unwrap()));
    drop(tx);
    let mut tx = dice.updater_with_data(user_data(&foreign)).commit().await;
    let warm = complete(tx.compute(&key).await.unwrap());
    assert!(warm.same_budget(&budget));
    assert!(
        !warm.same_budget(&foreign),
        "userdata cannot change warm value ownership"
    );
}

#[tokio::test]
async fn need_and_historical_facts_keep_credit_until_actual_last_drop() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let key = key(&budget);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(user_data(&budget));
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new(source_rows(&key, &budget, 1, 1, 0o755)).unwrap(),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    let PathOutcome::Need(need) = tx.compute(&key).await.unwrap() else {
        panic!()
    };
    let mut updater = tx.into_updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            epoch(&key, &budget, 1, 1, 0o755),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    drop(complete(tx.compute(&key).await.unwrap()));
    let mut updater = tx.into_updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::empty(),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    let held = budget.used_bytes();
    let pressure = budget.try_reserve(budget.limit_bytes() - held).unwrap();
    let mut limits = ElfMetadataLimits::default();
    limits.max_string_bytes -= 1;
    let changed = ObservedElfMetadataKey::from_charged(
        key.image.dupe(),
        key.file.dupe(),
        ElfMetadataPolicy::new(key.policy().profile(), limits).unwrap(),
    )
    .unwrap();
    let value = tx.compute(&changed).await.unwrap();
    assert!(!ObservedElfMetadataKey::validity(&value));
    drop(pressure);
    drop(value);
    drop(changed);
    drop(key);
    drop(tx);
    dice.wait_for_idle().await;
    assert!(budget.used_bytes() > 0);
    drop(dice);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while budget.used_bytes() > 4096 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(
        budget.used_bytes() > 0,
        "returned Need independently keeps descriptor and path credit"
    );
    drop(need);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while budget.used_bytes() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("all history and returned Need owners retired");
}

#[test]
fn key_identity_keeps_namespace_selector_and_complete_policy() {
    use std::hash::Hash;
    use std::hash::Hasher;
    fn hash(key: &ObservedElfMetadataKey) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut h);
        h.finish()
    }
    let budget = ImageDescriptorBudget::new(1 << 20);
    let base = key(&budget);
    assert_eq!(hash(&base), hash(&base.dupe()));
    let mut limits = ElfMetadataLimits::default();
    limits.max_proof_bytes -= 1;
    let policy = ObservedElfMetadataKey::from_charged(
        base.image.dupe(),
        base.file.dupe(),
        ElfMetadataPolicy::new(base.policy().profile(), limits).unwrap(),
    )
    .unwrap();
    let selector = ObservedElfMetadataKey::from_charged(
        base.image.dupe(),
        path("/image/other", &budget),
        base.policy(),
    )
    .unwrap();
    assert_ne!(base, policy);
    assert_ne!(base, selector);
    assert_ne!(hash(&base), hash(&policy));
    assert_ne!(hash(&base), hash(&selector));
    // Equal spelling and scope under another physical service remains semantic
    // equality; consumption, not key hashing, enforces actual budget ownership.
    let foreign = ImageDescriptorBudget::new(1 << 20);
    assert_eq!(base, key(&foreign));
}
