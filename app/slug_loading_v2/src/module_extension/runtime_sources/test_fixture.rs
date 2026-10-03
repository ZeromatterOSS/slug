// Trusted raw observation fixtures exercise normal producers; Core proves actual IO.
use slug_bzlmod_v2 as runtime_test_bzl;
use slug_workspace_v2 as runtime_test_ws;

use super::runtime_sources as runtime_test_api;

fn runtime_test_policy(
    resources: &runtime_test_bzl::SelectedScratchResources,
    declarations: &[runtime_test_bzl::RuntimeSourceDeclarationInput<'_>],
    limits: runtime_test_bzl::RuntimeSourceLimits,
) -> runtime_test_bzl::RuntimeSourceDeclarationPolicy {
    let workspace = runtime_test_ws::ImageChargedPath::pin_existing(
        NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
        resources.descriptor_budget(),
    )
    .unwrap();
    runtime_test_bzl::RuntimeSourceDeclarationPolicy::new(
        &workspace,
        declarations,
        limits,
        resources.descriptor_budget(),
    )
    .unwrap()
}
fn runtime_test_relative<'a>(
    relative: &'a str,
    guest: &'a str,
) -> runtime_test_bzl::RuntimeSourceDeclarationInput<'a> {
    runtime_test_bzl::RuntimeSourceDeclarationInput {
        source: runtime_test_bzl::RuntimeSourceSelectorInput::WorkspaceRelativeDirectory {
            relative,
        },
        guest_mapping: runtime_test_bzl::RuntimeSourceGuestMappingInput::ExplicitPrefix(guest),
    }
}
fn runtime_test_authorization(
    policy: &runtime_test_bzl::RuntimeSourceDeclarationPolicy,
    r: &runtime_test_bzl::SelectedScratchResources,
) -> runtime_test_bzl::RuntimeSourceDeclarationTransaction {
    runtime_test_bzl::RuntimeSourceDeclarationTransaction::new(
        policy.workspace().dupe(),
        Some(policy.dupe()),
        r.descriptor_budget().dupe(),
    )
    .unwrap()
}
async fn runtime_test_install(
    tx: dice::DiceTransaction,
    tracker: Arc<RepositoryRuleActivationTracker>,
    r: &runtime_test_bzl::SelectedScratchResources,
    raw_policy: Option<runtime_test_bzl::RuntimeSourceDeclarationPolicy>,
    current: Option<runtime_test_bzl::RuntimeSourceDeclarationTransaction>,
    scratch: Option<(
        SelectedScratchElfMetadataTransaction,
        Option<(
            SelectedScratchElfMetadataEpochKey,
            SelectedScratchElfMetadataEpoch,
        )>,
    )>,
    epoch: Option<PathObservationEpoch>,
) -> dice::DiceTransaction {
    runtime_test_install_with_guest(tx, tracker, r, raw_policy, current, scratch, epoch, None).await
}
async fn runtime_test_install_with_guest(
    tx: dice::DiceTransaction,
    tracker: Arc<RepositoryRuleActivationTracker>,
    r: &runtime_test_bzl::SelectedScratchResources,
    raw_policy: Option<runtime_test_bzl::RuntimeSourceDeclarationPolicy>,
    current: Option<runtime_test_bzl::RuntimeSourceDeclarationTransaction>,
    scratch: Option<(
        SelectedScratchElfMetadataTransaction,
        Option<(
            SelectedScratchElfMetadataEpochKey,
            SelectedScratchElfMetadataEpoch,
        )>,
    )>,
    epoch: Option<PathObservationEpoch>,
    guest: Option<(
        Option<runtime_test_bzl::GuestNamespacePolicy>,
        Option<runtime_test_bzl::GuestNamespaceTransaction>,
    )>,
) -> dice::DiceTransaction {
    let mut data = dice::UserComputationData {
        cycle_detector: Some(crate::cycle_detector::bzl_load_cycle_detector()),
        activation_tracker: Some(tracker),
        ..Default::default()
    };
    data.data.set(CaptureEvaluationEvents);
    data.data.set(r.descriptor_budget().dupe());
    data.data.set(r.dupe());
    data.data.set(RepositoryHostInputTransaction::new(
        RepositoryEnvironmentSnapshot::empty(),
        RepositoryEnvironmentNameFrontier::from_unsorted([
            CompactString::new("FACTOR"),
            CompactString::new("MISSING"),
        ]),
    ));
    data.data.set(SelectedExtensionDownloadTransaction::empty(
        NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
    ));
    if let Some(current) = current {
        data.data.set(current);
    }
    let mut updater = tx.into_updater();
    let (guest_policy, guest_current) = guest.unwrap_or((None, None));
    updater
        .changed_to([(
            runtime_test_bzl::RawGuestNamespacePolicyKey::new(
                NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
            ),
            guest_policy,
        )])
        .unwrap();
    if let Some(current) = guest_current {
        data.data.set(current);
    }
    updater
        .changed_to([(
            runtime_test_bzl::RuntimeSourceDeclarationPolicyKey::new(
                NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
            ),
            raw_policy,
        )])
        .unwrap();
    if let Some((scratch, raw)) = scratch {
        data.data.set(scratch);
        if let Some(raw) = raw {
            updater.changed_to([raw]).unwrap();
        }
    }
    if let Some(epoch) = epoch {
        updater.changed_to(path_observation_shards(&epoch)).unwrap();
        updater
            .changed_to([(PathObservationEpochKey, epoch)])
            .unwrap();
    }
    updater.commit_with_data(data).await
}
fn runtime_test_requests(
    value: <runtime_test_api::RuntimeSourceRequestsKey as Key>::Value,
) -> runtime_test_api::ObservedRuntimeSourceRequests {
    match value {
        SourcePreparationOutcome::Complete(Ok(v)) => v,
        other => panic!("{other:?}"),
    }
}
fn runtime_test_profile(
    value: <runtime_test_api::RuntimeSourceProfileKey as Key>::Value,
) -> runtime_test_api::ObservedRuntimeSourceProfile {
    match value {
        SourcePreparationOutcome::Complete(Ok(v)) => v,
        other => panic!("{other:?}"),
    }
}
fn runtime_test_ready_scratch(
    requests: &runtime_test_api::ObservedRuntimeSourceRequests,
    r: &runtime_test_bzl::SelectedScratchResources,
) -> (
    SelectedScratchElfMetadataTransaction,
    Option<(
        SelectedScratchElfMetadataEpochKey,
        SelectedScratchElfMetadataEpoch,
    )>,
) {
    let tool = requests.tool_requests();
    let request = tool.requests().next().unwrap().scratch_request().unwrap();
    let allowed = SelectedScratchElfMetadataNeeds::singleton(request, r).unwrap();
    let result = tool_test_fact(request, r, false);
    (
        SelectedScratchElfMetadataTransaction::new(
            tool.selector(),
            r,
            Some(&allowed),
            &[result.dupe()],
        )
        .unwrap(),
        Some((
            SelectedScratchElfMetadataEpochKey::new(request),
            SelectedScratchElfMetadataEpoch::Ready(result),
        )),
    )
}

// Raw image facts, including strict anchor-to-subtree ancestry. The optional file
// changes independently by digest and mode; no completed image/profile is injected.
fn runtime_test_image_epoch(
    key: &runtime_test_ws::ObservedInputImageKey,
    r: &runtime_test_bzl::SelectedScratchResources,
    freshness: i64,
    file: Option<(u8, u32)>,
) -> PathObservationEpoch {
    use runtime_test_ws::ImageObservationData as Data;
    use runtime_test_ws::ImageObservationOperation as Op;
    let budget = r.descriptor_budget();
    let anchor =
        runtime_test_ws::ImageChargedPath::pin_existing(key.anchor_root().dupe(), budget).unwrap();
    let root =
        runtime_test_ws::ImageChargedPath::pin_existing(key.source_root().dupe(), budget).unwrap();
    let mut rows = Vec::new();
    let mut add = |path: runtime_test_ws::ImageChargedPath, operation, data| {
        let request = runtime_test_ws::ImageObservationRequest::new(anchor.dupe(), operation);
        let demand =
            PathObservationDemand::input_image(key.namespace(), path.dupe(), request.dupe())
                .unwrap();
        let result = runtime_test_ws::ImageObservationResult::new(
            path,
            request,
            data,
            budget.try_reserve(512).unwrap(),
        )
        .unwrap();
        rows.push((demand, PathObservationResult::InputImage(result)));
    };
    let mut cursor = anchor.dupe();
    for component in root
        .as_path()
        .strip_prefix(anchor.as_path())
        .unwrap()
        .components()
    {
        add(
            cursor.dupe(),
            Op::Lstat,
            Data::Lstat(PathOperationResult::Present(PathLstat::new(
                PathNodeKind::Directory,
                0,
                freshness,
                freshness,
                freshness,
                0o755,
            ))),
        );
        cursor = cursor
            .try_join_component(
                &runtime_test_ws::PathDirectoryName::new(component.as_os_str()).unwrap(),
            )
            .unwrap();
    }
    add(
        root.dupe(),
        Op::Lstat,
        Data::Lstat(PathOperationResult::Present(PathLstat::new(
            PathNodeKind::Directory,
            0,
            freshness,
            freshness,
            freshness,
            0o755,
        ))),
    );
    add(
        root.dupe(),
        Op::BoundedDirectory(runtime_test_ws::DirectoryObservationLimits {
            max_entries: key.policy().limits().max_entries,
            max_name_bytes_total: key.policy().limits().max_descriptor_bytes,
            max_component_bytes: key.policy().limits().max_path_bytes,
        }),
        Data::Directory(PathOperationResult::Present(
            runtime_test_ws::PathDirectoryEntries::new(file.iter().map(|_| {
                runtime_test_ws::PathDirectoryEntry::new(
                    runtime_test_ws::PathDirectoryName::new("data").unwrap(),
                    runtime_test_ws::PathDirectoryEntryKind::File,
                )
            })),
        )),
    );
    if let Some((digest, mode)) = file {
        let path = root
            .try_join_component(&runtime_test_ws::PathDirectoryName::new("data").unwrap())
            .unwrap();
        add(
            path.dupe(),
            Op::Lstat,
            Data::Lstat(PathOperationResult::Present(PathLstat::new(
                PathNodeKind::RegularFile,
                1,
                freshness,
                freshness,
                freshness,
                i32::try_from(mode).unwrap(),
            ))),
        );
        add(
            path,
            Op::BoundedDigest {
                max_bytes: key.policy().limits().max_file_bytes,
            },
            Data::Digest(PathOperationResult::Present(
                runtime_test_ws::FileContentDigest::new([digest; 32], 1).unwrap(),
            )),
        );
    }
    PathObservationEpoch::new(rows).unwrap()
}
fn runtime_test_union(
    left: &PathObservationEpoch,
    right: &PathObservationEpoch,
) -> PathObservationEpoch {
    PathObservationEpoch::from_shared(
        left.observations()
            .iter()
            .chain(right.observations())
            .map(|(d, v)| (d.dupe(), v.dupe())),
    )
    .unwrap()
}
