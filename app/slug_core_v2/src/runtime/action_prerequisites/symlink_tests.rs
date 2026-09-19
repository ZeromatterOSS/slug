use slug_build_api_v2::SymlinkSpec;
use slug_build_api_v2::SymlinkTarget;

use super::*;

fn alias(input: AnalysisArtifact, path: &str) -> ActionSpec {
    ActionSpec::symlink(SymlinkSpec::new(
        output(path),
        SymlinkTarget::Artifact {
            input,
            require_executable: false,
            use_exec_root_for_source: false,
        },
        None::<String>,
    ))
}

#[test]
fn nested_aliases_defer_manifest_tree_completion_and_preserve_selected_spawn_behavior() {
    let tool = key("tool", true);
    let owner = key("owner", false);
    let (provider, tool_actions, source) = super::runfiles_tests::fixture(&tool);
    let support = provider.support.unwrap();
    let manifest = support.manifest.as_ref().unwrap();
    let closure = ValidatedActionClosure::new(Arc::from([
        node(tool, tool_actions),
        node(
            owner.clone(),
            vec![
                alias(manifest.clone(), "first"),
                alias(artifact(&owner, "first", ActionOutputKind::File), "last"),
                spawn(vec![manifest.clone()], vec![output("consumer")]),
            ],
        ),
    ]))
    .unwrap();
    let roots = [source, artifact(&owner, "last", ActionOutputKind::File)];
    let forest = plan_roots(&closure, PlanRoots::Artifacts(&roots)).unwrap();
    assert_eq!(forest.producers[0], None);
    assert_eq!(
        forest.actions[forest.producers[1].unwrap()]
            .action()
            .outputs()[0]
            .path(),
        "last"
    );
    assert_eq!(forest.actions.len(), 8); // six support/backing actions plus both aliases
    assert!(
        forest
            .actions
            .iter()
            .any(|step| step.action().outputs()[0].kind() == ActionOutputKind::RunfilesTree)
    );
    assert!(
        forest
            .actions
            .iter()
            .any(|step| step.action().outputs()[0].kind() == ActionOutputKind::Directory)
    );
    for (index, step) in forest.actions.iter().enumerate() {
        assert!(
            step.inputs()
                .iter()
                .filter_map(|input| input.producer())
                .all(|producer| producer < index)
        );
    }
    let selected = ActionPrerequisitePlan::new(&closure, &owner, 2).unwrap();
    assert_eq!(selected.actions().len(), 3);
    assert_eq!(
        selected.actions().last().unwrap().action().outputs()[0].path(),
        "consumer"
    );
    assert!(
        ActionPrerequisitePlan::new(&closure, &owner, 1)
            .unwrap_err()
            .contains("requested-artifact")
    );
}

#[test]
fn alias_edges_preserve_exact_producers_and_reject_cycles_kinds_and_source_policy() {
    let owner = key("owner", false);
    let source = AnalysisArtifact::Source(CanonicalLabel::parse("@@//:source").unwrap());
    let closure = ValidatedActionClosure::new(Arc::from([node(
        owner.clone(),
        vec![
            write("backing"),
            alias(artifact(&owner, "backing", ActionOutputKind::File), "one"),
            alias(artifact(&owner, "one", ActionOutputKind::File), "two"),
            alias(
                artifact(&key("owner", true), "backing", ActionOutputKind::File),
                "wrong_config",
            ),
            alias(
                artifact(&owner, "backing", ActionOutputKind::Directory),
                "wrong_kind",
            ),
            alias(artifact(&owner, "cycle", ActionOutputKind::File), "cycle"),
            ActionSpec::symlink(SymlinkSpec::new(
                output("source_policy"),
                SymlinkTarget::Artifact {
                    input: source,
                    require_executable: false,
                    use_exec_root_for_source: true,
                },
                None::<String>,
            )),
        ],
    )]))
    .unwrap();
    let roots = [
        artifact(&owner, "one", ActionOutputKind::File),
        artifact(&owner, "two", ActionOutputKind::File),
    ];
    let forest = plan_roots(&closure, PlanRoots::Artifacts(&roots)).unwrap();
    assert_eq!(forest.actions.len(), 3);
    assert_eq!(forest.producers, [Some(1), Some(2)]);
    assert_eq!(forest.actions[2].inputs()[0].producer(), Some(1));
    for (path, expected) in [
        ("wrong_config", "missing generated producer"),
        ("wrong_kind", "unsupported"),
        ("cycle", "cycle"),
        ("source_policy", "unsupported"),
    ] {
        let roots = [
            roots[0].clone(),
            artifact(&owner, path, ActionOutputKind::File),
        ];
        assert!(
            plan_roots(&closure, PlanRoots::Artifacts(&roots))
                .unwrap_err()
                .contains(expected),
            "{path}"
        );
    }
}
