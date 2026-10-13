//! Loading replay tests use explicitly supplied semantic records. Native
//! capture/extraction and final publication are separate Core acceptance gates.

use slug_bzlmod_v2::RepositoryFilesystemEpochKey;
use slug_bzlmod_v2::RepositoryFilesystemRecord;
use slug_bzlmod_v2::RepositoryFilesystemResult;

use super::*;

const ARCHIVES: &str = r#"
def write(ctx):
    ctx.file('same', 'before', executable=False)
    first = ctx.download_and_extract(
        url=('https://unrelated.test/first.tar.xz', 'file:///tmp/another.payload'),
        sha256='a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1',
        strip_prefix='some/tree', auth={}, headers={}, rename_files={})
    if first.success != True: fail('first result')
    ctx.file('same', 'middle', executable=True)
    second = ctx.download_and_extract('https://elsewhere.test/input.crate', '', '', 'tar.gz', 'other',
        integrity='sha256-oaGhoaGhoaGhoaGhoaGhoaGhoaGhoaGhoaGhoaGhoaE=')
    ctx.file('after', first.sha256 + second.sha256, executable=False)
    return ctx.repo_metadata(reproducible=True)
independent_repository = repository_rule(implementation=write)
def impl(ctx):
    independent_repository(name='first')
ext = module_extension(implementation=impl)
"#;

pub(super) async fn epoch_transaction(
    transaction: dice::DiceTransaction,
    epoch: RepositoryFilesystemEpoch,
) -> dice::DiceTransaction {
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let mut data = UserComputationData {
        cycle_detector: Some(crate::cycle_detector::bzl_load_cycle_detector()),
        ..Default::default()
    };
    data.data.set(CaptureEvaluationEvents);
    data.data.set(RepositoryHostInputTransaction::new(
        RepositoryEnvironmentSnapshot::empty(),
        RepositoryEnvironmentNameFrontier::empty(),
    ));
    data.data.set(SelectedExtensionDownloadTransaction::empty(
        workspace.dupe(),
    ));
    data.data.set(RepositoryExecuteEpoch::revoked());
    data.data.set(epoch.clone());
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(RepositoryFilesystemEpochKey::new(workspace), epoch)])
        .unwrap();
    updater.commit_with_data(data).await
}

#[tokio::test]
async fn archive_callback_accepts_all_nine_positional_default_arguments() {
    // Bazel 9.2 StarlarkBaseExternalContext:967-994 leaves allow_fail,
    // canonical_id, auth and headers positional-or-named too.
    let source = r#"
def write(ctx):
    ctx.download_and_extract('https://independent.test/input.tar.xz', '',
        'a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1',
        'tar.xz', 'arbitrary/prefix', False, '', {}, {})
independent_repository = repository_rule(implementation=write)
def impl(ctx):
    independent_repository(name='first')
ext = module_extension(implementation=impl)
"#;
    // Opaque cache-partition metadata is retained even without a cache consumer.
    for canonical_id in ["", "independent namespace Ω", "another partition"] {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let actual_source = source.replace(
            "False, '', {}, {}",
            &format!("False, {canonical_id:?}, {{}}, {{}}"),
        );
        let mut transaction = transaction_untracked(&dice, MODULE, &actual_source, true).await;
        let owner = owner(&mut transaction).await;
        let key = HostSelectedRepositoryFileEffectKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            owner,
            0,
        );
        transaction = epoch_transaction(
            transaction,
            RepositoryFilesystemEpoch::active(17, RepositoryFilesystemTranscript::empty()),
        )
        .await;
        let SourcePreparationOutcome::Need(needs) = transaction.compute(&key).await.unwrap() else {
            panic!("valid positional defaults must reach the actual archive boundary")
        };
        assert_eq!(needs.repository_filesystem_operations().len(), 1);
        let request = needs
            .repository_filesystem_operations()
            .values()
            .next()
            .unwrap();
        assert_eq!(request.archive().unwrap().sha256(), [0xa1; 32]);
        assert_eq!(
            request.archive().unwrap().strip_prefix(),
            "arbitrary/prefix"
        );
        // Loading uses the admitted Bazel byte-carrier string representation.
        let expected_id: String = canonical_id.bytes().map(char::from).collect();
        assert_eq!(request.archive().unwrap().canonical_id(), expected_id);
        assert!(request.archive().unwrap().declared_integrity().is_empty());
        assert_eq!(request.id().call_index(), 0);
        assert_eq!(request.prefix().effect_count(), 0);
    }
}

#[tokio::test]
async fn general_archive_calls_suspend_then_replay_exact_order_and_terminal_reference() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut transaction = transaction_untracked(&dice, MODULE, ARCHIVES, true).await;
    let owner = owner(&mut transaction).await;
    let key = HostSelectedRepositoryFileEffectKey::new(
        NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        owner.clone(),
        0,
    );
    let mut transcript = RepositoryFilesystemTranscript::empty();
    for index in 0..2 {
        transaction = epoch_transaction(
            transaction,
            RepositoryFilesystemEpoch::active(23, transcript.clone()),
        )
        .await;
        let SourcePreparationOutcome::Need(needs) = transaction.compute(&key).await.unwrap() else {
            panic!("unverified archive must suspend before callback continuation")
        };
        assert_eq!(needs.repository_filesystem_operations().len(), 1);
        let request = needs
            .repository_filesystem_operations()
            .values()
            .next()
            .unwrap()
            .clone();
        assert_eq!(request.id().call_index(), index);
        assert_eq!(request.prefix().archive_count(), index);
        assert_eq!(request.prefix().effects().len(), index + 1);
        transcript = transcript
            .append(RepositoryFilesystemRecord::new(
                request.clone(),
                RepositoryFilesystemResult::Archive(request.archive().unwrap().sha256()),
            ))
            .unwrap();
    }
    transaction = epoch_transaction(
        transaction,
        RepositoryFilesystemEpoch::active(23, transcript.clone()),
    )
    .await;
    let SourcePreparationOutcome::Complete(result) = transaction.compute(&key).await.unwrap()
    else {
        panic!("complete records must replay")
    };
    let result = result.as_ref().as_ref().unwrap();
    assert_eq!(result.plan().archive_count(), 2);
    assert_eq!(result.plan().effects().len(), 3);
    assert_eq!(result.plan().effects()[0].content(), b"before");
    assert_eq!(result.plan().effects()[1].content(), b"middle");
    assert!(result.plan().effects()[1].executable());
    assert_eq!(
        result.plan().effects()[2].content(),
        "a1".repeat(64).as_bytes()
    );
    assert!(result.metadata().reproducible());
    let realization = result
        .filesystem_realization()
        .expect("terminal archive handoff");
    assert_eq!(realization.lease_nonce(), 23);
    assert_eq!(realization.transcript(), &transcript);
    assert_eq!(realization.terminal_plan(), result.plan());
    let warm = transaction.compute(&key).await.unwrap();
    assert!(HostSelectedRepositoryFileEffectKey::equality(
        &SourcePreparationOutcome::Complete(Arc::new(Ok(result.clone()))),
        &warm
    ));
}

#[tokio::test]
async fn changed_prefix_revoked_epoch_and_swallowed_generated_observation_fail_closed() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut transaction = transaction_untracked(&dice, MODULE, ARCHIVES, true).await;
    let owner = owner(&mut transaction).await;
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let key = HostSelectedRepositoryFileEffectKey::new(workspace.dupe(), owner, 0);
    transaction = epoch_transaction(
        transaction,
        RepositoryFilesystemEpoch::active(31, RepositoryFilesystemTranscript::empty()),
    )
    .await;
    let SourcePreparationOutcome::Need(needs) = transaction.compute(&key).await.unwrap() else {
        panic!("first archive must suspend")
    };
    let request = needs
        .repository_filesystem_operations()
        .values()
        .next()
        .unwrap()
        .clone();
    let transcript = RepositoryFilesystemTranscript::empty()
        .append(RepositoryFilesystemRecord::new(
            request.clone(),
            RepositoryFilesystemResult::Archive(request.archive().unwrap().sha256()),
        ))
        .unwrap();
    for (source, epoch) in [
        (ARCHIVES.replace("'before'", "'changed'"), RepositoryFilesystemEpoch::active(31, transcript.clone())),
        (ARCHIVES.to_owned(), RepositoryFilesystemEpoch::revoked()),
        (ARCHIVES.replace("    ctx.file('same', 'middle', executable=True)", "    hasattr(ctx.path('archive-produced').get_child('unknown'), 'exists')\n    ctx.file('same', 'middle', executable=True)"), RepositoryFilesystemEpoch::active(31, transcript.clone())),
    ] {
        let fresh = transaction_untracked(&dice, MODULE, &source, true).await;
        let mut fresh = epoch_transaction(fresh, epoch).await;
        let SourcePreparationOutcome::Complete(result) = fresh.compute(&key).await.unwrap() else { panic!("invalid replay or observation must fail") };
        assert!(matches!(result.as_ref(), Err(HostSelectedRepositoryFileEffectError::Invocation { .. })));
    }

    let file_only = "def write(ctx):\n    ctx.file('same', 'before', executable=False)\nindependent_repository=repository_rule(implementation=write)\ndef impl(ctx):\n    independent_repository(name='first')\next=module_extension(implementation=impl)\n";
    let fresh = transaction_untracked(&dice, MODULE, file_only, true).await;
    let mut fresh =
        epoch_transaction(fresh, RepositoryFilesystemEpoch::active(31, transcript)).await;
    let SourcePreparationOutcome::Complete(result) = fresh.compute(&key).await.unwrap() else {
        panic!("surplus records must terminate")
    };
    assert!(result.is_err());
    fresh = epoch_transaction(
        fresh,
        RepositoryFilesystemEpoch::active(31, RepositoryFilesystemTranscript::empty()),
    )
    .await;
    let SourcePreparationOutcome::Complete(result) = fresh.compute(&key).await.unwrap() else {
        panic!("cleared surplus records must recover")
    };
    assert!(result.is_ok());
}

#[tokio::test]
async fn repository_metadata_only_change_invalidates_and_restores_without_archive_epoch() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let source = |version: &str| {
        format!(
            "def write(ctx):\n    ctx.file('same', 'content', executable=False)\n    return ctx.repo_metadata(attrs_for_reproducibility={{'version': {version:?}}})\nindependent_repository=repository_rule(implementation=write)\ndef impl(ctx):\n    independent_repository(name='first')\next=module_extension(implementation=impl)\n"
        )
    };
    let mut results = Vec::new();
    for version in ["A", "B", "A"] {
        let mut transaction = transaction_untracked(&dice, MODULE, &source(version), true).await;
        let owner = owner(&mut transaction).await;
        let key = HostSelectedRepositoryFileEffectKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            owner,
            0,
        );
        // No archive epoch is installed: file/metadata-only rules must not
        // acquire an unrelated archive observation dependency.
        let SourcePreparationOutcome::Complete(result) = transaction.compute(&key).await.unwrap()
        else {
            panic!("metadata must complete")
        };
        results.push(result.as_ref().as_ref().unwrap().clone());
    }
    assert_eq!(results[0].plan(), results[1].plan());
    assert_ne!(results[0].metadata(), results[1].metadata());
    assert_eq!(results[0].metadata(), results[2].metadata());
    assert!(
        results
            .iter()
            .all(|result| result.filesystem_realization().is_none())
    );
}
