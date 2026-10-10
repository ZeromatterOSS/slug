// Borrowed admission for the exact original immutable owner in the current epoch.
use super::*;

pub(super) fn policy(
    bodies: &Bodies,
    request: &slug_bzlmod_v2::RepositoryMaterializationRequest,
    root: &Path,
    demand: &PathObservationDemand,
) -> Option<NamespacePolicy> {
    if !immutable_archive_preparation::matches(bodies, request) {
        return None;
    }
    let path = demand.path().as_path();
    Some(NamespacePolicy {
        owner_plan_allowed: true,
        path_class: if path.starts_with(root) {
            "immutable_tree"
        } else if root.starts_with(path) {
            "ancestor"
        } else {
            "other"
        },
    })
}

#[cfg(test)]
mod tests {
    use immutable_archive_preparation::MODULE_SRI;
    use immutable_archive_preparation::PROTOBUF_DIGEST;
    use immutable_archive_preparation::PROTOBUF_SRI;
    use immutable_archive_preparation::PROTOBUF_URL;
    use slug_bzlmod_v2::RepositoryMaterializationRequest;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
    use slug_workspace_v2::PathObservationInstanceId;

    use super::*;
    use crate::runtime::dice::source_service::OfflineArchiveBacking;

    fn path(value: &str) -> NormalizedAbsolutePath {
        NormalizedAbsolutePath::new(value).unwrap()
    }
    fn fixture() -> (Bodies, Arc<RepositoryMaterializationRequest>) {
        let bodies = immutable_archive_preparation::configure(
            namespace_test_bodies(),
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
            OfflineArchiveBacking::new(PROTOBUF_URL, PROTOBUF_DIGEST, path("/archive.tar.gz")),
            path("/prepared/MODULE.bazel"),
        );
        let string = |value: &str| OverrideAttributeValue::String(value.into());
        let list = |value: &str| OverrideAttributeValue::Iterable(Arc::new([string(value)]));
        let empty =
            || OverrideAttributeValue::Map(Arc::new(starlark_map::small_map::SmallMap::new()));
        let fields: [(compact_str::CompactString, OverrideAttributeValue); 9] = [
            ("urls".into(), list(PROTOBUF_URL)),
            ("integrity".into(), string(PROTOBUF_SRI)),
            ("strip_prefix".into(), string("protobuf-33.4")),
            ("remote_patches".into(), empty()),
            ("remote_file_urls".into(), empty()),
            ("remote_file_integrity".into(), empty()),
            (
                "remote_module_file_urls".into(),
                list("file:///prepared/MODULE.bazel"),
            ),
            ("remote_module_file_integrity".into(), string(MODULE_SRI)),
            ("remote_patch_strip".into(), OverrideAttributeValue::Int(0)),
        ];
        let mut request = namespace_test_request(&bodies, false);
        request.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new("protobuf+").unwrap();
        request.repo_spec.rule_id.bzl_file =
            CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:http.bzl").unwrap();
        request.repo_spec.rule_id.rule_name = "http_archive".into();
        request.repo_spec.attributes =
            Arc::new(starlark_map::small_map::SmallMap::from_iter(fields));
        request.kind = RepositoryMaterializationKind::Immutable;
        (bodies, Arc::new(request))
    }
    fn demand(
        instance: u64,
        spelling: &str,
        operation: PathObservationOperation,
    ) -> PathObservationDemand {
        PathObservationDemand::new(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(instance)),
            path(spelling),
            operation,
        )
    }
    fn admits(
        bodies: &Bodies,
        demand: &PathObservationDemand,
        epoch: Option<&RepositoryMaterializationResultEpoch>,
    ) -> bool {
        namespace_policy(bodies, demand, epoch, &small_namespace_plan_matches)
            .admits(&demand.operation())
    }

    #[test]
    fn original_immutable_namespace_is_confined_and_exact() {
        let (mut bodies, request) = fixture();
        let weak = Arc::downgrade(&request);
        let backing_weak = Arc::downgrade(bodies.offline_archive.as_ref().unwrap());
        let instance = PathObservationInstanceId::new(17);
        let epoch =
            namespace_test_epoch_at(&bodies, [request.clone()], instance, "/parent/generation");
        let view = epoch
            .borrowed_instance_for_namespace(&bodies.roots[0], instance, 32)
            .unwrap()
            .unwrap();
        let root_ptr = view
            .generation_root()
            .as_os_str()
            .as_encoded_bytes()
            .as_ptr();
        let attrs_ptr = Arc::as_ptr(&request.repo_spec.attributes);
        let count = Arc::strong_count(&request);
        assert!(std::ptr::eq(view.request(), request.as_ref()));
        assert_eq!(Arc::strong_count(&request.repo_spec.attributes), 1);
        for (spelling, class) in [
            ("/parent/generation", "immutable_tree"),
            ("/parent/generation/file", "immutable_tree"),
            ("/parent/generation/dir/file", "immutable_tree"),
            ("/parent", "ancestor"),
            ("/", "ancestor"),
            ("/parent/generation-sibling/file", "other"),
            ("/parent/sibling", "other"),
            ("/outside", "other"),
        ] {
            for operation in [
                PathObservationOperation::Lstat,
                PathObservationOperation::FileBytes,
                PathObservationOperation::FileDigest,
                PathObservationOperation::DirectoryEntries,
                PathObservationOperation::ReadLink,
            ] {
                let d = demand(17, spelling, operation.dupe());
                let p = namespace_policy(&bodies, &d, Some(&epoch), &small_namespace_plan_matches);
                assert_eq!(p.path_class, class);
                assert_eq!(
                    p.admits(&operation),
                    class == "immutable_tree"
                        || class == "ancestor" && operation == PathObservationOperation::Lstat
                );
            }
        }
        let valid = demand(
            17,
            "/parent/generation/file",
            PathObservationOperation::FileBytes,
        );
        let p = namespace_policy(&bodies, &valid, Some(&epoch), &small_namespace_plan_matches);
        let budget = slug_workspace_v2::ImageDescriptorBudget::new(4096);
        let anchor =
            slug_workspace_v2::ImageChargedPath::pin_existing(path("/parent/generation"), &budget)
                .unwrap();
        for operation in [
            PathObservationOperation::WindowsLongPath,
            PathObservationOperation::WindowsOptionPathLongName,
            PathObservationOperation::InputImage(slug_workspace_v2::ImageObservationRequest::new(
                anchor,
                slug_workspace_v2::ImageObservationOperation::Lstat,
            )),
        ] {
            assert!(!p.admits(&operation));
        }
        assert!(!admits(&bodies, &valid, None));
        for id in [0, 18] {
            assert!(!admits(
                &bodies,
                &demand(
                    id,
                    "/parent/generation/file",
                    PathObservationOperation::FileBytes
                ),
                Some(&epoch)
            ));
        }
        let mut foreign = request.as_ref().clone();
        foreign.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new("foreign+").unwrap();
        let foreign = Arc::new(foreign);
        let oversized = (0..=MATERIALIZATION_NAMESPACE_ENTRIES).map(|index| {
            let mut r = request.as_ref().clone();
            r.id.canonical_repo =
                slug_identity_v2::CanonicalRepoName::new(format!("entry{index}+")).unwrap();
            Arc::new(r)
        });
        for invalid in [
            namespace_test_epoch(&bodies, [], instance),
            namespace_test_epoch_at(
                &bodies,
                [request.clone(), foreign.clone()],
                instance,
                "/parent/generation",
            ),
            namespace_test_epoch_at(&bodies, oversized, instance, "/parent/generation"),
            namespace_test_epoch_at(&bodies, [request.clone()], instance, "relative"),
            namespace_test_epoch_at(&bodies, [foreign.clone()], instance, "/parent/generation"),
        ] {
            assert!(!admits(&bodies, &valid, Some(&invalid)));
        }
        let mut wrong_type = request.as_ref().clone();
        Arc::make_mut(&mut wrong_type.repo_spec.attributes).insert(
            "remote_patch_strip".into(),
            OverrideAttributeValue::Bool(false),
        );
        let wrong_type = namespace_test_epoch_at(
            &bodies,
            [Arc::new(wrong_type)],
            instance,
            "/parent/generation",
        );
        assert!(!admits(&bodies, &valid, Some(&wrong_type)));
        drop(wrong_type);
        let foreign_bodies = Bodies::new([path("/foreign"), path("/rules"), path("/platforms")]);
        let mut foreign_workspace = request.as_ref().clone();
        foreign_workspace.id.workspace = path("/foreign");
        let wrong_workspace = namespace_test_epoch_at(
            &foreign_bodies,
            [Arc::new(foreign_workspace)],
            instance,
            "/parent/generation",
        );
        assert!(!admits(&bodies, &valid, Some(&wrong_workspace)));
        let failed = RepositoryMaterializationResultEpoch::new(
            bodies.roots[0].dupe(),
            [slug_bzlmod_v2::RepositoryMaterializationEpochEntry {
                request: request.clone(),
                result: slug_bzlmod_v2::RepositoryMaterializationResult::SpecError(
                    "fixture".into(),
                ),
            }],
        )
        .unwrap();
        assert!(!admits(&bodies, &valid, Some(&failed)));
        let backing = bodies.offline_archive.take();
        assert!(!admits(&bodies, &valid, Some(&epoch)));
        bodies.offline_archive = backing;
        let module = bodies.offline_archive_module.take();
        assert!(!admits(&bodies, &valid, Some(&epoch)));
        bodies.offline_archive_module = module;
        drop(failed);
        drop(foreign);
        drop(wrong_workspace);
        assert_eq!(Arc::strong_count(&request), count);
        assert_eq!(Arc::as_ptr(&view.request().repo_spec.attributes), attrs_ptr);
        assert_eq!(
            view.generation_root()
                .as_os_str()
                .as_encoded_bytes()
                .as_ptr(),
            root_ptr
        );
        eprintln!(
            "WP1082_NAMESPACE_LAYOUT policy={}/{} view={}/{} bodies={}/{}",
            std::mem::size_of::<NamespacePolicy>(),
            std::mem::align_of::<NamespacePolicy>(),
            std::mem::size_of_val(&view),
            std::mem::align_of_val(&view),
            std::mem::size_of::<Bodies>(),
            std::mem::align_of::<Bodies>()
        );
        drop(epoch);
        drop(request);
        drop(bodies);
        assert!(weak.upgrade().is_none());
        assert!(backing_weak.upgrade().is_none());
    }

    #[tokio::test]
    async fn original_immutable_namespace_uses_current_epoch_and_refuses_mixed_need() {
        let (bodies, request) = fixture();
        let weak = Arc::downgrade(&request);
        let dice = dice::Dice::builder().build(dice::DetectCycles::Enabled);
        let instance = PathObservationInstanceId::new(17);
        let mut old = None;
        let a = demand(
            17,
            "/generation-a/file",
            PathObservationOperation::FileBytes,
        );
        let b = demand(
            17,
            "/generation-b/file",
            PathObservationOperation::FileBytes,
        );
        for (index, root) in ["/generation-a", "/generation-b", "/generation-a"]
            .into_iter()
            .enumerate()
        {
            let mut updater = dice.updater();
            updater
                .changed_to(vec![(
                    slug_bzlmod_v2::RepositoryMaterializationResultEpochKey {
                        workspace: bodies.roots[0].dupe(),
                    },
                    namespace_test_epoch_at(&bodies, [request.clone()], instance, root),
                )])
                .unwrap();
            let mut tx = updater.commit().await;
            let current = current_materialization_epoch(&mut tx, &bodies.roots[0])
                .await
                .unwrap();
            let count = Arc::strong_count(&request);
            let view = current
                .borrowed_instance_for_namespace(&bodies.roots[0], instance, 32)
                .unwrap()
                .unwrap();
            assert!(std::ptr::eq(view.request(), request.as_ref()));
            assert_eq!(view.generation_root(), Path::new(root));
            assert_eq!(admits(&bodies, &a, Some(&current)), index != 1);
            assert_eq!(admits(&bodies, &b, Some(&current)), index == 1);
            assert!(admits(
                &bodies,
                &demand(17, "/", PathObservationOperation::Lstat),
                Some(&current)
            ));
            let valid =
                SourcePreparationNeeds::path(NeedPathObservations::singleton(if index == 1 {
                    b.dupe()
                } else {
                    a.dupe()
                }));
            let mut progress = 0;
            for excluded in [
                demand(17, "/outside", PathObservationOperation::Lstat),
                demand(18, root, PathObservationOperation::Lstat),
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    path("/outside"),
                    PathObservationOperation::Lstat,
                ),
            ] {
                let bad =
                    SourcePreparationNeeds::path(NeedPathObservations::singleton(excluded.dupe()));
                for mixed in [
                    valid.try_union(&bad).unwrap(),
                    bad.try_union(&valid).unwrap(),
                ] {
                    if admit_with_materialization_epoch(&bodies, &mixed, Some(&current)).is_ok() {
                        progress += 1;
                    }
                    let denied = admit_with_materialization_epoch(&bodies, &mixed, Some(&current))
                        .unwrap_err();
                    let (first, family) = bodies
                        .first_denied_path_with_namespace_matcher(&mixed, &|d| {
                            admits(&bodies, d, Some(&current))
                        })
                        .unwrap();
                    assert_eq!(denied.family, family);
                    assert_eq!(denied.path.as_ref(), Some(excluded.path()));
                    assert_eq!(first, &excluded);
                }
            }
            let mut foreign = request.as_ref().clone();
            foreign.id.canonical_repo =
                slug_identity_v2::CanonicalRepoName::new("foreign+").unwrap();
            let repository = SourcePreparationNeeds::repository(foreign);
            for mixed in [
                valid.try_union(&repository).unwrap(),
                repository.try_union(&valid).unwrap(),
            ] {
                if admit_with_materialization_epoch(&bodies, &mixed, Some(&current)).is_ok() {
                    progress += 1;
                }
                assert_eq!(
                    admit_with_materialization_epoch(&bodies, &mixed, Some(&current))
                        .unwrap_err()
                        .family,
                    "materialization"
                );
            }
            assert_eq!(progress, 0);
            assert_eq!(Arc::strong_count(&request), count);
            if index == 0 {
                old = Some(current);
            }
        }
        assert!(admits(&bodies, &a, old.as_ref()));
        drop(old);
        dice.wait_for_idle().await;
        drop(dice);
        drop(request);
        tokio::time::timeout(Duration::from_secs(2), async {
            while weak.upgrade().is_some() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("original request retires after bounded DICE actor shutdown");
        assert!(weak.upgrade().is_none());
    }
}
