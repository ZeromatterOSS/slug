//! Pure protocol metadata fixtures; these values create no native root or grant.
use super::*;
use crate::HostSelectedExtensionOwner;
use crate::RepoRuleId;
use crate::RepositoryWorkspaceEpoch;
use crate::RepositoryWorkspaceReservation;
use crate::RepositoryWorkspaceRetentionLimits;
use crate::RepositoryWorkspaceTranscript;

fn workspace() -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new("/workspace").unwrap()
}
fn command_context(lease: u64) -> Arc<RepositoryWorkspaceContext> {
    let limits =
        RepositoryWorkspaceRetentionLimits::new(128, 16 * 1024 * 1024, 1024 * 1024 * 1024).unwrap();
    RepositoryWorkspaceContext::from_epoch(
        &RepositoryWorkspaceEpoch::fresh(workspace(), lease, limits).unwrap(),
        &workspace(),
    )
    .unwrap()
}
fn request() -> Arc<RepositoryMaterializationRequest> {
    let spec = RepoSpec {
        rule_id: RepoRuleId {
            bzl_file: CanonicalLabel::parse("@@//:repo.bzl").unwrap(),
            rule_name: "authored_generic_repository".into(),
        },
        attributes: Arc::new(SmallMap::new()),
    };
    let reservation = Arc::new(
        RepositoryWorkspaceReservation::new(
            workspace(),
            Arc::new(HostSelectedExtensionOwner::testing("owner")),
            CanonicalRepoName::new("owner+repo").unwrap(),
            0,
            Arc::new(spec.clone()),
            NormalizedAbsolutePath::new("/logical/repo").unwrap(),
            PathObservationInstanceId::new(11),
            7,
            command_context(7).limits(),
        )
        .unwrap(),
    );
    let realization = RepositoryWorkspaceTranscript::new(reservation.clone(), 0o755)
        .unwrap()
        .realize(0)
        .unwrap();
    Arc::new(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: workspace(),
            canonical_repo: reservation.repository().clone(),
        },
        repo_spec: spec,
        kind: RepositoryMaterializationKind::GeneralWorkspace(realization),
    })
}
fn entry(request: Arc<RepositoryMaterializationRequest>) -> RepositoryMaterializationEpochEntry {
    let RepositoryMaterializationKind::GeneralWorkspace(realization) = &request.kind else {
        unreachable!()
    };
    let result =
        RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable {
            source_identity: realization.source_identity().clone(),
            generation_root: realization
                .reservation()
                .display_root()
                .as_path()
                .to_path_buf(),
            observation_instance: realization.reservation().instance(),
            path_policy: RepositoryMaterializationPathPolicy::GenerationRoot,
        });
    RepositoryMaterializationEpochEntry { request, result }
}

#[tokio::test]
async fn materialization_projection_returns_actual_bound_row_and_preserves_absence_and_errors() {
    use dice::DetectCycles;
    use dice::Dice;
    let request = request();
    let expected = Arc::new(request.as_ref().clone());
    assert!(!Arc::ptr_eq(&request, &expected));
    let context = command_context(7);
    let key = RepositoryMaterializationProjection::new(context.clone(), expected.clone()).unwrap();
    let empty = RepositoryMaterializationResultEpoch::new(workspace(), []).unwrap();
    let absent =
        RepositoryMaterializationObservation::from_epoch(&empty, context.clone(), expected.clone())
            .unwrap();
    assert!(absent.entry().is_none());
    assert!(RepositoryMaterializationProjection::validity(&Ok(
        absent.clone()
    )));
    let actual = entry(request.clone());
    let published =
        RepositoryMaterializationResultEpoch::new(workspace(), [actual.clone()]).unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let base = RepositoryMaterializationResultEpochKey {
        workspace: workspace(),
    };
    let mut update = dice.updater();
    update.changed_to([(base.clone(), empty)]).unwrap();
    let mut tx = update.commit().await;
    let opaque = tx.compute_opaque(&base).await.unwrap();
    assert_eq!(tx.projection(&opaque, &key).unwrap().unwrap(), absent);
    drop(tx);
    let mut update = dice.updater();
    update
        .changed_to([(base.clone(), published.clone())])
        .unwrap();
    let mut tx = update.commit().await;
    let opaque = tx.compute_opaque(&base).await.unwrap();
    let observed = tx.projection(&opaque, &key).unwrap().unwrap();
    assert_eq!(observed.entry(), Some(&actual));
    assert!(
        Arc::ptr_eq(&observed.entry().unwrap().request, &request),
        "retain the actual selected request, not the equal foreign expected request"
    );
    assert!(!Arc::ptr_eq(&observed.entry().unwrap().request, &expected));
    let failed = RepositoryMaterializationEpochEntry {
        request: request.clone(),
        result: RepositoryMaterializationResult::SpecError("actual selected failure".into()),
    };
    let failed_epoch =
        RepositoryMaterializationResultEpoch::new(workspace(), [failed.clone()]).unwrap();
    let failure = RepositoryMaterializationObservation::from_epoch(
        &failed_epoch,
        context.clone(),
        request.clone(),
    )
    .unwrap();
    assert_eq!(failure.entry(), Some(&failed));
    assert_ne!(observed, failure);
    assert_eq!(
        RepositoryMaterializationObservation::from_epoch(
            &RepositoryMaterializationResultEpoch::new(
                NormalizedAbsolutePath::new("/foreign").unwrap(),
                []
            )
            .unwrap(),
            context,
            request
        ),
        Err(RepositoryWorkspaceError::ForeignReservation)
    );
}

#[test]
fn materialization_projection_refuses_foreign_full_request_spec_and_general_lease() {
    let request = request();
    let context = command_context(7);
    let published =
        RepositoryMaterializationResultEpoch::new(workspace(), [entry(request.clone())]).unwrap();
    assert_eq!(
        RepositoryMaterializationProjection::new(command_context(8), request.clone()),
        Err(RepositoryWorkspaceError::ForeignReservation)
    );
    let mut changed = request.as_ref().clone();
    changed.repo_spec.rule_id.rule_name = "SECRET_SPEC_NOT_DEBUG".into();
    assert_eq!(
        RepositoryMaterializationProjection::new(context.clone(), Arc::new(changed)),
        Err(RepositoryWorkspaceError::ForeignReservation)
    );
    // Same final tree is not full upstream request identity: an extra genuine
    // protocol observation changes history and must refuse the prior row.
    let mut changed = request.as_ref().clone();
    let RepositoryMaterializationKind::GeneralWorkspace(realization) = &request.kind else {
        unreachable!()
    };
    let transcript = realization.transcript();
    let next = transcript
        .complete(
            transcript
                .request(crate::RepositoryWorkspaceOperation::Exists("".into()))
                .unwrap(),
            crate::RepositoryWorkspaceResult::Exists(true),
            crate::RepositoryWorkspaceMode::Archive,
            transcript.current().entries().to_vec(),
        )
        .unwrap()
        .realize(1)
        .unwrap();
    changed.kind = RepositoryMaterializationKind::GeneralWorkspace(next);
    let changed = Arc::new(changed);
    assert!(RepositoryMaterializationProjection::new(context.clone(), changed.clone()).is_ok());
    assert_eq!(
        RepositoryMaterializationObservation::from_epoch(
            &published,
            context.clone(),
            changed.clone()
        ),
        Err(RepositoryWorkspaceError::ReplayConflict)
    );
    let key = RepositoryMaterializationProjection::new(context, request).unwrap();
    assert!(!format!("{key:?}").contains("authored_generic_repository"));
    assert!(!RepositoryMaterializationProjection::validity(&Err(
        RepositoryWorkspaceError::ReplayConflict
    )));
}

#[test]
fn ordinary_materialization_projection_refuses_same_id_changed_spec_and_kind() {
    use crate::OverrideAttributeValue;

    let context = command_context(7);
    let attributes = |url: &str| {
        let mut values = SmallMap::new();
        values.insert(
            CompactString::new("url"),
            OverrideAttributeValue::String(url.into()),
        );
        values.insert(
            CompactString::new("sha256"),
            OverrideAttributeValue::String("11".repeat(32).into()),
        );
        Arc::new(values)
    };
    let request = Arc::new(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: workspace(),
            canonical_repo: CanonicalRepoName::new("ordinary_archive").unwrap(),
        },
        repo_spec: RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:http.bzl")
                    .unwrap(),
                rule_name: "http_archive".into(),
            },
            attributes: attributes("https://origin.invalid/archive.tar.gz"),
        },
        kind: RepositoryMaterializationKind::Immutable,
    });
    // This is only an ordinary pure result row. It grants no archive/root/FD
    // authority and bypasses no native acquisition producer.
    let actual = RepositoryMaterializationEpochEntry {
        request: request.clone(),
        result: RepositoryMaterializationResult::SpecError("selected fixture outcome".into()),
    };
    let epoch = RepositoryMaterializationResultEpoch::new(workspace(), [actual.clone()]).unwrap();
    assert_eq!(
        RepositoryMaterializationObservation::from_epoch(&epoch, context.clone(), request.clone())
            .unwrap()
            .entry(),
        Some(&actual)
    );
    let mut changed = request.as_ref().clone();
    changed.repo_spec.attributes = attributes("https://mirror.invalid/archive.tar.gz");
    assert_eq!(changed.id, request.id);
    assert_eq!(changed.kind, request.kind);
    let changed = Arc::new(changed);
    assert!(RepositoryMaterializationProjection::new(context.clone(), changed.clone()).is_ok());
    assert_eq!(
        RepositoryMaterializationObservation::from_epoch(&epoch, context.clone(), changed),
        Err(RepositoryWorkspaceError::ReplayConflict)
    );
    // An independently foreign ordinary kind is also a conflict, not absence.
    let mut changed_kind = request.as_ref().clone();
    changed_kind.kind = RepositoryMaterializationKind::Local {
        logical_root: NormalizedAbsolutePath::new("/workspace/archive").unwrap(),
    };
    let changed_kind = Arc::new(changed_kind);
    assert!(
        RepositoryMaterializationProjection::new(context.clone(), changed_kind.clone()).is_ok()
    );
    assert_eq!(
        RepositoryMaterializationObservation::from_epoch(&epoch, context, changed_kind),
        Err(RepositoryWorkspaceError::ReplayConflict)
    );
}
