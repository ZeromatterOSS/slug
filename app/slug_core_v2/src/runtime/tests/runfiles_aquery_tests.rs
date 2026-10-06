use slug_analysis_v2::ConfiguredRunfilesSupportActionView;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisDepset;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::RunfilesSupport;
use slug_build_api_v2::RunfilesSupportActionSpec;
use slug_query_v2::AqueryScope;

use super::*;

const RUNFILES_DEFS: &str = r#"def _tool(ctx):
    out = ctx.actions.declare_file(ctx.attr.out)
    ctx.actions.write(out, 'tool')
    runfiles = ctx.runfiles(files=[ctx.file.src], symlinks={'alias':ctx.file.src},
        root_symlinks={'root-alias':ctx.file.src})
    return [DefaultInfo(executable=out, default_runfiles=runfiles)]
tool = rule(implementation=_tool, executable=True, attrs={
    'src':attr.label(allow_single_file=True), 'out':attr.string(),
})
def _subject(ctx):
    provider = ctx.attr.tool[DefaultInfo].files_to_run
    first = ctx.actions.declare_file('consumer-first')
    last = ctx.actions.declare_file('consumer-last')
    ctx.actions.run(outputs=[first], executable=provider, tools=[provider, provider])
    ctx.actions.write(last, 'last')
    return [DefaultInfo(files=depset([first, last]))]
subject = rule(implementation=_subject, attrs={'tool':attr.label(cfg='exec', executable=True)})
"#;

fn runfiles_epoch(revision: i64, replacements: &[(&str, &str)]) -> PathObservationEpoch {
    let mut epoch = BuildRootEpoch::base(revision);
    epoch.file(
        "/workspace/MODULE.bazel",
        "module(name='root')\nregister_execution_platforms('//:platform')\n",
        revision,
    );
    epoch.file("/workspace/defs.bzl", RUNFILES_DEFS, revision);
    let mut build = "load(':defs.bzl','tool','subject')\nplatform(name='platform')\ntool(name='tool',src='input-a.txt',out='tool-a')\nsubject(name='subject',tool=':tool')\n".to_owned();
    for (from, to) in replacements {
        build = build.replace(from, to);
    }
    epoch.package("", &build, revision);
    for source in ["input-a.txt", "input-b.txt"] {
        epoch.file(&format!("/workspace/{source}"), "source", revision);
    }
    epoch.build()
}

fn runfiles_key(target: &str) -> BuildCommandRootKey {
    BuildCommandRootKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        &[TargetPattern::parse(target).unwrap()],
        build_test_configuration("query"),
    )
    .unwrap()
}

fn runfiles_evaluation(outcome: &BuildCommandOutcome) -> &BuildCommandEvaluation {
    let slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) = outcome else {
        panic!("runfiles query fixture retained Needs: {outcome:?}")
    };
    value.as_ref().as_ref().unwrap()
}

fn runfiles_text(evaluation: &BuildCommandEvaluation, scope: AqueryScope) -> String {
    crate::runtime::format_aquery_text_output_for_scope(evaluation, scope).unwrap()
}

fn runfiles_records(text: &str) -> Vec<&str> {
    assert!(text.ends_with("\n\n"));
    text.split_inclusive("\n\n")
        .map(|record| record.strip_suffix('\n').unwrap())
        .collect()
}

fn tool_owner(evaluation: &BuildCommandEvaluation) -> &Arc<slug_analysis_v2::ConfiguredNodeResult> {
    evaluation
        .action_closure
        .owners()
        .iter()
        .find(|owner| {
            owner
                .configured_target_key()
                .is_some_and(|key| key.label().to_string() == "@@//:tool")
        })
        .unwrap()
}

fn support_spec(spec: &mut RunfilesSupportActionSpec, replacement: Arc<RunfilesSupport>) {
    match spec {
        RunfilesSupportActionSpec::RepoMappingManifest { support, .. }
        | RunfilesSupportActionSpec::SourceSymlinkManifest { support, .. }
        | RunfilesSupportActionSpec::SymlinkTree { support, .. }
        | RunfilesSupportActionSpec::RunfilesTree { support, .. } => *support = replacement,
    }
}

fn with_specs(
    owner: &slug_analysis_v2::ConfiguredNodeResult,
    specs: Vec<ActionSpec>,
    context: Arc<slug_analysis_v2::ConfiguredActionOwnerContext>,
) -> slug_analysis_v2::ConfiguredNodeResult {
    owner
        .clone()
        .with_action_specs(specs, vec![context])
        .unwrap()
}

fn render_support(owner: &slug_analysis_v2::ConfiguredNodeResult) -> String {
    let mut text = String::new();
    for action in owner.actions() {
        crate::runtime::runfiles_support_aquery_text::append_runfiles_support(
            &mut text,
            ConfiguredRunfilesSupportActionView::from_configured_action(action).unwrap(),
        )
        .unwrap();
    }
    text
}

#[tokio::test]
async fn runfiles_aquery_real_support_fields_and_scopes() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut transaction = build_root_transaction(&dice, runfiles_epoch(1, &[])).await;
    let outcome = transaction
        .compute(&runfiles_key("//:subject"))
        .await
        .unwrap();
    let evaluation = runfiles_evaluation(&outcome);
    let owner = tool_owner(evaluation);
    assert_eq!(owner.actions().len(), 5);
    let provider = &owner.providers().default_info().unwrap().files_to_run;
    let support = provider.support.as_ref().unwrap();
    let target = "//:tool";
    let configuration = owner.configured_target_key().unwrap().configuration();
    assert_eq!(
        configuration.kind(),
        slug_analysis_v2::ConfigurationKind::Exec
    );
    let token = configuration
        .slug_configuration()
        .unwrap()
        .projection()
        .display_token();
    let executable = provider.executable.as_ref().unwrap().path().into_owned();
    let public_manifest = support.manifest.as_ref().unwrap().path().into_owned();
    let mapping = support
        .repo_mapping_manifest
        .as_ref()
        .unwrap()
        .path()
        .into_owned();
    let mut tree_inputs = vec![
        executable,
        "input-a.txt".to_owned(),
        public_manifest.clone(),
        mapping,
    ];
    tree_inputs.sort();
    let deps = runfiles_text(evaluation, AqueryScope::Deps);
    let blocks = runfiles_records(&deps);
    assert_eq!(blocks.len(), 7);
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
    for (index, header, inputs, tail) in [
        (
            1,
            format!("action 'Writing repo mapping manifest for {target} [for tool]'"),
            String::new(),
            "  IsExecutable: false\n",
        ),
        (
            2,
            format!("action 'Creating source manifest for {target} [for tool]'"),
            String::new(),
            "  IsExecutable: false\n",
        ),
        (
            3,
            format!(
                "action 'Creating runfiles tree {} [for tool]'",
                public_manifest.rsplit_once('/').unwrap().0
            ),
            support.input_manifest.path().into_owned(),
            "  Environment: [PATH=/bin:/usr/bin:/usr/local/bin]\n",
        ),
        (
            4,
            format!("runfiles for {target}"),
            tree_inputs.join(", "),
            "",
        ),
    ] {
        let action = &owner.actions()[index];
        let view = ConfiguredRunfilesSupportActionView::from_configured_action(action).unwrap();
        assert!(std::ptr::eq(view.action(), action));
        assert!(std::ptr::eq(
            view.spec(),
            action.runfiles_support_spec().unwrap()
        ));
        assert!(std::ptr::eq(view.context(), action.context().as_ref()));
        assert!(std::ptr::eq(view.owner(), action.context().owner()));
        assert!(std::ptr::eq(
            view.execution_platform(),
            action.context().execution_platform().unwrap()
        ));
        assert!(std::ptr::eq(view.output(), view.spec().output()));
        assert!(Arc::ptr_eq(view.spec().support(), support));
        let expected = format!(
            "{header}\n  Mnemonic: {}\n  Target: {target}\n  Configuration: {token}\n  Execution platform: //:platform\n  Inputs: [{inputs}]\n  Outputs: [{}]\n{tail}",
            view.spec().mnemonic(),
            view.output().path()
        );
        assert_eq!(
            blocks.iter().filter(|record| **record == expected).count(),
            1,
            "{expected}\n{deps}"
        );
        for omitted in [
            "  Command Line:",
            "  ActionKey:",
            "  SlugActionToken:",
            "  ExecutionInfo:",
            "(TreeArtifact)",
        ] {
            assert!(!expected.contains(omitted));
        }
        if index != 3 {
            assert!(!expected.contains("  Environment:"));
        }
    }
    let literal = runfiles_text(evaluation, AqueryScope::Literal);
    assert_eq!(runfiles_records(&literal).len(), 2);
    assert!(!literal.contains("  Mnemonic: RunfilesTree\n"));
    assert!(deps.contains(&literal));
    let tool_outcome = transaction.compute(&runfiles_key("//:tool")).await.unwrap();
    let direct = runfiles_text(runfiles_evaluation(&tool_outcome), AqueryScope::Literal);
    assert_eq!(runfiles_records(&direct).len(), 5);
    assert!(!direct.contains(" [for tool]"));
    assert_eq!(direct.matches("runfiles for //:tool\n").count(), 1);
}

#[tokio::test]
async fn runfiles_aquery_restores_owned_results() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let key = runfiles_key("//:subject");
    let mut first_tx = build_root_transaction(&dice, runfiles_epoch(10, &[])).await;
    let first = first_tx.compute(&key).await.unwrap();
    let held = runfiles_evaluation(&first);
    let original = runfiles_text(held, AqueryScope::Deps);
    let owner = tool_owner(held);
    let snapshot = owner.as_ref().clone();
    let support = owner
        .providers()
        .default_info()
        .unwrap()
        .files_to_run
        .support
        .as_ref()
        .unwrap()
        .clone();
    let support_snapshot = support.as_ref().clone();
    drop(first_tx);
    for (index, (from, to)) in [("input-a.txt", "input-b.txt"), ("tool-a", "tool-b")]
        .into_iter()
        .enumerate()
    {
        let mut changed_tx =
            build_root_transaction(&dice, runfiles_epoch(11 + index as i64 * 2, &[(from, to)]))
                .await;
        let changed = changed_tx.compute(&key).await.unwrap();
        let changed_evaluation = runfiles_evaluation(&changed);
        assert_ne!(
            runfiles_text(changed_evaluation, AqueryScope::Deps),
            original
        );
        assert_ne!(tool_owner(changed_evaluation).as_ref(), &snapshot);
        assert_eq!(owner.as_ref(), &snapshot);
        assert_eq!(support.as_ref(), &support_snapshot);
        assert_eq!(runfiles_text(held, AqueryScope::Deps), original);
        drop(changed_tx);
        let mut restored_tx =
            build_root_transaction(&dice, runfiles_epoch(12 + index as i64 * 2, &[])).await;
        let restored = restored_tx.compute(&key).await.unwrap();
        let restored_evaluation = runfiles_evaluation(&restored);
        assert_eq!(tool_owner(restored_evaluation).as_ref(), &snapshot);
        assert_eq!(
            runfiles_text(restored_evaluation, AqueryScope::Deps),
            original
        );
    }
    drop(first);
    drop(support);
    drop(dice);
    assert_eq!(runfiles_records(&original).len(), 7);
    assert!(original.contains("input-a.txt"));
    assert!(!original.contains("input-b.txt"));
}

include!("runfiles_aquery_metadata_tests.rs");
