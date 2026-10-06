use slug_analysis_v2::ConfiguredSymlinkActionView;
use slug_build_api_v2::SymlinkTarget;
use slug_query_v2::AqueryScope;

use super::*;

const SYMLINK_DEFS: &str = r#"def _dep(ctx):
    out = ctx.actions.declare_file('dep.txt')
    ctx.actions.write(out, 'dependency')
    return [DefaultInfo(files=depset([out]))]
dep = rule(implementation=_dep)
def _links(ctx):
    first = ctx.actions.declare_file('z-first.txt')
    default = ctx.actions.declare_file(ctx.attr.out)
    executable = ctx.actions.declare_file('b-moon.link')
    empty = ctx.actions.declare_file('c-empty.link')
    derived = ctx.actions.declare_file('d-derived.link')
    last = ctx.actions.declare_file('a-last.txt')
    ctx.actions.write(first, 'first')
    ctx.actions.symlink(output=default, target_file=ctx.file.src)
    ctx.actions.symlink(output=executable, target_file=ctx.file.src,
        is_executable=ctx.attr.executable, progress_message=ctx.attr.message)
    ctx.actions.symlink(output=empty, target_file=ctx.file.src, progress_message='')
    ctx.actions.symlink(output=derived, target_file=ctx.file.dep,
        progress_message='Derived %{input} -> %{output}')
    ctx.actions.write(last, 'last')
    return [DefaultInfo(files=depset([first, default, executable, empty, derived, last]))]
links = rule(implementation=_links, attrs={
    'src':attr.label(allow_single_file=True), 'dep':attr.label(allow_single_file=True),
    'out':attr.string(), 'message':attr.string(), 'executable':attr.bool(),
})
"#;

fn symlink_epoch(revision: i64, replacements: &[(&str, &str)]) -> PathObservationEpoch {
    let mut epoch = BuildRootEpoch::base(revision);
    epoch.file(
        "/workspace/MODULE.bazel",
        "module(name='root')\nregister_execution_platforms('//:platform')\n",
        revision,
    );
    epoch.package("", "platform(name='platform')\n", revision);
    epoch.file("/workspace/pkg/defs.bzl", SYMLINK_DEFS, revision);
    let mut build = "load(':defs.bzl','dep','links')\ndep(name='dep')\nlinks(name='subject',src='input-a.txt',dep=':dep',out='a-default.link',message='Link %{label} %{output} <- %{input}',executable=True)\n".to_owned();
    for (from, to) in replacements {
        build = build.replace(from, to);
    }
    epoch.package("pkg", &build, revision);
    for name in ["input-a.txt", "input-b.txt"] {
        epoch.file(&format!("/workspace/pkg/{name}"), "source", revision);
    }
    epoch.build()
}

fn symlink_key() -> BuildCommandRootKey {
    BuildCommandRootKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        &[TargetPattern::parse("//pkg:subject").unwrap()],
        build_test_configuration("query"),
    )
    .unwrap()
}

fn complete_symlink_evaluation(outcome: &BuildCommandOutcome) -> &BuildCommandEvaluation {
    let slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) = outcome else {
        panic!("symlink fixture retained Needs: {outcome:?}")
    };
    value.as_ref().as_ref().unwrap()
}

fn symlink_text(evaluation: &BuildCommandEvaluation, scope: AqueryScope) -> String {
    crate::runtime::format_aquery_text_output_for_scope(evaluation, scope).unwrap()
}

fn symlink_records(text: &str) -> Vec<&str> {
    assert!(text.ends_with("\n\n"));
    text.split_inclusive("\n\n")
        .map(|record| record.strip_suffix('\n').unwrap())
        .collect()
}

#[tokio::test]
async fn symlink_aquery_real_producers_have_exact_fields_and_scopes() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut transaction = build_root_transaction(&dice, symlink_epoch(1, &[])).await;
    let outcome = transaction.compute(&symlink_key()).await.unwrap();
    let evaluation = complete_symlink_evaluation(&outcome);
    let owner = evaluation.targets[0].analysis.as_ref().unwrap();
    let literal = symlink_text(evaluation, AqueryScope::Literal);
    let blocks = symlink_records(&literal);
    assert_eq!(blocks.len(), 6);
    assert!(blocks[0].starts_with("action 'Writing file pkg/z-first.txt'\n"));
    assert!(blocks[5].starts_with("action 'Writing file pkg/a-last.txt'\n"));
    let configuration = owner
        .configured_target_key()
        .unwrap()
        .configuration()
        .slug_configuration()
        .unwrap()
        .projection()
        .display_token()
        .to_owned();
    for (index, mnemonic, progress, input) in [
        (
            1,
            "Symlink",
            "Creating symlink pkg/a-default.link",
            "pkg/input-a.txt",
        ),
        (
            2,
            "ExecutableSymlink",
            "Link //pkg:subject pkg/b-moon.link <- pkg/input-a.txt",
            "pkg/input-a.txt",
        ),
        (3, "Symlink", "", "pkg/input-a.txt"),
        (
            4,
            "Symlink",
            "Derived pkg/dep.txt -> pkg/d-derived.link",
            "",
        ),
    ] {
        let action = &owner.actions()[index];
        let view = ConfiguredSymlinkActionView::from_configured_action(action).unwrap();
        assert!(std::ptr::eq(view.action(), action));
        assert!(std::ptr::eq(view.spec(), action.symlink_spec().unwrap()));
        assert!(std::ptr::eq(view.context(), action.context().as_ref()));
        assert_eq!(view.mnemonic(), mnemonic);
        let SymlinkTarget::Artifact { input: target, .. } = view.spec().target() else {
            panic!("real artifact producer")
        };
        let input = if index == 4 {
            target.path().into_owned()
        } else {
            input.to_owned()
        };
        let output = view.spec().output().path().to_owned();
        assert_eq!(
            blocks[index],
            format!(
                "action '{progress}'\n  Mnemonic: {mnemonic}\n  Target: //pkg:subject\n  Configuration: {configuration}\n  Execution platform: //:platform\n  Inputs: [{input}]\n  Outputs: [{output}]\n"
            )
        );
    }
    assert_eq!(
        owner.actions()[1]
            .symlink_spec()
            .unwrap()
            .progress_message(),
        Some("Creating symlink %{output}")
    );
    assert_eq!(
        owner.actions()[3]
            .symlink_spec()
            .unwrap()
            .progress_message(),
        Some("")
    );
    assert_eq!(literal.matches("  SlugActionToken:").count(), 2);
    for absent in [
        "  ActionKey:",
        "  IsExecutable:",
        "  UnresolvedSymlinkTarget:",
        "  Environment:",
        "  ExecutionInfo:",
        "  Command Line:",
    ] {
        assert!(
            blocks[1..5].iter().all(|record| !record.contains(absent)),
            "{absent}"
        );
    }
    let deps = symlink_text(evaluation, AqueryScope::Deps);
    assert_eq!(symlink_records(&deps).len(), 7);
    assert_eq!(deps.matches("action 'Writing file pkg/dep.txt'").count(), 1);
    for block in &blocks {
        assert!(deps.contains(block));
    }
    assert_eq!(symlink_text(evaluation, AqueryScope::Literal), literal);
    drop(transaction);
    drop(outcome);
    drop(dice);
    assert_eq!(symlink_records(&literal).len(), 6);
}

#[tokio::test]
async fn symlink_aquery_target_output_progress_and_type_restore_with_old_results() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let key = symlink_key();
    let mut transaction = build_root_transaction(&dice, symlink_epoch(10, &[])).await;
    let first = transaction.compute(&key).await.unwrap();
    let held = complete_symlink_evaluation(&first);
    let owner = held.targets[0].analysis.as_ref().unwrap();
    let old_spec = owner.actions()[2].symlink_spec().unwrap();
    let snapshot = old_spec.clone();
    let original = symlink_text(held, AqueryScope::Literal);
    drop(transaction);
    for (index, (from, to)) in [
        ("input-a.txt", "input-b.txt"),
        ("a-default.link", "changed.link"),
        ("Link %{label}", "Changed %{label}"),
        ("executable=True", "executable=False"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut changed_tx =
            build_root_transaction(&dice, symlink_epoch(11 + index as i64 * 2, &[(from, to)]))
                .await;
        let changed = changed_tx.compute(&key).await.unwrap();
        let changed_evaluation = complete_symlink_evaluation(&changed);
        let changed_text = symlink_text(changed_evaluation, AqueryScope::Literal);
        assert_ne!(original, changed_text, "{from}");
        assert_eq!(symlink_text(held, AqueryScope::Literal), original);
        assert_eq!(old_spec, &snapshot);
        if index == 0 {
            assert_ne!(
                old_spec.target(),
                changed_evaluation.targets[0]
                    .analysis
                    .as_ref()
                    .unwrap()
                    .actions()[2]
                    .symlink_spec()
                    .unwrap()
                    .target()
            );
        }
        drop(changed_tx);
        let mut restored_tx =
            build_root_transaction(&dice, symlink_epoch(12 + index as i64 * 2, &[])).await;
        let restored = restored_tx.compute(&key).await.unwrap();
        assert_eq!(
            symlink_text(complete_symlink_evaluation(&restored), AqueryScope::Literal),
            original
        );
    }
    drop(first);
    drop(dice);
    assert!(original.contains("Link //pkg:subject pkg/b-moon.link <- pkg/input-a.txt"));
    assert!(!original.contains("Changed"));
}

#[tokio::test]
async fn symlink_aquery_keeps_distinct_owners_and_preserves_equivalence_refusal() {
    const DEFS: &str = "def _alias(ctx):\n    out = ctx.actions.declare_file(ctx.attr.out)\n    ctx.actions.symlink(output=out, target_file=ctx.file.src)\n    return [DefaultInfo(files=depset([out]))]\nalias = rule(implementation=_alias, attrs={'out':attr.string(), 'src':attr.label(allow_single_file=True)})\n";
    let epoch = |revision, right: &str| {
        let mut epoch = BuildRootEpoch::base(revision);
        epoch.file(
            "/workspace/MODULE.bazel",
            "module(name='root')\nregister_execution_platforms('//:platform')\n",
            revision,
        );
        epoch.package("", &format!("load(':defs.bzl','alias')\nplatform(name='platform')\nalias(name='left',out='left.link',src='input.txt')\nalias(name='right',out='{right}',src='input.txt')\n"), revision);
        epoch.file("/workspace/defs.bzl", DEFS, revision);
        epoch.file("/workspace/input.txt", "source", revision);
        epoch.build()
    };
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let key = BuildCommandRootKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        &[
            TargetPattern::parse("//:left").unwrap(),
            TargetPattern::parse("//:right").unwrap(),
        ],
        build_test_configuration("query"),
    )
    .unwrap();
    let mut transaction = build_root_transaction(&dice, epoch(30, "right.link")).await;
    let outcome = transaction.compute(&key).await.unwrap();
    let evaluation = complete_symlink_evaluation(&outcome);
    let text = symlink_text(evaluation, AqueryScope::Deps);
    let records = symlink_records(&text);
    assert_eq!(records.len(), 2);
    for label in ["//:left", "//:right"] {
        assert_eq!(
            records
                .iter()
                .filter(|record| record.contains(&format!("  Target: {label}\n")))
                .count(),
            1
        );
    }
    assert_ne!(
        evaluation.targets[0].analysis.as_ref().unwrap().actions()[0]
            .context()
            .owner(),
        evaluation.targets[1].analysis.as_ref().unwrap().actions()[0]
            .context()
            .owner()
    );
    drop(transaction);
    let mut transaction = build_root_transaction(&dice, epoch(31, "left.link")).await;
    let refused = transaction.compute(&key).await.unwrap();
    let slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) = refused else {
        panic!("equivalence fixture retained Needs")
    };
    let error = value.as_ref().as_ref().unwrap_err();
    assert!(matches!(
        error.kind,
        BuildCommandErrorKind::ActionClosure(
            ConfiguredActionClosureError::UnsupportedEquivalence { .. }
        )
    ));
    assert_eq!(
        error.terminal_error(),
        ("unsupported_action_equivalence", 2)
    );
    assert_eq!(symlink_text(evaluation, AqueryScope::Deps), text);
}
