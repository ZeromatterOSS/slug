use slug_query_v2::AqueryScope;

use super::*;

const QUERY_DEFS: &str = r#"def _dep(ctx):
    out = ctx.actions.declare_file('dep.txt')
    ctx.actions.write(out, 'dependency')
    return [DefaultInfo(files=depset([out]))]
dep = rule(implementation=_dep)
def _mixed(ctx):
    z = ctx.actions.declare_file('z-root.txt')
    a = ctx.actions.declare_file('a-root.txt')
    tree = ctx.actions.declare_directory(ctx.attr.out)
    extra = ctx.actions.declare_file('aa-extra.out')
    conditional = ctx.actions.declare_file('conditional.out')
    shell = ctx.actions.declare_file('shell.out')
    ctx.actions.write(z, 'first')
    args = ctx.actions.args()
    args.add(ctx.attr.arg)
    args.add(ctx.file.argv_only)
    args.use_param_file('@%s', use_always=True)
    ctx.actions.run(outputs=[tree, extra], executable='runner', arguments=[args],
        inputs=[ctx.file.src, ctx.file.src], env={'z':'last', 'E':ctx.attr.env, 'a':'first'},
        execution_requirements={'requires-network':'explicit', 'no-cache':'1'},
        mnemonic='Compile', progress_message='Compile %{label} %{output}')
    args.add('later')
    args.use_param_file('@%s', use_always=False)
    ctx.actions.run(outputs=[conditional], executable='runner', arguments=[args],
        inputs=[ctx.file.src], mnemonic='Conditional')
    ctx.actions.run_shell(outputs=[shell], command='echo shell', arguments=['shell-arg'])
    ctx.actions.write(a, 'last')
    return [DefaultInfo(files=depset([z, tree, extra, conditional, shell, a]))]
mixed = rule(implementation=_mixed, attrs={
    'dep':attr.label(), 'src':attr.label(allow_single_file=True),
    'argv_only':attr.label(allow_single_file=True), 'arg':attr.string(),
    'env':attr.string(), 'out':attr.string(),
})
"#;

fn query_epoch(revision: i64, replacements: &[(&str, &str)]) -> PathObservationEpoch {
    let mut epoch = BuildRootEpoch::base(revision);
    epoch.file(
        "/workspace/MODULE.bazel",
        "module(name='root')\nregister_execution_platforms('//:platform')\n",
        revision,
    );
    epoch.file("/workspace/defs.bzl", QUERY_DEFS, revision);
    let mut build = "load(':defs.bzl','dep','mixed')\nplatform(name='platform')\ndep(name='dep')\nmixed(name='subject',dep=':dep',src='input-a.txt',argv_only='argv-only.txt',arg='arg-A',env='env-A',out='tree-a')\n".to_owned();
    for (from, to) in replacements {
        build = build.replace(from, to);
    }
    epoch.package("", &build, revision);
    for path in ["input-a.txt", "input-b.txt", "argv-only.txt"] {
        epoch.file(&format!("/workspace/{path}"), "source", revision);
    }
    epoch.build()
}

fn query_key(target: &str, configuration: ConfigurationKey) -> BuildCommandRootKey {
    BuildCommandRootKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        &[TargetPattern::parse(target).unwrap()],
        configuration,
    )
    .unwrap()
}

fn query_text(evaluation: &BuildCommandEvaluation, scope: AqueryScope) -> String {
    crate::runtime::format_aquery_text_output_for_scope(evaluation, scope).unwrap()
}

fn records(text: &str) -> Vec<&str> {
    assert!(text.ends_with("\n\n"));
    text.split_inclusive("\n\n")
        .map(|record| record.strip_suffix('\n').unwrap())
        .collect()
}

#[tokio::test]
async fn spawn_aquery_mixed_scope_order_and_unspilled_recipes() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let configuration = build_test_configuration("query");
    let mut transaction = build_root_transaction(&dice, query_epoch(1, &[])).await;
    let outcome = transaction
        .compute(&query_key("//:subject", configuration.clone()))
        .await
        .unwrap();
    let evaluation = complete_build_evaluation(&outcome);
    let literal = query_text(evaluation, AqueryScope::Literal);
    let blocks = records(&literal);
    assert_eq!(blocks.len(), 5);
    assert!(blocks[0].starts_with("action 'Writing file z-root.txt'\n"));
    assert!(blocks[1].starts_with("action 'Compile //:subject tree-a'\n"));
    assert!(blocks[2].starts_with("action 'Conditional conditional.out'\n"));
    assert!(blocks[3].contains("  Mnemonic: Action\n"));
    assert!(blocks[4].starts_with("action 'Writing file a-root.txt'\n"));
    assert_eq!(literal.matches("  SlugActionToken:").count(), 2);
    assert!(!literal.contains("  ActionKey:"));
    assert!(blocks[1].contains("  Inputs: [input-a.txt]\n"));
    let output_root = evaluation.targets[0].analysis.as_ref().unwrap().actions()[1].outputs()[0]
        .root()
        .path();
    assert!(blocks[1].contains(&format!(
        "  Outputs: [{output_root}/aa-extra.out, {output_root}/tree-a (TreeArtifact)]\n"
    )));
    assert!(blocks[1].contains("  Environment: [E=env-A, a=first, z=last]\n"));
    assert!(
        blocks[1].contains("  ExecutionInfo: {no-cache: 1, requires-network: explicit}\n"),
        "{}",
        blocks[1]
    );
    let field_positions = [
        "  Mnemonic:",
        "  Target:",
        "  Configuration:",
        "  Execution platform:",
        "  Inputs:",
        "  Outputs:",
        "  Environment:",
        "  Command Line:",
        "\n# Configuration:",
        "\n# Execution platform:",
        "  ExecutionInfo:",
    ]
    .map(|field| blocks[1].find(field).unwrap());
    assert!(field_positions.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(
        blocks[1].contains("  Command Line: (exec runner \\\n    arg-A \\\n    argv-only.txt)\n")
    );
    assert!(!blocks[1].contains("later"));
    assert!(blocks[2].contains("argv-only.txt \\\n    later)\n"));
    assert!(blocks[3].contains("  Command Line: (exec /bin/bash \\\n    -c \\\n    'echo shell' \\\n    '' \\\n    shell-arg)\n"));
    assert!(!blocks[3].contains("  Environment:"));
    for block in &blocks[1..4] {
        assert!(!block.contains(".params"));
        assert!(!block.contains("Params File Content"));
        assert!(!block.contains("  SlugActionToken:"));
    }
    let deps = query_text(evaluation, AqueryScope::Deps);
    assert_eq!(records(&deps).len(), 6);
    assert_eq!(deps.matches("Writing file dep.txt").count(), 1);
    let dep_outcome = transaction
        .compute(&query_key("//:dep", configuration))
        .await
        .unwrap();
    let dep_evaluation = complete_build_evaluation(&dep_outcome);
    let old_filewrite =
        crate::runtime::format_file_write_aquery_text_output(dep_evaluation).unwrap();
    assert_eq!(
        query_text(dep_evaluation, AqueryScope::Literal),
        old_filewrite
    );
    assert!(deps.contains(&old_filewrite));
}

#[tokio::test]
async fn spawn_aquery_fields_restore_without_mutating_retained_results() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let key = query_key("//:subject", build_test_configuration("query"));
    let mut first_tx = build_root_transaction(&dice, query_epoch(10, &[])).await;
    let first = first_tx.compute(&key).await.unwrap();
    let held = complete_build_evaluation(&first);
    let original = query_text(held, AqueryScope::Literal);
    let owner = held
        .analyses()
        .find(|owner| {
            owner
                .configured_target_key()
                .is_some_and(|key| key.label().to_string() == "@@//:subject")
        })
        .unwrap();
    let held_spawn = owner.actions()[1].spawn_spec().unwrap();
    let original_argv = held_spawn.render_argv();
    drop(first_tx);
    for (index, (from, to)) in [
        ("arg-A", "arg-B"),
        ("env-A", "env-B"),
        ("input-a.txt", "input-b.txt"),
        ("tree-a", "tree-b"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut changed_tx =
            build_root_transaction(&dice, query_epoch(11 + index as i64 * 2, &[(from, to)])).await;
        let changed = changed_tx.compute(&key).await.unwrap();
        let changed_text = query_text(complete_build_evaluation(&changed), AqueryScope::Literal);
        assert_ne!(original, changed_text, "{from}");
        assert!(changed_text.contains(to));
        assert_eq!(query_text(held, AqueryScope::Literal), original);
        assert_eq!(held_spawn.render_argv(), original_argv);
        drop(changed_tx);
        let mut restored_tx =
            build_root_transaction(&dice, query_epoch(12 + index as i64 * 2, &[])).await;
        let restored = restored_tx.compute(&key).await.unwrap();
        assert_eq!(
            query_text(complete_build_evaluation(&restored), AqueryScope::Literal),
            original
        );
    }
    drop(first);
    drop(dice);
    assert_eq!(records(&original).len(), 5);
    assert!(original.contains("arg-A"));
    assert!(!original.contains("arg-B"));
}

const PROVIDER_DEFS: &str = r#"def _tool(ctx):
    executable = ctx.actions.declare_file('tool')
    ctx.actions.write(executable, 'tool', is_executable=True)
    return [DefaultInfo(executable=executable)]
tool = rule(implementation=_tool, executable=True)
def _subject(ctx):
    provider = ctx.attr.tool[DefaultInfo].files_to_run
    first = ctx.actions.declare_file('provider.out')
    second = ctx.actions.declare_file('artifact.out')
    named = ctx.actions.declare_file('named.out')
    ctx.actions.run(outputs=[first], executable=provider, arguments=['provider-arg'])
    ctx.actions.run(outputs=[second], executable=ctx.file.src, inputs=[ctx.file.src])
    ctx.actions.run(outputs=[named], executable='runner', exec_group='named')
    return [DefaultInfo(files=depset([first, second, named]))]
subject = rule(implementation=_subject, attrs={
    'tool':attr.label(cfg='exec', executable=True),
    'src':attr.label(allow_single_file=True),
}, exec_groups={'named':exec_group()})
"#;

#[tokio::test]
async fn spawn_aquery_files_to_run_and_named_group_keep_declared_owners() {
    let mut epoch = BuildRootEpoch::base(30);
    epoch.file(
        "/workspace/MODULE.bazel",
        "module(name='root')\nregister_execution_platforms('//:platform')\n",
        30,
    );
    epoch.file("/workspace/defs.bzl", PROVIDER_DEFS, 30);
    epoch.package("", "load(':defs.bzl','subject','tool')\nplatform(name='platform')\ntool(name='tool')\nsubject(name='subject',tool=':tool',src='source-tool')\n", 30);
    epoch.file("/workspace/source-tool", "source executable", 30);
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut transaction = build_root_transaction(&dice, epoch.build()).await;
    let outcome = transaction
        .compute(&query_key("//:subject", build_test_configuration("query")))
        .await
        .unwrap();
    let evaluation = complete_build_evaluation(&outcome);
    let text = query_text(evaluation, AqueryScope::Literal);
    let blocks = records(&text);
    assert_eq!(blocks.len(), 3);
    let subject = evaluation
        .analyses()
        .find(|owner| {
            owner
                .configured_target_key()
                .is_some_and(|key| key.label().to_string() == "@@//:subject")
        })
        .unwrap();
    let tool = evaluation
        .analyses()
        .find(|owner| {
            owner
                .configured_target_key()
                .is_some_and(|key| key.label().to_string() == "@@//:tool")
        })
        .unwrap();
    assert_ne!(
        subject.configured_target_key().unwrap().configuration(),
        tool.configured_target_key().unwrap().configuration()
    );
    let provider = &tool.providers().default_info().unwrap().files_to_run;
    let executable = provider.executable.as_ref().unwrap().path().into_owned();
    assert!(blocks[0].contains(&format!(
        "  Command Line: (exec {executable} \\\n    provider-arg)\n"
    )));
    assert!(blocks[0].contains("tool.runfiles"));
    let inputs = blocks[0]
        .lines()
        .find(|line| line.starts_with("  Inputs:"))
        .unwrap();
    assert!(inputs.contains(&executable));
    assert!(blocks[1].contains("  Inputs: [source-tool]\n"));
    assert!(blocks[1].contains("  Command Line: (exec source-tool)\n"));
    assert!(blocks[2].contains("  Inputs: []\n"));
    assert_eq!(
        subject.actions()[2].context().exec_group(),
        &slug_analysis_v2::ConfiguredExecGroup::Named("named".into())
    );
    for action in subject.actions() {
        let view =
            slug_analysis_v2::ConfiguredSpawnActionView::from_configured_action(action).unwrap();
        assert!(std::ptr::eq(view.action(), action));
        assert!(std::ptr::eq(view.spec(), action.spawn_spec().unwrap()));
        assert!(std::ptr::eq(view.context(), action.context().as_ref()));
    }
    for block in &blocks {
        assert!(block.contains("  Target: //:subject\n"));
        assert!(block.contains("  Execution platform: //:platform\n"));
    }
    assert!(
        crate::runtime::format_aquery_text_output_for_scope(evaluation, AqueryScope::Deps).is_err(),
        "unadmitted executable FileWrite/runfiles actions must not disappear"
    );
    drop(transaction);
    drop(outcome);
    drop(dice);
    assert_eq!(records(&text).len(), 3);
}

#[tokio::test]
async fn spawn_aquery_preserves_filewrite_validation_before_render_priority() {
    use slug_build_api_v2::ActionKind;
    use slug_build_api_v2::ActionOutput;
    use slug_build_api_v2::ActionOutputKind;
    use slug_build_api_v2::ActionSpec;
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut transaction =
        build_root_transaction(&dice, resolved_write_epoch(40, "setting_a", &[])).await;
    let outcome = transaction
        .compute(&query_key("//:write", build_test_configuration("target")))
        .await
        .unwrap();
    let evaluation = complete_build_evaluation(&outcome);
    let owner = evaluation.targets[0].analysis.as_ref().unwrap();
    let original_action = &owner.actions()[0];
    let malformed = Arc::new(
        owner
            .as_ref()
            .clone()
            .with_action_specs(
                vec![
                    ActionSpec::new(
                        ActionKind::Write {
                            content: String::new(),
                            is_executable: true,
                        },
                        "FileWrite",
                        original_action.outputs().to_vec(),
                    ),
                    ActionSpec::new(
                        ActionKind::Write {
                            content: String::new(),
                            is_executable: false,
                        },
                        "FileWrite",
                        vec![ActionOutput::new(
                            original_action.outputs()[0].root().clone(),
                            "directory",
                            ActionOutputKind::Directory,
                        )],
                    ),
                ],
                vec![original_action.context().clone()],
            )
            .unwrap(),
    );
    let mut targets = evaluation.targets.to_vec();
    targets[0].analysis = Some(malformed.clone());
    let altered = BuildCommandEvaluation {
        workspace: evaluation.workspace.dupe(),
        anchor: evaluation.anchor.clone(),
        targets: targets.into(),
        action_closure: super::ValidatedActionClosure::new(
            evaluation
                .action_closure
                .owners()
                .iter()
                .map(|candidate| {
                    if candidate.key() == owner.key() {
                        malformed.clone()
                    } else {
                        candidate.clone()
                    }
                })
                .collect(),
        )
        .unwrap(),
    };
    let expected = Err("configured FileWrite action requires exactly one file output");
    assert_eq!(
        crate::runtime::format_file_write_aquery_text_output(&altered),
        expected
    );
    assert_eq!(
        crate::runtime::format_aquery_text_output_for_scope(&altered, AqueryScope::Literal),
        expected
    );
    assert_eq!(
        crate::runtime::format_aquery_text_output_for_scope(&altered, AqueryScope::Deps),
        expected
    );
}
