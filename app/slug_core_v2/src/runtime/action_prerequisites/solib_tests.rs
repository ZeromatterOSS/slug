use slug_analysis_v2::ConfiguredActionAspectProvenance;
use slug_analysis_v2::ConfiguredExecGroup;
use slug_analysis_v2::PlatformSemanticFact;
use slug_build_api_v2::SolibSymlinkSpec;
use slug_build_api_v2::SymlinkSpec;
use slug_build_api_v2::SymlinkTarget;

use super::*;
use crate::runtime::configured_action_closure::ConfiguredActionClosureError;

fn solib(input: AnalysisArtifact) -> ActionSpec {
    ActionSpec::solib_symlink(SolibSymlinkSpec::new(input, "_solib_k8").unwrap())
}

fn resolved_node(owner: ConfiguredTargetKey, action: ActionSpec) -> Arc<ConfiguredNodeResult> {
    let context = ConfiguredActionOwnerContext::new(
        owner.clone(),
        action
            .exec_group()
            .map_or(ConfiguredExecGroup::Default, |name| {
                ConfiguredExecGroup::Named(name.into())
            }),
        key("platform", true),
        PlatformSemanticFact {
            exec_properties: Arc::from([("property".into(), "platform-value".into())]),
            missing_toolchain_error: Some(Arc::from("platform diagnostic")),
        },
        &Default::default(),
        &Default::default(),
        vec![],
        None,
        ConfiguredActionAspectProvenance::Absent,
    )
    .unwrap();
    Arc::new(
        ConfiguredNodeResult::new_rule(
            owner,
            ProviderCollection::from_values(vec![], false).unwrap(),
            None,
            RunfilesPackageDepset::empty(),
        )
        .with_action_specs(vec![action], vec![Arc::new(context)])
        .unwrap(),
    )
}

#[test]
fn solib_sharing_preserves_owners_and_ignores_native_owner_platform_facts() {
    let input = artifact(&key("library", false), "pkg/lib.so", ActionOutputKind::File);
    let action = solib(input);
    let first = node(key("first", false), vec![action.clone()]);
    let second = resolved_node(key("second", false), action.clone());
    assert_ne!(first.actions()[0].context(), second.actions()[0].context());
    let closure = ValidatedActionClosure::new(Arc::from([first.clone(), second.clone()])).unwrap();
    assert_eq!(closure.owners(), &[first, second]);
    assert!(closure.execution_representative(0, 0));
    assert!(!closure.execution_representative(1, 0));
    let second_output = artifact(
        &key("second", false),
        action.outputs()[0].path(),
        ActionOutputKind::File,
    );
    assert_eq!(
        plan_roots(&closure, PlanRoots::Artifacts(&[second_output]))
            .unwrap_err()
            .as_ref(),
        "unsupported shared action prerequisite"
    );

    // Same spelling in a different consumer configuration is a separate root.
    let closure = ValidatedActionClosure::new(Arc::from([
        node(key("target", false), vec![action.clone()]),
        node(key("exec", true), vec![action]),
    ]))
    .unwrap();
    assert!(closure.execution_representative(0, 0));
    assert!(closure.execution_representative(1, 0));
}

#[test]
fn solib_collisions_compare_full_configured_input_and_reject_mixed_families() {
    let first = key("first", false);
    let second = key("second", false);
    let input_owner = key("library", false);
    let input = artifact(&input_owner, "pkg/lib.so", ActionOutputKind::File);
    let action = solib(input.clone());
    for changed_owner in [key("library", true), key("other_library", false)] {
        let changed = solib(artifact(
            &changed_owner,
            "pkg/lib.so",
            ActionOutputKind::File,
        ));
        assert_eq!(action.outputs(), changed.outputs());
        assert_ne!(action, changed);
        assert!(matches!(
            ValidatedActionClosure::new(Arc::from([
                node(first.clone(), vec![action.clone()]),
                node(second.clone(), vec![changed]),
            ])),
            Err(ConfiguredActionClosureError::OutputConflict { .. })
        ));
    }
    let ordinary = ActionSpec::symlink(SymlinkSpec::new(
        action.outputs()[0].clone(),
        SymlinkTarget::Artifact {
            input,
            require_executable: false,
            use_exec_root_for_source: false,
        },
        None::<String>,
    ));
    let decorated = action.clone().with_exec_properties(
        [("custom".to_owned(), "property".to_owned())]
            .into_iter()
            .collect(),
    );
    for unsupported in [ordinary, decorated, write(action.outputs()[0].path())] {
        assert!(matches!(
            ValidatedActionClosure::new(Arc::from([
                node(first.clone(), vec![action.clone()]),
                node(second.clone(), vec![unsupported]),
            ])),
            Err(ConfiguredActionClosureError::UnsupportedEquivalence { .. })
        ));
    }
    assert!(matches!(
        ValidatedActionClosure::new(Arc::from([
            node(first, vec![action.clone()]),
            resolved_node(second, action.with_exec_group("custom")),
        ])),
        Err(ConfiguredActionClosureError::UnsupportedEquivalence { .. })
    ));
}

#[test]
fn solib_requested_outputs_and_spawn_inputs_remain_runtime_unsupported() {
    let owner = key("owner", false);
    let action = solib(artifact(&owner, "lib.so", ActionOutputKind::File));
    let requested = artifact(&owner, action.outputs()[0].path(), ActionOutputKind::File);
    assert!(action.symlink_spec().is_none());
    let closure = ValidatedActionClosure::new(Arc::from([node(
        owner.clone(),
        vec![
            write("lib.so"),
            action,
            spawn(vec![requested.clone()], vec![output("done")]),
        ],
    )]))
    .unwrap();
    assert_eq!(
        plan_roots(&closure, PlanRoots::Artifacts(&[requested]))
            .unwrap_err()
            .as_ref(),
        "unsupported action family in generated prerequisites"
    );
    assert_eq!(
        ActionPrerequisitePlan::new(&closure, &owner, 2)
            .unwrap_err()
            .as_ref(),
        "unsupported action family in generated prerequisites"
    );
}
