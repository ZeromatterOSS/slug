use super::*;

const DELETE_ONLY: &str = r#"
def _assemble(ctx):
    ctx.file('tree/one', 'first')
    ctx.file('tree/nested/leaf', 'second')
    removed = ctx.delete(ctx.path('tree'))
    if not removed: fail('existing directory was not removed')
    if ctx.path('tree').exists: fail('deleted path remained present')
    if ctx.delete('tree'): fail('absent directory reported removed')
    ctx.file('tree/recreated', 'third')
    ctx.file('shared.txt', str(removed))
    ctx.file('BUILD.bazel', "exports_files(['shared.txt'])\n")
    return ctx.repo_metadata(reproducible=True)
assemble = repository_rule(implementation=_assemble, attrs={'first':attr.string(), 'second':attr.string(), 'first_sha':attr.string(), 'second_sha':attr.string()})
"#;

fn successful(result: Result<DrivenCommand<ObservedRootQueryCommand>, NativeDemandSessionError>) {
    let result = result.unwrap();
    assert!(
        result
            .accepted
            .terminal_for_test()
            .result()
            .as_ref()
            .as_ref()
            .is_ok(),
        "{:?}",
        result.accepted.terminal_for_test().result()
    );
}

#[test]
fn genuine_delete_only_repository_seals_actual_boolean_and_restores_source() {
    let fixture = ArchiveFixture::new();
    let source = fixture.workspace.path().join("archives.bzl");
    fs::write(&source, DELETE_ONLY).unwrap();
    let (outcome, _, records) = fixture.query(SourceChange::None);
    successful(outcome);
    let records = records.lock().unwrap().clone();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].result().deleted(), Some(true));
    assert_eq!(records[1].result().deleted(), Some(false));
    assert!(
        records
            .iter()
            .all(|record| record.request().archive().is_none())
    );
    let root = fixture.roots().pop().unwrap();
    assert_eq!(fs::read(root.join("shared.txt")).unwrap(), b"True");
    assert!(!root.join("tree/one").exists());
    assert!(!root.join("tree/nested").exists());
    assert_eq!(fs::read(root.join("tree/recreated")).unwrap(), b"third");
    let first = fs::read(root.join("shared.txt")).unwrap();
    fs::write(
        &source,
        DELETE_ONLY.replace("str(removed)", "'changed source'"),
    )
    .unwrap();
    successful(fixture.query(SourceChange::None).0);
    let changed = fixture.roots().pop().unwrap();
    assert_eq!(
        fs::read(changed.join("shared.txt")).unwrap(),
        b"changed source"
    );
    fs::write(&source, DELETE_ONLY).unwrap();
    successful(fixture.query(SourceChange::None).0);
    assert_eq!(
        fs::read(fixture.roots().pop().unwrap().join("shared.txt")).unwrap(),
        first
    );
}

#[test]
fn genuine_archive_delete_write_order_preserves_one_tree_and_final_manifest() {
    let fixture = ArchiveFixture::new();
    let source = fixture.workspace.path().join("archives.bzl");
    let original = fs::read_to_string(&source).unwrap();
    fs::write(&source, original.replace("ctx.file('shared.txt', 'between')",
        "if not ctx.delete('only-first.txt'): fail('archive file missing')\n    if ctx.delete('only-first.txt'): fail('archive file stale')\n    ctx.file('shared.txt', 'between')")).unwrap();
    let (outcome, _, records) = fixture.query(SourceChange::None);
    successful(outcome);
    let records = records.lock().unwrap().clone();
    assert_eq!(records.len(), 4);
    assert!(matches!(
        records[0].result(),
        slug_bzlmod_v2::RepositoryFilesystemResult::Archive(_)
    ));
    assert_eq!(records[1].result().deleted(), Some(true));
    assert_eq!(records[2].result().deleted(), Some(false));
    assert!(matches!(
        records[3].result(),
        slug_bzlmod_v2::RepositoryFilesystemResult::Archive(_)
    ));
    let root = fixture.roots().pop().unwrap();
    assert_eq!(fs::read(root.join("shared.txt")).unwrap(), b"after");
    assert!(!root.join("only-first.txt").exists());
    assert_eq!(
        fs::read(root.join("second-digest")).unwrap(),
        fixture.second_sha.as_bytes()
    );
}

#[test]
fn deletion_failure_discards_taken_tree_and_never_seals_partial_mutation() {
    use slug_bzlmod_v2::RepositoryFilesystemCallId;
    use slug_bzlmod_v2::RepositoryFilesystemOperation;
    use slug_bzlmod_v2::RepositoryFilesystemRequest;

    use crate::runtime::repository_filesystem_progress::RepositoryFilesystemProgress;
    let fixture = ArchiveFixture::new();
    let source = fixture.workspace.path().join("archives.bzl");
    fs::write(source, DELETE_ONLY).unwrap();
    let (outcome, _, records) = fixture.query(SourceChange::None);
    successful(outcome);
    let records = records.lock().unwrap().clone();
    let mut progress = RepositoryFilesystemProgress::default();
    for record in &records {
        assert_eq!(
            &progress
                .complete_call(
                    &Arc::new(record.request().clone()),
                    &fixture.runtime.runtime,
                    &|| true,
                    None,
                    &crate::runtime::ArchiveDistdirPolicy::default()
                )
                .unwrap(),
            record.result()
        );
    }
    let root = progress.provisional_paths().pop().unwrap();
    let last = records.last().unwrap().request();
    let prefix = realized_plan(last, Some(("block", b"file")));
    let request = Arc::new(RepositoryFilesystemRequest::new(
        RepositoryFilesystemCallId::new(last.id().owner().clone(), last.id().ordinal(), 2),
        last.generated_repository().into(),
        RepositoryFilesystemOperation::delete("block/child".into()).unwrap(),
        prefix,
    ));
    let error = progress
        .complete_call(
            &request,
            &fixture.runtime.runtime,
            &|| true,
            None,
            &crate::runtime::ArchiveDistdirPolicy::default(),
        )
        .unwrap_err();
    assert!(
        error.message.contains("non-directory ancestor"),
        "{}",
        error.message
    );
    assert!(progress.provisional_paths().is_empty());
    assert!(!root.exists());
}

#[test]
fn mid_delete_cancellation_after_mutation_discards_prefix_without_success_record() {
    use slug_bzlmod_v2::RepositoryFilesystemCallId;
    use slug_bzlmod_v2::RepositoryFilesystemOperation;
    use slug_bzlmod_v2::RepositoryFilesystemRequest;

    use crate::runtime::repository_filesystem_progress::RepositoryFilesystemProgress;
    let fixture = ArchiveFixture::new();
    fs::write(fixture.workspace.path().join("archives.bzl"), DELETE_ONLY).unwrap();
    let (outcome, _, records) = fixture.query(SourceChange::None);
    successful(outcome);
    let records = records.lock().unwrap().clone();
    let mut progress = RepositoryFilesystemProgress::default();
    for record in &records {
        progress
            .complete_call(
                &Arc::new(record.request().clone()),
                &fixture.runtime.runtime,
                &|| true,
                None,
                &crate::runtime::ArchiveDistdirPolicy::default(),
            )
            .unwrap();
    }
    let root = progress.provisional_paths().pop().unwrap();
    let last = records.last().unwrap().request();
    let prefix = realized_plan(last, Some(("tree/recreated", b"will be deleted")));
    let request = Arc::new(RepositoryFilesystemRequest::new(
        RepositoryFilesystemCallId::new(last.id().owner().clone(), last.id().ordinal(), 2),
        last.generated_repository().into(),
        RepositoryFilesystemOperation::delete("tree".into()).unwrap(),
        prefix,
    ));
    let armed = AtomicBool::new(false);
    let mutated = AtomicBool::new(false);
    let active = || {
        if root.join("tree/recreated").exists() {
            armed.store(true, Ordering::SeqCst);
        } else if armed.load(Ordering::SeqCst) {
            mutated.store(true, Ordering::SeqCst);
            return false;
        }
        true
    };
    let result = progress.complete_call(
        &request,
        &fixture.runtime.runtime,
        &active,
        None,
        &crate::runtime::ArchiveDistdirPolicy::default(),
    );
    assert!(
        mutated.load(Ordering::SeqCst),
        "cancellation must follow actual file deletion"
    );
    assert!(result.unwrap_err().message.contains("no longer active"));
    assert_eq!(records.len(), 2, "failed operation must yield no record");
    assert!(progress.provisional_paths().is_empty());
    assert!(!root.exists(), "partly mutated root must be destroyed");
}
