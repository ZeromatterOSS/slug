use slug_analysis_v2::ConfiguredActionAspectProvenance;
use slug_analysis_v2::ConfiguredActionOwnerContext;
use slug_analysis_v2::ConfiguredExecGroup;
use slug_analysis_v2::ConfiguredNodeResult;
use slug_analysis_v2::ConfiguredSolibActionView;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_analysis_v2::PlatformSemanticFact;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::SolibSymlinkSpec;
use slug_query_v2::AqueryScope;

use super::*;

// Authored fixture in the existing source-restricted binder domain. This does
// not reproduce a full rules_cc implementation or execute a library/link.
const SOLIB_DEFS: &str = r#"internal = cc_common.internal_DO_NOT_USE()
def _library(ctx):
    out = ctx.actions.declare_file(ctx.attr.out)
    ctx.actions.write(out, 'library')
    return [DefaultInfo(files=depset([out]))]
library = rule(implementation=_library, attrs={'out':attr.string()})
def _consumer(ctx):
    first = ctx.actions.declare_file('first')
    last = ctx.actions.declare_file('last')
    ctx.actions.write(first, 'first')
    link = internal.dynamic_library_symlink(ctx.actions, ctx.file.library, '_solib_k8', False, False)
    ctx.actions.write(last, 'last')
    return [DefaultInfo(files=depset([first, link, last]))]
consumer = rule(implementation=_consumer, attrs={'library':attr.label(cfg='exec',allow_single_file=True)})
def _wrapper(ctx):
    out = ctx.actions.declare_file('wrapper')
    ctx.actions.write(out, 'wrapper')
    return [DefaultInfo(files=depset([out]))]
wrapper = rule(implementation=_wrapper, attrs={'dep':attr.label(cfg='exec')})
"#;

fn solib_epoch(revision: i64, filename: &str) -> PathObservationEpoch {
    let mut epoch = BuildRootEpoch::base(revision);
    epoch.file("/workspace/MODULE.bazel", "module(name='root')\nbazel_dep(name='rules_cc',version='0.2.17')\nlocal_path_override(module_name='rules_cc',path='rules_cc')\nregister_execution_platforms('//:platform')\n", revision);
    epoch.file(
        "/workspace/rules_cc/MODULE.bazel",
        "module(name='rules_cc',version='0.2.17')\n",
        revision,
    );
    epoch.file("/workspace/rules_cc/defs.bzl", SOLIB_DEFS, revision);
    epoch.package("rules_cc", "exports_files(['defs.bzl'])\n", revision);
    epoch.missing("/workspace/rules_cc/REPO.bazel");
    epoch.missing("/workspace/rules_cc/.bazelignore");
    epoch.package("", &format!("load('@rules_cc//:defs.bzl','library','consumer','wrapper')\nplatform(name='platform')\nlibrary(name='library',out='{filename}')\nconsumer(name='consumer',library=':library')\nwrapper(name='wrapper',dep=':consumer')\n"), revision);
    epoch.build()
}

async fn solib_transaction(
    dice: &Arc<Dice>,
    revision: i64,
    filename: &str,
) -> dice::DiceTransaction {
    let transaction = build_root_transaction(dice, solib_epoch(revision, filename)).await;
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    let entries = [
        (
            "bazel_tools+",
            "/workspace/.slug_test_builtin/bazel_tools",
            "/workspace/.slug_test_builtin/bazel_tools",
        ),
        ("rules_cc+", "/workspace/rules_cc", "rules_cc"),
    ]
    .map(|(repo, path, declared_path)| {
        let mut attributes = SmallMap::new();
        attributes.insert(
            "path".into(),
            slug_bzlmod_v2::OverrideAttributeValue::String(declared_path.into()),
        );
        slug_bzlmod_v2::RepositoryMaterializationEpochEntry {
            request: Arc::new(RepositoryMaterializationRequest {
                id: RepositoryMaterializationRequestId {
                    workspace: workspace.clone(),
                    canonical_repo: slug_identity_v2::CanonicalRepoName::new(repo).unwrap(),
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
                    logical_root: NormalizedAbsolutePath::new(path).unwrap(),
                },
            }),
            result: slug_bzlmod_v2::RepositoryMaterializationResult::Success(
                slug_bzlmod_v2::RepositoryMaterializationSuccess::Local,
            ),
        }
    });
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: workspace.clone(),
            },
            RepositoryMaterializationResultEpoch::new(workspace, entries).unwrap(),
        )])
        .unwrap();
    updater.commit().await
}

fn solib_key(target: &str) -> BuildCommandRootKey {
    BuildCommandRootKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        &[TargetPattern::parse(target).unwrap()],
        build_test_configuration("query"),
    )
    .unwrap()
}

fn solib_evaluation(outcome: &BuildCommandOutcome) -> &BuildCommandEvaluation {
    let slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) = outcome else {
        panic!("Solib fixture retained Needs: {outcome:?}")
    };
    value.as_ref().as_ref().unwrap()
}

fn solib_owner(evaluation: &BuildCommandEvaluation) -> &Arc<ConfiguredNodeResult> {
    evaluation
        .action_closure
        .owners()
        .iter()
        .find(|owner| {
            owner
                .actions()
                .iter()
                .any(|action| action.solib_symlink_spec().is_some())
        })
        .unwrap()
}

fn solib_text(evaluation: &BuildCommandEvaluation, scope: AqueryScope) -> String {
    crate::runtime::format_aquery_text_output_for_scope(evaluation, scope).unwrap()
}

fn solib_records(text: &str) -> Vec<&str> {
    assert!(text.ends_with("\n\n"));
    text.split_inclusive("\n\n")
        .map(|record| record.strip_suffix('\n').unwrap())
        .collect()
}

#[tokio::test]
async fn solib_aquery_real_binder_fields_scopes_and_tool_owners() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut transaction = solib_transaction(&dice, 1, "lib-a.so").await;
    for (target, tool, literal_count, deps_count) in
        [("//:consumer", false, 3, 4), ("//:wrapper", true, 1, 5)]
    {
        let outcome = transaction.compute(&solib_key(target)).await.unwrap();
        let evaluation = solib_evaluation(&outcome);
        let owner = solib_owner(evaluation);
        let action = &owner.actions()[1];
        let view = ConfiguredSolibActionView::from_configured_action(action).unwrap();
        assert!(std::ptr::eq(view.action(), action));
        assert!(std::ptr::eq(
            view.spec(),
            action.solib_symlink_spec().unwrap()
        ));
        assert!(std::ptr::eq(view.context(), action.context().as_ref()));
        assert!(std::ptr::eq(view.owner(), action.context().owner()));
        assert!(std::ptr::eq(
            view.execution_platform(),
            action.context().execution_platform().unwrap()
        ));
        assert!(std::ptr::eq(view.library(), view.spec().input()));
        assert!(std::ptr::eq(view.output(), view.spec().output()));
        assert_eq!(
            view.owner().configuration().kind() == slug_analysis_v2::ConfigurationKind::Exec,
            tool
        );
        let AnalysisArtifact::Derived {
            owner: library_owner,
            output: library,
        } = view.library()
        else {
            panic!("production generated library")
        };
        assert_ne!(library_owner.label(), view.owner().label());
        assert_eq!(
            library.root().configuration().canonical_bytes(),
            library_owner.configuration()
        );
        if !tool {
            assert_ne!(
                library_owner.configuration(),
                view.owner()
                    .configuration()
                    .slug_configuration()
                    .unwrap()
                    .canonical_bytes()
            );
        }
        let literal = solib_text(evaluation, AqueryScope::Literal);
        let deps = solib_text(evaluation, AqueryScope::Deps);
        assert_eq!(solib_records(&literal).len(), literal_count);
        let blocks = solib_records(&deps);
        assert_eq!(blocks.len(), deps_count);
        let expected = format!(
            "action 'SolibSymlink {}'\n  Mnemonic: SolibSymlink\n  Target: //:consumer\n  Configuration: {}\n  Execution platform: //:platform\n  Inputs: [{}]\n  Outputs: [{}]\n",
            view.output().root_relative_path(),
            view.owner()
                .configuration()
                .slug_configuration()
                .unwrap()
                .projection()
                .display_token(),
            view.library().path(),
            view.output().path()
        );
        assert_eq!(
            blocks.iter().filter(|record| **record == expected).count(),
            1,
            "{deps}"
        );
        assert_eq!(literal.contains("  Mnemonic: SolibSymlink\n"), !tool);
        let expected_order = evaluation
            .action_closure
            .owners()
            .iter()
            .flat_map(|owner| owner.actions().iter().map(|action| action.mnemonic()))
            .collect::<Vec<_>>();
        let actual_order = blocks
            .iter()
            .map(|record| {
                record
                    .lines()
                    .find_map(|line| line.strip_prefix("  Mnemonic: "))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(actual_order, expected_order);
        for omitted in [
            "[for tool]",
            "  Environment:",
            "  ExecutionInfo:",
            "  Command Line:",
            "  IsExecutable:",
            "  ActionKey:",
            "  SlugActionToken:",
            "(TreeArtifact)",
        ] {
            assert!(!expected.contains(omitted));
        }
    }
}

#[tokio::test]
async fn solib_aquery_restores_held_results_and_owned_text() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let key = solib_key("//:wrapper");
    let mut first_tx = solib_transaction(&dice, 10, "lib-a.so").await;
    let first = first_tx.compute(&key).await.unwrap();
    let held = solib_evaluation(&first);
    let snapshot = solib_owner(held).as_ref().clone();
    let text = solib_text(held, AqueryScope::Deps);
    drop(first_tx);
    let mut changed_tx = solib_transaction(&dice, 11, "lib-b.so").await;
    let changed = changed_tx.compute(&key).await.unwrap();
    assert_ne!(solib_owner(solib_evaluation(&changed)).as_ref(), &snapshot);
    assert_ne!(
        solib_text(solib_evaluation(&changed), AqueryScope::Deps),
        text
    );
    assert_eq!(solib_owner(held).as_ref(), &snapshot);
    assert_eq!(solib_text(held, AqueryScope::Deps), text);
    drop(changed_tx);
    drop(changed);
    let mut restored_tx = solib_transaction(&dice, 12, "lib-a.so").await;
    let restored = restored_tx.compute(&key).await.unwrap();
    assert_eq!(solib_owner(solib_evaluation(&restored)).as_ref(), &snapshot);
    assert_eq!(
        solib_text(solib_evaluation(&restored), AqueryScope::Deps),
        text
    );
    drop(restored_tx);
    drop(restored);
    drop(first);
    drop(snapshot);
    drop(dice);
    assert_eq!(solib_records(&text).len(), 5);
    assert!(text.contains("lib-a.so"));
    assert!(!text.contains("lib-b.so"));
}

fn synthetic_solib_node(
    key: ConfiguredTargetKey,
    specs: Vec<ActionSpec>,
    resolved: bool,
) -> Arc<ConfiguredNodeResult> {
    let context = if resolved {
        ConfiguredActionOwnerContext::new(
            key.clone(),
            ConfiguredExecGroup::Default,
            ConfiguredTargetKey::new(
                CanonicalLabel::parse("@@//:platform").unwrap(),
                ConfigurationKey::from_slug(
                    key.configuration()
                        .slug_configuration()
                        .unwrap()
                        .to_exec()
                        .unwrap(),
                ),
            ),
            PlatformSemanticFact {
                exec_properties: Arc::from([("retained".into(), "owner-value".into())]),
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
            slug_build_api_v2::ProviderCollection::from_values(Vec::new(), false).unwrap(),
            None,
            slug_build_api_v2::RunfilesPackageDepset::empty(),
        )
        .with_action_specs(specs, vec![Arc::new(context)])
        .unwrap(),
    )
}

#[tokio::test]
async fn solib_aquery_metadata_guards_literal_progress_multiplicity_and_priority() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut transaction = solib_transaction(&dice, 30, "lib-a.so").await;
    let outcome = transaction
        .compute(&solib_key("//:consumer"))
        .await
        .unwrap();
    let evaluation = solib_evaluation(&outcome);
    let configuration = build_test_configuration("query");
    let owner = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:first").unwrap(),
        configuration.clone(),
    );
    let root = slug_build_api_v2::DerivedArtifactRoot::bin(
        configuration.slug_configuration().unwrap().clone(),
    )
    .unwrap();
    let library_key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:library").unwrap(),
        ConfigurationKey::from_slug(
            configuration
                .slug_configuration()
                .unwrap()
                .to_exec()
                .unwrap(),
        ),
    );
    let library_root = slug_build_api_v2::DerivedArtifactRoot::bin(
        library_key
            .configuration()
            .slug_configuration()
            .unwrap()
            .clone(),
    )
    .unwrap();
    let input = AnalysisArtifact::Derived {
        owner: library_key.artifact_owner(),
        output: ActionOutput::new(
            library_root.clone(),
            "lib-%{label}-%{input}-%{output}.so",
            ActionOutputKind::File,
        ),
    };
    let action = |input: AnalysisArtifact, output_root| {
        ActionSpec::solib_symlink(
            SolibSymlinkSpec::new(input, output_root, "_solib_k8", owner.label(), false, false)
                .unwrap(),
        )
    };
    for (input, expected) in [
        (
            AnalysisArtifact::Source(CanonicalLabel::parse("@@//:lib.so").unwrap()),
            "configured Solib query requires a generated ordinary regular File library",
        ),
        (
            AnalysisArtifact::Derived {
                owner: library_key.artifact_owner(),
                output: ActionOutput::new_constant(library_root.clone(), "lib.so"),
            },
            "configured Solib query requires a generated ordinary regular File library",
        ),
        (
            AnalysisArtifact::Derived {
                owner: owner.artifact_owner(),
                output: ActionOutput::new(library_root.clone(), "lib.so", ActionOutputKind::File),
            },
            "configured Solib library root differs from its own configured owner",
        ),
    ] {
        let node = synthetic_solib_node(owner.clone(), vec![action(input, root.clone())], true);
        let saved = node.as_ref().clone();
        assert_eq!(
            ConfiguredSolibActionView::from_configured_action(&node.actions()[0]).unwrap_err(),
            expected
        );
        assert_eq!(node.as_ref(), &saved);
    }
    let unresolved = synthetic_solib_node(
        owner.clone(),
        vec![action(input.clone(), root.clone())],
        false,
    );
    assert_eq!(
        ConfiguredSolibActionView::from_configured_action(&unresolved.actions()[0]).unwrap_err(),
        "configured Solib action requires a selected execution platform"
    );
    let custom = synthetic_solib_node(
        owner.clone(),
        vec![action(input.clone(), root.clone()).with_exec_properties(
            std::collections::BTreeMap::from([("custom".to_owned(), "value".to_owned())]),
        )],
        true,
    );
    assert_eq!(
        ConfiguredSolibActionView::from_configured_action(&custom.actions()[0]).unwrap_err(),
        "configured Solib query requires default action group and properties"
    );
    let other_root = slug_build_api_v2::DerivedArtifactRoot::bin(
        configuration
            .slug_configuration()
            .unwrap()
            .to_exec()
            .unwrap(),
    )
    .unwrap();
    let context = unresolved.actions()[0].context().clone();
    assert_eq!(
        unresolved
            .as_ref()
            .clone()
            .with_action_specs(vec![action(input.clone(), other_root)], vec![context])
            .unwrap_err(),
        "configured action output root differs from action configuration"
    );
    let spec = action(input.clone(), root.clone());
    let first = synthetic_solib_node(owner.clone(), vec![spec.clone()], true);
    let second = synthetic_solib_node(
        ConfiguredTargetKey::new(CanonicalLabel::parse("@@//:second").unwrap(), configuration),
        vec![spec],
        true,
    );
    let closure =
        super::ValidatedActionClosure::new(Arc::from([first.clone(), second.clone()])).unwrap();
    assert!(closure.execution_representative(0, 0));
    assert!(!closure.execution_representative(1, 0));
    let altered = BuildCommandEvaluation {
        workspace: evaluation.workspace.dupe(),
        anchor: evaluation.anchor.clone(),
        targets: evaluation.targets.clone(),
        action_closure: closure,
    };
    let text = solib_text(&altered, AqueryScope::Deps);
    let blocks = solib_records(&text);
    assert_eq!(blocks.len(), 2);
    for block in &blocks {
        assert!(block.starts_with(&format!(
            "action 'SolibSymlink {}'\n",
            first.actions()[0].outputs()[0].root_relative_path()
        )));
        for placeholder in ["%{label}", "%{input}", "%{output}"] {
            assert!(block.lines().next().unwrap().contains(placeholder));
        }
        for omitted in ["  Environment:", "  ExecutionInfo:", "[for tool]"] {
            assert!(!block.contains(omitted));
        }
    }
    assert!(blocks[0].contains("  Target: //:first\n"));
    assert!(blocks[1].contains("  Target: //:second\n"));
    let invalid = synthetic_solib_node(
        owner.clone(),
        vec![
            ActionSpec::new(
                ActionKind::Write {
                    content: String::new(),
                    is_executable: true,
                },
                "FileWrite",
                vec![ActionOutput::new(
                    root.clone(),
                    "early'quoted",
                    ActionOutputKind::File,
                )],
            ),
            action(
                AnalysisArtifact::Source(CanonicalLabel::parse("@@//:lib.so").unwrap()),
                root,
            ),
        ],
        true,
    );
    let early_view = ResolvedFileWriteSemanticView::from_configured_action(
        slug_analysis_v2::ConfiguredActionView::from_configured_action(&invalid.actions()[0])
            .unwrap(),
    );
    assert_eq!(
        crate::runtime::format_file_write_aquery_text(&early_view),
        Err("FileWrite aquery text requires a normalized relative output")
    );
    let mut targets = evaluation.targets.to_vec();
    targets[0].analysis = Some(invalid.clone());
    let rejected = BuildCommandEvaluation {
        workspace: evaluation.workspace.dupe(),
        anchor: evaluation.anchor.clone(),
        targets: targets.into(),
        action_closure: super::ValidatedActionClosure::new(Arc::from([invalid])).unwrap(),
    };
    for scope in [AqueryScope::Literal, AqueryScope::Deps] {
        assert_eq!(
            crate::runtime::format_aquery_text_output_for_scope(&rejected, scope),
            Err("configured Solib query requires a generated ordinary regular File library".into())
        );
    }
    assert_eq!(
        solib_records(&solib_text(evaluation, AqueryScope::Literal)).len(),
        3
    );
}
