//! Shared finalization preserves native source values and typed terminal paths.
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;

use super::*;

fn label() -> CanonicalLabel {
    CanonicalLabel::parse("@@dep+//pkg:source.bzl").unwrap()
}
fn observations(path: &NormalizedAbsolutePath, bytes: &Arc<[u8]>) -> PathObservationEpoch {
    PathObservationEpoch::new([(
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            path.dupe(),
            PathObservationOperation::FileBytes,
        ),
        PathObservationResult::FileBytes(PathOperationResult::Present(bytes.dupe())),
    )])
    .unwrap()
}
#[test]
fn both_request_finalizers_preserve_exact_reader_paths_bytes_and_observation_guards() {
    let bytes: Arc<[u8]> = Arc::from(b"def source(): return 1\n".as_slice());
    let path = NormalizedAbsolutePath::new("/workspace/dep/pkg/source.bzl").unwrap();
    let observed = observations(&path, &bytes);
    let source = HostRepositorySourceFileValue::Present {
        bytes: bytes.dupe(),
        logical_path: path.dupe(),
    };
    let root = finish_external_bzl_source(
        ExternalBzlSourceChild::Root(Ok(source.clone()), observed.dupe()),
        label(),
    )
    .unwrap();
    let canonical = finish_external_bzl_source(
        ExternalBzlSourceChild::Observation(
            Arc::new(Ok(HostRepositorySourceObservation::Request(source))),
            observed.dupe(),
        ),
        label(),
    )
    .unwrap();
    for value in [&root, &canonical] {
        assert!(Arc::ptr_eq(&value.bytes, &bytes));
        assert_eq!(value.source_name, external_source_name(&path).unwrap());
        assert_eq!(value.presentation_path, path.as_path());
        assert_eq!(value.observations, observed);
        for (demand, original) in observed.observations() {
            assert!(Arc::ptr_eq(
                original,
                value.observations.observations().get(demand).unwrap()
            ));
        }
    }
    let other = NormalizedAbsolutePath::new("/workspace/other/source.bzl").unwrap();
    let distinct = present(bytes, other, label(), observed).unwrap();
    assert_ne!(root.presentation_path, distinct.presentation_path);
    assert_ne!(root.source_name, distinct.source_name);
}
#[test]
fn absent_and_compute_failures_keep_typed_attribution_and_observations() {
    let bytes: Arc<[u8]> = Arc::from(b"VALUE = 1\n".as_slice());
    let path = NormalizedAbsolutePath::new("/workspace/dep/pkg/source.bzl").unwrap();
    let observed = observations(&path, &bytes);
    for child in [
        ExternalBzlSourceChild::Root(Ok(HostRepositorySourceFileValue::Absent), observed.dupe()),
        ExternalBzlSourceChild::Observation(
            Arc::new(Ok(HostRepositorySourceObservation::Request(
                HostRepositorySourceFileValue::Absent,
            ))),
            observed.dupe(),
        ),
    ] {
        let SourcePreparationOutcome::Complete(Ok((result, actual))) =
            finish_external_bzl_source(child, label()).unwrap_err()
        else {
            panic!("absent remains a Complete typed module error")
        };
        assert!(
            matches!(result.as_ref(), Err(ExternalBzlModuleError::Absent { label: actual }) if actual == &label())
        );
        assert_eq!(actual, observed);
    }
    let SourcePreparationOutcome::Complete(Ok((result, actual))) = finish_external_bzl_source(
        ExternalBzlSourceChild::Compute(Arc::from("compute failed")),
        label(),
    )
    .unwrap_err() else {
        panic!("compute failure remains a Complete typed module error")
    };
    assert!(
        matches!(result.as_ref(), Err(ExternalBzlModuleError::SourceCompute { label: actual, message }) if actual == &label() && message.as_ref() == "compute failed")
    );
    assert_eq!(actual, PathObservationEpoch::empty());
}
