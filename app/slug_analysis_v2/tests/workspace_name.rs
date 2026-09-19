use super::*;

#[tokio::test]
async fn public_rule_workspace_name_is_main_and_source_changes_restore() {
    let workspace = scratch();
    fs::write(
        workspace.join("MODULE.bazel"),
        "module(name='unrelated_module_name')\n",
    )
    .unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        "load(':defs.bzl','subject')\nsubject(name='subject')\n",
    )
    .unwrap();
    let definitions = "def _impl(ctx):\n    if ctx.workspace_name != 'EXPECTED':\n        fail('wrong workspace name: ' + ctx.workspace_name)\n    return []\nsubject=rule(implementation=_impl)\n";
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:subject").unwrap(),
        test_configuration(),
    );
    fs::write(
        workspace.join("defs.bzl"),
        definitions.replace("EXPECTED", "_main"),
    )
    .unwrap();
    let first = analyze_request(&dice, &workspace, &key, None, false)
        .await
        .unwrap();
    fs::write(
        workspace.join("defs.bzl"),
        definitions.replace("EXPECTED", "module_name"),
    )
    .unwrap();
    assert!(
        analyze_request(&dice, &workspace, &key, None, false)
            .await
            .unwrap_err()
            .contains("wrong workspace name: _main")
    );
    fs::write(
        workspace.join("defs.bzl"),
        definitions.replace("EXPECTED", "_main"),
    )
    .unwrap();
    let restored = analyze_request(&dice, &workspace, &key, None, false)
        .await
        .unwrap();
    assert_eq!(first, restored);
    fs::remove_dir_all(workspace).unwrap();
}
