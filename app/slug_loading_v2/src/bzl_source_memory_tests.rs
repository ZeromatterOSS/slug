use dice::DetectCycles;
use dice::Dice;

use super::*;
use crate::keys::WorkspaceSnapshot;
use crate::keys::WorkspaceSnapshotKey;

#[tokio::test]
async fn parsed_source_shares_workspace_storage_and_survives_its_owners() {
    let workspace = PathBuf::from("/workspace");
    let path = workspace.join("shared.bzl");
    let source = Arc::new(
        "load(\":first.bzl\", \"first\")\nload(\":second.bzl\", \"second\")\nvalue = \"é\"\n"
            .to_owned(),
    );
    let expected_digest = digest(&source);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: workspace.clone(),
            },
            Arc::new(WorkspaceSnapshot {
                files: Arc::new(
                    [(path.clone(), WorkspaceFileValue::Present(source.clone()))]
                        .into_iter()
                        .collect(),
                ),
            }),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    let result = transaction
        .compute(&BzlParseKey { workspace, path })
        .await
        .unwrap();
    let parsed = result.as_ref().as_ref().unwrap();
    assert_eq!(parsed.source.as_ptr(), source.as_ptr());
    assert_eq!(parsed.source.as_str(), source.as_str());
    assert_eq!(parsed.source_digest, expected_digest);
    assert_eq!(parsed.loads, [":first.bzl", ":second.bzl"]);
    let held = parsed.clone();
    assert_eq!(held.source.as_ptr(), parsed.source.as_ptr());

    drop(result);
    drop(transaction);
    drop(dice);
    drop(source);
    assert!(held.source.contains("value = \"é\""));
    assert_eq!(held.source_digest, expected_digest);

    let independent = ParsedBzl {
        source: Arc::new(held.source.as_ref().clone()),
        loads: held.loads.clone(),
        source_digest: expected_digest,
    };
    assert_ne!(held.source.as_ptr(), independent.source.as_ptr());
    assert!(BzlParseKey::equality(
        &Arc::new(Ok(held)),
        &Arc::new(Ok(independent)),
    ));
}
