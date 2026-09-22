use super::*;

#[test]
fn build_info_marker_and_constant_outputs_fail_before_digest_projection() {
    use slug_build_api_v2::ActionKind;
    use slug_build_api_v2::ActionOutput;
    let marker = ActionSpec::new(ActionKind::BuildInfoTransform, "TranslateBuildInfo", vec![]);
    assert_eq!(
        ReapiInputTree::from_action(&marker),
        Err(InputTreeError::BuildInfoTransformUnsupported)
    );
    let forged = ActionSpec::new(
        ActionKind::Write {
            content: "not observed".into(),
            is_executable: false,
        },
        "FileWrite",
        vec![ActionOutput::new_constant(
            crate::test_roots::root(),
            "volatile.h",
        )],
    );
    assert_eq!(
        ReapiInputTree::from_action(&forged),
        Err(InputTreeError::ConstantMetadataUnsupported)
    );
}

fn file(path: &str, executable: bool) -> ReapiInputTreeEntry {
    ReapiInputTreeEntry::new(
        path,
        ReapiDigest::of_bytes(b"content"),
        InputTreeEntryKind::Input,
    )
    .with_executable(executable)
}

fn directory(tree: &ReapiInputTree, digest: &ReapiDigest) -> proto::Directory {
    let blob = tree
        .directory_blobs()
        .iter()
        .find(|blob| blob.digest() == digest)
        .unwrap();
    proto::Directory::decode(blob.data()).unwrap()
}

#[test]
fn explicit_directories_and_file_modes_have_canonical_wire_encoding() {
    let tree = ReapiInputTree::from_entries_and_directories(
        [file("tree/run", true), file("tree/data", false)],
        ["tree".into(), "empty".into(), "tree".into()],
    )
    .unwrap();
    assert_eq!(tree.directories(), ["empty", "tree"]);
    let root = directory(&tree, tree.root_digest());
    let empty_digest = ReapiDigest::of_bytes(b"");
    assert_eq!(
        root.directories,
        [
            proto::DirectoryNode {
                name: "empty".into(),
                digest: Some(digest_to_proto(&empty_digest)),
                ..Default::default()
            },
            proto::DirectoryNode {
                name: "tree".into(),
                digest: Some(digest_to_proto(&ReapiDigest::of_bytes(
                    &proto::Directory {
                        files: vec![
                            proto::FileNode {
                                name: "data".into(),
                                digest: Some(digest_to_proto(&ReapiDigest::of_bytes(b"content"))),
                                is_executable: false,
                                ..Default::default()
                            },
                            proto::FileNode {
                                name: "run".into(),
                                digest: Some(digest_to_proto(&ReapiDigest::of_bytes(b"content"))),
                                is_executable: true,
                                ..Default::default()
                            },
                        ],
                        ..Default::default()
                    }
                    .encode_to_vec(),
                ))),
                ..Default::default()
            },
        ]
    );
    assert_eq!(directory(&tree, &empty_digest), proto::Directory::default());
    let reordered = ReapiInputTree::from_entries_and_directories(
        [file("tree/data", false), file("tree/run", true)],
        ["empty".into(), "tree".into()],
    )
    .unwrap();
    assert_eq!(tree, reordered);
    let changed = ReapiInputTree::from_entries_and_directories(
        [file("tree/data", true), file("tree/run", true)],
        ["empty".into(), "tree".into()],
    )
    .unwrap();
    assert_ne!(tree.root_digest(), changed.root_digest());
}

#[test]
fn file_directory_ancestor_and_same_digest_mode_conflicts_reject() {
    for (files, directories) in [
        (vec![file("x", true), file("x", false)], vec![]),
        (vec![file("x", true), file("x/y", true)], vec![]),
        (vec![file("x", true)], vec!["x".into()]),
        (vec![file("x", true)], vec!["x/y".into()]),
    ] {
        assert!(matches!(
            ReapiInputTree::from_entries_and_directories(files, directories),
            Err(InputTreeError::ConflictingPath { .. })
        ));
    }
    let valid = ReapiInputTree::from_entries_and_directories(
        [file("x/y", true), file("x/y", true)],
        ["x".into(), "x/empty".into()],
    )
    .unwrap();
    assert_eq!(valid.entries().len(), 1);
    for invalid in ["", "/x", "x/", "x//y", "x/../y", "x/./y"] {
        assert!(matches!(
            ReapiInputTree::from_entries_and_directories([], [invalid.into()]),
            Err(InputTreeError::InvalidPath { .. })
        ));
    }
}

fn forced_command() -> ExpandedSpawnCommandLine {
    use slug_build_api_v2::*;
    use slug_configuration_v2::NormalizedBazelPath;
    use slug_configuration_v2::native::host::HostPathFlavor;

    SpawnSpec::new(
        RetainedSpawnInvocation::Executable(SpawnExecutable::Path(
            NormalizedBazelPath::new(HostPathFlavor::Unix, "tool").unwrap(),
        )),
        RetainedCommandLine::new(vec![RetainedCommandLineSegment::ArgsSnapshot(
            RetainedSpawnArgsSnapshot::new(
                RetainedArgsRecipe::new(
                    vec![RetainedArgCall::Scalar(RetainedScalarArg::new(
                        None::<&str>,
                        RetainedScalarValue::String("arg".into()),
                        None::<&str>,
                    ))],
                    RetainedParamFileFormat::Multiline,
                ),
                Some(RetainedSpawnParamFilePolicy::new("@%s", true)),
            ),
        )]),
        ArtifactInputs::new(Vec::new()),
        ArtifactInputs::new(Vec::new()),
        vec![ActionOutput::new(
            crate::test_roots::root(),
            "out",
            ActionOutputKind::File,
        )],
        None,
        Default::default(),
        Default::default(),
        "Test",
        None::<&str>,
    )
    .expand_forced_param_files()
    .unwrap()
}

#[test]
fn param_composition_preserves_directories_and_explicit_executable_policy() {
    let command = forced_command();
    let base = ReapiInputTree::from_entries_and_directories([], ["empty".into()]).unwrap();
    let original = base.clone();
    for executable in [false, true] {
        let tree = base
            .with_spawn_param_files_executable(&command, executable)
            .unwrap();
        assert_eq!(tree.directories(), base.directories());
        let mut node = directory(&tree, tree.root_digest());
        assert_eq!(
            node.directories
                .iter()
                .map(|node| node.name.as_str())
                .collect::<Vec<_>>(),
            ["bazel-out", "empty"]
        );
        for component in [
            "bazel-out",
            crate::test_roots::root().configuration_component(),
            "bin",
        ] {
            let child = node
                .directories
                .iter()
                .find(|child| child.name == component)
                .unwrap();
            node = directory(
                &tree,
                &crate::executor::digest_from_proto(child.digest.as_ref().unwrap()).unwrap(),
            );
        }
        assert_eq!(node.files[0].name, "out-0.params");
        assert_eq!(node.files[0].is_executable, executable);
        assert_eq!(tree.entries()[0].is_executable(), executable);
        assert_eq!(tree.inline_blobs()[0].data(), b"arg\n");
        if !executable {
            assert_eq!(tree, base.with_spawn_param_files(&command).unwrap());
        }
    }
    assert_eq!(base, original);
    let path = command.param_files()[0].path();
    for directory in [path.to_owned(), format!("{path}/empty")] {
        let tree = ReapiInputTree::from_entries_and_directories([], [directory]).unwrap();
        let original = tree.clone();
        assert!(matches!(
            tree.with_spawn_param_files_executable(&command, true),
            Err(InputTreeError::ConflictingPath { .. })
        ));
        assert_eq!(tree, original);
    }
}

#[test]
fn existing_entry_constructor_executable_policy_is_preserved() {
    for kind in [
        InputTreeEntryKind::Source,
        InputTreeEntryKind::Input,
        InputTreeEntryKind::Tool,
        InputTreeEntryKind::ParamFile,
        InputTreeEntryKind::FileWriteContent,
    ] {
        let entry = ReapiInputTreeEntry::new("file", ReapiDigest::of_bytes(b""), kind);
        assert_eq!(entry.is_executable(), kind == InputTreeEntryKind::Source);
    }
}
