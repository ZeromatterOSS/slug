use super::*;

fn generated(path: &str, output: &str, kind: ActionOutputKind) -> Binding {
    Binding::Generated {
        path: path.into(),
        producer: 0,
        output: ActionOutput::new(output, kind),
    }
}

fn remote() -> ActionChainStepResult {
    let digest = ReapiDigest::of_bytes(b"");
    let directory = |path: &str, files| GeneratedDirectory {
        path: path.into(),
        tree_digest: ReapiDigest::of_bytes(b"verified tree"),
        root_digest: ReapiDigest::of_bytes(b"verified root"),
        directories: vec![String::new(), "nested-empty".into()],
        files,
    };
    ActionChainStepResult::Remote(RemoteExecutionResult {
        action_digest: ReapiDigest::of_bytes(b"action"),
        platform_properties: Default::default(),
        result: ActionResult::new(vec![GeneratedOutput::new("backing", digest.clone(), false)])
            .with_output_directories(vec![
                directory(
                    "tree",
                    vec![GeneratedOutput::new("nested/file", digest, false)],
                ),
                directory("empty", Vec::new()),
            ]),
        output_blobs: Default::default(),
        evidence: ExecutionEvidence::reapi("synthetic verified producer metadata"),
    })
}

fn tree(entries: Vec<Binding>) -> Binding {
    Binding::Runfiles {
        producer: 2,
        output: ActionOutput::new("tool.runfiles", ActionOutputKind::RunfilesTree),
        entries,
    }
}

#[test]
fn runfiles_aliases_empty_files_and_trees_keep_distinct_provenance() {
    let mapping = ActionOutput::new("mapping", ActionOutputKind::File);
    let results = [
        remote(),
        ActionChainStepResult::from_runfiles(PreparedRunfilesAction::Manifest {
            output: mapping.clone(),
            bytes: Arc::from([]),
        }),
        ActionChainStepResult::RunfilesTree {
            output: ActionOutput::new("tool.runfiles", ActionOutputKind::RunfilesTree),
        },
    ];
    let binding = tree(vec![
        generated("tool.runfiles/alias", "backing", ActionOutputKind::File),
        generated(
            "tool.runfiles/tree-alias",
            "tree",
            ActionOutputKind::Directory,
        ),
        generated(
            "tool.runfiles/empty-tree",
            "empty",
            ActionOutputKind::Directory,
        ),
        Binding::Empty {
            path: "tool.runfiles/empty-file".into(),
        },
        Binding::Source {
            path: "tool.runfiles/MANIFEST".into(),
            digest: ReapiDigest::of_bytes(b""),
            index: 7,
        },
        Binding::Generated {
            path: "tool.runfiles/_repo_mapping".into(),
            producer: 1,
            output: mapping,
        },
    ]);
    let mut bound = BoundInputs::default();
    bound.add(&binding, &results).unwrap();
    let empty = ReapiDigest::of_bytes(b"");
    assert_eq!(bound.local.get(&empty).unwrap().as_ref(), b"");
    assert_eq!(bound.sources.get(&empty), Some(&7));
    assert_eq!(bound.generated, BTreeSet::from([empty.clone()]));
    let tree =
        ReapiInputTree::from_entries_and_directories(bound.files, bound.directories).unwrap();
    assert_eq!(
        tree.entries()
            .iter()
            .map(ReapiInputTreeEntry::path)
            .collect::<Vec<_>>(),
        [
            "tool.runfiles/MANIFEST",
            "tool.runfiles/_repo_mapping",
            "tool.runfiles/alias",
            "tool.runfiles/empty-file",
            "tool.runfiles/tree-alias/nested/file",
        ]
    );
    assert_eq!(
        tree.directories(),
        [
            "tool.runfiles",
            "tool.runfiles/empty-tree",
            "tool.runfiles/tree-alias"
        ]
    );
    assert!(
        tree.entries()
            .iter()
            .all(|file| file.is_executable() && file.digest() == &empty)
    );
    // All three provenance classes survive equality; generated-CAS loss must
    // still reject before source, local manifest or empty bytes can repair it.
}

#[test]
fn runfiles_binding_requires_exact_completed_tree_and_target_results() {
    let binding = tree(vec![generated(
        "tool.runfiles/alias",
        "backing",
        ActionOutputKind::File,
    )]);
    let completed = |path, kind| ActionChainStepResult::RunfilesTree {
        output: ActionOutput::new(path, kind),
    };
    for result in [
        completed("wrong.runfiles", ActionOutputKind::RunfilesTree),
        completed("tool.runfiles", ActionOutputKind::Directory),
        remote(),
    ] {
        assert!(
            BoundInputs::default()
                .add(&binding, &[remote(), remote(), result])
                .is_err()
        );
    }
    assert!(BoundInputs::default().add(&binding, &[]).is_err());
    let accepted = completed("tool.runfiles", ActionOutputKind::RunfilesTree);
    let wrong_file = tree(vec![generated(
        "tool.runfiles/alias",
        "undeclared",
        ActionOutputKind::File,
    )]);
    assert!(
        BoundInputs::default()
            .add(&wrong_file, &[remote(), remote(), accepted])
            .is_err()
    );
}

#[test]
fn runfiles_preflight_reserves_root_and_logical_tree_namespaces() {
    for paths in [
        vec!["tool.runfiles", "tool.runfiles/data"],
        vec!["tool.runfiles/tree", "tool.runfiles/tree/child"],
        vec![
            "tool.runfiles/tree",
            "tool.runfiles/tree.other",
            "tool.runfiles/tree/child",
        ],
        vec!["tool.runfiles/file", "tool.runfiles/file"],
    ] {
        assert!(validate_paths(paths.into_iter()).is_err());
    }
    validate_paths(["tool.runfiles", "tool", "tool.runfiles.other"].into_iter()).unwrap();
    for path in [
        "tool.runfiles/../escape",
        "tool.runfiles//empty-component",
        "/absolute",
    ] {
        let binding = tree(vec![Binding::Empty { path: path.into() }]);
        let mut files = Vec::new();
        let mut directories = Vec::new();
        binding.preflight(&mut files, &mut directories);
        assert!(ReapiInputTree::from_entries_and_directories(files, directories).is_err());
    }
}
