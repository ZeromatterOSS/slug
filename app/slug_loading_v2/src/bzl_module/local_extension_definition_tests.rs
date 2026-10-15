//! Genuine selected command Local definition dispatch over existing test owners.
use dice::Dice;
use dice::DiceTransaction;
use slug_bzlmod_v2 as bv;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use slug_workspace_v2::path_observation_shards;

use super::*;

pub(crate) const WORKSPACE: &str = crate::canonical_repository_route_tests::tests::WORKSPACE;
pub(crate) const MODULE: &str = "module(name='consumer')\nbazel_dep(name='local',version='1',repo_name='local_alias')\ne=use_extension('@local_alias//:ext.bzl','ext')\nuse_repo(e,'proof')\n";

pub(crate) fn extension(marker: &str) -> String {
    format!(
        "load('@local//:child.bzl','implementation')\next=module_extension(implementation=implementation,tag_classes={{'{marker}':tag_class(attrs={{}})}})\n"
    )
}

/// Retain real request/epoch ownership; never construct a selected source capability.
pub(crate) async fn transaction(
    dice: &Arc<Dice>,
    root: &str,
    extension: &str,
    child: Option<&str>,
    command: bool,
) -> (DiceTransaction, PathObservationEpoch) {
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let override_value = format!("local={root}");
    let policy = bv::BzlmodCommandPolicyKey::from_flags_with_module_overrides(
        None,
        false,
        workspace.as_path(),
        command.then_some(override_value.as_str()),
    )
    .unwrap();
    let module = if command {
        MODULE.to_owned()
    } else {
        format!("{MODULE}local_path_override(module_name='local',path='local')\n")
    };
    let mut tx = crate::canonical_repository_route_tests::tests::transaction_with_policy(
        dice, &module, "ext=1\n", true, None, policy,
    )
    .await;
    let old = tx.compute(&PathObservationEpochKey).await.unwrap();
    let mut rows = old
        .observations()
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<Vec<_>>();
    let mut add = |path: &std::path::Path, operation, result| {
        let key = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(path).unwrap(),
            operation,
        );
        rows.retain(|(old, _)| old != &key);
        rows.push((key, Arc::new(result)));
    };
    let root_path = std::path::Path::new(root);
    for parent in root_path.ancestors().filter(|p| p.parent().is_some()) {
        add(
            parent,
            PathObservationOperation::Lstat,
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                PathNodeKind::Directory,
                0,
                1,
                1,
                1,
                0o755,
            ))),
        );
    }
    for name in ["REPO.bazel", ".bazelignore", "BUILD"] {
        add(
            &root_path.join(name),
            PathObservationOperation::Lstat,
            PathObservationResult::Lstat(PathOperationResult::Missing),
        );
    }
    for (name, source) in [
        ("MODULE.bazel", Some("module(name='local',version='9')\n")),
        (
            "BUILD.bazel",
            Some("exports_files(['ext.bzl','child.bzl'])\n"),
        ),
        ("ext.bzl", Some(extension)),
        ("child.bzl", child),
    ] {
        let path = root_path.join(name);
        add(
            &path,
            PathObservationOperation::Lstat,
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                PathNodeKind::RegularFile,
                source.map_or(0, str::len) as i64,
                1,
                1,
                1,
                0o644,
            ))),
        );
        if let Some(source) = source {
            add(
                &path,
                PathObservationOperation::FileBytes,
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                    source.as_bytes(),
                ))),
            );
        }
    }
    let epoch = PathObservationEpoch::from_shared(rows).unwrap();
    let mut updater = tx.into_updater();
    updater.changed_to(path_observation_shards(&epoch)).unwrap();
    updater
        .changed_to([(PathObservationEpochKey, epoch.clone())])
        .unwrap();
    let mut tx = updater.commit().await;
    let key = crate::HostCanonicalRepositoryLoadRouteKey::new(
        workspace.dupe(),
        CanonicalRepoName::new("local+").unwrap(),
    );
    let SourcePreparationOutcome::Need(need) = tx.compute(&key).await.unwrap() else {
        panic!("the genuine selected Local must demand materialization")
    };
    let requests = need.repository_materializations();
    assert_eq!(requests.len(), 1);
    let request = requests.values().next().unwrap().clone();
    assert!(
        matches!(&request.kind, bv::RepositoryMaterializationKind::Local { logical_root }
        if logical_root.as_path() == root_path)
    );
    let mut updater = tx.into_updater();
    updater
        .changed_to([(
            bv::RepositoryMaterializationResultEpochKey {
                workspace: workspace.dupe(),
            },
            bv::RepositoryMaterializationResultEpoch::new(
                workspace,
                [bv::RepositoryMaterializationEpochEntry {
                    request,
                    result: bv::RepositoryMaterializationResult::Success(
                        bv::RepositoryMaterializationSuccess::Local,
                    ),
                }],
            )
            .unwrap(),
        )])
        .unwrap();
    (updater.commit().await, epoch)
}

pub(crate) fn assert_epoch(actual: &PathObservationEpoch, expected: &PathObservationEpoch) {
    assert!(!actual.observations().is_empty());
    for (demand, value) in actual.observations() {
        assert_eq!(
            value,
            expected.get(demand).expect("original observation"),
            "{demand:?}"
        );
    }
}

#[tokio::test]
async fn selected_command_local_definition_dispatch_is_external_and_fails_closed() {
    let dice = crate::canonical_repository_route_tests::tests::builtin_graph_dice();
    let root = format!("{WORKSPACE}/local");
    let a = extension("local_a");
    let (mut tx, epoch) = transaction(
        &dice,
        &root,
        &a,
        Some("def implementation(ctx):\n    pass\n"),
        true,
    )
    .await;
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let key = HostLoadedModuleExtensionDefinitionsObservationKey::new_with_scope(
        workspace.dupe(),
        slug_bzlmod_v2::HostModuleExtensionUsageScope::RootUsages,
    );
    let SourcePreparationOutcome::Complete(Ok(first)) = tx.compute(&key).await.unwrap() else {
        panic!("observed definitions must complete")
    };
    let held = first.result().dupe();
    let weak = Arc::downgrade(&held);
    let value = held.as_ref().as_ref().unwrap();
    assert_eq!(value.definitions.len(), 1);
    assert!(
        value.definitions[0]
            .definition
            .tag_classes
            .iter()
            .any(|(name, _)| name == "local_a")
    );
    assert_eq!(value.definitions[0].manifest.direct_children.len(), 1);
    let HostSelectedExtensionDefinitionSource::Selected { definition, .. } =
        value.definitions[0].request.source()
    else {
        panic!("genuine selected Local definition")
    };
    let invalid = HostSelectedExtensionDefinitionSource::Selected {
        definition: definition.clone(),
        apparent_repo: ApparentRepoName::root(),
    };
    assert!(selected_extension_definition_route(workspace.dupe(), &invalid).is_err());
    assert!(
        selected_extension_definition_route(
            workspace.dupe(),
            &HostSelectedExtensionDefinitionSource::Root
        )
        .unwrap()
        .is_none()
    );
    assert_epoch(first.observations(), &epoch);
    let SourcePreparationOutcome::Complete(warm) = tx
        .compute(&HostLoadedModuleExtensionDefinitionsKey::new_with_scope(
            workspace.dupe(),
            slug_bzlmod_v2::HostModuleExtensionUsageScope::RootUsages,
        ))
        .await
        .unwrap()
    else {
        panic!("legacy definitions must complete")
    };
    assert_eq!(warm.as_ref(), held.as_ref());
    let SourcePreparationOutcome::Complete(Ok(reused)) = tx.compute(&key).await.unwrap() else {
        panic!("warm observed result")
    };
    assert!(Arc::ptr_eq(first.result(), reused.result()));
    drop(tx);
    let b_root = format!("{WORKSPACE}/local-b");
    let b = extension("local_b");
    let (mut tx, _) = transaction(
        &dice,
        &b_root,
        &b,
        Some("def implementation(ctx):\n    pass\n"),
        true,
    )
    .await;
    let SourcePreparationOutcome::Complete(Ok(changed)) = tx.compute(&key).await.unwrap() else {
        panic!("changed definitions")
    };
    assert_ne!(changed.result(), &held);
    drop(tx);
    let (mut tx, restored_epoch) = transaction(
        &dice,
        &root,
        &a,
        Some("def implementation(ctx):\n    pass\n"),
        true,
    )
    .await;
    let SourcePreparationOutcome::Complete(Ok(restored)) = tx.compute(&key).await.unwrap() else {
        panic!("restored definitions")
    };
    assert_eq!(restored.result(), &held);
    assert_epoch(restored.observations(), &restored_epoch);
    drop(tx);
    let (mut tx, _) = transaction(&dice, &root, &a, None, true).await;
    assert!(matches!(
        tx.compute(&key).await.unwrap(),
        SourcePreparationOutcome::Need(_)
    ));
    drop(tx);
    let (mut tx, _) = transaction(
        &dice,
        &root,
        &a,
        Some("def implementation(ctx):\n    pass\n"),
        false,
    )
    .await;
    assert!(
        matches!(tx.compute(&key).await.unwrap(), SourcePreparationOutcome::Complete(Ok(value)) if matches!(value.result().as_ref(), Err(HostLoadedModuleExtensionDefinitionsError::Requests(_))))
    );
    drop(tx);
    drop(first);
    drop(reused);
    drop(warm);
    drop(changed);
    drop(restored);
    drop(dice);
    assert!(weak.upgrade().is_some());
    drop(held);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while weak.upgrade().is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("definition carrier retires after its last owner");
}
