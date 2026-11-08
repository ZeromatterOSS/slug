use std::io::Write;
use std::sync::atomic::AtomicBool;

use sha2::Digest;
use slug_bzlmod_v2::RepositoryMaterializationKind;
use slug_bzlmod_v2::SourcePreparationOutcome;

use super::*;

// Complete the unchanged builtin bazel_tools MODULE using the same unrelated
// Local declarations as source_input_tests. These modules supply no callbacks;
// the archive rules below still execute their actual authored Starlark.
pub(in crate::runtime::dice) const BUILTIN_MODULE_PREREQUISITES: &[(&str, &str)] = &[
    ("rules_license", "1.0.0"),
    ("buildozer", "8.5.1"),
    ("zlib", "1.3.1.bcr.5"),
    ("bazel_features", "1.42.1"),
    ("protobuf", "33.4"),
    ("rules_java", "9.1.0"),
    ("rules_cc", "0.2.17"),
    ("rules_python", "1.7.0"),
    ("rules_shell", "0.6.1"),
    ("apple_support", "1.24.2"),
    ("rules_apple", "4.1.0"),
    ("rules_swift", "3.1.2"),
    ("abseil-cpp", "20250814.1"),
];

struct ArchiveFixture {
    workspace: tempfile::TempDir,
    _archives: tempfile::TempDir,
    runtime: WorkspaceRuntime,
    policy: BzlmodCommandPolicyKey,
    first: PathBuf,
    first_sha: String,
    second_sha: String,
}

fn write_archive(path: &Path, entries: &[(&str, &[u8])]) -> String {
    let mut tar = tar::Builder::new(Vec::new());
    for (path, content) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_path(path).unwrap();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_mode(0o644);
        header.set_mtime(1_600_000_000);
        header.set_size(content.len() as u64);
        header.set_cksum();
        tar.append(&header, *content).unwrap();
    }
    let mut encoder = xz2::write::XzEncoder::new(Vec::new(), 1);
    encoder.write_all(&tar.into_inner().unwrap()).unwrap();
    let bytes = encoder.finish().unwrap();
    let digest = hex::encode(sha2::Sha256::digest(&bytes));
    fs::write(path, bytes).unwrap();
    digest
}

impl ArchiveFixture {
    fn new() -> Self {
        let workspace = tempfile::tempdir().unwrap();
        let archives = tempfile::tempdir().unwrap();
        let first = archives.path().join("arbitrary.payload");
        let second = archives.path().join("another.payload");
        let first_sha = write_archive(
            &first,
            &[
                ("component/shared.txt", b"first"),
                ("component/only-first.txt", b"retained"),
            ],
        );
        let second_sha = write_archive(
            &second,
            &[
                ("shared.txt", b"second"),
                ("block", b"file blocks directory"),
            ],
        );
        let first_url = url::Url::from_file_path(&first).unwrap();
        let second_url = url::Url::from_file_path(&second).unwrap();
        let policy = configured_test_command_policy(workspace.path());
        fs::create_dir_all(workspace.path().join("relocated/nested")).unwrap();
        fs::write(
            workspace.path().join("BUILD.bazel"),
            "filegroup(name='probe', srcs=['@one//:shared.txt', '@two//:shared.txt'])\n",
        )
        .unwrap();
        fs::write(workspace.path().join("relocated/nested/BUILD.bazel"), "").unwrap();
        let mut module = configured_test_module(&format!(
            "module(name='general_archives')\nfirst=use_repo_rule('//:archives.bzl','assemble')\nfirst(name='one', first='{first_url}', second='{second_url}', first_sha='{first_sha}', second_sha='{second_sha}')\nother=use_repo_rule('//relocated/nested:recipes.bzl','unrelated')\nother(name='two', source='{first_url}', checksum='{first_sha}')\n"
        ));
        for (name, version) in BUILTIN_MODULE_PREREQUISITES {
            let relative = format!(".archive_test_modules/{name}");
            let directory = workspace.path().join(&relative);
            fs::create_dir_all(&directory).unwrap();
            fs::write(
                directory.join("MODULE.bazel"),
                format!("module(name='{name}',version='{version}')\n"),
            )
            .unwrap();
            module.push_str(&format!(
                "local_path_override(module_name='{name}',path='{relative}')\n"
            ));
            if matches!(
                *name,
                "bazel_features" | "rules_apple" | "rules_swift" | "abseil-cpp"
            ) {
                module.push_str(&format!("bazel_dep(name='{name}',version='{version}')\n"));
            }
        }
        fs::write(workspace.path().join("MODULE.bazel"), module).unwrap();
        fs::write(workspace.path().join("archives.bzl"), r#"
def _assemble(ctx):
    ctx.file('shared.txt', 'before')
    first = ctx.download_and_extract(ctx.attr.first, sha256=ctx.attr.first_sha, strip_prefix='component', type='tar.xz')
    if not first.success: fail('unexpected unsuccessful capture')
    ctx.file('first-digest', first.sha256 + '|' + first.integrity)
    ctx.file('shared.txt', 'between')
    second = ctx.download_and_extract(url=[ctx.attr.second], sha256=ctx.attr.second_sha, stripPrefix='', type='tar.xz', auth={}, headers={}, rename_files={})
    ctx.file('second-digest', second.sha256)
    ctx.file('shared.txt', 'after')
    ctx.file('BUILD.bazel', "exports_files(['shared.txt', 'only-first.txt', 'first-digest', 'second-digest'])\n")
    print('archives-finished')
    return ctx.repo_metadata(reproducible=True)
assemble = repository_rule(implementation=_assemble, attrs={'first':attr.string(), 'second':attr.string(), 'first_sha':attr.string(), 'second_sha':attr.string()})
"#).unwrap();
        fs::write(workspace.path().join("relocated/nested/recipes.bzl"), r#"
def _independent(ctx):
    result = ctx.download_and_extract(ctx.attr.source, '', ctx.attr.checksum, 'tar.xz', 'component')
    ctx.file('independent-digest', result.sha256)
    ctx.file('BUILD.bazel', "exports_files(['shared.txt', 'independent-digest'])\n")
    return ctx.repo_metadata(reproducible=True)
unrelated = repository_rule(implementation=_independent, attrs={'source':attr.string(), 'checksum':attr.string()})
"#).unwrap();
        let runtime = test_runtime(workspace.path()).unwrap();
        Self {
            workspace,
            _archives: archives,
            runtime,
            policy,
            first,
            first_sha,
            second_sha,
        }
    }

    fn cquery(&self) -> AcceptedCommand<Arc<Result<CqueryCommandEvaluation, CqueryCommandError>>> {
        self.runtime
            .cquery_command_with_bzlmod_inputs(
                "deps(//:probe)",
                false,
                true,
                self.policy.clone(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                root_setting_overlay(None),
            )
            .unwrap()
    }

    fn roots(&self) -> Vec<PathBuf> {
        let guard = NativeDemandAbortGuard::new(
            self.runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        guard
            .command()
            .reusable_requests
            .values()
            .filter_map(|request| {
                if !matches!(
                    request.kind,
                    RepositoryMaterializationKind::GeneratedFilesystemEffects(_)
                ) {
                    return None;
                }
                let result = self.runtime.repository_materializer.active_result_for_test(
                    guard.command().repository_session,
                    request.id.canonical_repo.as_str(),
                );
                let slug_bzlmod_v2::RepositoryMaterializationResult::Success(
                    slug_bzlmod_v2::RepositoryMaterializationSuccess::Immutable {
                        generation_root,
                        ..
                    },
                ) = result
                else {
                    panic!("archive root not immutable: {result:?}")
                };
                Some(generation_root)
            })
            .collect()
    }

    fn query(
        &self,
        action: SourceChange,
    ) -> (
        Result<DrivenCommand<ObservedRootQueryCommand>, NativeDemandSessionError>,
        Arc<AtomicBool>,
        Arc<Mutex<Vec<RepositoryFilesystemRecord>>>,
    ) {
        let changed = Arc::new(AtomicBool::new(false));
        let records = Arc::new(Mutex::new(Vec::new()));
        let root = InterceptQuery {
            query: RootQueryCommandObservationKey::new(
                NormalizedAbsolutePath::new(self.runtime.workspace.clone()).unwrap(),
                "@one//:shared.txt",
                QueryOrder::Auto,
                QueryPolicy::default(),
                QueryOutputCompletion::Standard,
            )
            .unwrap(),
            workspace: NormalizedAbsolutePath::new(self.runtime.workspace.clone()).unwrap(),
            source: self.first.clone(),
            action,
            changed: changed.clone(),
            records: records.clone(),
        };
        let inputs = NativeDemandRequestInputBundle {
            archive_distdirs: crate::runtime::ArchiveDistdirPolicy::default(),
            runtime_sources: None,
            guest_namespace: None,
            command_policy: self.policy.clone(),
            environment_policy: BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None)
                .unwrap(),
            lockfile_mode: LockfileMode::Update,
            registry_urls: RegistryUrls::from_request(&self.runtime.workspace, &[]).unwrap(),
            repository_environment: slug_bzlmod_v2::RepositoryEnvironmentSnapshot::empty(),
        };
        (self.runtime.drive_command(inputs, root), changed, records)
    }
}

#[derive(Clone, Copy)]
enum SourceChange {
    None,
    Rewrite,
    Delete,
    Abort,
}

#[derive(Clone)]
struct InterceptQuery {
    query: RootQueryCommandObservationKey,
    workspace: NormalizedAbsolutePath,
    source: PathBuf,
    action: SourceChange,
    changed: Arc<AtomicBool>,
    records: Arc<Mutex<Vec<RepositoryFilesystemRecord>>>,
}

#[async_trait]
impl NativeCommandRoot for InterceptQuery {
    type Terminal = ObservedRootQueryCommand;
    fn observations<'a>(&self, terminal: &'a Self::Terminal) -> Option<&'a PathObservationEpoch> {
        self.query.observations(terminal)
    }
    fn observed_selection_association(&self) -> ObservedSelectionAssociation {
        self.query.observed_selection_association()
    }
    async fn compute(
        &self,
        transaction: &mut dice::DiceTransaction,
    ) -> Result<SourcePreparationOutcome<Self::Terminal>, NativeDemandSessionError> {
        let epoch = transaction
            .per_transaction_data()
            .data
            .get::<RepositoryFilesystemEpoch>()
            .unwrap()
            .clone();
        *self.records.lock().unwrap() = epoch.transcript().unwrap().records().to_vec();
        if epoch.transcript().unwrap().records().len() == 1
            && !self.changed.swap(true, Ordering::SeqCst)
        {
            match self.action {
                SourceChange::None => {}
                SourceChange::Rewrite => {
                    fs::write(&self.source, b"changed after verified extraction").unwrap()
                }
                SourceChange::Delete => fs::remove_file(&self.source).unwrap(),
                SourceChange::Abort => {
                    return Err(NativeDemandSessionError::Computation(anyhow::anyhow!(
                        "test request aborted after verified archive"
                    )));
                }
            }
        }
        let outcome = NativeCommandRoot::compute(&self.query, transaction).await?;
        if matches!(outcome, SourcePreparationOutcome::Complete(_)) {
            for record in epoch.transcript().unwrap().records() {
                let key = slug_loading_v2::HostSelectedRepositoryFileEffectObservationKey::new(
                    self.workspace.clone(),
                    record.request().id().owner().clone(),
                    record.request().id().ordinal(),
                );
                let SourcePreparationOutcome::Complete(Ok(observed)) =
                    transaction.compute(&key).await.unwrap()
                else {
                    panic!("terminal query must retain its actual repository effect")
                };
                let effect = observed.result().as_ref().as_ref().unwrap();
                assert!(effect.metadata().reproducible());
                assert!(effect.metadata().attrs_for_reproducibility().is_empty());
            }
        }
        Ok(outcome)
    }
}

#[test]
fn independent_innate_archive_rules_complete_cquery_preserve_order_and_actual_digests() {
    let fixture = ArchiveFixture::new();
    let outcome = fixture.cquery();
    let evaluation = outcome.terminal_for_test().as_ref().as_ref().unwrap();
    assert!(evaluation.analyses().count() >= 3);
    let roots = fixture.roots();
    assert_eq!(roots.len(), 2);
    let assembled = roots
        .iter()
        .find(|root| root.join("second-digest").is_file())
        .unwrap();
    let independent = roots
        .iter()
        .find(|root| root.join("independent-digest").is_file())
        .unwrap();
    assert_eq!(fs::read(assembled.join("shared.txt")).unwrap(), b"after");
    assert_eq!(
        fs::read(assembled.join("only-first.txt")).unwrap(),
        b"retained"
    );
    let expected_sri = slug_bzlmod_v2::generated_repository_archive_sha256_sri(
        sha2::Sha256::digest(fs::read(&fixture.first).unwrap()).into(),
    );
    assert_eq!(
        fs::read_to_string(assembled.join("first-digest")).unwrap(),
        format!("{}|{expected_sri}", fixture.first_sha)
    );
    assert_eq!(
        fs::read_to_string(assembled.join("second-digest")).unwrap(),
        fixture.second_sha
    );
    assert_eq!(
        fs::read_to_string(independent.join("independent-digest")).unwrap(),
        fixture.first_sha
    );
    assert_eq!(fs::read(independent.join("shared.txt")).unwrap(), b"first");
}

#[test]
fn verified_archive_sources_can_change_before_terminal_handoff_without_refetch() {
    for action in [SourceChange::Rewrite, SourceChange::Delete] {
        let fixture = ArchiveFixture::new();
        let (outcome, changed, _) = fixture.query(action);
        let outcome = outcome.unwrap();
        assert!(
            outcome
                .accepted
                .terminal_for_test()
                .result()
                .as_ref()
                .is_ok(),
            "native query terminal: {:?}",
            outcome.accepted.terminal_for_test().result()
        );
        assert!(changed.load(Ordering::SeqCst));
        assert_eq!(
            outcome
                .accepted
                .terminal_for_test()
                .result()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@one//:shared.txt\n"
        );
        let roots = fixture.roots();
        assert_eq!(roots.len(), 1);
        assert_eq!(fs::read(roots[0].join("shared.txt")).unwrap(), b"after");
        assert!(
            fs::read_to_string(roots[0].join("first-digest"))
                .unwrap()
                .starts_with(&fixture.first_sha)
        );
    }
}

#[test]
fn extraction_suffix_failure_and_request_abort_publish_no_archive_root() {
    for failure in ["extraction", "suffix", "abort"] {
        let fixture = ArchiveFixture::new();
        let source = fixture.workspace.path().join("archives.bzl");
        let original = fs::read_to_string(&source).unwrap();
        if failure == "extraction" {
            let checksum = write_archive(
                &fixture.first,
                &[
                    ("component/node", b"file"),
                    ("component/node/child", b"collision"),
                ],
            );
            let module = fixture.workspace.path().join("MODULE.bazel");
            let text = fs::read_to_string(&module).unwrap();
            fs::write(module, text.replace(&fixture.first_sha, &checksum)).unwrap();
        } else if failure == "suffix" {
            fs::write(
                &source,
                original.replace(
                    "ctx.file('shared.txt', 'after')",
                    "ctx.file('block/child', 'fails')",
                ),
            )
            .unwrap();
        }
        let prior = accepted_native_snapshot(&fixture.runtime).events;
        let (result, changed, _) = fixture.query(if failure == "abort" {
            SourceChange::Abort
        } else {
            SourceChange::None
        });
        let error = match result {
            Err(error) => error.to_string(),
            Ok(driven) => driven
                .accepted
                .terminal_for_test()
                .result()
                .as_ref()
                .as_ref()
                .expect_err("failure fixture must not produce a successful query terminal")
                .to_string(),
        };
        let expected = match failure {
            "extraction" => "collision",
            "suffix" => "creating generated file parent",
            _ => "aborted after verified archive",
        };
        assert!(error.contains(expected), "{failure}: {error}");
        if failure == "abort" {
            assert!(changed.load(Ordering::SeqCst));
        }
        assert!(fixture.roots().is_empty());
        assert_eq!(accepted_native_snapshot(&fixture.runtime).events, prior);
        assert!(!accepted_native_snapshot(&fixture.runtime).events.entries.iter().flat_map(|(_, batch)| batch.events()).any(|event| matches!(event, slug_events_v2::EvaluationEvent::StarlarkPrint { text, .. } if text.as_str() == "archives-finished")));
    }
}

fn realized_plan(
    request: &slug_bzlmod_v2::RepositoryFilesystemRequest,
    suffix: Option<(&str, &[u8])>,
) -> slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan {
    let mut builder = slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan::builder();
    for effect in request.prefix().ordered_effects() {
        match effect {
            slug_bzlmod_v2::GeneratedRepositoryOrderedEffectRef::File(file) => {
                builder
                    .push(
                        file.path().into(),
                        Arc::from(file.content()),
                        file.executable(),
                    )
                    .unwrap();
            }
            slug_bzlmod_v2::GeneratedRepositoryOrderedEffectRef::Archive(archive) => {
                builder.push_archive_effect(archive.clone()).unwrap();
            }
            slug_bzlmod_v2::GeneratedRepositoryOrderedEffectRef::Delete(path) => {
                builder.push_delete(path.clone()).unwrap();
            }
            slug_bzlmod_v2::GeneratedRepositoryOrderedEffectRef::Symlink(_) => {
                unreachable!("fixture has no symlinks")
            }
        }
    }
    match request.operation() {
        slug_bzlmod_v2::RepositoryFilesystemOperation::Archive(archive) => {
            builder.push_archive_effect(archive.clone()).unwrap()
        }
        slug_bzlmod_v2::RepositoryFilesystemOperation::Delete(path) => {
            builder.push_delete(path.clone()).unwrap()
        }
    }
    if let Some((path, bytes)) = suffix {
        builder.push(path.into(), Arc::from(bytes), false).unwrap();
    }
    builder.finish()
}

#[test]
fn actual_filesystem_progress_drops_physical_roots_on_stale_abort_suffix_and_descriptor_failure() {
    use slug_bzlmod_v2::RepositoryFilesystemRealization;
    use slug_bzlmod_v2::RepositoryFilesystemTranscript;

    use crate::runtime::repository_filesystem_progress::RepositoryFilesystemProgress;

    let fixture = ArchiveFixture::new();
    let (outcome, _, captured) = fixture.query(SourceChange::None);
    let outcome = outcome.unwrap();
    assert!(
        outcome
            .accepted
            .terminal_for_test()
            .result()
            .as_ref()
            .is_ok(),
        "native query terminal: {:?}",
        outcome.accepted.terminal_for_test().result()
    );
    let records = captured.lock().unwrap().clone();
    assert_eq!(
        records.len(),
        2,
        "actual query must complete both archive calls"
    );
    let transcript = records
        .iter()
        .cloned()
        .try_fold(
            RepositoryFilesystemTranscript::empty(),
            |transcript, record| transcript.append(record),
        )
        .unwrap();
    let last = records.last().unwrap().request();
    let realization = RepositoryFilesystemRealization::new(
        17,
        last.id().owner().clone(),
        last.id().ordinal(),
        transcript.clone(),
        realized_plan(last, None),
    )
    .unwrap();
    for failure in ["stale lease", "inactive", "suffix", "descriptor"] {
        let mut progress = RepositoryFilesystemProgress::default();
        for record in &records {
            let result = progress
                .complete_call(
                    &Arc::new(record.request().clone()),
                    &fixture.runtime.runtime,
                    &|| true,
                    None,
                    &crate::runtime::ArchiveDistdirPolicy::default(),
                )
                .unwrap();
            assert_eq!(&result, record.result());
        }
        let paths = progress.provisional_paths();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].join("shared.txt").is_file());
        assert_eq!(fs::read(paths[0].join("shared.txt")).unwrap(), b"second");
        let error = match failure {
            "stale lease" => progress
                .finish(&realization, 18, &|| true, None)
                .err()
                .expect("stale lease must refuse realization"),
            "inactive" => progress
                .finish(&realization, 17, &|| false, None)
                .err()
                .expect("inactive command must refuse realization"),
            "suffix" => {
                let invalid = RepositoryFilesystemRealization::new(
                    17,
                    last.id().owner().clone(),
                    last.id().ordinal(),
                    transcript.clone(),
                    realized_plan(last, Some(("block/child", b"must fail"))),
                )
                .unwrap();
                let error = progress
                    .finish(&invalid, 17, &|| true, None)
                    .err()
                    .expect("invalid suffix must fail");
                assert!(progress.provisional_paths().is_empty());
                assert!(
                    !paths[0].exists(),
                    "failed suffix must discard its taken tree"
                );
                error
            }
            "descriptor" => {
                let archive = last.archive().unwrap();
                let changed_archive = slug_bzlmod_v2::GeneratedRepositoryArchiveEffect::new(
                    Arc::from(archive.urls()),
                    archive.sha256(),
                    archive.strip_prefix().into(),
                    archive.format(),
                    "changed after verification".into(),
                    archive.declared_integrity().into(),
                )
                .unwrap();
                let changed = slug_bzlmod_v2::RepositoryFilesystemRequest::new(
                    last.id().clone(),
                    last.generated_repository().into(),
                    slug_bzlmod_v2::RepositoryFilesystemOperation::Archive(changed_archive),
                    last.prefix().clone(),
                );
                let changed_transcript = RepositoryFilesystemTranscript::empty()
                    .append(records[0].clone())
                    .unwrap()
                    .append(RepositoryFilesystemRecord::new(
                        Arc::new(changed.clone()),
                        records[1].result().clone(),
                    ))
                    .unwrap();
                let invalid = RepositoryFilesystemRealization::new(
                    17,
                    last.id().owner().clone(),
                    last.id().ordinal(),
                    changed_transcript,
                    realized_plan(&changed, None),
                )
                .unwrap();
                let error = progress
                    .finish(&invalid, 17, &|| true, None)
                    .err()
                    .expect("changed descriptor must refuse realization");
                assert!(progress.provisional_paths().is_empty());
                assert!(
                    !paths[0].exists(),
                    "changed descriptor must discard its taken tree"
                );
                error
            }
            _ => unreachable!(),
        };
        let expected = match failure {
            "stale lease" => "stale command",
            "inactive" => "no longer active",
            "suffix" => "creating generated file parent",
            _ => "differs from verified realization",
        };
        assert!(error.message.contains(expected), "{failure}: {error:?}");
        drop(progress);
        assert!(
            !paths[0].exists(),
            "dropping refused progress must release physical root"
        );
    }
}

#[test]
fn actual_filesystem_progress_binds_source_scope_and_releases_refused_physical_roots() {
    use crate::runtime::dice::source_service::SourceServiceLimits;
    use crate::runtime::dice::source_service::SourceServiceScope;
    use crate::runtime::repository_filesystem_progress::RepositoryFilesystemProgress;

    let fixture = ArchiveFixture::new();
    let (outcome, _, captured) = fixture.query(SourceChange::None);
    let outcome = outcome.unwrap();
    assert!(
        outcome
            .accepted
            .terminal_for_test()
            .result()
            .as_ref()
            .is_ok(),
        "native query terminal: {:?}",
        outcome.accepted.terminal_for_test().result()
    );
    let records = captured.lock().unwrap().clone();
    assert_eq!(records.len(), 2);
    let request = Arc::new(records[0].request().clone());
    let realization = slug_bzlmod_v2::RepositoryFilesystemRealization::new(
        23,
        request.id().owner().clone(),
        request.id().ordinal(),
        slug_bzlmod_v2::RepositoryFilesystemTranscript::empty()
            .append(records[0].clone())
            .unwrap(),
        realized_plan(&request, None),
    )
    .unwrap();
    let make_scope = || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        SourceServiceScope::new_for_test(
            NormalizedAbsolutePath::new(fixture.runtime.workspace.clone()).unwrap(),
            deadline,
            deadline + std::time::Duration::from_secs(1),
            SourceServiceLimits {
                allow_network: false,
                ..SourceServiceLimits::default()
            },
        )
        .unwrap()
    };
    for failure in [
        "foreign continuation",
        "foreign finish",
        "cancelled continuation",
        "cancelled finish",
    ] {
        let scope_a = make_scope();
        let scope_b = make_scope();
        assert!(!Arc::ptr_eq(&scope_a, &scope_b));
        let mut progress = RepositoryFilesystemProgress::default();
        let verified = progress
            .complete_call(
                &request,
                &fixture.runtime.runtime,
                &|| true,
                Some(&scope_a),
                &crate::runtime::ArchiveDistdirPolicy::default(),
            )
            .unwrap();
        assert_eq!(&verified, records[0].result());
        assert!(scope_a.snapshot().transferred_bytes > 0);
        assert!(scope_a.snapshot().decoded_bytes > 0);
        assert!(scope_a.snapshot().written_bytes > 0);
        let paths = progress.provisional_paths();
        assert_eq!(paths.len(), 1);
        assert_eq!(fs::read(paths[0].join("shared.txt")).unwrap(), b"first");
        let foreign = failure.starts_with("foreign");
        if !foreign {
            scope_a.cancel();
        }
        let supplied = if foreign { &scope_b } else { &scope_a };
        let error = if failure.ends_with("continuation") {
            progress
                .complete_call(
                    &Arc::new(records[1].request().clone()),
                    &fixture.runtime.runtime,
                    &|| true,
                    Some(supplied),
                    &crate::runtime::ArchiveDistdirPolicy::default(),
                )
                .err()
                .expect("foreign or cancelled scope must refuse continuation")
        } else {
            progress
                .finish(&realization, 23, &|| true, Some(supplied))
                .err()
                .expect("foreign or cancelled scope must refuse final handoff")
        };
        let expected = if foreign { "ForeignScope" } else { "Cancelled" };
        assert!(error.message.contains(expected), "{failure}: {error:?}");
        if foreign {
            assert!(progress.provisional_paths().is_empty());
            assert!(
                !paths[0].exists(),
                "foreign scope must discard the taken prefix"
            );
        }
        drop(progress);
        assert!(
            !paths[0].exists(),
            "refused source scope cannot retain a physical root after owner drop"
        );
    }
}

#[path = "general_repository_archive_tests/delete_tests.rs"]
mod delete_tests;
