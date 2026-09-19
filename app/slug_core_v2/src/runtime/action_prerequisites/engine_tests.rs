use super::*;

#[test]
fn selected_artifacts_preserve_exact_identity_and_shared_file_write_bindings() {
    let target = key("producer", false);
    let equivalent = key("equivalent", false);
    let exec = key("producer", true);
    let cooutputs = key("cooutputs", false);
    let closure = ValidatedActionClosure::new(Arc::from([
        node(target.clone(), vec![write("shared")]),
        node(equivalent.clone(), vec![write("shared")]),
        node(exec.clone(), vec![write("shared")]),
        node(
            cooutputs.clone(),
            vec![
                spawn(
                    vec![],
                    vec![
                        output("file"),
                        ActionOutput::new("tree", ActionOutputKind::Directory),
                    ],
                ),
                ActionSpec::new(ActionKind::Run, "unselected", vec![output("unused")]),
            ],
        ),
    ]))
    .unwrap();
    let roots = [
        AnalysisArtifact::Source(CanonicalLabel::parse("@@//:shared").unwrap()),
        artifact(&equivalent, "shared", ActionOutputKind::File),
        artifact(&exec, "shared", ActionOutputKind::File),
        artifact(&cooutputs, "tree", ActionOutputKind::Directory),
        artifact(&cooutputs, "file", ActionOutputKind::File),
        artifact(&target, "shared", ActionOutputKind::File),
    ];
    let forest = plan_roots(&closure, PlanRoots::Artifacts(&roots)).unwrap();
    assert_eq!(
        forest.producers,
        [None, Some(0), Some(1), Some(2), Some(2), Some(0)]
    );
    assert_eq!(forest.actions.len(), 3);
    assert_eq!(forest.actions[0].action().context().owner(), &target);
    assert_eq!(forest.actions[1].action().context().owner(), &exec);
    assert_eq!(forest.actions[2].action().context().owner(), &cooutputs);
    assert_ne!(roots[1], roots[2]);
    assert_ne!(roots[1], roots[5]);
    assert_eq!(forest.actions[2].action().outputs().len(), 2);
    assert!(
        forest.actions[2]
            .inputs()
            .iter()
            .all(|input| input.producer().is_none())
    );

    let sources = [roots[0].clone()];
    let source_only = plan_roots(&closure, PlanRoots::Artifacts(&sources)).unwrap();
    assert!(source_only.actions.is_empty());
    assert_eq!(source_only.producers, [None]);
    let empty = plan_roots(&closure, PlanRoots::Artifacts(&[])).unwrap();
    assert!(empty.actions.is_empty());
    assert!(empty.producers.is_empty());
}

#[test]
fn multiple_selected_roots_share_prerequisites_and_keep_cooutput_input_edges() {
    let owner = key("owner", false);
    let seed = artifact(&owner, "seed", ActionOutputKind::File);
    let file = artifact(&owner, "file", ActionOutputKind::File);
    let tree = artifact(&owner, "tree", ActionOutputKind::Directory);
    let left = artifact(&owner, "left", ActionOutputKind::File);
    let right = artifact(&owner, "right", ActionOutputKind::File);
    let closure = ValidatedActionClosure::new(Arc::from([node(
        owner,
        vec![
            write("seed"),
            spawn(
                vec![seed.clone()],
                vec![
                    output("file"),
                    ActionOutput::new("tree", ActionOutputKind::Directory),
                ],
            ),
            spawn(vec![file.clone(), tree.clone()], vec![output("left")]),
            spawn(vec![tree.clone()], vec![output("right")]),
        ],
    )]))
    .unwrap();
    let roots = [right.clone(), left, file.clone(), tree.clone(), right];
    let forest = plan_roots(&closure, PlanRoots::Artifacts(&roots)).unwrap();
    assert_eq!(
        forest.producers,
        [Some(2), Some(3), Some(1), Some(1), Some(2)]
    );
    assert_eq!(
        forest
            .actions
            .iter()
            .map(|step| step.action().outputs()[0].path())
            .collect::<Vec<_>>(),
        ["seed", "file", "right", "left"],
    );
    for (index, expected) in [(1, vec![0]), (2, vec![1]), (3, vec![1, 1])] {
        assert_eq!(
            forest.actions[index]
                .inputs()
                .iter()
                .filter_map(PlannedActionInput::producer)
                .collect::<Vec<_>>(),
            expected,
        );
    }
    assert_eq!(forest.actions[1].inputs()[0].artifact(), &seed);
    assert_eq!(forest.actions[2].inputs()[0].artifact(), &tree);
    assert_eq!(forest.actions[3].inputs()[0].artifact(), &file);
    assert_eq!(forest.actions[3].inputs()[1].artifact(), &tree);
}

#[test]
fn invalid_later_selected_roots_reject_the_entire_forest() {
    let owner = key("owner", false);
    let preferred = owner.clone().with_toolchain_execution_platform(Arc::new(
        CanonicalLabel::parse("@@//:platform").unwrap(),
    ));
    let closure = ValidatedActionClosure::new(Arc::from([node(
        owner.clone(),
        vec![
            write("good"),
            spawn(
                vec![artifact(&owner, "cycle_b", ActionOutputKind::File)],
                vec![output("cycle_a")],
            ),
            spawn(
                vec![artifact(&owner, "cycle_a", ActionOutputKind::File)],
                vec![output("cycle_b")],
            ),
            ActionSpec::new(ActionKind::Run, "unsupported", vec![output("legacy")]),
            spawn(
                vec![artifact(&owner, "legacy", ActionOutputKind::File)],
                vec![output("consumer")],
            ),
            ActionSpec::new(
                ActionKind::Run,
                "symlink",
                vec![ActionOutput::new("link", ActionOutputKind::Symlink)],
            ),
            ActionSpec::new(
                ActionKind::Run,
                "runfiles",
                vec![ActionOutput::new(
                    "runfiles",
                    ActionOutputKind::RunfilesTree,
                )],
            ),
            spawn(
                vec![],
                vec![ActionOutput::new("tree", ActionOutputKind::Directory)],
            ),
        ],
    )]))
    .unwrap();
    let good = artifact(&owner, "good", ActionOutputKind::File);
    for (invalid, expected) in [
        (
            artifact(&preferred, "good", ActionOutputKind::File),
            "missing generated producer",
        ),
        (
            artifact(&key("owner", true), "good", ActionOutputKind::File),
            "missing generated producer",
        ),
        (
            artifact(&owner, "absent", ActionOutputKind::File),
            "missing generated producer",
        ),
        (
            artifact(&owner, "good", ActionOutputKind::Directory),
            "missing generated producer",
        ),
        (
            artifact(&owner, "tree/child", ActionOutputKind::File),
            "missing generated producer",
        ),
        (artifact(&owner, "cycle_a", ActionOutputKind::File), "cycle"),
        (
            artifact(&owner, "consumer", ActionOutputKind::File),
            "unsupported action family",
        ),
        (
            artifact(&owner, "link", ActionOutputKind::Symlink),
            "unsupported",
        ),
        (
            artifact(&owner, "runfiles", ActionOutputKind::RunfilesTree),
            "unsupported",
        ),
    ] {
        let roots = [good.clone(), invalid];
        let error = plan_roots(&closure, PlanRoots::Artifacts(&roots)).unwrap_err();
        assert!(error.contains(expected), "{}: {error}", roots[1].path());
    }
    // Unsupported and cyclic actions in the validated closure are harmless when
    // no selected root reaches them; the forest is not an all-actions plan.
    let roots = [good];
    let forest = plan_roots(&closure, PlanRoots::Artifacts(&roots)).unwrap();
    assert_eq!(forest.actions.len(), 1);
    assert_eq!(forest.producers, [Some(0)]);
}
