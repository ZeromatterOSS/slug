use super::*;

#[tokio::test]
async fn work_bytes_floor_and_explicit_entry_refusals_precede_new_guards_or_writes() {
    let workspace = crate::scratch();
    let dice = dice::Dice::builder().build(dice::DetectCycles::Enabled);
    let root = NormalizedAbsolutePath::new(workspace.clone()).unwrap();
    let current = host(&workspace).0;
    let (tx, _) = again(
        &dice,
        &workspace,
        &need(&root, b"A", false),
        current.clone(),
    )
    .await;
    drop(tx);
    let SavedInputs { mut prior, .. } = capture(&dice.updater(), &workspace).await.unwrap();
    let request = Arc::new(request(&root, "ordinary+", &[("marker", b"A", false)]));
    let guards = Arc::strong_count(&request);
    let count = std::fs::read_dir(&workspace).unwrap().count();
    for mut budget in [
        Budget {
            records: 65_536,
            bytes: 0,
        },
        Budget {
            records: 0,
            bytes: 64 * 1024 * 1024,
        },
    ] {
        assert!(
            prior
                .as_mut()
                .unwrap()
                .reuse(&request, &current, &mut budget)
                .await
                .is_err()
        );
        assert_eq!(Arc::strong_count(&request), guards);
        assert_eq!(std::fs::read_dir(&workspace).unwrap().count(), count);
    }
    let need = SourcePreparationNeeds::repository(request.as_ref().clone());
    let tx = dice.updater().commit().await;
    let mut entries = Vec::new();
    assert!(
        super::super::super::materialize(
            tx,
            &workspace,
            &root,
            &need,
            &mut entries,
            &mut Budget::default(),
            &mut None,
            u64::MAX,
            &[],
        )
        .await
        .is_err()
    );
    assert!(entries.is_empty());
    assert_eq!(std::fs::read_dir(&workspace).unwrap().count(), count);
    let mut entries =
        crate::local_repository_materialization_entries(&root, &[("ordinary+", "caller")]);
    let original = entries[0].request.clone();
    let tx = dice.updater().commit().await;
    assert!(
        super::super::super::materialize(
            tx,
            &workspace,
            &root,
            &need,
            &mut entries,
            &mut Budget::default(),
            &mut prior,
            0,
            &[original.id.canonical_repo.clone()],
        )
        .await
        .is_err()
    );
    assert!(Arc::ptr_eq(&entries[0].request, &original));
    assert_eq!(std::fs::read_dir(&workspace).unwrap().count(), count);
    drop(prior);
    drop(dice);
    std::fs::remove_dir_all(workspace).unwrap();
}
