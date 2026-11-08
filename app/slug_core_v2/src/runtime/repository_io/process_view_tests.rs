//! Actual registry-selected normal Execute. Every source comes from the real
//! compiled CLI/claimed-root producer, never a synthetic process result.
use std::time::Duration;
use std::time::Instant;

use slug_bzlmod_v2::HostRepositoryLabelPathValue;
use slug_bzlmod_v2::RepositoryEnvironmentSnapshot;
use slug_bzlmod_v2::RepositoryWorkspaceArgument;
use slug_bzlmod_v2::RepositoryWorkspaceExecute;
use slug_bzlmod_v2::RepositoryWorkspaceOperation;
use slug_bzlmod_v2::RepositoryWorkspaceReadView;
use slug_bzlmod_v2::RepositoryWorkspaceReadViewManifest;
use slug_bzlmod_v2::RepositoryWorkspaceReservation;
use slug_bzlmod_v2::RepositoryWorkspaceRetentionLimits;
use slug_identity_v2::CanonicalLabel;
use starlark_map::small_map::SmallMap;

use super::*;
use crate::runtime::dice::source_service::SourceServiceLimits;
use crate::runtime::repository_physical_root::PhysicalRootError;
use crate::runtime::repository_physical_root::RepositoryRootResourceLimits;
use crate::runtime::repository_physical_root::RepositoryRootResources;
use crate::runtime::repository_physical_root::RepositoryWorkspaceCommand;

const ARCHIVE: u64 = 2 * 1024 * 1024 * 1024;
const PROCESS: u64 = 64 * 1024 * 1024;
struct Fixture {
    actor: RepositoryMaterializer,
    token: RepositorySessionToken,
    command: Option<RepositoryWorkspaceCommand>,
    sources: Vec<Arc<RepositoryMaterializationRequest>>,
    results: RepositoryMaterializationResultEpoch,
    executor: reservations::ReservedRepositoryWorkspace,
    scope: Arc<SourceServiceScope>,
    bank: Arc<RepositoryRootResources>,
    retained_baseline: usize,
    runtime: tokio::runtime::Runtime,
    _workspace: tempfile::TempDir,
}
fn spec() -> Arc<RepoSpec> {
    Arc::new(RepoSpec {
        rule_id: slug_bzlmod_v2::RepoRuleId {
            bzl_file: CanonicalLabel::parse("@@//:repo.bzl").unwrap(),
            rule_name: "generic".into(),
        },
        attributes: Arc::new(SmallMap::new()),
    })
}
fn materialization(
    command: &RepositoryWorkspaceCommand,
    reservation: &RepositoryWorkspaceReservation,
    spec: &RepoSpec,
) -> Arc<RepositoryMaterializationRequest> {
    let t = command
        .epoch()
        .transcripts()
        .unwrap()
        .iter()
        .find(|t| t.reservation().as_ref() == reservation)
        .unwrap();
    Arc::new(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: reservation.workspace().clone(),
            canonical_repo: reservation.repository().clone(),
        },
        repo_spec: spec.clone(),
        kind: RepositoryMaterializationKind::GeneralWorkspace(
            t.realize(t.records().len()).unwrap(),
        ),
    })
}
impl Fixture {
    fn new(large: bool) -> Self {
        let workspace = tempfile::tempdir().unwrap();
        let normalized = NormalizedAbsolutePath::new(workspace.path()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(26);
        let scope = SourceServiceScope::new_for_test(
            normalized.clone(),
            deadline,
            deadline + Duration::from_secs(1),
            SourceServiceLimits::default(),
        )
        .unwrap();
        let bank = RepositoryRootResources::new(
            RepositoryRootResourceLimits::new(512 * 1024 * 1024, 512, 4 * ARCHIVE).unwrap(),
        )
        .unwrap();
        let retained_baseline = bank.snapshot().retained_bytes;
        let actor = RepositoryMaterializer::new_scoped(normalized.clone(), scope.clone()).unwrap();
        let token = actor.begin().unwrap();
        actor.preflight_native(token, []).unwrap();
        let limits =
            RepositoryWorkspaceRetentionLimits::new(128, 16 * 1024 * 1024, ARCHIVE).unwrap();
        let lease = actor
            .begin_general_workspace(token, bank.clone(), limits, &scope)
            .unwrap();
        let owner = crate::runtime::dice::physical_root_owner_for_test();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let mut command = RepositoryWorkspaceCommand::new(
            normalized.clone(),
            lease,
            limits,
            bank.clone(),
            scope.clone(),
            &|| true,
        )
        .unwrap();
        let mut sources = Vec::new();
        for (ordinal, name) in ["source", "unused"].into_iter().enumerate() {
            let key = Arc::new(
                slug_bzlmod_v2::source_preparation::RepositoryWorkspaceStableKey::new(
                    normalized.clone(),
                    owner.clone(),
                    CanonicalRepoName::new(name).unwrap(),
                    ordinal,
                )
                .unwrap(),
            );
            let spec = spec();
            let ticket = actor
                .reserve_workspace(token, key, spec.clone(), &scope)
                .unwrap();
            command = runtime
                .block_on(command.initialize_claimed_root(
                    ticket.reservation().clone(),
                    ticket.claim().clone(),
                    bank.clone(),
                    scope.clone(),
                    &|| true,
                ))
                .unwrap();
            for (path, body, mode) in [
                ("data", b"alpha".as_slice(), 0o666),
                ("tool", b"#!/bin/sh\nprintf tool".as_slice(), 0o755),
            ] {
                let request = command
                    .request(
                        ticket.reservation(),
                        RepositoryWorkspaceOperation::File {
                            path: path.into(),
                            body: Arc::from(body),
                            mode,
                        },
                        &scope,
                        &|| true,
                    )
                    .unwrap();
                command = command
                    .apply(
                        request,
                        &runtime,
                        &crate::runtime::ArchiveDistdirPolicy::default(),
                        &scope,
                        &|| true,
                    )
                    .unwrap();
            }
            if large && ordinal == 0 {
                let request = command
                    .request(
                        ticket.reservation(),
                        RepositoryWorkspaceOperation::File {
                            path: "large".into(),
                            body: Arc::from(vec![0x5a; 65 * 1024 * 1024]),
                            mode: 0o644,
                        },
                        &scope,
                        &|| true,
                    )
                    .unwrap();
                command = command
                    .apply(
                        request,
                        &runtime,
                        &crate::runtime::ArchiveDistdirPolicy::default(),
                        &scope,
                        &|| true,
                    )
                    .unwrap();
            }
            let request = materialization(&command, ticket.reservation(), &spec);
            let (next, sealed) = runtime
                .block_on(command.finalize_workspace(request.clone(), &scope, &|| true))
                .unwrap();
            command = next;
            actor
                .adopt_workspace(
                    token,
                    &ticket,
                    RepositoryMaterializationGeneration(1),
                    sealed,
                    &scope,
                )
                .unwrap();
            sources.push(request);
        }
        let results = actor.selected_epoch(token, &sources).unwrap();
        let key = Arc::new(
            slug_bzlmod_v2::source_preparation::RepositoryWorkspaceStableKey::new(
                normalized,
                owner,
                CanonicalRepoName::new("executor").unwrap(),
                2,
            )
            .unwrap(),
        );
        let executor = actor.reserve_workspace(token, key, spec(), &scope).unwrap();
        command = runtime
            .block_on(command.initialize_claimed_root(
                executor.reservation().clone(),
                executor.claim().clone(),
                bank.clone(),
                scope.clone(),
                &|| true,
            ))
            .unwrap();
        Self {
            actor,
            token,
            command: Some(command),
            sources,
            results,
            executor,
            scope,
            bank,
            retained_baseline,
            runtime,
            _workspace: workspace,
        }
    }
    fn operation(&self, omit_unused: bool) -> RepositoryWorkspaceOperation {
        let views = self
            .sources
            .iter()
            .take(if omit_unused { 1 } else { 2 })
            .map(|r| {
                let RepositoryMaterializationKind::GeneralWorkspace(realization) = &r.kind else {
                    unreachable!()
                };
                RepositoryWorkspaceReadView::new(
                    r.repo_spec.clone(),
                    // Equal foreign metadata must be admitted at its complete
                    // size, then rehomed to the canonical native data guard.
                    Arc::new((**realization.final_state()).clone()),
                )
                .unwrap()
            })
            .collect();
        let manifest =
            RepositoryWorkspaceReadViewManifest::new(self.executor.reservation(), views).unwrap();
        let RepositoryMaterializationKind::GeneralWorkspace(source) = &self.sources[0].kind else {
            unreachable!()
        };
        RepositoryWorkspaceOperation::Execute(RepositoryWorkspaceExecute::new(
            ["/usr/bin/python3","-c",r#"import errno,os,pathlib,subprocess,sys
a=pathlib.Path(sys.argv[1])
assert os.statvfs(a).f_blocks*os.statvfs(a).f_frsize == 2147483648
assert os.statvfs('.').f_blocks*os.statvfs('.').f_frsize == 67108864
try: os.open(a/'data',os.O_WRONLY|os.O_APPEND)
except OSError as e: assert e.errno == errno.EROFS
else: raise AssertionError('source writable')
with open('repeated','ab') as f: f.write(b'x')
sys.stdout.buffer.write((a/'data').read_bytes()+b'|'+subprocess.check_output([a/'tool'])+b'|'+pathlib.Path('repeated').read_bytes())
sys.exit(7)
"#,source.reservation().display_root().as_path().to_str().unwrap()].iter().map(|v|RepositoryWorkspaceArgument::String((*v).into())).collect::<Vec<_>>().into(),
            RepositoryEnvironmentSnapshot::empty(),SmallMap::new(),600,true,
            HostRepositoryLabelPathValue::new(self.executor.reservation().display_root().clone(),PathObservationNamespace::Materialization(self.executor.reservation().instance())).unwrap(),manifest,
        ).unwrap())
    }
    fn execute(&mut self, operation: RepositoryWorkspaceOperation) {
        let command = self.command.take().unwrap();
        let request = command
            .request(self.executor.reservation(), operation, &self.scope, &|| {
                true
            })
            .unwrap();
        self.command = Some(
            self.runtime
                .block_on(self.actor.execute_workspace(
                    self.token,
                    command,
                    request,
                    &self.sources,
                    &self.results,
                    &self.scope,
                    &|| true,
                ))
                .unwrap(),
        );
    }
}

#[test]
#[ignore = "actual CLI/actor-selected views and normal records; finite native supervisor required"]
fn native_selected_execute_records_replays_and_retains_data_after_controller_drop() {
    let mut fixture = Fixture::new(true);
    let operation = fixture.operation(false);
    let RepositoryWorkspaceOperation::Execute(execute) = &operation else {
        unreachable!()
    };
    for view in execute.read_views().views() {
        let original = fixture
            .sources
            .iter()
            .find_map(|source| match &source.kind {
                RepositoryMaterializationKind::GeneralWorkspace(realization)
                    if realization.reservation() == view.state().reservation() =>
                {
                    Some(realization.final_state())
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(view.state(), original);
        assert!(!Arc::ptr_eq(view.state(), original));
    }
    let unused = match &fixture.sources[1].kind {
        RepositoryMaterializationKind::GeneralWorkspace(r) => {
            r.reservation().display_root().as_path().to_str().unwrap()
        }
        _ => unreachable!(),
    };
    assert!(
        execute
            .argv()
            .iter()
            .all(|arg| !arg.text().contains(unused)),
        "the unused declared source must not be inferred from argv"
    );
    fixture.execute(operation.clone());
    let before = fixture.command.as_ref().unwrap().epoch().clone();
    let command = fixture.command.take().unwrap();
    let replay = command
        .request_at(
            fixture.executor.reservation(),
            0,
            operation.clone(),
            &fixture.scope,
            &|| true,
        )
        .unwrap();
    fixture.command = Some(
        fixture
            .runtime
            .block_on(fixture.actor.execute_workspace(
                fixture.token,
                command,
                replay,
                &fixture.sources,
                &fixture.results,
                &fixture.scope,
                &|| true,
            ))
            .unwrap(),
    );
    assert_eq!(fixture.command.as_ref().unwrap().epoch(), &before);
    drop(before);
    fixture.execute(operation);
    let command = fixture.command.take().unwrap();
    let t = command
        .epoch()
        .transcripts()
        .unwrap()
        .iter()
        .find(|t| t.reservation() == fixture.executor.reservation())
        .unwrap();
    assert_eq!(t.records().len(), 2);
    for (record, expected) in t
        .records()
        .iter()
        .zip([b"alpha|tool|x".as_slice(), b"alpha|tool|xx".as_slice()])
    {
        let slug_bzlmod_v2::RepositoryWorkspaceResult::Execute(result) = record.result() else {
            panic!("actual Execute record")
        };
        assert_eq!(result.return_code(), 7);
        assert_eq!(result.stdout(), expected);
        let RepositoryWorkspaceOperation::Execute(e) = record.request().operation() else {
            unreachable!()
        };
        assert_eq!(e.read_views().views().len(), 2);
        for view in e.read_views().views() {
            let canonical = fixture
                .sources
                .iter()
                .find_map(|source| match &source.kind {
                    RepositoryMaterializationKind::GeneralWorkspace(realization)
                        if realization.reservation() == view.state().reservation() =>
                    {
                        Some(realization.final_state())
                    }
                    _ => None,
                })
                .unwrap();
            assert!(
                Arc::ptr_eq(view.state(), canonical),
                "accepted history must retain the canonical credited source state"
            );
        }
    }
    let terminal = materialization(&command, fixture.executor.reservation(), &spec());
    let (command, sealed) = fixture
        .runtime
        .block_on(command.finalize_workspace(terminal.clone(), &fixture.scope, &|| true))
        .unwrap();
    let mut reader = sealed
        .root()
        .open_member("repeated", &fixture.scope, &|| true)
        .unwrap();
    fixture
        .actor
        .adopt_workspace(
            fixture.token,
            &fixture.executor,
            RepositoryMaterializationGeneration(1),
            sealed,
            &fixture.scope,
        )
        .unwrap();
    fixture
        .actor
        .accept(fixture.token, std::slice::from_ref(&terminal), vec![])
        .unwrap();
    drop(command);
    drop(terminal);
    fixture.sources.clear();
    fixture.results =
        RepositoryMaterializationResultEpoch::new(fixture.actor.workspace().clone(), []).unwrap();
    assert_eq!(
        fixture.bank.snapshot().backing_bytes,
        2 * ARCHIVE + PROCESS,
        "B's native history keeps both declared source data roots after their rows/controllers retire"
    );
    assert_eq!(
        fixture.bank.snapshot().native_fds,
        4,
        "three data roots and one reader; no setup namespaces or mount tokens"
    );
    let Fixture {
        actor,
        bank,
        retained_baseline,
        scope,
        runtime,
        executor,
        sources,
        results,
        command,
        _workspace,
        token: _,
    } = fixture;
    drop(actor);
    drop((
        scope, runtime, executor, sources, results, command, _workspace,
    ));
    assert_eq!(
        bank.snapshot().backing_bytes,
        PROCESS,
        "B's data root itself has no history/dependency edge"
    );
    assert_eq!(bank.snapshot().native_fds, 2);
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut reader, &mut bytes).unwrap();
    assert_eq!(bytes, b"xx");
    drop(reader);
    assert_eq!(bank.snapshot().native_fds, 0);
    assert_eq!(bank.snapshot().backing_bytes, 0);
    assert_eq!(bank.snapshot().retained_bytes, retained_baseline);
}

#[test]
#[ignore = "actual claimed selected roots, complete-set prelaunch refusal; finite native supervisor required"]
fn native_selected_execute_refuses_omitted_declared_source_before_helper() {
    for case in 0..3 {
        let mut fixture = Fixture::new(false);
        let operation = fixture.operation(case == 0);
        let command = fixture.command.take().unwrap();
        let request = command
            .request(
                fixture.executor.reservation(),
                operation,
                &fixture.scope,
                &|| true,
            )
            .unwrap();
        let mut selected = fixture.sources.clone();
        let results = if case == 2 {
            let mut entries = fixture
                .actor
                .state
                .lock()
                .unwrap()
                .active
                .as_ref()
                .unwrap()
                .entries
                .clone();
            entries[1].result = RepositoryMaterializationResult::MaterializationError {
                generation: RepositoryMaterializationGeneration(2),
                message: "stale selected result".into(),
            };
            RepositoryMaterializationResultEpoch::new(fixture.actor.workspace().clone(), entries)
                .unwrap()
        } else {
            fixture.results.clone()
        };
        if case == 1 {
            let mut foreign = (*selected[0]).clone();
            foreign.repo_spec.rule_id.rule_name = "different_spec".into();
            selected[0] = Arc::new(foreign);
        }
        let before = fixture.bank.snapshot().peak_native_fds;
        let refused = fixture.runtime.block_on(fixture.actor.execute_workspace(
            fixture.token,
            command,
            request,
            &selected,
            &results,
            &fixture.scope,
            &|| true,
        ));
        assert!(matches!(refused, Err(PhysicalRootError::Reservation)));
        assert_eq!(
            fixture.bank.snapshot().peak_native_fds,
            before,
            "case {case} must fail before helper admission"
        );
        let bank = fixture.bank.clone();
        let baseline = fixture.retained_baseline;
        drop((selected, results, fixture));
        assert_eq!(bank.snapshot().native_fds, 0);
        assert_eq!(bank.snapshot().backing_bytes, 0);
        assert_eq!(bank.snapshot().retained_bytes, baseline);
    }
}

#[test]
#[ignore = "actual normal selected execution then same-token association replacement; finite native supervisor required"]
fn native_selected_execute_rechecks_unused_source_after_actual_record_before_publication() {
    let mut fixture = Fixture::new(false);
    let operation = fixture.operation(false);
    let command = fixture.command.take().unwrap();
    let request = command
        .request(
            fixture.executor.reservation(),
            operation,
            &fixture.scope,
            &|| true,
        )
        .unwrap();
    let called = std::cell::Cell::new(false);
    let refused =
        fixture
            .runtime
            .block_on(fixture.actor.execute_workspace_with_recheck_hook_for_test(
                fixture.token,
                command,
                request,
                &fixture.sources,
                &fixture.results,
                &fixture.scope,
                &|| true,
                |private| {
                    let t = private
                        .epoch()
                        .transcripts()
                        .unwrap()
                        .iter()
                        .find(|t| t.reservation() == fixture.executor.reservation())
                        .unwrap();
                    assert_eq!(t.records().len(), 1);
                    let slug_bzlmod_v2::RepositoryWorkspaceResult::Execute(result) =
                        t.records()[0].result()
                    else {
                        panic!("actual private record")
                    };
                    assert_eq!(result.stdout(), b"alpha|tool|x");
                    assert_eq!(result.return_code(), 7);
                    called.set(true);
                    fixture.actor.replace_source_identity_for_test(
                        fixture.token,
                        &fixture.sources[1].id.canonical_repo,
                    );
                },
            ));
    assert!(called.get());
    assert!(matches!(refused, Err(PhysicalRootError::Reservation)));
    assert!(fixture.actor.session_is_active(fixture.token));
    assert!(
        fixture
            .actor
            .state
            .lock()
            .unwrap()
            .active
            .as_ref()
            .unwrap()
            .entries
            .iter()
            .all(|e| e.request.id.canonical_repo != *fixture.executor.reservation().repository()),
        "the privately completed executor must not become a materialized success"
    );
}
