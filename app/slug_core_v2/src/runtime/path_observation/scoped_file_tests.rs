//! Focused Linux controls for the original epoch and scoped refusal boundary.
use std::time::Duration;
use std::time::Instant;

use super::*;
use crate::runtime::dice::source_service::HOST_FILE_OBJECT_BYTES;
use crate::runtime::dice::source_service::SourceCounter;
use crate::runtime::dice::source_service::SourceServiceLimits;

fn scope(root: &NormalizedAbsolutePath) -> Arc<SourceServiceScope> {
    let deadline = Instant::now() + Duration::from_secs(2);
    SourceServiceScope::new_for_test(
        root.clone(),
        deadline,
        deadline + Duration::from_secs(1),
        SourceServiceLimits::default(),
    )
    .unwrap()
}

fn bytes_demand(path: &std::path::Path) -> PathObservationDemand {
    PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(path.to_path_buf()).unwrap(),
        PathObservationOperation::FileBytes,
    )
}

#[test]
fn scoped_file_epoch_preserves_bytes_missing_directory_and_original_arcs() {
    let directory = tempfile::tempdir().unwrap();
    let root = NormalizedAbsolutePath::new(directory.path().to_path_buf()).unwrap();
    let file = directory.path().join("body");
    let absent = directory.path().join("absent");
    let bytes = b"a\0b\xff";
    std::fs::write(&file, bytes).unwrap();
    let demands = [
        bytes_demand(&file),
        bytes_demand(&absent),
        bytes_demand(directory.path()),
    ];
    let owner = scope(&root);
    let observed = observe_native_scoped(&directory, [], demands.clone(), Some(&owner)).unwrap();
    let ordinary = observe_native(&directory, [], demands.clone()).unwrap();
    assert_eq!(observed, ordinary);
    match observed.get(&demands[0]).unwrap().as_ref() {
        PathObservationResult::FileBytes(PathOperationResult::Present(body)) => {
            assert_eq!(body.as_ref(), bytes);
            let retained = body.clone();
            assert!(Arc::ptr_eq(body, &retained));
        }
        value => panic!("unexpected file observation: {value:?}"),
    }
    assert_eq!(
        observed.get(&demands[1]).unwrap().as_ref(),
        &PathObservationResult::FileBytes(PathOperationResult::Missing)
    );
    assert_eq!(
        observed.get(&demands[2]).unwrap().as_ref(),
        &PathObservationResult::FileBytes(PathOperationResult::Error(
            PathObservationError::WrongKind {
                expected: PathNodeKind::RegularFile,
                actual: PathNodeKind::Directory,
            }
        ))
    );
    let counters = owner.snapshot();
    assert_eq!(counters.source_attempts, 3);
    assert_eq!(counters.decoded_bytes, bytes.len() as u64);
    assert_eq!(counters.registry_attempts, 0);
    assert_eq!(counters.registry_admitted_bytes, 0);
    assert_eq!(counters.transferred_bytes, 0);
    assert_eq!(counters.written_bytes, 0);
    assert_eq!(counters.live_buffer_bytes, 0);
}

#[test]
fn scoped_file_epoch_validates_whole_demand_set_and_aborts_operational_failures() {
    let directory = tempfile::tempdir().unwrap();
    let root = NormalizedAbsolutePath::new(directory.path().to_path_buf()).unwrap();
    let file = directory.path().join("body");
    std::fs::write(&file, b"value").unwrap();
    let valid = bytes_demand(&file);
    let unknown = PathObservationDemand::new(
        PathObservationNamespace::Materialization(PathObservationInstanceId::new(9)),
        root.clone(),
        PathObservationOperation::Lstat,
    );
    let owner = scope(&root);
    assert_eq!(
        observe_native_scoped(
            &directory,
            [],
            [valid.clone(), unknown.clone()],
            Some(&owner)
        ),
        Err(PathObservationKernelError::UnknownDemandInstance(unknown))
    );
    assert_eq!(owner.snapshot().source_attempts, 0);
    assert_eq!(
        observe_native_scoped(&directory, [], [valid.clone(), valid.clone()], Some(&owner)),
        Err(PathObservationKernelError::DuplicateDemand(valid.clone()))
    );
    assert_eq!(owner.snapshot().source_attempts, 0);
    owner.cancel();
    assert_eq!(
        observe_native_scoped(&directory, [], [valid.clone()], Some(&owner)),
        Err(PathObservationKernelError::SourceService(
            SourceServiceError::Cancelled
        ))
    );
    assert_eq!(owner.snapshot().source_attempts, 0);
    let owner = scope(&root);
    std::fs::File::create(&file)
        .unwrap()
        .set_len((HOST_FILE_OBJECT_BYTES + 1) as u64)
        .unwrap();
    assert_eq!(
        observe_native_scoped(&directory, [], [valid], Some(&owner)),
        Err(PathObservationKernelError::SourceService(
            SourceServiceError::Limit(SourceCounter::DecodedBytes)
        ))
    );
    assert_eq!(owner.snapshot().source_attempts, 1);
    assert_eq!(owner.snapshot().decoded_bytes, 0);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
}
