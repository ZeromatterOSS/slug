use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;

use starlark_map::small_set::SmallSet;

use super::*;
use crate::runtime::action_output_staging::tests::Workspace;
use crate::runtime::action_output_staging::tests::configurations;
use crate::runtime::action_output_staging::tests::root;
use crate::runtime::action_output_staging::tests::scratch;

fn output() -> ActionOutput {
    ActionOutput::new("pkg/alias", ActionOutputKind::File)
}

#[test]
fn aliases_and_regular_files_replace_leaves_without_following_or_chmodding_targets() {
    let workspace = Workspace::new();
    let outside = Workspace::new();
    let target = outside.path().join("target");
    fs::write(&target, b"untouched").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
    let (config, _) = configurations();
    let output = output();
    let outputs = std::slice::from_ref(&output);
    let root = root(&workspace, &config);
    let published = root.join(output.path());
    fs::create_dir_all(published.parent().unwrap()).unwrap();
    fs::write(&published, b"old").unwrap();
    for _ in 0..2 {
        let mut stage = Staging::new_alias(
            workspace.path(),
            &config,
            &output,
            target.to_str().unwrap(),
            &SmallSet::new(),
        )
        .unwrap();
        assert!(stage.create_file(outputs, 0, "").is_err());
        assert!(stage.create_directory(outputs, 0, "").is_err());
        stage.seal(outputs).unwrap();
        stage.publish(outputs).unwrap();
        drop(stage);
        assert_eq!(fs::read_link(&published).unwrap(), target);
        assert_eq!(fs::read(&published).unwrap(), b"untouched");
        assert!(scratch(&root).is_empty());
    }
    // An arbitrary existing leaf link is safe to replace, even when it points
    // to a directory outside the workspace. Its target is never traversed.
    fs::remove_file(&published).unwrap();
    symlink(outside.path(), &published).unwrap();
    let mut regular = Staging::new(workspace.path(), &config, outputs, &SmallSet::new()).unwrap();
    regular
        .create_file(outputs, 0, "")
        .unwrap()
        .write_all(b"new")
        .unwrap();
    regular.seal(outputs).unwrap();
    regular.publish(outputs).unwrap();
    drop(regular);
    assert!(!fs::symlink_metadata(&published).unwrap().is_symlink());
    assert_eq!(fs::read(&published).unwrap(), b"new");
    assert_eq!(fs::read(&target).unwrap(), b"untouched");
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 1);
    assert!(scratch(&root).is_empty());
}

#[test]
fn alias_staging_rejects_ancestors_stale_destinations_and_changed_link_text() {
    let (config, _) = configurations();
    let outside = Workspace::new();
    let target = outside.path().join("target");
    fs::write(&target, b"untouched").unwrap();
    let output = output();
    let outputs = std::slice::from_ref(&output);
    for mutation in [
        "ancestor",
        "before_seal",
        "after_seal",
        "destination",
        "abort",
    ] {
        let workspace = Workspace::new();
        let root = root(&workspace, &config);
        let published = root.join(output.path());
        fs::create_dir_all(&root).unwrap();
        if mutation == "ancestor" {
            symlink(outside.path(), root.join("pkg")).unwrap();
            assert!(
                Staging::new_alias(
                    workspace.path(),
                    &config,
                    &output,
                    target.to_str().unwrap(),
                    &SmallSet::new()
                )
                .is_err()
            );
            continue;
        }
        fs::create_dir_all(published.parent().unwrap()).unwrap();
        fs::write(&published, b"old").unwrap();
        let mut stage = Staging::new_alias(
            workspace.path(),
            &config,
            &output,
            target.to_str().unwrap(),
            &SmallSet::new(),
        )
        .unwrap();
        let hidden = scratch(&root).pop().unwrap();
        if mutation == "before_seal" || mutation == "after_seal" {
            if mutation == "after_seal" {
                stage.seal(outputs).unwrap();
            }
            fs::remove_file(&hidden).unwrap();
            symlink(outside.path(), &hidden).unwrap();
            if mutation == "before_seal" {
                assert!(stage.seal(outputs).is_err());
            } else {
                assert!(stage.publish(outputs).is_err());
            }
            drop(stage);
            // A replacement inode is not owned by the retired stage.
            assert_eq!(fs::read_link(&hidden).unwrap(), outside.path());
            fs::remove_file(hidden).unwrap();
            assert_eq!(fs::read(&published).unwrap(), b"old");
        } else if mutation == "destination" {
            stage.seal(outputs).unwrap();
            fs::remove_file(&published).unwrap();
            symlink(outside.path(), &published).unwrap();
            assert!(stage.publish(outputs).is_err());
            drop(stage);
            assert_eq!(fs::read_link(&published).unwrap(), outside.path());
        } else {
            drop(stage);
        }
        assert!(scratch(&root).is_empty());
        assert_eq!(fs::read(&target).unwrap(), b"untouched");
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 1);
    }
}

#[test]
fn alias_creation_never_claims_a_substituted_directory_for_cleanup() {
    let workspace = Workspace::new();
    let (config, _) = configurations();
    let output = output();
    let mut staging = Staging::new(
        workspace.path(),
        &config,
        std::slice::from_ref(&output),
        &SmallSet::new(),
    )
    .unwrap();
    let destination = &mut staging.destinations[0];
    let hidden = fd_path(&destination.parent).join(&destination.name);
    // Model the exact post-unlink/pre-open creation window with an unrelated
    // directory substitution. The live descriptor still names the old inode.
    fs::remove_file(&hidden).unwrap();
    destination.cleanup = None;
    fs::create_dir(&hidden).unwrap();
    fs::write(hidden.join("foreign"), b"untouched").unwrap();
    assert!(capture_link(destination, "/unused").is_err());
    assert!(destination.cleanup.is_none());
    let real_hidden = root(&workspace, &config)
        .join("pkg")
        .join(&destination.name);
    drop(staging);
    assert_eq!(fs::read(real_hidden.join("foreign")).unwrap(), b"untouched");
    fs::remove_dir_all(real_hidden).unwrap();
}
