//! Selected Local owner invocation keeps original inputs, epochs and child failures.
use dice::DetectCycles;
use dice::Dice;
use slug_bzlmod_v2::HostSelectedExtensionDefinitionLoadRequestsKey;
use slug_bzlmod_v2::HostSelectedExtensionDemandKey;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::PathObservationOperation;

use super::*;
use crate::bzl_module::local_extension_definition_tests as fixture;

async fn owner(tx: &mut dice::DiceTransaction) -> Arc<HostSelectedExtensionOwner> {
    let workspace = NormalizedAbsolutePath::new(fixture::WORKSPACE).unwrap();
    let SourcePreparationOutcome::Complete(requests) = tx
        .compute(&HostSelectedExtensionDefinitionLoadRequestsKey::new(
            workspace.dupe(),
        ))
        .await
        .unwrap()
    else {
        panic!("original definition requests")
    };
    let requests = requests.as_ref().as_ref().unwrap();
    assert_eq!(requests.parts().1.len(), 1);
    let requested = CanonicalRepoName::new(format!(
        "{}+proof",
        requests.parts().1[0].namespace_parts().0.as_str()
    ))
    .unwrap();
    let SourcePreparationOutcome::Complete(demand) = tx
        .compute(&HostSelectedExtensionDemandKey::new(workspace, requested))
        .await
        .unwrap()
    else {
        panic!("original selected demand")
    };
    demand.as_ref().as_ref().unwrap().owner().dupe()
}

fn child(marker: &str) -> String {
    format!(
        "repo=repository_rule(lambda ctx: None,attrs={{'value':attr.string()}})\ndef implementation(ctx):\n    repo(name='proof',value='{marker}')\n"
    )
}

#[tokio::test]
async fn selected_command_local_owner_dispatch_preserves_need_and_observations() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let root = format!("{}/local", fixture::WORKSPACE);
    let extension = fixture::extension("local");
    let a = child("A");
    let (mut tx, epoch) = fixture::transaction(&dice, &root, &extension, Some(&a), true).await;
    let owner = owner(&mut tx).await;
    let workspace = NormalizedAbsolutePath::new(fixture::WORKSPACE).unwrap();
    let aggregate_key = HostPureModuleExtensionInvocationsObservationKey::new(workspace.dupe());
    let aggregate_legacy_key = HostPureModuleExtensionInvocationsKey::new(workspace.dupe());
    let observed_key =
        HostSelectedExtensionOwnerPureObservationKey::new(workspace.dupe(), owner.dupe());
    let legacy_key = HostSelectedExtensionOwnerPureKey::new(workspace.dupe(), owner.dupe());
    // Request the legacy path first; the contradictory workspace ext=1 cannot supply it.
    let SourcePreparationOutcome::Complete(legacy) = tx.compute(&legacy_key).await.unwrap() else {
        panic!("legacy selected owner")
    };
    let SourcePreparationOutcome::Complete(Ok(first)) = tx.compute(&observed_key).await.unwrap()
    else {
        panic!("observed selected owner")
    };
    let held = first.result().dupe();
    let weak = Arc::downgrade(&held);
    let value = held.as_ref().as_ref().unwrap();
    assert_eq!(value.receipt.repository_rule_calls.len(), 1);
    assert!(
        matches!(value.inputs.request().source(), HostSelectedExtensionDefinitionSource::Selected { definition, .. }
        if definition.view().kind() == slug_bzlmod_v2::HostCanonicalSelectedModuleKind::SelectedNonregistry)
    );
    assert_eq!(legacy.as_ref(), held.as_ref());
    fixture::assert_epoch(first.observations(), &epoch);
    // The ordinary aggregate reacquires through pure_host_bzl, the third consumer.
    let SourcePreparationOutcome::Complete(aggregate_legacy) =
        tx.compute(&aggregate_legacy_key).await.unwrap()
    else {
        panic!("legacy aggregate selected Local invocation")
    };
    let SourcePreparationOutcome::Complete(Ok(aggregate)) =
        tx.compute(&aggregate_key).await.unwrap()
    else {
        panic!("observed aggregate selected Local invocation")
    };
    let aggregate_held = aggregate.result().dupe();
    let aggregate_weak = Arc::downgrade(&aggregate_held);
    let aggregate_value = aggregate_held.as_ref().as_ref().unwrap();
    assert_eq!(aggregate_value.invoked.len(), 1);
    assert_eq!(
        aggregate_value.invoked[0].repository_rule_calls,
        value.receipt.repository_rule_calls
    );
    assert_eq!(aggregate_legacy.as_ref(), aggregate_held.as_ref());
    fixture::assert_epoch(aggregate.observations(), &epoch);
    let SourcePreparationOutcome::Complete(Ok(aggregate_warm)) =
        tx.compute(&aggregate_key).await.unwrap()
    else {
        panic!("warm aggregate")
    };
    assert!(Arc::ptr_eq(aggregate_warm.result(), &aggregate_held));
    let SourcePreparationOutcome::Complete(Ok(warm)) = tx.compute(&observed_key).await.unwrap()
    else {
        panic!("warm owner")
    };
    assert!(Arc::ptr_eq(warm.result(), &held));
    drop(tx);
    let b = child("B");
    let (mut tx, _) = fixture::transaction(&dice, &root, &extension, Some(&b), true).await;
    let SourcePreparationOutcome::Complete(Ok(changed)) = tx.compute(&observed_key).await.unwrap()
    else {
        panic!("changed owner")
    };
    assert_ne!(changed.result(), &held);
    let SourcePreparationOutcome::Complete(Ok(aggregate_changed)) =
        tx.compute(&aggregate_key).await.unwrap()
    else {
        panic!("changed aggregate")
    };
    assert_ne!(aggregate_changed.result(), &aggregate_held);
    drop(tx);
    let (mut tx, restored_epoch) =
        fixture::transaction(&dice, &root, &extension, Some(&a), true).await;
    // Reverse order after restoration, retaining the first result through every change.
    let SourcePreparationOutcome::Complete(Ok(restored)) = tx.compute(&observed_key).await.unwrap()
    else {
        panic!("restored owner")
    };
    let SourcePreparationOutcome::Complete(restored_legacy) =
        tx.compute(&legacy_key).await.unwrap()
    else {
        panic!("restored legacy owner")
    };
    assert_eq!(restored.result(), &held);
    assert_eq!(restored_legacy.as_ref(), held.as_ref());
    fixture::assert_epoch(restored.observations(), &restored_epoch);
    let SourcePreparationOutcome::Complete(Ok(aggregate_restored)) =
        tx.compute(&aggregate_key).await.unwrap()
    else {
        panic!("restored aggregate")
    };
    let SourcePreparationOutcome::Complete(aggregate_restored_legacy) =
        tx.compute(&aggregate_legacy_key).await.unwrap()
    else {
        panic!("restored legacy aggregate")
    };
    assert_eq!(aggregate_restored.result(), &aggregate_held);
    assert_eq!(aggregate_restored_legacy.as_ref(), aggregate_held.as_ref());
    fixture::assert_epoch(aggregate_restored.observations(), &restored_epoch);
    drop(tx);
    let (mut tx, _) = fixture::transaction(&dice, &root, &extension, None, true).await;
    let SourcePreparationOutcome::Need(need) = tx.compute(&observed_key).await.unwrap() else {
        panic!("absent child observation must be a Need")
    };
    assert!(
        need.path_observations()
            .is_some_and(
                |paths| paths.demands().iter().any(|demand| demand.operation()
                    == PathObservationOperation::FileBytes
                    && demand.path().as_path() == std::path::Path::new(&root).join("child.bzl"))
            )
    );
    assert!(matches!(
        tx.compute(&aggregate_key).await.unwrap(),
        SourcePreparationOutcome::Need(_)
    ));
    drop(tx);
    let (mut tx, error_epoch) = fixture::transaction(
        &dice,
        &root,
        &extension,
        Some("def implementation(:\n"),
        true,
    )
    .await;
    let SourcePreparationOutcome::Complete(Ok(error)) = tx.compute(&observed_key).await.unwrap()
    else {
        panic!("child parse error remains semantic")
    };
    let Err(HostSelectedExtensionOwnerPureError::AfterInputs {
        inputs,
        current_calls,
        ..
    }) = error.result().as_ref()
    else {
        panic!("original input-bearing owner error")
    };
    assert!(current_calls.is_empty());
    assert_eq!(inputs.owner(), &owner);
    fixture::assert_epoch(error.observations(), &error_epoch);
    drop(tx);
    drop(first);
    drop(legacy);
    drop(warm);
    drop(changed);
    drop(restored);
    drop(restored_legacy);
    drop(error);
    drop(aggregate);
    drop(aggregate_legacy);
    drop(aggregate_warm);
    drop(aggregate_changed);
    drop(aggregate_restored);
    drop(aggregate_restored_legacy);
    drop(owner);
    drop(dice);
    assert!(weak.upgrade().is_some());
    assert!(aggregate_weak.upgrade().is_some());
    drop(held);
    drop(aggregate_held);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while weak.upgrade().is_some() || aggregate_weak.upgrade().is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("original owner result retires after its final held owner");
}
