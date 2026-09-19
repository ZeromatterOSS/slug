use super::*;
use crate::command::digest_to_proto;

fn file(name: &str, executable: bool) -> proto::FileNode {
    proto::FileNode {
        name: name.into(),
        digest: Some(digest_to_proto(&ReapiDigest::of_bytes(b"content"))),
        is_executable: executable,
        ..Default::default()
    }
}
fn node(name: &str, directory: &proto::Directory) -> proto::DirectoryNode {
    proto::DirectoryNode {
        name: name.into(),
        digest: Some(digest_to_proto(&ReapiDigest::of_bytes(
            &directory.encode_to_vec(),
        ))),
        ..Default::default()
    }
}
fn tree() -> proto::Tree {
    let empty = proto::Directory::default();
    let shared = proto::Directory {
        files: vec![file("item", true)],
        ..Default::default()
    };
    proto::Tree {
        root: Some(proto::Directory {
            files: vec![file("top", false)],
            directories: vec![
                node("a", &shared),
                node("b", &shared),
                node("empty", &empty),
            ],
            ..Default::default()
        }),
        children: vec![shared.clone(), empty, shared],
    }
}
fn decode(
    tree: &proto::Tree,
    budget: &mut Budget,
) -> Result<GeneratedDirectory, RemoteExecutionError> {
    let bytes = tree.encode_to_vec();
    let digest = ReapiDigest::of_bytes(&bytes);
    budget.tree(digest.size_bytes())?;
    decode_tree(
        &proto::OutputDirectory {
            path: "out".into(),
            tree_digest: Some(digest_to_proto(&digest)),
            ..Default::default()
        },
        &digest,
        &bytes,
        budget,
    )
}

#[test]
fn verified_tree_preserves_empty_directories_repeated_subtrees_and_modes() {
    let manifest = decode(&tree(), &mut Budget::default()).unwrap();
    let mut root_duplicate = tree();
    root_duplicate
        .children
        .push(root_duplicate.root.clone().unwrap());
    let duplicated = decode(&root_duplicate, &mut Budget::default()).unwrap();
    assert_eq!(duplicated.directories(), manifest.directories());
    assert_eq!(duplicated.files(), manifest.files());
    assert_eq!(duplicated.root_digest(), manifest.root_digest());
    assert_ne!(duplicated.tree_digest(), manifest.tree_digest());
    assert_eq!(manifest.directories(), ["", "a", "b", "empty"]);
    assert_eq!(
        manifest
            .files()
            .iter()
            .map(|file| (file.path(), file.is_executable()))
            .collect::<Vec<_>>(),
        [("a/item", true), ("b/item", true), ("top", false)]
    );
    assert!(
        manifest
            .files()
            .iter()
            .all(|file| file.digest() == &ReapiDigest::of_bytes(b"content"))
    );
    let result = crate::ActionResult::new(vec![]).with_output_directories(vec![manifest.clone()]);
    assert!(
        matches!(result.validate_local_outputs(&[]), crate::ActionCacheStatus::StaleLocal { missing_paths } if missing_paths == ["out"])
    );
    assert!(matches!(
        result.validate_remote_cas(&[manifest.tree_digest().clone()]),
        crate::ActionCacheStatus::OrphanedRemote { .. }
    ));
    assert_eq!(
        result.validate_remote_cas(&[
            manifest.tree_digest().clone(),
            ReapiDigest::of_bytes(b"content")
        ]),
        crate::ActionCacheStatus::Hit
    );
}

#[test]
fn malformed_tree_rejects_paths_topology_properties_and_integrity() {
    let failure = |tree: proto::Tree, message: &str| {
        let error = decode(&tree, &mut Budget::default())
            .unwrap_err()
            .to_string();
        assert!(error.contains(message), "{error}; expected {message}");
    };
    failure(proto::Tree::default(), "no root");
    for name in ["", ".", "..", "a/b", "a\\b", "a\0b"] {
        let mut value = tree();
        value.root.as_mut().unwrap().files[0].name = name.into();
        failure(value, "component");
    }
    for names in [["z", "a"], ["a", "a"]] {
        let mut value = tree();
        value.root.as_mut().unwrap().files = names.map(|name| file(name, false)).into();
        failure(value, "sorted and unique");
    }
    let mut value = tree();
    value.root.as_mut().unwrap().files[0].name = "a".into();
    failure(value, "duplicate");
    let mut value = tree();
    value.children.clear();
    failure(value, "missing Directory");
    let mut value = tree();
    value.root.as_mut().unwrap().files[0].digest = None;
    failure(value, "no digest");
    let mut value = tree();
    value.children.push(proto::Directory {
        files: vec![file("unused", false)],
        ..Default::default()
    });
    failure(value, "unreferenced");
    let mut value = tree();
    value
        .root
        .as_mut()
        .unwrap()
        .symlinks
        .push(proto::SymlinkNode {
            name: "link".into(),
            target: "top".into(),
            ..Default::default()
        });
    failure(value, "unsupported");
    let mut value = tree();
    value.root.as_mut().unwrap().node_properties = Some(Default::default());
    failure(value, "unsupported");
    let mut value = tree();
    value.root.as_mut().unwrap().files[0].node_properties = Some(Default::default());
    failure(value, "unsupported");
    let bytes = tree().encode_to_vec();
    let digest = ReapiDigest::of_bytes(&bytes);
    let output = proto::OutputDirectory {
        path: "out".into(),
        root_directory_digest: Some(digest_to_proto(&ReapiDigest::of_bytes(b"not root"))),
        ..Default::default()
    };
    assert!(
        decode_tree(&output, &digest, &bytes, &mut Budget::default())
            .unwrap_err()
            .to_string()
            .contains("root digest mismatch")
    );
    assert!(
        decode_tree(
            &output,
            &ReapiDigest::of_bytes(b"bad"),
            &bytes,
            &mut Budget::default()
        )
        .unwrap_err()
        .to_string()
        .contains("Tree digest mismatch")
    );
    let malformed = [0x0a, 0xff];
    assert!(
        decode_tree(
            &output,
            &ReapiDigest::of_bytes(&malformed),
            &malformed,
            &mut Budget::default()
        )
        .is_err()
    );
    fn message(tag: u8, bytes: &[u8]) -> Vec<u8> {
        let mut wire = vec![tag << 3 | 2];
        prost::encoding::encode_varint(bytes.len() as u64, &mut wire);
        wire.extend_from_slice(bytes);
        wire
    }
    // Unknown fields at every admitted message type must fail before prost
    // drops them. Exercise both FileNode and DirectoryNode digest nesting.
    for payload in [vec![0x78, 1], vec![0x08, 1], vec![0x0a, 0xff]] {
        for bytes in [
            payload.clone(),
            message(1, &payload),
            message(1, &message(1, &payload)),
            message(1, &message(2, &payload)),
            message(1, &message(1, &message(2, &payload))),
            message(1, &message(2, &message(2, &payload))),
        ] {
            let error = decode_tree(
                &output,
                &ReapiDigest::of_bytes(&bytes),
                &bytes,
                &mut Budget::default(),
            )
            .unwrap_err()
            .to_string();
            assert!(
                error.contains("unsupported output Tree protobuf field")
                    || error.contains("wire type")
                    || error.contains("buffer underflow")
                    || error.contains("invalid varint")
                    || error.contains("truncated"),
                "{error}"
            );
        }
    }
}

#[test]
fn tree_resource_limits_are_aggregate_and_checked_before_expansion() {
    for (budget, expected) in [
        (
            Budget {
                wire: 1,
                ..Default::default()
            },
            "wire",
        ),
        (
            Budget {
                entries: 2,
                ..Default::default()
            },
            "entry",
        ),
        (
            Budget {
                records: 1,
                ..Default::default()
            },
            "decode record",
        ),
        (
            Budget {
                paths: 3,
                ..Default::default()
            },
            "path",
        ),
        (
            Budget {
                depth: 0,
                ..Default::default()
            },
            "depth",
        ),
    ] {
        let error = decode(&tree(), &mut { budget }).unwrap_err().to_string();
        assert!(
            error.contains(expected) && error.contains("resource limit"),
            "{error}"
        );
    }
    let empty = proto::Tree {
        root: Some(Default::default()),
        children: vec![],
    };
    for mut budget in [
        Budget {
            wire: 3,
            ..Default::default()
        },
        Budget {
            entries: 1,
            ..Default::default()
        },
        Budget {
            paths: 7,
            ..Default::default()
        },
        Budget {
            records: 1,
            ..Default::default()
        },
    ] {
        decode(&empty, &mut budget).unwrap();
        assert!(
            decode(&empty, &mut budget)
                .unwrap_err()
                .to_string()
                .contains("resource limit")
        );
    }
    // Raw duplicate empty messages consume decoder allocation even if the
    // eventual expanded tree would contain only one root.
    let repeated = proto::Tree {
        root: Some(Default::default()),
        children: vec![Default::default(); 3],
    };
    assert!(
        decode(
            &repeated,
            &mut Budget {
                records: 2,
                ..Default::default()
            }
        )
        .unwrap_err()
        .to_string()
        .contains("decode record")
    );
}

#[test]
fn result_shape_requires_exact_output_types_and_sets() {
    let command = crate::ReapiCommand {
        argv: vec!["tool".into()],
        env: Default::default(),
        output_files: vec!["plain".into()],
        output_directories: vec!["tree".into()],
        platform_properties: Default::default(),
    };
    let valid = proto::ActionResult {
        output_files: vec![proto::OutputFile {
            path: "plain".into(),
            ..Default::default()
        }],
        output_directories: vec![proto::OutputDirectory {
            path: "tree".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    crate::executor::validate_result_shape(&valid, &command).unwrap();
    for case in 0..6 {
        let mut result = valid.clone();
        match case {
            0 => result.output_directories.clear(),
            1 => result
                .output_directories
                .push(result.output_directories[0].clone()),
            2 => result.output_directories[0].path = "extra".into(),
            3 => {
                result.output_files[0].path = "tree".into();
                result.output_directories[0].path = "plain".into();
            }
            4 => result.output_symlinks.push(proto::OutputSymlink {
                path: "link".into(),
                target: "plain".into(),
                ..Default::default()
            }),
            _ => result.output_files[0].node_properties = Some(Default::default()),
        }
        assert!(crate::executor::validate_result_shape(&result, &command).is_err());
    }
}
