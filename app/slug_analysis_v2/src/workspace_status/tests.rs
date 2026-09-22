use dice::DetectCycles;
use dice::Dice;

use super::*;

#[tokio::test]
async fn workspace_status_descriptors_are_shared_by_normalized_workspace_not_request() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let direct = WorkspaceStatusArtifactsKey(NormalizedAbsolutePath::new("/work/main").unwrap());
    let normalized =
        WorkspaceStatusArtifactsKey(NormalizedAbsolutePath::new("/work/other/../main/.").unwrap());
    assert_eq!(direct, normalized);
    let mut transaction = dice.updater().commit().await;
    let first = transaction.compute(&direct).await.unwrap();
    let second = transaction.compute(&normalized).await.unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first.stable().root(), first.volatile().root());
    assert!(!first.stable().is_constant_metadata());
    assert!(first.volatile().is_constant_metadata());
    assert_ne!(first.stable(), first.volatile());
    drop(transaction);
    let mut transaction = dice.updater().commit().await;
    let repeated = transaction.compute(&direct).await.unwrap();
    assert!(Arc::ptr_eq(&first, &repeated));
    let other = transaction
        .compute(&WorkspaceStatusArtifactsKey(
            NormalizedAbsolutePath::new("/work/second").unwrap(),
        ))
        .await
        .unwrap();
    assert_ne!(first, other);
    assert_eq!(first.stable().path(), other.stable().path());
    assert_eq!(first.volatile().path(), other.volatile().path());
    assert!(!<WorkspaceStatusArtifactsKey as Key>::equality(
        &first, &other
    ));
}
