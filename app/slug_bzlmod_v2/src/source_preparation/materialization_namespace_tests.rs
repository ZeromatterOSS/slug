use dice::DetectCycles;
use dice::Dice;

use super::*;

fn workspace() -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new("/workspace").unwrap()
}

fn request(
    name: &str,
    kind: RepositoryMaterializationKind,
) -> Arc<RepositoryMaterializationRequest> {
    Arc::new(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: workspace(),
            canonical_repo: CanonicalRepoName::new(name).unwrap(),
        },
        repo_spec: crate::RepoSpec {
            rule_id: crate::RepoRuleId {
                bzl_file: slug_identity_v2::CanonicalLabel::parse("@@owner+//private:repo.bzl")
                    .unwrap(),
                rule_name: "repo".into(),
            },
            attributes: Arc::default(),
        },
        kind,
    })
}

fn success(
    request: &Arc<RepositoryMaterializationRequest>,
    instance: u64,
    root: &str,
) -> RepositoryMaterializationEpochEntry {
    RepositoryMaterializationEpochEntry {
        request: request.clone(),
        result: RepositoryMaterializationResult::Success(
            RepositoryMaterializationSuccess::Immutable {
                path_policy: crate::RepositoryMaterializationPathPolicy::LegacyUnbounded,
                source_identity: Arc::from("phase-only-fixture"),
                generation_root: PathBuf::from(root),
                observation_instance: PathObservationInstanceId::new(instance),
            },
        ),
    }
}

#[test]
fn materialization_namespace_lookup_is_bounded_unique_and_borrowed() {
    use RepositoryMaterializationInstanceLookupError as Error;
    let instance = PathObservationInstanceId::new(7);
    let original = request("match+", RepositoryMaterializationKind::Immutable);
    let weak = Arc::downgrade(&original);
    let local = request(
        "local+",
        RepositoryMaterializationKind::Local {
            logical_root: NormalizedAbsolutePath::new("/local").unwrap(),
        },
    );
    let failed = request("error+", RepositoryMaterializationKind::Immutable);
    let generated = request(
        "generated+",
        RepositoryMaterializationKind::GeneratedFileEffects(
            GeneratedRepositoryFileEffectPlan::build([("file".into(), Arc::from(&b"x"[..]), true)])
                .unwrap(),
        ),
    );
    let epoch = RepositoryMaterializationResultEpoch::new(
        workspace(),
        [
            success(&original, 7, "/inert-generation"),
            RepositoryMaterializationEpochEntry {
                request: local,
                result: RepositoryMaterializationResult::Success(
                    RepositoryMaterializationSuccess::Local,
                ),
            },
            RepositoryMaterializationEpochEntry {
                request: failed,
                result: RepositoryMaterializationResult::SpecError("error".into()),
            },
            success(&generated, 9, "/inert-generation"),
        ],
    )
    .unwrap();
    let count = Arc::strong_count(&original);
    assert!(std::ptr::eq(
        epoch
            .borrowed_instance_for_namespace(&workspace(), instance, 4)
            .unwrap()
            .unwrap()
            .request(),
        original.as_ref()
    ));
    assert_eq!(Arc::strong_count(&original), count);
    let view = epoch
        .borrowed_instance_for_namespace(&workspace(), instance, 4)
        .unwrap()
        .unwrap();
    let RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable {
        generation_root,
        ..
    }) = &epoch.get(original.as_ref()).unwrap().result
    else {
        panic!("fixture must hold the original successful entry");
    };
    assert_eq!(view.generation_root(), Path::new("/inert-generation"));
    assert_eq!(
        view.generation_root()
            .as_os_str()
            .as_encoded_bytes()
            .as_ptr(),
        generation_root.as_os_str().as_encoded_bytes().as_ptr()
    );
    assert!(std::ptr::eq(
        epoch
            .borrowed_instance_for_namespace(&workspace(), PathObservationInstanceId::new(9), 4)
            .unwrap()
            .unwrap()
            .request(),
        generated.as_ref()
    ));
    assert!(
        epoch
            .borrowed_instance_for_namespace(&workspace(), PathObservationInstanceId::new(8), 4)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        epoch.borrowed_instance_for_namespace(&workspace(), instance, 3),
        Err(Error::TooManyEntries)
    );
    assert_eq!(
        epoch.borrowed_instance_for_namespace(&workspace(), PathObservationInstanceId::new(0), 0),
        Err(Error::InvalidInstance)
    );
    assert_eq!(
        epoch.borrowed_instance_for_namespace(
            &NormalizedAbsolutePath::new("/other").unwrap(),
            PathObservationInstanceId::new(0),
            0
        ),
        Err(Error::WrongWorkspace)
    );
    let duplicate = request("duplicate+", RepositoryMaterializationKind::Immutable);
    let ambiguous = RepositoryMaterializationResultEpoch::new(
        workspace(),
        [
            success(&original, 7, "/inert-generation"),
            success(&duplicate, 7, "invalid-relative-root"),
        ],
    )
    .unwrap();
    assert_eq!(
        ambiguous.borrowed_instance_for_namespace(&workspace(), instance, 2),
        Err(Error::AmbiguousInstance)
    );
    assert_eq!(
        ambiguous.borrowed_instance_for_namespace(&workspace(), instance, 1),
        Err(Error::TooManyEntries)
    );
    for root in ["relative-root", "/generation/../other", "/../generation"] {
        let invalid =
            RepositoryMaterializationResultEpoch::new(workspace(), [success(&duplicate, 7, root)])
                .unwrap();
        assert_eq!(
            invalid.borrowed_instance_for_namespace(&workspace(), instance, 1),
            Err(Error::InvalidGenerationRoot)
        );
        assert_eq!(
            invalid.borrowed_instance_for_namespace(&workspace(), instance, 0),
            Err(Error::TooManyEntries)
        );
        assert!(
            invalid
                .borrowed_instance_for_namespace(&workspace(), PathObservationInstanceId::new(8), 1)
                .unwrap()
                .is_none()
        );
    }
    let empty = RepositoryMaterializationResultEpoch::new(workspace(), []).unwrap();
    assert!(
        empty
            .borrowed_instance_for_namespace(&workspace(), instance, 0)
            .unwrap()
            .is_none()
    );
    drop(ambiguous);
    drop(epoch);
    drop(original);
    assert!(weak.upgrade().is_none());
}

#[tokio::test]
async fn materialization_namespace_lookup_tracks_same_dice_epoch() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let key = RepositoryMaterializationResultEpochKey {
        workspace: workspace(),
    };
    let mut absent = dice.updater().commit().await;
    assert!(absent.compute(&key).await.is_err());
    let a = request("a+", RepositoryMaterializationKind::Immutable);
    let b = request("b+", RepositoryMaterializationKind::Immutable);
    let weak = Arc::downgrade(&a);
    let epoch_a = RepositoryMaterializationResultEpoch::new(
        workspace(),
        [success(&a, 7, "/inert-generation")],
    )
    .unwrap();
    let epoch_b = RepositoryMaterializationResultEpoch::new(
        workspace(),
        [success(&b, 9, "/inert-other-generation")],
    )
    .unwrap();
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(key.clone(), epoch_a.clone())])
        .unwrap();
    let mut tx_a = updater.commit().await;
    let held = tx_a.compute(&key).await.unwrap();
    let a_count = Arc::strong_count(&a);
    let held_root = held
        .borrowed_instance_for_namespace(&workspace(), PathObservationInstanceId::new(7), 1)
        .unwrap()
        .unwrap()
        .generation_root();
    assert_eq!(held_root, Path::new("/inert-generation"));
    let root_pointer = held_root.as_os_str().as_encoded_bytes().as_ptr();
    assert!(std::ptr::eq(
        held.borrowed_instance_for_namespace(&workspace(), PathObservationInstanceId::new(7), 1)
            .unwrap()
            .unwrap()
            .request(),
        a.as_ref()
    ));
    assert_eq!(Arc::strong_count(&a), a_count);
    let mut updater = dice.updater();
    updater.changed_to(vec![(key.clone(), epoch_b)]).unwrap();
    let mut tx_b = updater.commit().await;
    let current = tx_b.compute(&key).await.unwrap();
    assert!(
        current
            .borrowed_instance_for_namespace(&workspace(), PathObservationInstanceId::new(7), 1)
            .unwrap()
            .is_none()
    );
    assert!(std::ptr::eq(
        current
            .borrowed_instance_for_namespace(&workspace(), PathObservationInstanceId::new(9), 1)
            .unwrap()
            .unwrap()
            .request(),
        b.as_ref()
    ));
    let current_root = current
        .borrowed_instance_for_namespace(&workspace(), PathObservationInstanceId::new(9), 1)
        .unwrap()
        .unwrap()
        .generation_root();
    assert_eq!(current_root, Path::new("/inert-other-generation"));
    assert_ne!(
        current_root.as_os_str().as_encoded_bytes().as_ptr(),
        root_pointer
    );
    assert_eq!(
        held_root.as_os_str().as_encoded_bytes().as_ptr(),
        root_pointer
    );
    assert!(std::ptr::eq(
        held.borrowed_instance_for_namespace(&workspace(), PathObservationInstanceId::new(7), 1)
            .unwrap()
            .unwrap()
            .request(),
        a.as_ref()
    ));
    let mut updater = dice.updater();
    updater.changed_to(vec![(key.clone(), epoch_a)]).unwrap();
    let mut tx_restored = updater.commit().await;
    let restored = tx_restored.compute(&key).await.unwrap();
    let restored_root = restored
        .borrowed_instance_for_namespace(&workspace(), PathObservationInstanceId::new(7), 1)
        .unwrap()
        .unwrap()
        .generation_root();
    assert_eq!(restored_root, Path::new("/inert-generation"));
    assert_eq!(
        restored_root.as_os_str().as_encoded_bytes().as_ptr(),
        root_pointer
    );
    assert!(std::ptr::eq(
        restored
            .borrowed_instance_for_namespace(&workspace(), PathObservationInstanceId::new(7), 1)
            .unwrap()
            .unwrap()
            .request(),
        a.as_ref()
    ));
    assert!(
        restored
            .borrowed_instance_for_namespace(&workspace(), PathObservationInstanceId::new(9), 1)
            .unwrap()
            .is_none()
    );
    drop(a);
    assert!(weak.upgrade().is_some()); // The original epoch remains owned, not copied by lookup.
    drop((absent, tx_a, tx_b, tx_restored, held, current, restored));
    dice.wait_for_idle().await;
    drop(dice);
    // The dedicated DICE actor owns injected history until its last sender
    // closes; transaction/cancellation retirement alone does not drop it.
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while weak.upgrade().is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("original request must retire after bounded DICE actor shutdown");
    assert!(weak.upgrade().is_none());
}
