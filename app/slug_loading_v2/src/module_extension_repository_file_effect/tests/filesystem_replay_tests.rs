//! Explicit semantic records test Loading continuation; Core owns actual I/O.

use slug_bzlmod_v2::RepositoryFilesystemOperation;
use slug_bzlmod_v2::RepositoryFilesystemRecord;
use slug_bzlmod_v2::RepositoryFilesystemResult;
use slug_bzlmod_v2::RepositoryFilesystemTranscriptError;

use super::archive_replay_tests::epoch_transaction;
use super::*;

const DELETE: &str = r#"
def write(ctx):
    ctx.file('dir/tree/child', 'before', executable=False)
    if ctx.delete(ctx.path('dir/tree')) != True: fail('existing directory result')
    if ctx.path('dir/tree/child').exists: fail('known descendant retained')
    if not ctx.path('dir').exists: fail('parent discarded')
    ctx.file('dir/tree/child', 'rewritten', executable=False)
    if ctx.delete('dir/tree/child') != True: fail('existing file result')
    if ctx.delete('absent') != False: fail('absent path result')
    ctx.file('dir/tree/child', 'final', executable=True)
independent_repository = repository_rule(implementation=write)
def impl(ctx):
    independent_repository(name='first')
ext = module_extension(implementation=impl)
"#;

#[tokio::test]
async fn delete_callback_replays_actual_booleans_prunes_known_descendants_and_seals_delete_only() {
    let dice = crate::canonical_repository_route_tests::tests::builtin_graph_dice();
    let mut transaction = transaction_untracked(&dice, MODULE, DELETE, true).await;
    let owner = owner(&mut transaction).await;
    let key = HostSelectedRepositoryFileEffectKey::new(
        NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        owner,
        0,
    );
    let mut transcript = RepositoryFilesystemTranscript::empty();
    for (index, (path, deleted)) in [
        ("dir/tree", true),
        ("dir/tree/child", true),
        ("absent", false),
    ]
    .into_iter()
    .enumerate()
    {
        transaction = epoch_transaction(
            transaction,
            RepositoryFilesystemEpoch::active(71, transcript.clone()),
        )
        .await;
        let SourcePreparationOutcome::Need(needs) = transaction.compute(&key).await.unwrap() else {
            panic!("delete must suspend before the Boolean-dependent continuation")
        };
        assert_eq!(needs.repository_filesystem_operations().len(), 1);
        let request = needs
            .repository_filesystem_operations()
            .values()
            .next()
            .unwrap()
            .clone();
        assert_eq!(request.id().call_index(), index);
        assert_eq!(request.operation().delete_path(), Some(path));
        assert!(request.archive().is_none());
        assert_eq!(request.prefix().filesystem_operation_count(), index);
        assert_eq!(
            request.prefix().effects().len(),
            if index == 0 { 1 } else { 2 }
        );
        transcript = transcript
            .append(RepositoryFilesystemRecord::new(
                request,
                RepositoryFilesystemResult::Delete(deleted),
            ))
            .unwrap();
    }
    transaction = epoch_transaction(
        transaction,
        RepositoryFilesystemEpoch::active(71, transcript.clone()),
    )
    .await;
    let SourcePreparationOutcome::Complete(result) = transaction.compute(&key).await.unwrap()
    else {
        panic!("all operations must replay")
    };
    let result = result.as_ref().as_ref().unwrap();
    assert_eq!(result.plan().archive_count(), 0);
    assert_eq!(result.plan().filesystem_operation_count(), 3);
    assert!(result.plan().has_filesystem_operations());
    assert_eq!(result.plan().effects()[2].content(), b"final");
    assert!(result.plan().effects()[2].executable());
    let realization = result
        .filesystem_realization()
        .expect("delete-only invocation needs a terminal seal");
    assert_eq!(realization.lease_nonce(), 71);
    assert_eq!(realization.transcript(), &transcript);
    assert_eq!(realization.terminal_plan(), result.plan());
}

const MIXED: &str = r#"
def write(ctx):
    ctx.download_and_extract('https://independent.test/input.tar.xz', sha256='a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1')
    if ctx.delete(ctx.path('remote/tree')) != True: fail('delete result')
    ctx.file('suffix', 'after delete', executable=False)
    ctx.download_and_extract('https://other.test/another.tar.gz', sha256='b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2')
independent_repository = repository_rule(implementation=write)
def impl(ctx):
    independent_repository(name='first')
ext = module_extension(implementation=impl)
"#;

#[tokio::test]
async fn archive_delete_archive_share_ordered_prefix_without_generated_read_authority() {
    let dice = crate::canonical_repository_route_tests::tests::builtin_graph_dice();
    let mut transaction = transaction_untracked(&dice, MODULE, MIXED, true).await;
    let owner = owner(&mut transaction).await;
    let key = HostSelectedRepositoryFileEffectKey::new(
        NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        owner,
        0,
    );
    let mut transcript = RepositoryFilesystemTranscript::empty();
    for index in 0..3 {
        transaction = epoch_transaction(
            transaction,
            RepositoryFilesystemEpoch::active(73, transcript.clone()),
        )
        .await;
        let SourcePreparationOutcome::Need(needs) = transaction.compute(&key).await.unwrap() else {
            panic!("unobserved operation must suspend")
        };
        let request = needs
            .repository_filesystem_operations()
            .values()
            .next()
            .unwrap()
            .clone();
        assert_eq!(request.id().call_index(), index);
        assert_eq!(request.prefix().filesystem_operation_count(), index);
        let result = match request.operation() {
            RepositoryFilesystemOperation::Archive(effect) => {
                RepositoryFilesystemResult::Archive(effect.expected_sha256().unwrap())
            }
            RepositoryFilesystemOperation::Delete(path) => {
                assert_eq!(index, 1);
                assert_eq!(path, "remote/tree");
                RepositoryFilesystemResult::Delete(true)
            }
        };
        transcript = transcript
            .append(RepositoryFilesystemRecord::new(request, result))
            .unwrap();
    }
    transaction = epoch_transaction(
        transaction,
        RepositoryFilesystemEpoch::active(73, transcript.clone()),
    )
    .await;
    let SourcePreparationOutcome::Complete(result) = transaction.compute(&key).await.unwrap()
    else {
        panic!("mixed transcript must complete")
    };
    let result = result.as_ref().as_ref().unwrap();
    assert_eq!(result.plan().archive_count(), 2);
    assert_eq!(result.plan().filesystem_operation_count(), 3);
    assert_eq!(result.plan().effects()[0].content(), b"after delete");
    assert_eq!(
        result.filesystem_realization().unwrap().transcript(),
        &transcript
    );

    let source = MIXED.replace(
        "    ctx.file('suffix'",
        "    hasattr(ctx.path('remote/tree'), 'exists')\n    ctx.file('suffix'",
    );
    let fresh = transaction_untracked(&dice, MODULE, &source, true).await;
    let mut fresh =
        epoch_transaction(fresh, RepositoryFilesystemEpoch::active(73, transcript)).await;
    let SourcePreparationOutcome::Complete(result) = fresh.compute(&key).await.unwrap() else {
        panic!("unknown generated read must fail closed")
    };
    assert!(
        result.is_err(),
        "delete must not introduce archive path observation authority"
    );
}

#[tokio::test]
async fn delete_changed_path_prefix_kind_and_revoked_epoch_do_not_resume() {
    let dice = crate::canonical_repository_route_tests::tests::builtin_graph_dice();
    let mut transaction = transaction_untracked(&dice, MODULE, DELETE, true).await;
    let owner = owner(&mut transaction).await;
    let key = HostSelectedRepositoryFileEffectKey::new(
        NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        owner,
        0,
    );
    transaction = epoch_transaction(
        transaction,
        RepositoryFilesystemEpoch::active(79, RepositoryFilesystemTranscript::empty()),
    )
    .await;
    let SourcePreparationOutcome::Need(needs) = transaction.compute(&key).await.unwrap() else {
        panic!("delete must demand observation")
    };
    let request = needs
        .repository_filesystem_operations()
        .values()
        .next()
        .unwrap()
        .clone();
    assert!(matches!(
        RepositoryFilesystemTranscript::empty().append(RepositoryFilesystemRecord::new(
            request.clone(),
            RepositoryFilesystemResult::Archive([0xa1; 32])
        )),
        Err(RepositoryFilesystemTranscriptError::ResultKindMismatch(_))
    ));
    let transcript = RepositoryFilesystemTranscript::empty()
        .append(RepositoryFilesystemRecord::new(
            request,
            RepositoryFilesystemResult::Delete(true),
        ))
        .unwrap();
    for (source, epoch) in [
        (DELETE.replace("ctx.path('dir/tree')", "ctx.path('dir/elsewhere')"), RepositoryFilesystemEpoch::active(79, transcript.clone())),
        (DELETE.replace("'before'", "'changed'"), RepositoryFilesystemEpoch::active(79, transcript.clone())),
        (DELETE.replace("ctx.delete(ctx.path('dir/tree'))", "ctx.download_and_extract('https://unrelated.test/a.tar.xz', sha256='a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1')"), RepositoryFilesystemEpoch::active(79, transcript.clone())),
        (DELETE.to_owned(), RepositoryFilesystemEpoch::revoked()),
    ] {
        let fresh = transaction_untracked(&dice, MODULE, &source, true).await;
        let mut fresh = epoch_transaction(fresh, epoch).await;
        let SourcePreparationOutcome::Complete(result) = fresh.compute(&key).await.unwrap() else { panic!("invalid replay must fail instead of continuing") };
        assert!(matches!(result.as_ref(), Err(HostSelectedRepositoryFileEffectError::Invocation { .. })));
    }
}

#[tokio::test]
async fn disabled_execute_refuses_in_all_filesystem_orders_without_process_need() {
    use slug_bzlmod_v2::RepositoryEnvironmentEntry;

    use crate::repository_rule_context::PreparedRepositoryWhichCandidates;
    use crate::repository_rule_context::RepositoryRuleInvocationInput;
    use crate::repository_rule_context::RepositoryWhichCandidate;
    use crate::repository_rule_context::invoke_repository_rule;

    #[derive(Default)]
    struct Prints(std::cell::RefCell<Vec<String>>);
    impl starlark::PrintHandler for Prints {
        fn println(&self, _: starlark::PrintLocation, text: &str) -> starlark::Result<()> {
            self.0.borrow_mut().push(text.to_owned());
            Ok(())
        }
    }

    let dice = crate::canonical_repository_route_tests::tests::builtin_graph_dice();
    let mut transaction = transaction_untracked(&dice, MODULE, DELETE, true).await;
    let owner = owner(&mut transaction).await;
    let identity = crate::bzl_module::BzlModuleIdentity {
        label: slug_identity_v2::CanonicalLabel::parse("@@//:independent.bzl").unwrap(),
        source: slug_build_api_v2::ModuleSourceOrigin::Workspace(std::path::PathBuf::from(
            "independent.bzl",
        )),
        repository_mapping: Arc::from([]),
    };
    let manifest = crate::bzl_module::BzlLoadManifest {
        root: identity.clone(),
        direct_children: Arc::from([]),
        reachable: [crate::BzlModuleSourceProvenance::new(identity, [0; 32])].into(),
        fingerprint: [0; 32],
    };
    let which = PreparedRepositoryWhichCandidates::from_iter([(
        NormalizedAbsolutePath::new("/bin/tool").unwrap(),
        RepositoryWhichCandidate::Executable,
    )]);
    let snapshot =
        RepositoryEnvironmentSnapshot::from_canonical([RepositoryEnvironmentEntry::new(
            "PATH", "/bin",
        )])
        .unwrap();
    let execute = "ctx.execute([ctx.which('tool')])";
    let delete = "ctx.delete('unrelated/path')";
    let archive = "ctx.download_and_extract('https://independent.test/a.tar.xz', sha256='a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1')";
    for (first, second) in [
        (execute, delete),
        (execute, archive),
        (delete, execute),
        (archive, execute),
    ] {
        let source = format!(
            "def implementation(ctx):\n    {first}\n    {second}\n    print('forbidden continuation')\n"
        );
        let module = starlark::environment::Module::new();
        let ast = starlark::syntax::AstModule::parse(
            "independent.bzl",
            source,
            &starlark::syntax::Dialect::Bazel,
        )
        .unwrap();
        starlark::eval::Evaluator::new(&module)
            .eval_module(
                ast,
                &starlark::environment::Globals::extended_by(&[
                    starlark::environment::LibraryExtension::Print,
                ]),
            )
            .unwrap();
        let module = module.freeze().unwrap();
        let implementation = unsafe {
            module
                .get("implementation")
                .unwrap()
                .unchecked_frozen_value()
        };
        let prints = Prints::default();
        let base = RepositoryRuleInvocationInput::new(
            "repo".into(),
            None,
            Arc::new(SmallMap::new()),
            Arc::from([]),
        )
        .unwrap()
        .with_filesystem_operations(owner.clone(), 0, RepositoryFilesystemTranscript::empty())
        .with_filesystem_lease_nonce(Some(83));
        let invoke = |input| {
            invoke_repository_rule(
                implementation,
                &manifest,
                &SmallMap::new(),
                &SmallMap::new(),
                &which,
                input,
                RepositoryPlatform::new("linux", "x86_64"),
                snapshot.clone(),
                Some(&prints),
            )
        };
        let first_need = invoke(base.clone())
            .err()
            .expect("first call must demand its actual result");
        let input = match first_need {
            RepositoryRuleInvocationError::ExecuteUnsupported => {
                assert_eq!(first, execute);
                assert!(prints.0.borrow().is_empty());
                continue;
            }
            RepositoryRuleInvocationError::FilesystemNeed(request) => {
                let result = match request.operation() {
                    RepositoryFilesystemOperation::Archive(effect) => {
                        RepositoryFilesystemResult::Archive(effect.expected_sha256().unwrap())
                    }
                    RepositoryFilesystemOperation::Delete(_) => {
                        RepositoryFilesystemResult::Delete(false)
                    }
                };
                base.clone().with_filesystem_operations(
                    owner.clone(),
                    0,
                    RepositoryFilesystemTranscript::empty()
                        .append(RepositoryFilesystemRecord::new(request.clone(), result))
                        .unwrap(),
                )
            }
            other => panic!("unexpected first boundary {other:?}"),
        };
        assert!(
            matches!(
                invoke(input),
                Err(RepositoryRuleInvocationError::ExecuteUnsupported)
            ),
            "{first} then {second}"
        );
        assert!(
            prints.0.borrow().is_empty(),
            "disabled Execute must not run the continuation"
        );
    }
}
