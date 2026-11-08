//! Real CLI -> claimed tmpfs -> command snapshot -> sealed registry -> source
//! reader/image proof. No synthetic native root, result or lease constructor.
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::time::Duration;
use std::time::Instant;

use dice::DetectCycles;
use dice::Dice;
use sha2::Digest;
use sha2::Sha256;
use slug_bzlmod_v2::RepoRuleId;
use slug_bzlmod_v2::RepoSpec;
use slug_bzlmod_v2::RepositoryMaterialization;
use slug_bzlmod_v2::RepositoryMaterializationGeneration;
use slug_bzlmod_v2::RepositoryMaterializationKind;
use slug_bzlmod_v2::RepositoryMaterializationRequest;
use slug_bzlmod_v2::RepositoryMaterializationRequestId;
use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
use slug_bzlmod_v2::RepositoryWorkspaceOperation;
use slug_bzlmod_v2::RepositoryWorkspaceRetentionLimits;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_bzlmod_v2::source_preparation::RepositoryWorkspaceStableKey;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::AnchoredPathFileDigestObservationKey;
use slug_workspace_v2::FileContentDigest;
use slug_workspace_v2::ImageChargedPath;
use slug_workspace_v2::ImageDescriptorBudget;
use slug_workspace_v2::ImageObservationData;
use slug_workspace_v2::ImageObservationOperation;
use slug_workspace_v2::ImageObservationRequest;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathFileDigest;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationInstanceId;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use slug_workspace_v2::PathOutcome;
use starlark_map::small_map::SmallMap;

use super::*;
use crate::runtime::SourceArtifactInput;
use crate::runtime::dice::source_service::SourceServiceLimits;
use crate::runtime::native_source_access::NativeSourceReader;
use crate::runtime::native_source_access::new_reader_resources;
use crate::runtime::repository_io::RepositoryMaterializer;
use crate::runtime::repository_io::physical_roots::SelectedRepositoryRoots;

struct Generation {
    request: Arc<RepositoryMaterializationRequest>,
    results: RepositoryMaterializationResultEpoch,
    roots: SelectedRepositoryRoots,
    source: SourceArtifactInput,
    materialized: RepositoryMaterialization,
}
fn digest(bytes: &[u8]) -> FileContentDigest {
    FileContentDigest::new(Sha256::digest(bytes).into(), bytes.len() as u64).unwrap()
}
fn source(
    selected: &Generation,
    namespace: PathObservationNamespace,
    anchor: Option<MaterializationPathAnchor>,
    content: FileContentDigest,
) -> SourceArtifactInput {
    SourceArtifactInput::for_native_lease_test(
        selected.source.label().clone(),
        namespace,
        selected.source.requested_path().clone(),
        selected.source.real_path().clone(),
        anchor,
        content,
    )
}
fn generation(
    materializer: &RepositoryMaterializer,
    key: &Arc<RepositoryWorkspaceStableKey>,
    resources: &Arc<RepositoryRootResources>,
    readers: &Arc<crate::runtime::native_source_access::NativeReaderResources>,
    scope: &Arc<SourceServiceScope>,
    runtime: &tokio::runtime::Runtime,
    body: &[u8],
) -> (
    crate::runtime::repository_io::RepositorySessionToken,
    Generation,
) {
    generation_with_adoption_spec(
        materializer,
        key,
        resources,
        readers,
        scope,
        runtime,
        body,
        false,
    )
    .expect("matching ticket spec must adopt")
}
fn generation_with_adoption_spec(
    materializer: &RepositoryMaterializer,
    key: &Arc<RepositoryWorkspaceStableKey>,
    resources: &Arc<RepositoryRootResources>,
    readers: &Arc<crate::runtime::native_source_access::NativeReaderResources>,
    scope: &Arc<SourceServiceScope>,
    runtime: &tokio::runtime::Runtime,
    body: &[u8],
    foreign_spec: bool,
) -> Option<(
    crate::runtime::repository_io::RepositorySessionToken,
    Generation,
)> {
    let token = materializer.begin().unwrap();
    materializer.preflight_native(token, []).unwrap();
    let limits =
        RepositoryWorkspaceRetentionLimits::new(128, 16 * 1024 * 1024, ARCHIVE_BYTES).unwrap();
    let lease = materializer
        .begin_general_workspace(token, resources.clone(), limits, scope)
        .unwrap();
    let spec = Arc::new(RepoSpec {
        rule_id: RepoRuleId {
            bzl_file: CanonicalLabel::parse("@@//:repo.bzl").unwrap(),
            rule_name: "generic_repository".into(),
        },
        attributes: Arc::new(SmallMap::new()),
    });
    let ticket = materializer
        .reserve_workspace(token, key.clone(), spec.clone(), scope)
        .unwrap();
    assert!(!ticket.reservation().display_root().as_path().exists());
    let command = RepositoryWorkspaceCommand::new(
        key.workspace().clone(),
        lease,
        limits,
        resources.clone(),
        scope.clone(),
        &|| true,
    )
    .unwrap();
    let command = runtime
        .block_on(command.initialize_claimed_root(
            ticket.reservation().clone(),
            ticket.claim().clone(),
            resources.clone(),
            scope.clone(),
            &|| true,
        ))
        .unwrap();
    let write = command
        .request(
            ticket.reservation(),
            RepositoryWorkspaceOperation::File {
                path: "input".into(),
                body: Arc::from(body),
                mode: 0o644,
            },
            scope,
            &|| true,
        )
        .unwrap();
    let command = command
        .apply(
            write,
            runtime,
            &crate::runtime::ArchiveDistdirPolicy::default(),
            scope,
            &|| true,
        )
        .unwrap();
    let transcript = &command.epoch().transcripts().unwrap()[0];
    let realization = transcript.realize(transcript.records().len()).unwrap();
    let mut terminal_spec = (*spec).clone();
    if foreign_spec {
        terminal_spec.rule_id.rule_name = "different_terminal_spec".into();
        assert_ne!(terminal_spec, *spec);
    }
    let request = Arc::new(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: key.workspace().clone(),
            canonical_repo: key.repository().clone(),
        },
        repo_spec: terminal_spec,
        kind: RepositoryMaterializationKind::GeneralWorkspace(realization),
    });
    let (command, sealed) = command
        .seal_for_materialization(request.clone(), scope, &|| true)
        .unwrap();
    let adopted = materializer.adopt_workspace(
        token,
        &ticket,
        RepositoryMaterializationGeneration(1),
        sealed,
        scope,
    );
    drop(command);
    drop(ticket);
    if foreign_spec {
        assert!(
            matches!(adopted, Err(crate::runtime::repository_io::RepositorySessionError::NativeWorkspace(message)) if message.as_ref() == "native workspace reservation refused")
        );
        assert_eq!(
            (
                resources.snapshot().native_fds,
                resources.snapshot().backing_bytes
            ),
            (0, 0)
        );
        materializer.discard(token).unwrap();
        return None;
    }
    let results = adopted.unwrap();
    let roots = materializer
        .pin_selected_roots(token, std::slice::from_ref(&request), &results, readers)
        .unwrap();
    // The anchor is projected by the ordinary DICE result producer from the
    // actual accepted native result, rather than reconstructed by this test.
    let materialized = runtime.block_on(async {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut updater = dice.updater();
        updater
            .changed_to([(
                RepositoryMaterializationResultEpochKey {
                    workspace: key.workspace().clone(),
                },
                results.clone(),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let SourcePreparationOutcome::Complete(value) =
            slug_bzlmod_v2::source_preparation::compute_repository_materialization_result(
                &mut transaction,
                request.clone(),
            )
            .await
            .unwrap()
        else {
            panic!("actual adopted root must project a complete materialization")
        };
        value.as_ref().clone().unwrap()
    });
    let anchor = materialized.path_anchor().unwrap().clone();
    let path = NormalizedAbsolutePath::new(anchor.root().as_path().join("input")).unwrap();
    let observed_digest = runtime.block_on(async {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let selected =
            AnchoredPathFileDigestObservationKey::new(anchor.clone(), path.as_path()).unwrap();
        let mut observations = PathObservationEpoch::empty();
        let mut updater = dice.updater();
        updater
            .changed_to(slug_workspace_v2::path_observation_shards(&observations))
            .unwrap();
        updater
            .changed_to([(PathObservationEpochKey, observations.clone())])
            .unwrap();
        let mut transaction = updater.commit().await;
        for round in 0..8 {
            match transaction.compute(&selected).await.unwrap() {
                PathOutcome::Need(need) => {
                    assert!(!need.demands().is_empty());
                    assert!(
                        need.demands()
                            .iter()
                            .all(|demand| demand.namespace() == anchor.namespace()
                                && demand.path().as_path().starts_with(anchor.root().as_path()))
                    );
                    if round == 0 {
                        assert_eq!(need.demands().len(), 1);
                        assert_eq!(need.demands()[0].path(), anchor.root());
                    }
                    // Actual dispatch selects the sealed FD owner. The display
                    // spelling remains absent for every native observation.
                    assert!(!anchor.root().as_path().exists());
                    let added = materializer
                        .observe_native(token, need.demands().iter().cloned())
                        .unwrap();
                    observations = PathObservationEpoch::from_shared(
                        observations
                            .observations()
                            .iter()
                            .chain(added.observations().iter())
                            .map(|(demand, result)| (demand.clone(), result.clone())),
                    )
                    .unwrap();
                    assert!(observations.observations().len() <= 4);
                    let mut updater = transaction.into_updater();
                    updater
                        .changed_to(slug_workspace_v2::path_observation_shards(&observations))
                        .unwrap();
                    updater
                        .changed_to([(PathObservationEpochKey, observations.clone())])
                        .unwrap();
                    transaction = updater.commit().await;
                }
                PathOutcome::Complete(Ok(observed)) => {
                    assert!(
                        round > 0,
                        "the empty epoch must first demand native observations"
                    );
                    assert_eq!(
                        observed.result(),
                        &Ok(PathFileDigest::Present(digest(body)))
                    );
                    assert!(
                        observed
                            .observations()
                            .observations()
                            .keys()
                            .all(|demand| demand.namespace() == anchor.namespace()
                                && demand.path().as_path().starts_with(anchor.root().as_path()))
                    );
                    let Ok(PathFileDigest::Present(actual)) = observed.result() else {
                        panic!("actual selected file must have a digest")
                    };
                    return *actual;
                }
                PathOutcome::Complete(Err(error)) => {
                    panic!("anchored native digest failed: {error:?}")
                }
            }
        }
        panic!("bounded native digest demand loop did not complete")
    });
    let source = SourceArtifactInput::for_native_lease_test(
        CanonicalLabel::parse(&format!("@@{}//:input", key.repository().as_str())).unwrap(),
        anchor.namespace(),
        path.clone(),
        path,
        Some(anchor),
        observed_digest,
    );
    assert!(
        roots
            .lease_for(&source)
            .unwrap()
            .unwrap()
            .physical()
            .is_some()
    );
    Some((
        token,
        Generation {
            request,
            results,
            roots,
            source,
            materialized,
        },
    ))
}

#[test]
#[ignore = "actual compiled CLI claimed root/registry/source integration; finite native supervisor and SLUG_V2_BIN required"]
fn native_claimed_registry_sources_keep_same_display_instances_separate_and_readers_alive() {
    let temporary = tempfile::tempdir().unwrap();
    let workspace = NormalizedAbsolutePath::new(temporary.path()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(26);
    let scope = SourceServiceScope::new_for_test(
        workspace.clone(),
        deadline,
        deadline + Duration::from_secs(1),
        SourceServiceLimits::default(),
    )
    .unwrap();
    let resources = RepositoryRootResources::new(
        RepositoryRootResourceLimits::new(512 * 1024 * 1024, 512, 4 * ARCHIVE_BYTES).unwrap(),
    )
    .unwrap();
    let root_baseline = resources.snapshot().retained_bytes;
    let readers = new_reader_resources().unwrap();
    let reader_baseline = readers.snapshot().retained_bytes;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let owner = crate::runtime::dice::physical_root_owner_for_test();
    let repository =
        CanonicalRepoName::new(format!("{}+source", owner.unique_name().as_str())).unwrap();
    let key = Arc::new(
        RepositoryWorkspaceStableKey::new(workspace.clone(), owner, repository, 0).unwrap(),
    );
    let materializer = RepositoryMaterializer::new_scoped(workspace, scope.clone()).unwrap();

    let (token_a, a) = generation(
        &materializer,
        &key,
        &resources,
        &readers,
        &scope,
        &runtime,
        b"root A\0\xff",
    );
    let display = a.source.anchor().unwrap().root().clone();
    let namespace_a = a.source.namespace();
    assert!(!display.as_path().exists());
    let mut old_reader = NativeSourceReader::open_selected_for_test(
        &a.source,
        &a.roots,
        Some(scope.clone()),
        readers.clone(),
    )
    .unwrap();
    let image = materializer
        .image_source(
            token_a,
            a.source.namespace(),
            &display,
            Some((&a.request, &a.materialized)),
        )
        .unwrap();
    let host_path = temporary.path().join("host-body");
    std::fs::write(&host_path, b"real Host bytes").unwrap();
    let host_demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(&host_path).unwrap(),
        PathObservationOperation::FileBytes,
    );
    let invalid_image_budget = ImageDescriptorBudget::new(64 * 1024);
    let unsupported_physical = PathObservationDemand::input_image(
        a.source.namespace(),
        ImageChargedPath::pin_existing(a.source.real_path().clone(), &invalid_image_budget)
            .unwrap(),
        ImageObservationRequest::new(
            ImageChargedPath::pin_existing(display.clone(), &invalid_image_budget).unwrap(),
            ImageObservationOperation::Lstat,
        ),
    )
    .unwrap();
    for invalid in [
        unsupported_physical,
        PathObservationDemand::new(
            a.source.namespace(),
            NormalizedAbsolutePath::new(display.as_path().parent().unwrap()).unwrap(),
            PathObservationOperation::Lstat,
        ),
        PathObservationDemand::new(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(0)),
            a.source.real_path().clone(),
            PathObservationOperation::FileBytes,
        ),
    ] {
        let before = scope.snapshot();
        assert!(
            materializer
                .observe_native(token_a, [host_demand.clone(), invalid])
                .is_err()
        );
        let after = scope.snapshot();
        assert_eq!(
            (after.source_attempts, after.decoded_bytes),
            (before.source_attempts, before.decoded_bytes),
            "the complete batch must be authorized before the existing Host file is read"
        );
    }
    materializer
        .accept(token_a, std::slice::from_ref(&a.request), vec![])
        .unwrap();

    let (token_b, b) = generation(
        &materializer,
        &key,
        &resources,
        &readers,
        &scope,
        &runtime,
        b"root B\0\xfe",
    );
    assert_eq!(a.source.requested_path(), b.source.requested_path());
    assert_eq!(
        a.source.anchor().unwrap().root(),
        b.source.anchor().unwrap().root()
    );
    assert_ne!(a.source.namespace(), b.source.namespace());
    assert!(a.roots.lease_for(&b.source).is_err());
    assert!(b.roots.lease_for(&a.source).is_err());
    assert!(
        materializer
            .pin_selected_roots(
                token_b,
                std::slice::from_ref(&a.request),
                &a.results,
                &readers
            )
            .is_err()
    );
    // A distinct materializer must not restart the incarnation allocator. This
    // third reservation never creates a root or launches another helper.
    let other_actor =
        RepositoryMaterializer::new_scoped(key.workspace().clone(), scope.clone()).unwrap();
    let other_token = other_actor.begin().unwrap();
    other_actor.preflight_native(other_token, []).unwrap();
    other_actor
        .begin_general_workspace(
            other_token,
            resources.clone(),
            RepositoryWorkspaceRetentionLimits::new(128, 16 * 1024 * 1024, ARCHIVE_BYTES).unwrap(),
            &scope,
        )
        .unwrap();
    let other_ticket = other_actor
        .reserve_workspace(
            other_token,
            key.clone(),
            Arc::new(b.request.repo_spec.clone()),
            &scope,
        )
        .unwrap();
    assert_eq!(other_ticket.reservation().display_root(), &display);
    assert_ne!(
        PathObservationNamespace::Materialization(other_ticket.reservation().instance()),
        a.source.namespace()
    );
    assert_ne!(
        PathObservationNamespace::Materialization(other_ticket.reservation().instance()),
        b.source.namespace()
    );
    drop(other_ticket);
    drop(other_actor);
    // Forged metadata cannot turn this physical selected repo into Host IO or
    // bypass sealed body membership, even if an upload would be a CAS hit.
    for forged in [
        source(&b, PathObservationNamespace::Host, None, b.source.digest()),
        source(&b, b.source.namespace(), None, b.source.digest()),
        source(
            &b,
            b.source.namespace(),
            a.source.anchor().cloned(),
            b.source.digest(),
        ),
        source(
            &b,
            b.source.namespace(),
            b.source.anchor().cloned(),
            digest(b"forged"),
        ),
    ] {
        let before = readers.snapshot();
        assert!(b.roots.lease_for(&forged).is_err());
        assert!(
            NativeSourceReader::open_selected_for_test(
                &forged,
                &b.roots,
                Some(scope.clone()),
                readers.clone()
            )
            .is_err()
        );
        let after = readers.snapshot();
        assert_eq!(
            (
                after.native_fds,
                after.peak_native_fds,
                after.retained_bytes
            ),
            (
                before.native_fds,
                before.peak_native_fds,
                before.retained_bytes
            )
        );
    }
    let counters = scope.snapshot();
    let mut new_reader = NativeSourceReader::open_selected_for_test(
        &b.source,
        &b.roots,
        Some(scope.clone()),
        readers.clone(),
    )
    .unwrap();
    let mut bytes = Vec::new();
    new_reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"root B\0\xfe");
    drop(new_reader);
    assert_eq!(scope.snapshot().decoded_bytes, counters.decoded_bytes);
    assert_eq!(scope.snapshot().source_attempts, counters.source_attempts);
    materializer
        .accept(token_b, std::slice::from_ref(&b.request), vec![])
        .unwrap();
    // Preserve the A -> B accepted replacement proof. Reuse the same real
    // producer for C, then invalidate C's association only after all actual
    // Host/physical reads, without changing its active session token.
    let (token_c, c) = generation(
        &materializer,
        &key,
        &resources,
        &readers,
        &scope,
        &runtime,
        b"root C\0\xfd",
    );
    let before_recheck = scope.snapshot();
    let callback_called = std::cell::Cell::new(false);
    let refused = materializer.observe_with_recheck_hook_for_test(
        token_c,
        vec![
            host_demand,
            PathObservationDemand::new(
                c.source.namespace(),
                c.source.real_path().clone(),
                PathObservationOperation::FileBytes,
            ),
        ],
        || {
            assert!(materializer.session_is_active(token_c));
            callback_called.set(true);
            materializer.replace_source_identity_for_test(token_c, key.repository());
        },
    );
    assert!(callback_called.get());
    assert!(
        matches!(refused, Err(crate::runtime::repository_io::RepositorySessionError::NativeWorkspace(message)) if message.as_ref() == "physical observation association changed")
    );
    assert!(materializer.session_is_active(token_c));
    assert_eq!(
        scope.snapshot().source_attempts,
        before_recheck.source_attempts + 2
    );
    assert!(scope.snapshot().decoded_bytes > before_recheck.decoded_bytes);
    materializer.discard(token_c).unwrap();
    drop(c);
    let counters = scope.snapshot();
    drop(a);
    drop(b);
    drop(materializer);
    assert!(!display.as_path().exists());
    bytes.clear();
    old_reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"root A\0\xff");
    assert_eq!(scope.snapshot().decoded_bytes, counters.decoded_bytes);
    assert_eq!(scope.snapshot().source_attempts, counters.source_attempts);

    let budget = ImageDescriptorBudget::new(1024 * 1024);
    let query = PathObservationDemand::input_image(
        // The exact A instance comes from its retained native lease.
        namespace_a,
        ImageChargedPath::pin_existing(
            NormalizedAbsolutePath::new(display.as_path().join("input")).unwrap(),
            &budget,
        )
        .unwrap(),
        ImageObservationRequest::new(
            ImageChargedPath::pin_existing(display.clone(), &budget).unwrap(),
            ImageObservationOperation::BoundedDigest { max_bytes: 1024 },
        ),
    )
    .unwrap();
    let PathObservationResult::InputImage(result) =
        image.observe(&query, &budget, &|| true).unwrap()
    else {
        panic!("image result")
    };
    assert!(
        matches!(result.data(), ImageObservationData::Digest(PathOperationResult::Present(value)) if *value == digest(b"root A\0\xff"))
    );
    drop(result);
    drop(query);
    drop(image);
    assert_eq!(resources.snapshot().backing_bytes, ARCHIVE_BYTES);
    assert_eq!(readers.snapshot().native_fds, 1);
    old_reader.seek(SeekFrom::Start(0)).unwrap();
    scope.cancel();
    assert!(old_reader.read_to_end(&mut Vec::new()).is_err());
    drop(old_reader);
    assert_eq!(
        (
            readers.snapshot().native_fds,
            readers.snapshot().retained_bytes
        ),
        (0, reader_baseline)
    );
    assert_eq!(
        (
            resources.snapshot().native_fds,
            resources.snapshot().backing_bytes,
            resources.snapshot().retained_bytes
        ),
        (0, 0, root_baseline)
    );
}

#[test]
#[ignore = "actual compiled CLI claimed root/foreign-spec adoption proof; finite native supervisor and SLUG_V2_BIN required"]
fn native_adoption_rejects_foreign_spec_after_actual_history_seal_and_releases_root() {
    let temporary = tempfile::tempdir().unwrap();
    let workspace = NormalizedAbsolutePath::new(temporary.path()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(26);
    let scope = SourceServiceScope::new_for_test(
        workspace.clone(),
        deadline,
        deadline + Duration::from_secs(1),
        SourceServiceLimits::default(),
    )
    .unwrap();
    let resources = RepositoryRootResources::new(
        RepositoryRootResourceLimits::new(512 * 1024 * 1024, 512, 2 * ARCHIVE_BYTES).unwrap(),
    )
    .unwrap();
    let baseline = resources.snapshot().retained_bytes;
    let readers = new_reader_resources().unwrap();
    let reader_baseline = readers.snapshot().retained_bytes;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let owner = crate::runtime::dice::physical_root_owner_for_test();
    let key = Arc::new(
        RepositoryWorkspaceStableKey::new(
            workspace.clone(),
            owner,
            CanonicalRepoName::new("foreign_spec").unwrap(),
            0,
        )
        .unwrap(),
    );
    let materializer = RepositoryMaterializer::new_scoped(workspace, scope.clone()).unwrap();
    assert!(
        generation_with_adoption_spec(
            &materializer,
            &key,
            &resources,
            &readers,
            &scope,
            &runtime,
            b"actual sealed bytes",
            true,
        )
        .is_none()
    );
    assert!(resources.snapshot().peak_backing_bytes >= ARCHIVE_BYTES);
    assert!(resources.snapshot().peak_native_fds > 0);
    drop(materializer);
    let after = resources.snapshot();
    assert_eq!(
        (after.native_fds, after.backing_bytes, after.retained_bytes),
        (0, 0, baseline)
    );
    assert_eq!(
        (
            readers.snapshot().native_fds,
            readers.snapshot().retained_bytes
        ),
        (0, reader_baseline)
    );
}

#[test]
fn initialization_refuses_foreign_binding_before_helper_or_backing_admission() {
    let workspace = NormalizedAbsolutePath::new("/initialization-refusal").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let make_scope = || {
        SourceServiceScope::new_for_test(
            workspace.clone(),
            deadline,
            deadline + Duration::from_secs(1),
            SourceServiceLimits::default(),
        )
        .unwrap()
    };
    let scope = make_scope();
    let foreign_scope = make_scope();
    let make_bank = || {
        RepositoryRootResources::new(
            RepositoryRootResourceLimits::new(16 * 1024 * 1024, 128, 2 * ARCHIVE_BYTES).unwrap(),
        )
        .unwrap()
    };
    let resources = make_bank();
    let foreign_resources = make_bank();
    let limits =
        RepositoryWorkspaceRetentionLimits::new(128, 16 * 1024 * 1024, ARCHIVE_BYTES).unwrap();
    let actor = RepositoryMaterializer::new_scoped(workspace.clone(), scope.clone()).unwrap();
    let token = actor.begin().unwrap();
    actor.preflight_native(token, []).unwrap();
    let lease = actor
        .begin_general_workspace(token, resources.clone(), limits, &scope)
        .unwrap();
    let owner = crate::runtime::dice::physical_root_owner_for_test();
    let spec = Arc::new(RepoSpec {
        rule_id: RepoRuleId {
            bzl_file: CanonicalLabel::parse("@@//:repo.bzl").unwrap(),
            rule_name: "generic".into(),
        },
        attributes: Arc::new(SmallMap::new()),
    });
    let mut tickets = Vec::new();
    for name in ["dep", "other"] {
        let key = Arc::new(
            RepositoryWorkspaceStableKey::new(
                workspace.clone(),
                owner.clone(),
                CanonicalRepoName::new(name).unwrap(),
                0,
            )
            .unwrap(),
        );
        tickets.push(
            actor
                .reserve_workspace(token, key, spec.clone(), &scope)
                .unwrap(),
        );
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    for case in 0..4 {
        let command = RepositoryWorkspaceCommand::new(
            workspace.clone(),
            if case == 0 {
                lease.checked_add(1).unwrap()
            } else {
                lease
            },
            limits,
            if case == 1 {
                foreign_resources.clone()
            } else {
                resources.clone()
            },
            scope.clone(),
            &|| true,
        )
        .unwrap();
        let result = runtime.block_on(command.initialize_claimed_root(
            tickets[0].reservation().clone(),
            tickets[if case == 3 { 1 } else { 0 }].claim().clone(),
            resources.clone(),
            if case == 2 {
                foreign_scope.clone()
            } else {
                scope.clone()
            },
            &|| true,
        ));
        match case {
            0 => assert!(matches!(
                result,
                Err(PhysicalRootError::Protocol(
                    slug_bzlmod_v2::RepositoryWorkspaceError::ForeignReservation
                ))
            )),
            1 | 3 => assert!(matches!(result, Err(PhysicalRootError::Reservation))),
            2 => assert!(matches!(
                result,
                Err(PhysicalRootError::SourceService(
                    crate::runtime::dice::source_service::SourceServiceError::ForeignScope
                ))
            )),
            _ => unreachable!(),
        }
        for bank in [&resources, &foreign_resources] {
            assert_eq!(
                (
                    bank.snapshot().peak_native_fds,
                    bank.snapshot().peak_backing_bytes
                ),
                (0, 0)
            );
        }
    }
    actor.discard(token).unwrap();
}
