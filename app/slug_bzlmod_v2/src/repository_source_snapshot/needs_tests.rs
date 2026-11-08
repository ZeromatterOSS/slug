use super::*;
use crate::SourcePreparationNeeds;
use crate::SourcePreparationNeedsError;

// These private mutations isolate map-slot/full-value equality and its count
// bound. They do not test source projection or grant native source authority.
fn slot(index: usize) -> RepositorySourceSnapshotRequest {
    let mut value = request(7).as_ref().clone();
    value.repository = CanonicalRepoName::new(format!("source{index}")).unwrap();
    value.mapping_context = value.repository.clone();
    value
}
fn materialization(spec_value: &str) -> Arc<RepositoryMaterializationRequest> {
    let mut attributes = starlark_map::small_map::SmallMap::new();
    attributes.insert(
        CompactString::new("url"),
        crate::OverrideAttributeValue::String(spec_value.into()),
    );
    Arc::new(RepositoryMaterializationRequest {
        id: crate::RepositoryMaterializationRequestId {
            workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
            canonical_repo: CanonicalRepoName::new("bazel_tools").unwrap(),
        },
        repo_spec: RepoSpec {
            rule_id: crate::RepoRuleId {
                bzl_file: slug_identity_v2::CanonicalLabel::parse(
                    "@@bazel_tools//tools/build_defs/repo:http.bzl",
                )
                .unwrap(),
                rule_name: "http_archive".into(),
            },
            attributes: Arc::new(attributes),
        },
        kind: RepositoryMaterializationKind::Immutable,
    })
}

#[test]
fn source_only_need_keeps_other_channels_empty_and_unions_without_loss() {
    let request = request(7);
    let source = SourcePreparationNeeds::source_snapshot(request.clone());
    assert_eq!(source.source_snapshots().len(), 1);
    assert!(Arc::ptr_eq(
        source.source_snapshots().get(request.repository()).unwrap(),
        &request
    ));
    assert!(source.repository_materializations().is_empty());
    assert!(source.selected_extension_downloads().is_empty());
    assert!(source.repository_executes().is_empty());
    assert!(source.repository_filesystem_operations().is_empty());
    assert!(source.workspace_operations().is_empty());
    assert!(source.workspace_reservations().is_empty());
    assert!(source.path_observations().is_none());
    assert!(source.repository_environment().is_none());
    assert!(source.scratch_elf_metadata_requests().is_none());
    assert!(source.root_module_bootstrap_request().is_none());
    assert_eq!(source, source.try_union(&source).unwrap());
    let environment = SourcePreparationNeeds::environment(
        crate::NeedRepositoryEnvironmentNames::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            crate::RepositoryEnvironmentNameFrontier::from_unsorted([CompactString::new("PATH")]),
        )
        .unwrap(),
    );
    let materialization = SourcePreparationNeeds::repository(
        materialization("https://source.test/a").as_ref().clone(),
    );
    let ordinary = environment.try_union(&materialization).unwrap();
    for union in [
        source.try_union(&ordinary).unwrap(),
        ordinary.try_union(&source).unwrap(),
    ] {
        assert_eq!(
            union.repository_environment(),
            ordinary.repository_environment()
        );
        assert_eq!(
            union.repository_materializations(),
            ordinary.repository_materializations()
        );
        assert_eq!(union.source_snapshots(), source.source_snapshots());
    }
}

#[test]
fn source_need_same_slot_compares_full_lease_kind_mapping_and_spec() {
    let original = request(7);
    let mut changed_kind = original.as_ref().clone();
    changed_kind.subject = Subject::Workspace;
    let mut changed_mapping = original.as_ref().clone();
    changed_mapping.mapping = Arc::from([(
        ApparentRepoName::new("dep").unwrap(),
        CanonicalRepoName::new("dep+").unwrap(),
    )]);
    let base = SourcePreparationNeeds::source_snapshot(original);
    for changed in [
        request(8),
        Arc::new(changed_kind),
        Arc::new(changed_mapping),
    ] {
        assert!(matches!(
            base.try_union(&SourcePreparationNeeds::source_snapshot(changed)),
            Err(SourcePreparationNeedsError::ConflictingSourceSnapshot { .. })
        ));
    }
    let make = |value: &str| {
        let mut request = request(7).as_ref().clone();
        request.subject = Subject::Immutable {
            request: materialization(value),
            policy: HostRepositoryLocalPathPolicy::LocalUnsupported,
        };
        Arc::new(request)
    };
    let a = SourcePreparationNeeds::source_snapshot(make("https://source.test/a"));
    let equal = SourcePreparationNeeds::source_snapshot(make("https://source.test/a"));
    assert_eq!(a, a.try_union(&equal).unwrap());
    assert!(matches!(
        a.try_union(&SourcePreparationNeeds::source_snapshot(make(
            "https://source.test/b"
        ))),
        Err(SourcePreparationNeedsError::ConflictingSourceSnapshot { .. })
    ));
}

#[test]
fn source_need_union_admits_128_distinct_slots_and_refuses_the_next() {
    let mut need = SourcePreparationNeeds::source_snapshot(Arc::new(slot(0)));
    for index in 1..128 {
        need = need
            .try_union(&SourcePreparationNeeds::source_snapshot(Arc::new(slot(
                index,
            ))))
            .unwrap();
    }
    assert_eq!(need.source_snapshots().len(), 128);
    assert_eq!(
        need,
        need.try_union(&SourcePreparationNeeds::source_snapshot(Arc::new(slot(
            127
        ))))
        .unwrap()
    );
    assert!(matches!(
        need.try_union(&SourcePreparationNeeds::source_snapshot(Arc::new(slot(
            128
        )))),
        Err(SourcePreparationNeedsError::SourceSnapshotLimit)
    ));
    assert_eq!(need.source_snapshots().len(), 128);
}
