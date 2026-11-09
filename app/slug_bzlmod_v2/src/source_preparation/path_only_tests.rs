//! Pure Need-shape tests, granting no materialization or source authority.
use super::*;

fn paths(operation: PathObservationOperation) -> SourcePreparationNeeds {
    SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new("/path-only/input").unwrap(),
        operation,
    )))
}

#[test]
fn only_path_accessor_borrows_whole_channel_without_filtering_operations() {
    for operation in [
        PathObservationOperation::Lstat,
        PathObservationOperation::FileBytes,
    ] {
        let need = paths(operation);
        assert!(std::ptr::eq(
            need.only_path_observations().unwrap(),
            need.path_observations().unwrap(),
        ));
        assert_eq!(need.only_path_observations().unwrap().demands().len(), 1);
    }
}

#[test]
fn mixed_and_nonpath_needs_never_expose_a_partial_path_channel() {
    let path = paths(PathObservationOperation::Lstat);
    let environment = SourcePreparationNeeds::environment(
        crate::NeedRepositoryEnvironmentNames::new(
            NormalizedAbsolutePath::new("/path-only").unwrap(),
            crate::RepositoryEnvironmentNameFrontier::from_unsorted([CompactString::new("ENV")]),
        )
        .unwrap(),
    );
    let repository = SourcePreparationNeeds::repository(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: NormalizedAbsolutePath::new("/path-only").unwrap(),
            canonical_repo: CanonicalRepoName::new("input").unwrap(),
        },
        repo_spec: RepoSpec {
            rule_id: crate::RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@//:repository.bzl").unwrap(),
                rule_name: "repository".into(),
            },
            attributes: Arc::default(),
        },
        kind: RepositoryMaterializationKind::Local {
            logical_root: NormalizedAbsolutePath::new("/path-only/input").unwrap(),
        },
    });
    for other in [environment, repository] {
        assert!(other.only_path_observations().is_none());
        for mixed in [
            path.try_union(&other).unwrap(),
            other.try_union(&path).unwrap(),
        ] {
            assert!(mixed.path_observations().is_some());
            assert!(mixed.only_path_observations().is_none());
        }
    }
    let mut absent = path;
    absent.path_observations = None;
    assert!(absent.only_path_observations().is_none());
}
