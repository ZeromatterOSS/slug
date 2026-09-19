use std::fs;
use std::io::Write;

use slug_build_api_v2::ActionOutputKind;

use super::*;
use crate::runtime::action_output_staging::tests::Workspace;
use crate::runtime::action_output_staging::tests::configurations;
use crate::runtime::action_output_staging::tests::root;
use crate::runtime::action_output_staging::tests::scratch;

fn stages(workspace: &Workspace) -> Vec<PlannedActionOutputStaging> {
    let (a, b) = configurations();
    [a, b]
        .iter()
        .enumerate()
        .map(|(index, config)| {
            let output_root = root(workspace, config);
            fs::create_dir_all(&output_root).unwrap();
            fs::write(output_root.join("same"), b"old").unwrap();
            let mut staging = ActionOutputStaging::new(
                workspace.path(),
                config,
                &[ActionOutput::new("same", ActionOutputKind::File)],
            )
            .unwrap();
            staging
                .create_file(0, "")
                .unwrap()
                .write_all(if index == 0 { b"aaa" } else { b"bbb" })
                .unwrap();
            staging.seal().unwrap();
            PlannedActionOutputStaging {
                action_index: index + 2,
                staging,
            }
        })
        .collect()
}

#[test]
fn batch_preserves_configuration_destinations_and_producer_associations() {
    let workspace = Workspace::new();
    let mut stages = stages(&workspace);
    let published = PlannedActionOutputStaging::publish_all(&mut stages).unwrap();
    assert_eq!(
        published
            .iter()
            .map(|p| p.action_index())
            .collect::<Vec<_>>(),
        [2, 3]
    );
    assert_ne!(published[0].outputs().root(), published[1].outputs().root());
    for (group, bytes) in published.iter().zip([b"aaa", b"bbb"]) {
        assert_eq!(
            group.outputs().outputs(),
            [ActionOutput::new("same", ActionOutputKind::File)]
        );
        assert_eq!(
            fs::read(group.outputs().root().join("same")).unwrap(),
            bytes
        );
    }
    drop(stages);
    assert!(
        published
            .iter()
            .all(|p| scratch(p.outputs().root()).is_empty())
    );
}

#[test]
fn later_destination_change_blocks_every_group_before_first_rename() {
    let workspace = Workspace::new();
    let mut stages = stages(&workspace);
    let first = stages[0].staging.root.clone();
    let second = stages[1].staging.root.clone();
    fs::rename(second.join("same"), second.join("moved")).unwrap();
    fs::write(second.join("same"), b"external").unwrap();
    let error = PlannedActionOutputStaging::publish_all(&mut stages).unwrap_err();
    assert!(error.to_string().contains("destination changed"), "{error}");
    assert_eq!(fs::read(first.join("same")).unwrap(), b"old");
    assert_eq!(fs::read(second.join("same")).unwrap(), b"external");
    drop(stages);
    assert!(scratch(&first).is_empty());
    assert!(scratch(&second).is_empty());
}
