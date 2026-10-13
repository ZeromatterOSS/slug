use super::*;
use crate::GeneratedRepositoryArchiveEffect;
use crate::GeneratedRepositoryArchiveFormat;
use crate::RepositoryArchiveCallId;
use crate::RepositoryArchiveRequest;

fn request(index: usize, digest: u8) -> RepositoryArchiveRequest {
    let owner = Arc::new(crate::HostSelectedExtensionOwner::testing("independent+"));
    let archive = GeneratedRepositoryArchiveEffect::new(
        Arc::from(["https://example.test/independent.crate".into()]),
        [digest; 32],
        "".into(),
        GeneratedRepositoryArchiveFormat::TarGz,
        "ordinary id".into(),
        "".into(),
    )
    .unwrap();
    RepositoryArchiveRequest::new(
        RepositoryArchiveCallId::new(owner, 0, index),
        "independent+data".into(),
        archive,
        GeneratedRepositoryFileEffectPlan::builder().finish(),
    )
}

#[test]
fn archive_need_union_tracks_complete_requests_and_reuses_only_equal_frontiers() {
    let a = SourcePreparationNeeds::repository_archive(request(0, 1));
    let equal = SourcePreparationNeeds::repository_archive(request(0, 1));
    let reused = a.try_union(&equal).unwrap();
    assert!(Arc::ptr_eq(
        &a.repository_archives,
        &reused.repository_archives
    ));
    assert_eq!(reused.repository_archives().len(), 1);
    assert!(matches!(
        a.try_union(&SourcePreparationNeeds::repository_archive(request(0, 2))),
        Err(SourcePreparationNeedsError::ConflictingRepositoryArchive { .. })
    ));
    let other = SourcePreparationNeeds::repository_archive(request(1, 1));
    let combined = a.try_union(&other).unwrap();
    assert_eq!(combined.repository_archives().len(), 2);
    assert_eq!(combined, other.try_union(&a).unwrap());
    assert_eq!(a.try_union(&equal).unwrap(), a);
}
