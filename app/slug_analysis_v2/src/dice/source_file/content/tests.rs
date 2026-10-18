//! Actual observed main-Source acquisition and exact immutable response ownership.
use dice::DetectCycles;
use dice::Dice;
use slug_workspace_v2::PathIoErrorKind;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationError;

use super::*;
fn path(s: &str) -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new(s).unwrap()
}
fn label() -> CanonicalLabel {
    CanonicalLabel::parse("@@//:template").unwrap()
}
fn demand(s: &str, op: PathObservationOperation) -> PathObservationDemand {
    PathObservationDemand::new(PathObservationNamespace::Host, path(s), op)
}
fn lstat(kind: PathNodeKind) -> PathObservationResult {
    PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
        kind, 1, 2, 3, 4, 0o644,
    )))
}
fn epoch(
    state: PathOperationResult<slug_workspace_v2::PathLstat>,
    response: Option<Arc<PathObservationResult>>,
) -> PathObservationEpoch {
    let mut rows = vec![
        (
            demand("/", PathObservationOperation::Lstat),
            Arc::new(lstat(PathNodeKind::Directory)),
        ),
        (
            demand("/workspace", PathObservationOperation::Lstat),
            Arc::new(lstat(PathNodeKind::Directory)),
        ),
        (
            demand("/workspace/template", PathObservationOperation::Lstat),
            Arc::new(PathObservationResult::Lstat(state)),
        ),
    ];
    if let Some(value) = response {
        rows.push((
            demand("/workspace/template", PathObservationOperation::FileBytes),
            value,
        ));
    }
    PathObservationEpoch::from_shared(rows).unwrap()
}
#[tokio::test]
async fn observed_source_needs_actual_file_bytes_and_retains_exact_response_without_copy() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut ctx = dice.updater().commit().await;
    assert!(matches!(
        resolve_source_file_input_observed(&mut ctx, &path("/workspace"), &label()).await,
        LoadingPreparationOutcome::Need(_)
    ));
    drop(ctx);
    let present =
        PathOperationResult::Present(PathLstat::new(PathNodeKind::RegularFile, 1, 2, 3, 4, 0o644));
    let mut update = dice.updater();
    update
        .changed_to(vec![(
            PathObservationEpochKey,
            epoch(present.clone(), None),
        )])
        .unwrap();
    let mut ctx = update.commit().await;
    let LoadingPreparationOutcome::Need(needs) =
        resolve_source_file_input_observed(&mut ctx, &path("/workspace"), &label()).await
    else {
        panic!("content observation must be requested")
    };
    assert_eq!(
        needs.path_observations().unwrap().demands(),
        &[demand(
            "/workspace/template",
            PathObservationOperation::FileBytes
        )]
    );
    drop(ctx);
    let body: Arc<[u8]> = Arc::from(&b"actual\0\xFF"[..]);
    let response = Arc::new(PathObservationResult::FileBytes(
        PathOperationResult::Present(body.clone()),
    ));
    let mut update = dice.updater();
    update
        .changed_to(vec![(
            PathObservationEpochKey,
            epoch(present, Some(response.clone())),
        )])
        .unwrap();
    let mut ctx = update.commit().await;
    let LoadingPreparationOutcome::Complete(Ok(observed)) =
        resolve_source_file_input_observed(&mut ctx, &path("/workspace"), &label()).await
    else {
        panic!("observed content")
    };
    let content = observed.result().as_ref().unwrap();
    assert_eq!(content.bytes(), &*body);
    assert_eq!(content.bytes().as_ptr(), body.as_ptr());
    assert!(content.catalog_owner().is_none());
    let Content::Filesystem(owner) = &content.0 else {
        panic!("main source cannot acquire catalog identity")
    };
    assert!(Arc::ptr_eq(owner, &response));
    assert!(Arc::ptr_eq(
        observed
            .observations()
            .get(&demand(
                "/workspace/template",
                PathObservationOperation::FileBytes
            ))
            .unwrap(),
        &response
    ));
    assert!(observed.observations().observations().len() >= 4);
}
#[tokio::test]
async fn observed_missing_and_nonregular_sources_refuse_before_content_demand() {
    for state in [
        PathOperationResult::Missing,
        PathOperationResult::Present(PathLstat::new(PathNodeKind::Directory, 1, 2, 3, 4, 0o755)),
        PathOperationResult::Present(PathLstat::new(PathNodeKind::SpecialFile, 1, 2, 3, 4, 0o644)),
    ] {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut update = dice.updater();
        update
            .changed_to(vec![(PathObservationEpochKey, epoch(state, None))])
            .unwrap();
        let mut ctx = update.commit().await;
        let LoadingPreparationOutcome::Complete(Ok(observed)) =
            resolve_source_file_input_observed(&mut ctx, &path("/workspace"), &label()).await
        else {
            panic!("kind refusal precedes content need")
        };
        assert!(observed.result().is_err());
        assert!(
            observed
                .observations()
                .get(&demand(
                    "/workspace/template",
                    PathObservationOperation::FileBytes
                ))
                .is_none()
        );
    }
}
#[test]
fn file_bytes_response_missing_error_mismatch_conflict_and_last_guard_remain_closed() {
    let d = demand("/workspace/template", PathObservationOperation::FileBytes);
    for response in [
        PathObservationResult::FileBytes(PathOperationResult::Missing),
        PathObservationResult::FileBytes(PathOperationResult::Error(PathObservationError::Io {
            kind: PathIoErrorKind::PermissionDenied,
            raw_os_error: Some(13),
        })),
    ] {
        let response = Arc::new(response);
        let observed = finish_file_bytes(
            PathObservationEpoch::empty(),
            d.clone(),
            response.clone(),
            &label(),
        )
        .unwrap();
        assert!(observed.result().is_err());
        assert!(Arc::ptr_eq(
            observed.observations().get(&d).unwrap(),
            &response
        ));
    }
    assert!(
        finish_file_bytes(
            PathObservationEpoch::empty(),
            d.clone(),
            Arc::new(lstat(PathNodeKind::RegularFile)),
            &label()
        )
        .is_err()
    );
    let a = Arc::new(PathObservationResult::FileBytes(
        PathOperationResult::Present(Arc::from(&b"a"[..])),
    ));
    let b = Arc::new(PathObservationResult::FileBytes(
        PathOperationResult::Present(Arc::from(&b"b"[..])),
    ));
    assert!(
        finish_file_bytes(
            PathObservationEpoch::from_shared([(d.clone(), a)]).unwrap(),
            d.clone(),
            b,
            &label()
        )
        .is_err()
    );
    let bytes: Arc<[u8]> = Arc::from(&b"retained original bytes"[..]);
    let weak_bytes = Arc::downgrade(&bytes);
    let response = Arc::new(PathObservationResult::FileBytes(
        PathOperationResult::Present(bytes.clone()),
    ));
    let weak_response = Arc::downgrade(&response);
    let observed =
        finish_file_bytes(PathObservationEpoch::empty(), d, response.clone(), &label()).unwrap();
    drop(bytes);
    drop(response);
    let cloned = observed.clone();
    drop(observed);
    assert!(weak_bytes.upgrade().is_some());
    assert!(weak_response.upgrade().is_some());
    assert_eq!(
        cloned.result().as_ref().unwrap().bytes(),
        b"retained original bytes"
    );
    drop(cloned);
    assert!(weak_bytes.upgrade().is_none());
    assert!(weak_response.upgrade().is_none());
}
