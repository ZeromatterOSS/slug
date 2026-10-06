use slug_analysis_v2::ConfiguredActionAspectProvenance;
use slug_analysis_v2::ConfiguredActionOwnerContext;
use slug_analysis_v2::ConfiguredBuildInfoActionView;
use slug_analysis_v2::ConfiguredExecGroup;
use slug_analysis_v2::ConfiguredNodeResult;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_analysis_v2::PlatformSemanticFact;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::ActionOutputMetadata;
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::BuildInfoCallbackRecipe;
use slug_build_api_v2::BuildInfoTransformSpec;
use slug_build_api_v2::WorkspaceStatusArtifacts;
use slug_query_v2::AqueryScope;

use super::*;

// The Builtin route supplies the integrity-checked catalog source and templates.
// Local module declarations close its selected mapping; they are not callbacks.
const BUILD_INFO_MODULES: &[(&str, &str)] = &[
    ("rules_license", "1.0.0"),
    ("buildozer", "8.5.1"),
    ("platforms", "1.0.0"),
    ("zlib", "1.3.1.bcr.5"),
    ("bazel_features", "1.42.1"),
    ("protobuf", "33.4"),
    ("rules_java", "9.1.0"),
    ("rules_cc", "0.2.17"),
    ("rules_python", "1.7.0"),
    ("rules_shell", "0.6.1"),
    ("apple_support", "1.24.2"),
    ("rules_apple", "4.1.0"),
    ("rules_swift", "3.1.2"),
    ("abseil-cpp", "20250814.1"),
];

fn build_info_epoch() -> PathObservationEpoch {
    let mut epoch = BuildRootEpoch::base(1);
    let mut module =
        "module(name='root')\nregister_execution_platforms('//:platform')\n".to_owned();
    for (name, version) in BUILD_INFO_MODULES {
        epoch.node(&format!("/workspace/{name}"), PathNodeKind::Directory, 1);
        epoch.file(
            &format!("/workspace/{name}/MODULE.bazel"),
            &format!("module(name='{name}',version='{version}')\n"),
            1,
        );
        module.push_str(&format!(
            "local_path_override(module_name='{name}',path='{name}')\n"
        ));
        // Match the existing source-staging fixture's explicit visibility of
        // these builtin MODULE dependencies with repo_name=None.
        if matches!(
            *name,
            "bazel_features" | "rules_apple" | "rules_swift" | "abseil-cpp"
        ) {
            module.push_str(&format!("bazel_dep(name='{name}',version='{version}')\n"));
        }
    }
    epoch.file("/workspace/MODULE.bazel", &module, 1);
    epoch.file("/workspace/wrapper.bzl", "def _impl(ctx):\n    out=ctx.actions.declare_file('wrapper')\n    ctx.actions.write(out,'wrapper')\n    return [DefaultInfo(files=depset([out]))]\nwrapper=rule(implementation=_impl,attrs={'dep':attr.label(cfg='exec')})\n", 1);
    epoch.package("", "load('@bazel_tools//tools/build_defs/build_info:bazel_cc_build_info.bzl','bazel_cc_build_info')\nload(':wrapper.bzl','wrapper')\nplatform(name='platform')\nbazel_cc_build_info(name='info')\nwrapper(name='wrapper',dep=':info')\n", 1);
    epoch.build()
}

async fn build_info_transaction(dice: &Arc<Dice>) -> dice::DiceTransaction {
    build_info_transaction_with_epoch(dice, build_info_epoch()).await
}

async fn build_info_transaction_with_epoch(
    dice: &Arc<Dice>,
    epoch: PathObservationEpoch,
) -> dice::DiceTransaction {
    let transaction = build_root_transaction(dice, epoch).await;
    let mut updater = transaction.into_updater();
    // The generic helper uses a Local bazel_tools override for small authored
    // fixtures. This authenticated builtin requires the actual Builtin route.
    inject_root_module_request_inputs(
        &mut updater,
        Path::new("/workspace"),
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    // Complete every declared Local module: the mapping requests four explicit
    // root dependencies first, then the other ten builtin dependency routes.
    let entries = BUILD_INFO_MODULES.iter().map(|&(name, _)| {
        let mut attributes = SmallMap::new();
        attributes.insert(
            "path".into(),
            slug_bzlmod_v2::OverrideAttributeValue::String(name.into()),
        );
        slug_bzlmod_v2::RepositoryMaterializationEpochEntry {
            request: Arc::new(RepositoryMaterializationRequest {
                id: RepositoryMaterializationRequestId {
                    workspace: workspace.clone(),
                    canonical_repo: slug_identity_v2::CanonicalRepoName::new(format!("{name}+"))
                        .unwrap(),
                },
                repo_spec: slug_bzlmod_v2::RepoSpec {
                    rule_id: slug_bzlmod_v2::RepoRuleId {
                        bzl_file: CanonicalLabel::parse(
                            "@@bazel_tools//tools/build_defs/repo:local.bzl",
                        )
                        .unwrap(),
                        rule_name: "local_repository".into(),
                    },
                    attributes: Arc::new(attributes),
                },
                kind: slug_bzlmod_v2::RepositoryMaterializationKind::Local {
                    logical_root: NormalizedAbsolutePath::new(format!("/workspace/{name}"))
                        .unwrap(),
                },
            }),
            result: slug_bzlmod_v2::RepositoryMaterializationResult::Success(
                slug_bzlmod_v2::RepositoryMaterializationSuccess::Local,
            ),
        }
    });
    updater
        .changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: workspace.clone(),
            },
            RepositoryMaterializationResultEpoch::new(workspace.dupe(), entries).unwrap(),
        )])
        .unwrap();
    updater.commit().await
}

mod template_inputs {
    use super::*;
    include!("build_info_template_inputs_tests.rs");
}

fn build_info_key(target: &str, changed: bool) -> BuildCommandRootKey {
    let base = build_test_configuration("query");
    let configuration = if changed {
        let overlay: slug_configuration_v2::CommandConfigurationOverlay = vec![
            slug_configuration_v2::CommandConfigurationOccurrence::native(
                slug_configuration_v2::NativeCommandOption::Copt,
                Some("-DBUILD_INFO_B"),
                false,
            ),
        ]
        .into();
        ConfigurationKey::from_slug(
            base.slug_configuration()
                .unwrap()
                .with_command_configuration(base.starlark_options().clone(), &overlay)
                .unwrap(),
        )
    } else {
        base
    };
    BuildCommandRootKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        &[TargetPattern::parse(target).unwrap()],
        configuration,
    )
    .unwrap()
}

fn build_info_evaluation(outcome: &BuildCommandOutcome) -> &BuildCommandEvaluation {
    let slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) = outcome else {
        panic!("BuildInfo fixture retained Needs: {outcome:?}")
    };
    value.as_ref().as_ref().unwrap()
}

fn build_info_owner(evaluation: &BuildCommandEvaluation) -> &Arc<ConfiguredNodeResult> {
    evaluation
        .action_closure
        .owners()
        .iter()
        .find(|owner| {
            owner
                .actions()
                .iter()
                .any(|action| action.build_info_transform_spec().is_some())
        })
        .unwrap()
}

fn build_info_text(evaluation: &BuildCommandEvaluation, scope: AqueryScope) -> String {
    crate::runtime::format_aquery_text_output_for_scope(evaluation, scope).unwrap()
}

fn build_info_records(text: &str) -> Vec<&str> {
    assert!(text.ends_with("\n\n"));
    text.split_inclusive("\n\n")
        .map(|record| record.strip_suffix('\n').unwrap())
        .collect()
}

#[tokio::test]
async fn build_info_aquery_real_producer_fields_scopes_and_borrowed_owners() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut transaction = build_info_transaction(&dice).await;
    for (target, tool, literal_count, deps_count) in
        [("//:info", false, 3, 3), ("//:wrapper", true, 1, 4)]
    {
        let key = build_info_key(target, false);
        let outcome = transaction.compute(&key).await.unwrap();
        let evaluation = build_info_evaluation(&outcome);
        assert!(std::ptr::eq(
            key.workspace.as_path(),
            evaluation.workspace().as_path()
        ));
        let owner = build_info_owner(evaluation);
        assert_eq!(owner.actions().len(), 3);
        let literal = build_info_text(evaluation, AqueryScope::Literal);
        let deps = build_info_text(evaluation, AqueryScope::Deps);
        assert_eq!(build_info_records(&literal).len(), literal_count);
        let blocks = build_info_records(&deps);
        assert_eq!(blocks.len(), deps_count);
        for (index, role, metadata) in [
            (0, "volatile", ActionOutputMetadata::Constant),
            (1, "stable", ActionOutputMetadata::Ordinary),
        ] {
            let action = &owner.actions()[index];
            let view = ConfiguredBuildInfoActionView::from_configured_action(
                action,
                evaluation.workspace(),
            )
            .unwrap();
            let spec = action.build_info_transform_spec().unwrap();
            assert!(std::ptr::eq(view.action(), action));
            assert!(std::ptr::eq(view.spec(), spec));
            assert!(std::ptr::eq(view.context(), action.context().as_ref()));
            assert!(std::ptr::eq(view.owner(), action.context().owner()));
            assert!(std::ptr::eq(
                view.execution_platform(),
                action.context().execution_platform().unwrap()
            ));
            assert!(std::ptr::eq(view.status(), spec.status()));
            assert!(std::ptr::eq(view.template(), spec.template()));
            assert!(std::ptr::eq(view.callback(), spec.callback()));
            assert!(std::ptr::eq(view.output(), spec.output()));
            assert_eq!(
                view.callback().source().to_string(),
                BuildInfoCallbackRecipe::SOURCE_LABEL
            );
            assert_eq!(
                view.callback().digest(),
                &BuildInfoCallbackRecipe::SOURCE_DIGEST
            );
            assert_eq!(view.output().metadata(), metadata);
            assert_eq!(view.status().root().workspace(), evaluation.workspace());
            assert_eq!(
                view.owner().configuration().kind() == slug_analysis_v2::ConfigurationKind::Exec,
                tool
            );
            let mut inputs = spec
                .inputs()
                .iter()
                .map(AnalysisArtifact::path)
                .collect::<Vec<_>>();
            inputs.sort();
            let expected = format!(
                "action 'Translating {role} BuildInfo file{}'\n  Mnemonic: TranslateBuildInfo\n  Target: //:info\n  Configuration: {}\n  Execution platform: //:platform\n  Inputs: [{}]\n  Outputs: [{}]\n",
                if tool { " [for tool]" } else { "" },
                view.owner()
                    .configuration()
                    .slug_configuration()
                    .unwrap()
                    .projection()
                    .display_token(),
                inputs.join(", "),
                view.output().path()
            );
            assert_eq!(
                blocks.iter().filter(|record| **record == expected).count(),
                1,
                "{deps}"
            );
            for omitted in [
                "  Environment:",
                "  ExecutionInfo:",
                "  Command Line:",
                "  IsExecutable:",
                "  Template:",
                "  Substitutions:",
                "  FileWriteContents:",
                "  ActionKey:",
                "  SlugActionToken:",
                "(TreeArtifact)",
                "(Constant)",
            ] {
                assert!(!expected.contains(omitted));
            }
        }
        assert_eq!(literal.contains("  Mnemonic: TranslateBuildInfo\n"), !tool);
        let actual_order = blocks
            .iter()
            .map(|record| {
                record
                    .lines()
                    .find_map(|line| line.strip_prefix("  Mnemonic: "))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let expected_order = evaluation
            .action_closure
            .owners()
            .iter()
            .flat_map(|owner| owner.actions().iter().map(|action| action.mnemonic()))
            .collect::<Vec<_>>();
        assert_eq!(actual_order, expected_order);
    }
}

#[tokio::test]
async fn build_info_aquery_restores_held_results_and_owned_text() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut transaction = build_info_transaction(&dice).await;
    let a = build_info_key("//:info", false);
    let b = build_info_key("//:info", true);
    let first = transaction.compute(&a).await.unwrap();
    let held = build_info_evaluation(&first);
    let snapshot = held.clone();
    let text = build_info_text(held, AqueryScope::Deps);
    let changed = transaction.compute(&b).await.unwrap();
    let changed_evaluation = build_info_evaluation(&changed);
    assert_ne!(held, changed_evaluation);
    assert_ne!(text, build_info_text(changed_evaluation, AqueryScope::Deps));
    for (left, right) in build_info_owner(held).actions()[..2]
        .iter()
        .zip(&build_info_owner(changed_evaluation).actions()[..2])
    {
        let left = left.build_info_transform_spec().unwrap();
        let right = right.build_info_transform_spec().unwrap();
        assert_eq!(left.status(), right.status());
        assert_eq!(left.template(), right.template());
        assert_eq!(left.callback(), right.callback());
        assert_ne!(left.output().root(), right.output().root());
    }
    assert_eq!(held, &snapshot);
    let restored = transaction.compute(&a).await.unwrap();
    assert_eq!(build_info_evaluation(&restored), &snapshot);
    assert_eq!(
        build_info_text(build_info_evaluation(&restored), AqueryScope::Deps),
        text
    );
    drop(restored);
    drop(changed);
    drop(first);
    drop(snapshot);
    drop(transaction);
    drop(dice);
    assert_eq!(build_info_records(&text).len(), 3);
    assert!(text.contains("Translating stable BuildInfo file"));
    assert!(text.contains("Translating volatile BuildInfo file"));
}

fn build_info_node(
    original: &ConfiguredNodeResult,
    specs: Vec<ActionSpec>,
    resolved: bool,
) -> Arc<ConfiguredNodeResult> {
    let key = original.configured_target_key().unwrap().clone();
    let previous = original.actions()[0].context();
    let context = if resolved {
        ConfiguredActionOwnerContext::new(
            key.clone(),
            ConfiguredExecGroup::Default,
            previous.execution_platform().unwrap().clone(),
            PlatformSemanticFact {
                exec_properties: Arc::from([
                    ("a".into(), "a value".into()),
                    ("z".into(), "z value".into()),
                ]),
                missing_toolchain_error: None,
            },
            &Default::default(),
            &Default::default(),
            Vec::new(),
            None,
            ConfiguredActionAspectProvenance::Absent,
        )
        .unwrap()
    } else {
        ConfiguredActionOwnerContext::unresolved_default(key.clone()).unwrap()
    };
    Arc::new(
        ConfiguredNodeResult::new_rule(
            key,
            original.providers().clone(),
            None,
            original.runfiles_packages().clone(),
        )
        .with_action_specs(specs, vec![Arc::new(context)])
        .unwrap(),
    )
}

fn build_info_replaced(
    evaluation: &BuildCommandEvaluation,
    node: Arc<ConfiguredNodeResult>,
) -> BuildCommandEvaluation {
    let mut targets = evaluation.targets.to_vec();
    targets[0].analysis = Some(node.clone());
    BuildCommandEvaluation {
        workspace: evaluation.workspace().clone(),
        anchor: evaluation.anchor.clone(),
        targets: targets.into(),
        action_closure: ValidatedActionClosure::new(Arc::from([node])).unwrap(),
    }
}

#[tokio::test]
async fn build_info_aquery_metadata_guards_info_identity_and_priority() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut transaction = build_info_transaction(&dice).await;
    let outcome = transaction
        .compute(&build_info_key("//:info", false))
        .await
        .unwrap();
    let evaluation = build_info_evaluation(&outcome);
    let original = build_info_owner(evaluation);
    let spec = original.actions()[1].build_info_transform_spec().unwrap();
    let make = |template: AnalysisArtifact| {
        ActionSpec::build_info_transform(
            BuildInfoTransformSpec::new(
                spec.status().clone(),
                template,
                spec.output().clone(),
                spec.callback().clone(),
            )
            .unwrap(),
        )
    };
    let foreign_workspace = NormalizedAbsolutePath::new("/foreign").unwrap();
    let foreign = BuildCommandEvaluation {
        workspace: foreign_workspace.clone(),
        ..evaluation.clone()
    };
    assert_ne!(evaluation, &foreign);
    assert_eq!(evaluation, &evaluation.clone());
    for action in &original.actions()[..2] {
        assert_eq!(
            ConfiguredBuildInfoActionView::from_configured_action(action, &foreign_workspace)
                .unwrap_err(),
            "configured BuildInfo status belongs to a different command workspace"
        );
    }
    for scope in [AqueryScope::Literal, AqueryScope::Deps] {
        assert!(
            crate::runtime::format_aquery_text_output_for_scope(&foreign, scope)
                .unwrap_err()
                .to_string()
                .contains("configured BuildInfo status belongs to a different command workspace")
        );
    }
    let foreign_status = WorkspaceStatusArtifacts::new(foreign_workspace);
    assert_eq!(foreign_status.stable().path(), spec.status().path());
    assert_ne!(foreign_status.stable(), spec.status());
    let input_key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:template_owner").unwrap(),
        ConfigurationKey::from_slug(
            original
                .configured_target_key()
                .unwrap()
                .configuration()
                .slug_configuration()
                .unwrap()
                .to_exec()
                .unwrap(),
        ),
    );
    let input_root = slug_build_api_v2::DerivedArtifactRoot::bin(
        input_key
            .configuration()
            .slug_configuration()
            .unwrap()
            .clone(),
    )
    .unwrap();
    let derived = AnalysisArtifact::Derived {
        owner: input_key.artifact_owner(),
        output: ActionOutput::new(input_root.clone(), "template", ActionOutputKind::File),
    };
    let accepted = build_info_node(original, vec![make(derived.clone())], true);
    let view = ConfiguredBuildInfoActionView::from_configured_action(
        &accepted.actions()[0],
        evaluation.workspace(),
    )
    .unwrap();
    assert_ne!(input_key.configuration(), view.owner().configuration());
    let text = build_info_text(
        &build_info_replaced(evaluation, accepted.clone()),
        AqueryScope::Deps,
    );
    assert!(
        text.contains("  ExecutionInfo: {a: 'a value', z: 'z value'}\n"),
        "{text}"
    );
    assert!(!text.contains("  Environment:"));
    let same_display =
        AnalysisArtifact::Source(CanonicalLabel::parse("@@//bazel-out:stable-status.txt").unwrap());
    assert_eq!(same_display.path(), spec.status().path());
    let identical_paths = build_info_node(original, vec![make(same_display)], true);
    assert_eq!(
        identical_paths.actions()[0]
            .build_info_transform_spec()
            .unwrap()
            .inputs()
            .len(),
        2
    );
    let text = build_info_text(
        &build_info_replaced(evaluation, identical_paths),
        AqueryScope::Deps,
    );
    assert!(
        text.contains("  Inputs: [bazel-out/stable-status.txt, bazel-out/stable-status.txt]\n"),
        "{text}"
    );
    let ordinary = "configured BuildInfo query requires a Source or Derived ordinary File template";
    for (template, expected) in [
        (
            AnalysisArtifact::WorkspaceStatus(spec.status().clone()),
            ordinary,
        ),
        (
            AnalysisArtifact::Derived {
                owner: input_key.artifact_owner(),
                output: ActionOutput::new(
                    input_root.clone(),
                    "directory",
                    ActionOutputKind::Directory,
                ),
            },
            ordinary,
        ),
        (
            AnalysisArtifact::Derived {
                owner: input_key.artifact_owner(),
                output: ActionOutput::new(input_root.clone(), "link", ActionOutputKind::Symlink),
            },
            ordinary,
        ),
        (
            AnalysisArtifact::Derived {
                owner: input_key.artifact_owner(),
                output: ActionOutput::new_constant(input_root.clone(), "constant"),
            },
            ordinary,
        ),
        (
            AnalysisArtifact::Derived {
                owner: original.configured_target_key().unwrap().artifact_owner(),
                output: ActionOutput::new(input_root, "wrong-owner", ActionOutputKind::File),
            },
            "configured BuildInfo template root differs from its own owner",
        ),
    ] {
        let rejected = build_info_node(original, vec![make(template)], true);
        let saved = rejected.as_ref().clone();
        assert_eq!(
            ConfiguredBuildInfoActionView::from_configured_action(
                &rejected.actions()[0],
                evaluation.workspace()
            )
            .unwrap_err(),
            expected
        );
        assert_eq!(rejected.as_ref(), &saved);
    }
    for (action, resolved, expected) in [
        (
            make(derived.clone()),
            false,
            "configured BuildInfo action requires a selected execution platform",
        ),
        (
            make(derived.clone()).with_exec_properties(std::collections::BTreeMap::from([(
                "override".to_owned(),
                "value".to_owned(),
            )])),
            true,
            "configured BuildInfo query requires default action group and properties",
        ),
    ] {
        let rejected = build_info_node(original, vec![action], resolved);
        assert_eq!(
            ConfiguredBuildInfoActionView::from_configured_action(
                &rejected.actions()[0],
                evaluation.workspace()
            )
            .unwrap_err(),
            expected
        );
    }
    let write = ActionSpec::new(
        ActionKind::Write {
            content: String::new(),
            is_executable: true,
        },
        "FileWrite",
        vec![ActionOutput::new(
            spec.output().root().clone(),
            "early'quoted",
            ActionOutputKind::File,
        )],
    );
    let rejected = build_info_node(
        original,
        vec![
            write,
            make(AnalysisArtifact::WorkspaceStatus(spec.status().clone())),
        ],
        true,
    );
    let early_view = ResolvedFileWriteSemanticView::from_configured_action(
        slug_analysis_v2::ConfiguredActionView::from_configured_action(&rejected.actions()[0])
            .unwrap(),
    );
    assert_eq!(
        crate::runtime::format_file_write_aquery_text(&early_view),
        Err("FileWrite aquery text requires a normalized relative output")
    );
    let replaced = build_info_replaced(evaluation, rejected);
    for scope in [AqueryScope::Literal, AqueryScope::Deps] {
        assert!(
            crate::runtime::format_aquery_text_output_for_scope(&replaced, scope)
                .unwrap_err()
                .to_string()
                .contains(ordinary)
        );
    }
}
