use slug_analysis_v2::ConfigurationKind;
use slug_analysis_v2::ConfiguredActionView;
use slug_build_api_v2::ActionKind;
use slug_query_v2::AqueryScope;

use super::*;

const EXECUTABLE_WRITE_DEFS: &str = r#"UNICODE_OUTPUTS = json.decode('["\\u00E9\\uD83D\\uDE00.txt"]')
def _write(ctx):
    outputs = []
    for name in ctx.attr.outs:
        out = ctx.actions.declare_file(name)
        ctx.actions.write(out, 'same content\n', is_executable=ctx.attr.flag)
        outputs.append(out)
    return [DefaultInfo(files=depset(outputs))]
write = rule(implementation=_write, attrs={
    'flag':attr.bool(), 'outs':attr.string_list(default=['write.txt']),
    'dep':attr.label(),
})
def _wrapper(ctx): return [DefaultInfo()]
wrapper = rule(implementation=_wrapper, attrs={'dep':attr.label(cfg='exec')})
"#;

fn executable_write_epoch(revision: i64, flag: bool) -> PathObservationEpoch {
    let mut epoch = BuildRootEpoch::base(revision);
    epoch.file(
        "/workspace/MODULE.bazel",
        "module(name='root')\nregister_execution_platforms('//:platform')\n",
        revision,
    );
    epoch.file("/workspace/defs.bzl", EXECUTABLE_WRITE_DEFS, revision);
    epoch.package("", &format!(
        "load(':defs.bzl','write','wrapper','UNICODE_OUTPUTS')\nplatform(name='platform',exec_properties={{'z':'last','a':'first'}})\nwrite(name='write',flag={})\nwrapper(name='wrapper',dep=':write')\nwrite(name='ordered',flag=True,outs=['z-first','a-last'],dep=':write')\nwrite(name='placeholders',flag=True,outs=['%{{label}}','%{{output}}','%{{input}}'])\nwrite(name='intro-%{{output}}',flag=False,outs=['same-%{{label}}.txt'])\nwrite(name='unicode',flag=True,outs=UNICODE_OUTPUTS)\n",
        if flag { "True" } else { "False" }), revision);
    epoch.build()
}

fn executable_write_key(target: &str) -> BuildCommandRootKey {
    BuildCommandRootKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        &[TargetPattern::parse(target).unwrap()],
        build_test_configuration("write query"),
    )
    .unwrap()
}

fn executable_write_text(evaluation: &BuildCommandEvaluation, scope: AqueryScope) -> String {
    crate::runtime::format_aquery_text_output_for_scope(evaluation, scope).unwrap()
}

// Every byte of the class block is checked. Only Slug-native domains are read
// from the retained owner; the header, field order and Boolean are independent.
fn expected_write(action: &slug_analysis_v2::ConfiguredAction, header: &str, flag: bool) -> String {
    let view = ConfiguredActionView::from_configured_action(action).unwrap();
    assert!(std::ptr::eq(view.spec(), &**action));
    assert!(std::ptr::eq(view.context(), action.context()));
    assert!(std::ptr::eq(view.output(), &action.outputs()[0]));
    assert!(std::ptr::eq(view.owner(), action.context().owner()));
    assert!(!view.platform_fact().exec_properties.is_empty());
    let ActionKind::Write {
        content,
        is_executable,
    } = view.spec().kind()
    else {
        panic!("not Write")
    };
    assert_eq!(content, "same content\n");
    assert_eq!(*is_executable, flag);
    let resolved = ResolvedFileWriteSemanticView::from_configured_action(view);
    let identity =
        crate::runtime::file_write_identity::FileWriteSemanticIdentity::from_resolved(&resolved)
            .unwrap();
    let configuration = action.context().owner().configuration();
    let suffix = if configuration.kind() == ConfigurationKind::Exec {
        " [for tool]"
    } else {
        ""
    };
    let target = action.context().owner().label().to_string();
    let target = target.strip_prefix("@@").unwrap();
    format!(
        "action '{header}{suffix}'\n  Mnemonic: FileWrite\n  Target: {target}\n  Configuration: {}\n  Execution platform: //:platform\n  SlugActionToken: {}\n  Inputs: []\n  Outputs: [{}]\n  IsExecutable: {flag}\n\n",
        configuration
            .slug_configuration()
            .unwrap()
            .projection()
            .display_token(),
        identity.aquery_display_token(),
        action.outputs()[0].path()
    )
}

fn write_owner<'a>(
    evaluation: &'a BuildCommandEvaluation,
    target: &str,
) -> &'a Arc<slug_analysis_v2::ConfiguredNodeResult> {
    evaluation
        .action_closure
        .owners()
        .iter()
        .find(|owner| {
            owner
                .configured_target_key()
                .is_some_and(|key| key.label().to_string() == target)
        })
        .unwrap()
}

#[tokio::test]
async fn file_write_executable_aquery_fields_scopes_progress_and_identity() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    for (index, flag) in [false, true].into_iter().enumerate() {
        let mut tx =
            build_root_transaction(&dice, executable_write_epoch(1 + index as i64, flag)).await;
        let outcome = tx.compute(&executable_write_key("//:write")).await.unwrap();
        let evaluation = complete_build_evaluation(&outcome);
        let owner = write_owner(evaluation, "@@//:write");
        assert_eq!(owner.actions().len(), 1);
        assert_eq!(
            owner
                .configured_target_key()
                .unwrap()
                .configuration()
                .kind(),
            ConfigurationKind::Target
        );
        let header = if flag {
            "Writing script write.txt"
        } else {
            "Writing file write.txt"
        };
        let expected = expected_write(&owner.actions()[0], header, flag);
        for scope in [AqueryScope::Literal, AqueryScope::Deps] {
            assert_eq!(executable_write_text(evaluation, scope), expected);
        }
        assert_eq!(
            crate::runtime::format_file_write_aquery_text_output(evaluation).unwrap(),
            expected
        );
        let exec_outcome = tx
            .compute(&executable_write_key("//:wrapper"))
            .await
            .unwrap();
        let exec = complete_build_evaluation(&exec_outcome);
        let exec_owner = write_owner(exec, "@@//:write");
        assert_eq!(
            exec_owner
                .configured_target_key()
                .unwrap()
                .configuration()
                .kind(),
            ConfigurationKind::Exec
        );
        assert_eq!(exec_owner.actions().len(), 1);
        let expected_exec = expected_write(&exec_owner.actions()[0], header, flag);
        assert_eq!(
            executable_write_text(exec, AqueryScope::Deps),
            expected_exec
        );
        for text in [&expected, &expected_exec] {
            for omitted in [
                "  Environment:",
                "  ExecutionInfo:",
                "  Command Line:",
                "  ActionKey:",
                "Params File Content",
                "File Contents",
                "(TreeArtifact)",
            ] {
                assert!(!text.contains(omitted));
            }
        }
        let ordered_outcome = tx
            .compute(&executable_write_key("//:ordered"))
            .await
            .unwrap();
        let ordered = complete_build_evaluation(&ordered_outcome);
        let ordered_owner = write_owner(ordered, "@@//:ordered");
        assert_eq!(ordered_owner.actions().len(), 2);
        let ordered_text =
            expected_write(&ordered_owner.actions()[0], "Writing script z-first", true)
                + &expected_write(&ordered_owner.actions()[1], "Writing script a-last", true);
        assert_eq!(
            executable_write_text(ordered, AqueryScope::Literal),
            ordered_text
        );
        assert_eq!(
            ordered
                .action_closure
                .owners()
                .iter()
                .map(|node| node.actions().len())
                .sum::<usize>(),
            3
        );
        let dep_text = ordered
            .action_closure
            .owners()
            .iter()
            .map(|node| {
                if node.actions().is_empty() {
                    return String::new();
                }
                if node
                    .configured_target_key()
                    .is_some_and(|key| key.label().to_string() == "@@//:ordered")
                {
                    ordered_text.clone()
                } else {
                    assert_eq!(
                        node.configured_target_key().unwrap().label().to_string(),
                        "@@//:write"
                    );
                    expected_write(&node.actions()[0], header, flag)
                }
            })
            .collect::<String>();
        assert_eq!(executable_write_text(ordered, AqueryScope::Deps), dep_text);
        let placeholder_outcome = tx
            .compute(&executable_write_key("//:placeholders"))
            .await
            .unwrap();
        let placeholders = complete_build_evaluation(&placeholder_outcome);
        let actions = write_owner(placeholders, "@@//:placeholders").actions();
        assert_eq!(actions.len(), 3);
        let expected_placeholders = [
            "Writing script //:placeholders",
            "Writing script %{output}",
            "Writing script %{input}",
        ]
        .into_iter()
        .zip(actions)
        .map(|(header, action)| expected_write(action, header, true))
        .collect::<String>();
        assert_eq!(
            executable_write_text(placeholders, AqueryScope::Literal),
            expected_placeholders
        );
        // The existing JSON builtin decodes ASCII escapes to genuine Unicode;
        // the independent retained-path check proves the constructor receives it.
        let unicode_outcome = tx
            .compute(&executable_write_key("//:unicode"))
            .await
            .unwrap();
        let unicode = complete_build_evaluation(&unicode_outcome);
        let unicode_action = &write_owner(unicode, "@@//:unicode").actions()[0];
        assert_eq!(unicode_action.outputs()[0].root_relative_path(), "é😀.txt");
        let raw_path = unicode_action.outputs()[0].path();
        let expected_unicode = expected_write(unicode_action, "Writing script é😀.txt", true)
            .replace(
                &format!("  Outputs: [{raw_path}]"),
                &format!(
                    "  Outputs: [{}/\\u00E9\\U0001F600.txt]",
                    unicode_action.outputs()[0].root().path()
                ),
            );
        assert_eq!(
            executable_write_text(unicode, AqueryScope::Literal),
            expected_unicode
        );
        let intro_outcome = tx
            .compute(&executable_write_key("//:intro-%{output}"))
            .await
            .unwrap();
        let intro = complete_build_evaluation(&intro_outcome);
        let intro_owner = write_owner(intro, "@@//:intro-%{output}");
        assert_eq!(
            executable_write_text(intro, AqueryScope::Literal),
            expected_write(
                &intro_owner.actions()[0],
                "Writing file same-//:intro-same-%{label}.txt.txt",
                false
            )
        );
    }
}

#[tokio::test]
async fn file_write_executable_aquery_restores_held_results_and_owned_text() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let key = executable_write_key("//:write");
    let mut first_tx = build_root_transaction(&dice, executable_write_epoch(10, false)).await;
    let first = first_tx.compute(&key).await.unwrap();
    let held = complete_build_evaluation(&first);
    let held_action = &write_owner(held, "@@//:write").actions()[0];
    let held_spec = (**held_action).clone();
    let first_view = ResolvedFileWriteSemanticView::from_configured_action(
        ConfiguredActionView::from_configured_action(held_action).unwrap(),
    );
    let first_identity =
        crate::runtime::file_write_identity::FileWriteSemanticIdentity::from_resolved(&first_view)
            .unwrap();
    let original = executable_write_text(held, AqueryScope::Literal);
    assert_eq!(
        original,
        expected_write(held_action, "Writing file write.txt", false)
    );
    drop(first_tx);
    let mut changed_tx = build_root_transaction(&dice, executable_write_epoch(11, true)).await;
    let changed = changed_tx.compute(&key).await.unwrap();
    let changed_evaluation = complete_build_evaluation(&changed);
    let changed_action = &write_owner(changed_evaluation, "@@//:write").actions()[0];
    let changed_view = ResolvedFileWriteSemanticView::from_configured_action(
        ConfiguredActionView::from_configured_action(changed_action).unwrap(),
    );
    let changed_identity =
        crate::runtime::file_write_identity::FileWriteSemanticIdentity::from_resolved(
            &changed_view,
        )
        .unwrap();
    assert_eq!(held_action.context(), changed_action.context());
    assert_eq!(held_action.outputs(), changed_action.outputs());
    assert_eq!(held_action.mnemonic(), changed_action.mnemonic());
    assert_eq!(
        first_identity.as_bytes().len(),
        changed_identity.as_bytes().len()
    );
    let differences = first_identity
        .as_bytes()
        .iter()
        .zip(changed_identity.as_bytes())
        .filter(|(left, right)| left != right)
        .collect::<Vec<_>>();
    assert_eq!(differences.len(), 1);
    assert_eq!((*differences[0].0, *differences[0].1), (0, 1));
    assert_ne!(
        first_identity.aquery_display_token(),
        changed_identity.aquery_display_token()
    );
    let changed_text = executable_write_text(changed_evaluation, AqueryScope::Literal);
    assert_eq!(
        changed_text,
        expected_write(changed_action, "Writing script write.txt", true)
    );
    assert_ne!(original, changed_text);
    assert_eq!(&**held_action, &held_spec);
    assert_eq!(executable_write_text(held, AqueryScope::Literal), original);
    drop(changed_tx);
    let mut restored_tx = build_root_transaction(&dice, executable_write_epoch(12, false)).await;
    let restored = restored_tx.compute(&key).await.unwrap();
    assert_eq!(
        executable_write_text(complete_build_evaluation(&restored), AqueryScope::Literal),
        original
    );
    assert_eq!(&**held_action, &held_spec);
    drop(restored_tx);
    drop(restored);
    drop(changed_view);
    drop(changed);
    drop(first_view);
    drop(first);
    drop(differences);
    drop(first_identity);
    drop(changed_identity);
    drop(held_spec);
    drop(dice);
    assert!(original.starts_with("action 'Writing file write.txt'\n"));
    assert!(original.ends_with("  IsExecutable: false\n\n"));
    assert!(changed_text.starts_with("action 'Writing script write.txt'\n"));
    assert!(changed_text.ends_with("  IsExecutable: true\n\n"));
}
