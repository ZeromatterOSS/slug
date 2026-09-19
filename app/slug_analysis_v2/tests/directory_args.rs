use super::*;

#[tokio::test]
async fn unexpanded_directory_vectors_retain_literal_paths_and_reject_expansion() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), "module(name='root')\n").unwrap();
    fs::write(
        workspace.join("defs.bzl"),
        r#"def _impl(ctx):
    tree = ctx.actions.declare_directory('tree')
    tool = ctx.actions.declare_file('tool')
    ctx.actions.write(tool, 'tool', is_executable=True)
    values = depset([tree]) if ctx.attr.mode == 'depset' else [tree]
    args = ctx.actions.args()
    if ctx.attr.policy == 'default':
        args.add_all(values)
    else:
        args.add_all(values, expand_directories=ctx.attr.policy == 'true')
    args.use_param_file('@%s', use_always=True)
    ctx.actions.run(outputs=[tree], executable=tool, arguments=[args])
    return [DefaultInfo(files=depset([tree]))]
subject=rule(implementation=_impl, attrs={'mode':attr.string(), 'policy':attr.string()})
"#,
    )
    .unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:subject").unwrap(),
        typed_action_test_configuration(),
    );
    for mode in ["seq", "depset"] {
        let mut first = None;
        for policy in ["false", "default", "true", "false"] {
            fs::write(workspace.join("BUILD.bazel"), format!("load(':defs.bzl','subject')\nsubject(name='subject', mode='{mode}', policy='{policy}')\n")).unwrap();
            let result = analyze_request(&dice, &workspace, &key, None, false).await;
            if policy == "false" {
                let result = result.unwrap();
                let spawn = result.actions()[1].spawn_spec().unwrap();
                assert_eq!(spawn.render_argv(), ["tool", "tree"]);
                assert_eq!(
                    spawn.expand_forced_param_files().unwrap().param_files()[0].bytes(),
                    b"tree\n"
                );
                if let Some(first) = &first {
                    assert_eq!(first, &result);
                } else {
                    first = Some(result);
                }
            } else {
                let error = result.unwrap_err();
                assert!(
                    error.contains("directory") || error.contains("regular File"),
                    "{error}"
                );
            }
        }
    }
    fs::remove_dir_all(workspace).unwrap();
}
