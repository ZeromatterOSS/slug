use slug_analysis_v2::ConfigurationKind;
use slug_analysis_v2::ConfiguredActionView;
use slug_analysis_v2::ConfiguredNodeResult;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::ActionSpec;
use slug_query_v2::AqueryScope;

use super::*;

// Authored unrestricted rules: actions originate in the external BUILD, not
// merely an external rule body instantiated by a main-repository caller.
const EXTERNAL_WRITE_DEFS: &str = r#"def _write(ctx):
    outputs = []
    for name in ['z-first', '%{label}', '%{input}', 'a-last']:
        out = ctx.actions.declare_file(name)
        ctx.actions.write(out, ctx.attr.content, is_executable=ctx.attr.flag)
        outputs.append(out)
    return [DefaultInfo(files=depset(outputs))]
write = rule(implementation=_write, attrs={'content':attr.string(), 'flag':attr.bool()})
"#;
const EXTERNAL_BRIDGE_DEFS: &str = r#"def _bridge(ctx):
    out = ctx.actions.declare_file(ctx.attr.out)
    ctx.actions.write(out, 'root-content')
    return [DefaultInfo(files=depset([out]))]
bridge = rule(implementation=_bridge, attrs={'dep':attr.label(), 'out':attr.string()})
wrapper = rule(implementation=_bridge, attrs={'dep':attr.label(cfg='exec'), 'out':attr.string()})
"#;

async fn external_write_transaction(
    dice: &Arc<Dice>,
    revision: i64,
    changed: bool,
    swapped: bool,
) -> dice::DiceTransaction {
    let mut epoch = BuildRootEpoch::base(revision);
    let (alpha_alias, beta_alias) = if swapped {
        ("other", "dep")
    } else {
        ("dep", "other")
    };
    epoch.file("/workspace/MODULE.bazel", &format!(
        "module(name='root')\nbazel_dep(name='alpha',version='1',repo_name='{alpha_alias}')\nbazel_dep(name='beta',version='1',repo_name='{beta_alias}')\nlocal_path_override(module_name='alpha',path='alpha')\nlocal_path_override(module_name='beta',path='beta')\nregister_execution_platforms('//:platform')\n"), revision);
    epoch.file("/workspace/defs.bzl", EXTERNAL_BRIDGE_DEFS, revision);
    epoch.package("", "load(':defs.bzl','bridge','wrapper')\nplatform(name='platform',exec_properties={'z':'last','a':'first'})\nbridge(name='caller',dep='@dep//pkg:write',out='caller.txt')\nwrapper(name='wrapper',dep='@dep//pkg:write',out='wrapper.txt')\n", revision);
    for name in ["alpha", "beta"] {
        epoch.file(
            &format!("/workspace/{name}/MODULE.bazel"),
            &format!("module(name='{name}',version='1')\n"),
            revision,
        );
        epoch.package(name, "", revision);
        epoch.package(
            &format!("{name}/pkg"),
            &format!(
                "load(':defs.bzl','write')\nwrite(name='write',content='content-{}',flag={},visibility=['//visibility:public'])\n",
                if changed { "B" } else { "A" },
                if changed { "True" } else { "False" }
            ),
            revision,
        );
        epoch.file(
            &format!("/workspace/{name}/pkg/defs.bzl"),
            EXTERNAL_WRITE_DEFS,
            revision,
        );
        epoch.missing(&format!("/workspace/{name}/REPO.bazel"));
        epoch.missing(&format!("/workspace/{name}/.bazelignore"));
    }
    // Same accepted Local schema as solib_transaction; declared relative
    // override paths remain distinct from materialized absolute logical roots.
    let transaction = build_root_transaction(dice, epoch.build()).await;
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    let entries = [
        (
            "bazel_tools+",
            "/workspace/.slug_test_builtin/bazel_tools",
            "/workspace/.slug_test_builtin/bazel_tools",
        ),
        ("alpha+", "/workspace/alpha", "alpha"),
        ("beta+", "/workspace/beta", "beta"),
    ]
    .map(|(repo, path, declared)| {
        let mut attributes = SmallMap::new();
        attributes.insert(
            "path".into(),
            slug_bzlmod_v2::OverrideAttributeValue::String(declared.into()),
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

fn external_write_key(target: &str) -> BuildCommandRootKey {
    BuildCommandRootKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        &[TargetPattern::parse(target).unwrap()],
        build_test_configuration("external query"),
    )
    .unwrap()
}
fn external_write_evaluation(outcome: &BuildCommandOutcome) -> &BuildCommandEvaluation {
    let slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) = outcome else {
        panic!("external Write fixture retained Needs: {outcome:?}")
    };
    value.as_ref().as_ref().unwrap()
}
fn external_write_text(evaluation: &BuildCommandEvaluation, scope: AqueryScope) -> String {
    crate::runtime::format_aquery_text_output_for_scope(evaluation, scope).unwrap()
}
fn external_owner<'a>(
    evaluation: &'a BuildCommandEvaluation,
    repo: &str,
) -> &'a Arc<ConfiguredNodeResult> {
    let expected = format!("@@{repo}//pkg:write");
    evaluation
        .action_closure
        .owners()
        .iter()
        .find(|node| {
            node.configured_target_key()
                .is_some_and(|key| key.label().to_string() == expected)
        })
        .unwrap()
}

fn expected_scalar(
    action: &slug_analysis_v2::ConfiguredAction,
    relative: &str,
    header: &str,
    target: &str,
    content: &str,
    flag: bool,
) -> String {
    let view = ConfiguredActionView::from_configured_action(action).unwrap();
    assert!(std::ptr::eq(view.spec(), &**action));
    assert!(std::ptr::eq(view.context(), action.context()));
    assert!(std::ptr::eq(view.owner(), action.context().owner()));
    assert!(std::ptr::eq(view.output(), &action.outputs()[0]));
    assert_eq!(
        view.owner().label().to_string().trim_start_matches("@@//"),
        target.trim_start_matches("//")
    );
    assert_eq!(view.output().root_relative_path(), relative);
    let configuration = view.owner().configuration().slug_configuration().unwrap();
    assert_eq!(view.output().root().configuration(), configuration);
    assert_eq!(
        view.output().root(),
        &slug_build_api_v2::DerivedArtifactRoot::bin(configuration.clone()).unwrap()
    );
    assert_eq!(
        view.output().path(),
        format!("{}/{relative}", view.output().root().path())
    );
    assert!(!view.platform_fact().exec_properties.is_empty());
    assert_eq!(
        view.spec().kind(),
        &ActionKind::Write {
            content: content.to_owned(),
            is_executable: flag
        }
    );
    let identity = crate::runtime::file_write_identity::FileWriteSemanticIdentity::from_resolved(
        &ResolvedFileWriteSemanticView::from_configured_action(view),
    )
    .unwrap();
    let suffix = if view.owner().configuration().kind() == ConfigurationKind::Exec {
        " [for tool]"
    } else {
        ""
    };
    format!(
        "action 'Writing {} {header}{suffix}'\n  Mnemonic: FileWrite\n  Target: {target}\n  Configuration: {}\n  Execution platform: //:platform\n  SlugActionToken: {}\n  Inputs: []\n  Outputs: [{}]\n  IsExecutable: {flag}\n\n",
        if flag { "script" } else { "file" },
        configuration.projection().display_token(),
        identity.aquery_display_token(),
        view.output().path()
    )
}
fn expected_external(owner: &ConfiguredNodeResult, repo: &str, changed: bool) -> String {
    assert_eq!(owner.actions().len(), 4);
    let target = format!("@@{repo}//pkg:write");
    let content = if changed { "content-B" } else { "content-A" };
    ["z-first", "%{label}", "%{input}", "a-last"]
        .into_iter()
        .zip(owner.actions())
        .map(|(name, action)| {
            let relative = format!("external/{repo}/pkg/{name}");
            let header = if name == "%{label}" {
                format!("external/{repo}/pkg/{target}")
            } else {
                relative.clone()
            };
            expected_scalar(action, &relative, &header, &target, content, changed)
        })
        .collect()
}
fn expected_bridge(owner: &ConfiguredNodeResult, name: &str) -> String {
    assert_eq!(owner.actions().len(), 1);
    let relative = format!("{name}.txt");
    expected_scalar(
        &owner.actions()[0],
        &relative,
        &relative,
        &format!("//:{name}"),
        "root-content",
        false,
    )
}

#[tokio::test]
async fn file_write_external_aquery_fields_scopes_and_borrowed_roots() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    for (revision, changed) in [(1, false), (2, true)] {
        let mut tx = external_write_transaction(&dice, revision, changed, false).await;
        // The existing top-level external command route accepts exported
        // sources, not rules. External actions below are real dependency owners.
        let refused = tx
            .compute(&external_write_key("@dep//pkg:write"))
            .await
            .unwrap();
        let slug_bzlmod_v2::SourcePreparationOutcome::Complete(refused) = &refused else {
            panic!("standalone external rule retained Needs: {refused:?}")
        };
        let error = refused.as_ref().as_ref().unwrap_err();
        assert!(matches!(
            error.kind,
            BuildCommandErrorKind::ExternalTargetKind
        ));
        assert_eq!(
            error.to_string(),
            "external build target is not an exported source file"
        );
        assert!(error.source_certificate().is_none());
        for (requested, name, kind) in [
            ("//:caller", "caller", ConfigurationKind::Target),
            ("//:wrapper", "wrapper", ConfigurationKind::Exec),
        ] {
            let outcome = tx.compute(&external_write_key(requested)).await.unwrap();
            let evaluation = external_write_evaluation(&outcome);
            let external = external_owner(evaluation, "alpha+");
            assert_eq!(
                external
                    .configured_target_key()
                    .unwrap()
                    .configuration()
                    .kind(),
                kind
            );
            let external_text = expected_external(external, "alpha+", changed);
            let direct_external = external
                .actions()
                .iter()
                .map(|action| {
                    let view = ResolvedFileWriteSemanticView::from_configured_action(
                        ConfiguredActionView::from_configured_action(action).unwrap(),
                    );
                    format!(
                        "{}\n\n",
                        crate::runtime::format_file_write_aquery_text(&view).unwrap()
                    )
                })
                .collect::<String>();
            assert_eq!(direct_external, external_text);
            let action_owners = evaluation
                .action_closure
                .owners()
                .iter()
                .filter(|owner| !owner.actions().is_empty())
                .collect::<Vec<_>>();
            assert_eq!(action_owners.len(), 2);
            assert_eq!(
                action_owners[0]
                    .configured_target_key()
                    .unwrap()
                    .label()
                    .to_string(),
                format!("@@//:{name}")
            );
            assert_eq!(
                action_owners[1]
                    .configured_target_key()
                    .unwrap()
                    .label()
                    .to_string(),
                "@@alpha+//pkg:write"
            );
            let bridge = expected_bridge(action_owners[0], name);
            assert_eq!(
                external_write_text(evaluation, AqueryScope::Literal),
                bridge
            );
            if kind == ConfigurationKind::Exec {
                assert_ne!(
                    external.actions()[0].outputs()[0].root(),
                    action_owners[0].actions()[0].outputs()[0].root()
                );
            }
            let expected = bridge + &external_text;
            assert_eq!(external_write_text(evaluation, AqueryScope::Deps), expected);
            assert_eq!(crate::runtime::file_write_aquery_text::format_file_write_aquery_text_output_for_scope(
                evaluation, AqueryScope::Deps).unwrap(), expected);
            assert_eq!(expected.matches("  Mnemonic: FileWrite\n").count(), 5);
            assert!(!expected.contains("  Target: @dep"));
            for omitted in [
                "  Environment:",
                "  ExecutionInfo:",
                "  Command Line:",
                "  ActionKey:",
                "(TreeArtifact)",
            ] {
                assert!(!expected.contains(omitted));
            }
        }
    }
    // Synthetic negative metadata through the existing checked publication
    // API: an external structural failure wins over a separate root render error.
    let mut tx = external_write_transaction(&dice, 3, false, false).await;
    let outcome = tx.compute(&external_write_key("//:caller")).await.unwrap();
    let evaluation = external_write_evaluation(&outcome);
    let root = evaluation.targets[0].analysis.as_ref().unwrap();
    let external = external_owner(evaluation, "alpha+");
    let replace = |owner: &ConfiguredNodeResult, path: &str, kind| {
        Arc::new(
            owner
                .clone()
                .with_action_specs(
                    vec![ActionSpec::new(
                        ActionKind::Write {
                            content: "synthetic".to_owned(),
                            is_executable: false,
                        },
                        "FileWrite",
                        vec![ActionOutput::new(
                            owner.actions()[0].outputs()[0].root().clone(),
                            path,
                            kind,
                        )],
                    )],
                    vec![owner.actions()[0].context().clone()],
                )
                .unwrap(),
        )
    };
    let early = replace(root, "early'quoted", ActionOutputKind::File);
    let bad = replace(
        external,
        "external/alpha+/pkg/rejected",
        ActionOutputKind::Directory,
    );
    let early_view = ResolvedFileWriteSemanticView::from_configured_action(
        ConfiguredActionView::from_configured_action(&early.actions()[0]).unwrap(),
    );
    let render_error = Err("FileWrite aquery text requires a normalized relative output");
    assert_eq!(
        crate::runtime::format_file_write_aquery_text(&early_view),
        render_error
    );
    assert_eq!(
        ConfiguredActionView::from_configured_action(&bad.actions()[0]).unwrap_err(),
        "configured FileWrite action requires exactly one file output"
    );
    let mut targets = evaluation.targets.to_vec();
    targets[0].analysis = Some(early.clone());
    let altered = BuildCommandEvaluation {
        workspace: evaluation.workspace.dupe(),
        anchor: evaluation.anchor.clone(),
        targets: targets.into(),
        action_closure: super::ValidatedActionClosure::new(
            evaluation
                .action_closure
                .owners()
                .iter()
                .map(|owner| {
                    if owner.key() == root.key() {
                        early.clone()
                    } else if owner.key() == external.key() {
                        bad.clone()
                    } else {
                        owner.clone()
                    }
                })
                .collect(),
        )
        .unwrap(),
    };
    assert_eq!(
        crate::runtime::format_aquery_text_output_for_scope(&altered, AqueryScope::Literal),
        render_error
    );
    assert_eq!(
        crate::runtime::format_aquery_text_output_for_scope(&altered, AqueryScope::Deps),
        Err("configured FileWrite action requires exactly one file output")
    );
}

#[tokio::test]
async fn file_write_external_aquery_source_mapping_restore_and_owned_text() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let key = external_write_key("//:caller");
    let mut first_tx = external_write_transaction(&dice, 10, false, false).await;
    let first = first_tx.compute(&key).await.unwrap();
    let held = external_write_evaluation(&first);
    let held_owner = external_owner(held, "alpha+").clone();
    let held_spec = (*held_owner.actions()[0]).clone();
    let held_identity =
        crate::runtime::file_write_identity::FileWriteSemanticIdentity::from_resolved(
            &ResolvedFileWriteSemanticView::from_configured_action(
                ConfiguredActionView::from_configured_action(&held_owner.actions()[0]).unwrap(),
            ),
        )
        .unwrap();
    let original = external_write_text(held, AqueryScope::Deps);
    let held_external = expected_external(&held_owner, "alpha+", false);
    assert!(original.ends_with(&held_external));
    drop(first_tx);
    let mut last_changed_text = String::new();
    for (revision, changed, swapped, repo) in [
        (11, true, false, "alpha+"),
        (12, false, false, "alpha+"),
        (13, false, true, "beta+"),
        (14, false, false, "alpha+"),
    ] {
        let mut tx = external_write_transaction(&dice, revision, changed, swapped).await;
        let outcome = tx.compute(&key).await.unwrap();
        let evaluation = external_write_evaluation(&outcome);
        let selected = external_owner(evaluation, repo);
        let text = external_write_text(evaluation, AqueryScope::Deps);
        let identity =
            crate::runtime::file_write_identity::FileWriteSemanticIdentity::from_resolved(
                &ResolvedFileWriteSemanticView::from_configured_action(
                    ConfiguredActionView::from_configured_action(&selected.actions()[0]).unwrap(),
                ),
            )
            .unwrap();
        let expected = expected_bridge(evaluation.targets[0].analysis.as_ref().unwrap(), "caller")
            + &expected_external(selected, repo, changed);
        assert_eq!(text, expected);
        if changed || swapped {
            assert_ne!(identity, held_identity);
            assert_ne!(text, original);
            if swapped {
                assert_ne!(
                    selected.configured_target_key(),
                    held_owner.configured_target_key()
                );
                assert_ne!(
                    selected.actions()[0].outputs()[0].root_relative_path(),
                    held_owner.actions()[0].outputs()[0].root_relative_path()
                );
                assert_eq!(
                    selected.actions()[0].outputs()[0].root(),
                    held_owner.actions()[0].outputs()[0].root()
                );
            } else {
                assert_eq!(
                    selected.configured_target_key(),
                    held_owner.configured_target_key()
                );
                assert_eq!(
                    selected.actions()[0].outputs(),
                    held_owner.actions()[0].outputs()
                );
                assert_ne!(selected.actions()[0].kind(), held_owner.actions()[0].kind());
            }
            last_changed_text = text;
        } else {
            assert_eq!(identity, held_identity);
            assert_eq!(text, original);
        }
        assert_eq!(&*held_owner.actions()[0], &held_spec);
        assert_eq!(external_write_text(held, AqueryScope::Deps), original);
        assert_eq!(
            expected_external(&held_owner, "alpha+", false),
            held_external
        );
        drop(identity);
        drop(tx);
        drop(outcome);
    }
    drop(first);
    drop(held_owner);
    drop(held_spec);
    drop(held_identity);
    drop(dice);
    assert!(original.contains("  Target: @@alpha+//pkg:write\n"));
    assert!(original.contains("external/alpha+/pkg/z-first"));
    assert!(last_changed_text.contains("  Target: @@beta+//pkg:write\n"));
    assert!(last_changed_text.contains("external/beta+/pkg/z-first"));
    assert_ne!(original, last_changed_text);
}
