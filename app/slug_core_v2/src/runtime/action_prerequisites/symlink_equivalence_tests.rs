use slug_analysis_v2::ConfiguredActionAspectProvenance;
use slug_analysis_v2::ConfiguredExecGroup;
use slug_analysis_v2::PlatformSemanticFact;
use slug_build_api_v2::SymlinkSpec;
use slug_build_api_v2::SymlinkTarget;

use super::*;
use crate::runtime::configured_action_closure::ConfiguredActionClosureError;
use crate::runtime::symlink_action_equivalence::admitted_artifact_symlink;
use crate::runtime::symlink_action_equivalence::equivalent_artifact_symlinks;

fn source(label: &str) -> AnalysisArtifact {
    AnalysisArtifact::Source(CanonicalLabel::parse(label).unwrap())
}
fn link(
    input: AnalysisArtifact,
    out: ActionOutput,
    executable: bool,
    policy: bool,
    progress: Option<&str>,
) -> ActionSpec {
    ActionSpec::symlink(SymlinkSpec::new(
        out,
        SymlinkTarget::Artifact {
            input,
            require_executable: executable,
            use_exec_root_for_source: policy,
        },
        progress,
    ))
}
fn resolved(
    owner: ConfiguredTargetKey,
    action: ActionSpec,
    platform: &str,
    property: &str,
) -> Arc<ConfiguredNodeResult> {
    let context = ConfiguredActionOwnerContext::new(
        owner.clone(),
        action
            .exec_group()
            .map_or(ConfiguredExecGroup::Default, |name| {
                ConfiguredExecGroup::Named(name.into())
            }),
        key(platform, true),
        PlatformSemanticFact {
            exec_properties: Arc::from([("property".into(), property.into())]),
            missing_toolchain_error: Some(Arc::from(property)),
        },
        &Default::default(),
        &Default::default(),
        Vec::new(),
        None,
        ConfiguredActionAspectProvenance::Absent,
    )
    .unwrap();
    Arc::new(
        ConfiguredNodeResult::new_rule(
            owner,
            ProviderCollection::from_values(Vec::new(), false).unwrap(),
            None,
            RunfilesPackageDepset::empty(),
        )
        .with_action_specs(vec![action], vec![Arc::new(context)])
        .unwrap(),
    )
}

#[test]
fn symlink_equivalence_keeps_progress_and_platform_agnostic_contexts() {
    for executable in [false, true] {
        for input in [
            source("@@dep+//pkg:in"),
            artifact(&key("backing", false), "seed", ActionOutputKind::File),
        ] {
            let left = resolved(
                key("left", false),
                link(input.clone(), output("shared"), executable, false, None),
                "platform_a",
                "raw_a",
            );
            let right = resolved(
                key("right", false),
                link(input, output("shared"), executable, false, Some("second")),
                "platform_b",
                "raw_b",
            );
            let a = &left.actions()[0];
            let b = &right.actions()[0];
            assert_ne!(a.context(), b.context());
            assert_eq!(equivalent_artifact_symlinks(a, b), Some(true));
            assert!(std::ptr::eq(
                admitted_artifact_symlink(a).unwrap(),
                a.symlink_spec().unwrap()
            ));
            let closure =
                ValidatedActionClosure::new(Arc::from([left.clone(), right.clone()])).unwrap();
            assert_eq!(closure.owners(), &[left, right]);
            assert!(closure.execution_representative(0, 0));
            assert!(!closure.execution_representative(1, 0));
            assert_eq!(closure.owners()[0].actions()[0].progress_message(), None);
            assert_eq!(
                closure.owners()[1].actions()[0].progress_message(),
                Some("second")
            );
        }
    }
}

#[test]
fn symlink_equivalence_compares_identity_and_refuses_unadmitted_facts() {
    let original = source("@@//pkg:in");
    let collision = source("@@//:pkg/in");
    assert_eq!(original.path(), collision.path());
    assert_ne!(original, collision);
    let backing = key("backing", false);
    let derived = artifact(&backing, "seed", ActionOutputKind::File);
    let other_owner = artifact(&key("other", false), "seed", ActionOutputKind::File);
    assert_eq!(derived.path(), other_owner.path());
    assert_ne!(derived, other_owner);
    for (input, alternatives) in [
        (original.clone(), vec![collision, source("@@other//pkg:in")]),
        (
            derived.clone(),
            vec![
                other_owner,
                artifact(&key("backing", true), "seed", ActionOutputKind::File),
            ],
        ),
    ] {
        let first = node(
            key("left", false),
            vec![link(input.clone(), output("shared"), false, false, None)],
        );
        let mut conflicts = alternatives
            .into_iter()
            .map(|input| link(input, output("shared"), false, false, None))
            .collect::<Vec<_>>();
        conflicts.push(link(input.clone(), output("shared"), true, false, None));
        for action in conflicts {
            let second = node(key("right", false), vec![action]);
            assert_eq!(
                equivalent_artifact_symlinks(&first.actions()[0], &second.actions()[0]),
                Some(false)
            );
            assert!(matches!(
                ValidatedActionClosure::new(Arc::from([first.clone(), second])),
                Err(ConfiguredActionClosureError::OutputConflict { .. })
            ));
        }
        let root = output("shared").root().clone();
        let mut refused = vec![
            link(input.clone(), output("shared"), false, true, None),
            link(
                input.clone(),
                ActionOutput::new_constant(root.clone(), "shared"),
                false,
                false,
                None,
            ),
            link(input.clone(), output("shared"), false, false, None).with_exec_group("custom"),
            link(input.clone(), output("shared"), false, false, None)
                .with_exec_properties([("k".to_owned(), "v".to_owned())].into_iter().collect()),
            ActionSpec::symlink(SymlinkSpec::new(
                output("shared"),
                SymlinkTarget::AbsolutePath {
                    target: slug_configuration_v2::NormalizedAbsoluteBazelPath::new(
                        HostPathFlavor::Unix,
                        "/absolute",
                    )
                    .unwrap(),
                },
                None::<&str>,
            )),
            link(
                AnalysisArtifact::Derived {
                    owner: backing.artifact_owner(),
                    output: ActionOutput::new_constant(root.clone(), "seed"),
                },
                output("shared"),
                false,
                false,
                None,
            ),
        ];
        for kind in [
            ActionOutputKind::Directory,
            ActionOutputKind::Symlink,
            ActionOutputKind::RunfilesTree,
        ] {
            refused.push(link(
                input.clone(),
                ActionOutput::new(root.clone(), "shared", kind),
                false,
                false,
                None,
            ));
            refused.push(link(
                artifact(&backing, "seed", kind),
                output("shared"),
                false,
                false,
                None,
            ));
        }
        for action in refused {
            let second = resolved(key("right", false), action, "platform", "property");
            assert_eq!(
                equivalent_artifact_symlinks(&first.actions()[0], &second.actions()[0]),
                None
            );
            assert!(matches!(
                ValidatedActionClosure::new(Arc::from([first.clone(), second])),
                Err(ConfiguredActionClosureError::UnsupportedEquivalence { .. })
            ));
        }
    }
    let exec = key("exec", true);
    let exec_output = match artifact(&exec, "shared", ActionOutputKind::File) {
        AnalysisArtifact::Derived { output, .. } => output,
        _ => unreachable!(),
    };
    let closure = ValidatedActionClosure::new(Arc::from([
        node(
            key("left", false),
            vec![link(original.clone(), output("shared"), false, false, None)],
        ),
        node(exec, vec![link(original, exec_output, false, false, None)]),
    ]))
    .unwrap();
    assert!(closure.execution_representative(0, 0));
    assert!(closure.execution_representative(1, 0));
}

#[test]
fn symlink_equivalence_same_owner_coordinates_keep_first_canonical_producer() {
    let first = key("first", false);
    let local = key("local", false);
    let a = link(
        source("@@//:in"),
        output("shared"),
        false,
        false,
        Some("first"),
    );
    let b = link(
        source("@@//:in"),
        output("shared"),
        false,
        false,
        Some("second"),
    );
    for earlier_owner in [false, true] {
        let mut owners = Vec::new();
        if earlier_owner {
            owners.push(node(first.clone(), vec![a.clone()]));
        }
        owners.push(node(local.clone(), vec![a.clone(), b.clone()]));
        let closure = ValidatedActionClosure::new(owners.into()).unwrap();
        let local_index = usize::from(earlier_owner);
        assert!(!closure.execution_representative(local_index, 1));
        if earlier_owner {
            assert!(!closure.execution_representative(local_index, 0));
        }
        let roots = [
            artifact(&local, "shared", ActionOutputKind::File),
            artifact(
                if earlier_owner { &first } else { &local },
                "shared",
                ActionOutputKind::File,
            ),
        ];
        let forest = plan_roots(&closure, PlanRoots::Artifacts(&roots)).unwrap();
        assert_eq!(forest.producers, [Some(0), Some(0)]);
        assert_eq!(forest.actions.len(), 1);
        let selected = ActionPrerequisitePlan::new(&closure, &local, 1).unwrap();
        assert_eq!(selected.actions().len(), 1);
        assert!(std::ptr::eq(
            selected.actions()[0].action(),
            &closure.owners()[0].actions()[0]
        ));
        assert_eq!(
            closure.owners()[local_index].actions()[1].progress_message(),
            Some("second")
        );
    }
}

#[test]
fn symlink_equivalence_shared_derived_lookup_remains_exact_and_cycle_safe() {
    let backing = key("backing", false);
    let left = key("left", false);
    let right = key("right", false);
    let consumer = key("consumer", false);
    let shared = link(
        artifact(&backing, "seed", ActionOutputKind::File),
        output("shared"),
        false,
        false,
        None,
    );
    let right_artifact = artifact(&right, "shared", ActionOutputKind::File);
    let closure = ValidatedActionClosure::new(Arc::from([
        node(backing.clone(), vec![write("seed")]),
        node(left.clone(), vec![shared.clone()]),
        node(right.clone(), vec![shared]),
        node(
            consumer.clone(),
            vec![spawn(vec![right_artifact.clone()], vec![output("done")])],
        ),
    ]))
    .unwrap();
    let roots = [
        artifact(&left, "shared", ActionOutputKind::File),
        right_artifact.clone(),
    ];
    let forest = plan_roots(&closure, PlanRoots::Artifacts(&roots)).unwrap();
    assert_eq!(forest.producers, [Some(1), Some(1)]);
    assert_eq!(forest.actions.len(), 2);
    assert_eq!(forest.actions[1].inputs()[0].producer(), Some(0));
    let selected = ActionPrerequisitePlan::new(&closure, &right, 0).unwrap();
    assert_eq!(selected.actions().len(), 2);
    assert!(std::ptr::eq(
        selected.actions()[1].action(),
        &closure.owners()[1].actions()[0]
    ));
    let downstream = ActionPrerequisitePlan::new(&closure, &consumer, 0).unwrap();
    assert_eq!(downstream.actions().len(), 3);
    assert_eq!(
        downstream.actions()[2].inputs()[0].artifact(),
        &right_artifact
    );
    assert_eq!(downstream.actions()[2].inputs()[0].producer(), Some(1));
    for (root, message) in [
        (
            artifact(&key("absent", false), "shared", ActionOutputKind::File),
            "missing generated producer",
        ),
        (
            artifact(&key("right", true), "shared", ActionOutputKind::File),
            "missing generated producer",
        ),
        (
            artifact(&right, "shared", ActionOutputKind::Directory),
            "missing generated producer",
        ),
        (
            artifact(&right, "shared", ActionOutputKind::Symlink),
            "unsupported generated prerequisite artifact kind",
        ),
    ] {
        assert!(
            plan_roots(&closure, PlanRoots::Artifacts(&[root]))
                .unwrap_err()
                .contains(message)
        );
    }
    for (inputs, expected) in [
        (vec![("a", "missing")], "missing generated producer"),
        (vec![("a", "a")], "cycle"),
        (vec![("a", "b"), ("b", "a")], "cycle"),
    ] {
        let actions = inputs
            .into_iter()
            .map(|(out, target)| {
                link(
                    artifact(&left, target, ActionOutputKind::File),
                    output(out),
                    false,
                    false,
                    None,
                )
            })
            .collect();
        let closure =
            ValidatedActionClosure::new(Arc::from([node(left.clone(), actions)])).unwrap();
        assert!(
            ActionPrerequisitePlan::new(&closure, &left, 0)
                .unwrap_err()
                .contains(expected)
        );
    }
}
